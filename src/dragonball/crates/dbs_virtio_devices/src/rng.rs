// Copyright 2026 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud Computing. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::any::Any;
use std::fs::File;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use dbs_device::resources::ResourceConstraint;
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use log::{debug, error, trace};
use virtio_bindings::bindings::virtio_config::VIRTIO_F_VERSION_1;
use virtio_queue::{QueueOwnedT, QueueSync, QueueT};
use vm_memory::{Bytes, GuestAddressSpace, GuestMemoryRegion, GuestRegionMmap};

use crate::device::{VirtioDevice, VirtioDeviceConfig, VirtioDeviceInfo};
use crate::{ActivateError, ActivateResult, ConfigResult, DbsGuestAddressSpace, Result, TYPE_RNG};

const RNG_DRIVER_NAME: &str = "virtio-rng";

const QUEUE_SIZE: u16 = 256;
const NUM_QUEUES: usize = 1;
const QUEUE_SIZES: &[u16] = &[QUEUE_SIZE];

pub(crate) struct RngEpollHandler<
    AS: GuestAddressSpace,
    Q: QueueT + Send = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    pub(crate) config: VirtioDeviceConfig<AS, Q, R>,
    pub(crate) random_file: File,
    id: String,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> RngEpollHandler<AS, Q, R> {
    fn process_queue(&mut self, queue_index: usize) -> bool {
        let guard = self.config.lock_guest_memory();
        let mem = guard.deref().memory();
        let queue = &mut self.config.queues[queue_index];
        let mut used_desc_heads = Vec::with_capacity(QUEUE_SIZE as usize);
        let mut queue_guard = queue.queue_mut().lock();

        let mut iter = match queue_guard.iter(mem) {
            Err(e) => {
                error!("{}: failed to process queue. {}", self.id, e);
                return false;
            }
            Ok(iter) => iter,
        };

        for mut desc_chain in &mut iter {
            let mut len = 0;

            if let Some(avail_desc) = desc_chain.next() {
                // Drivers can only read from the random device.
                if avail_desc.is_write_only() {
                    // Fill the read with data from the random device on the host.
                    if let Ok(count) = mem.read_volatile_from(
                        avail_desc.addr(),
                        &mut self.random_file,
                        avail_desc.len() as usize,
                    ) {
                        len = count as u32;
                    }
                }
            }

            used_desc_heads.push((desc_chain.head_index(), len));
        }

        drop(queue_guard);

        for &(desc_index, len) in &used_desc_heads {
            queue.add_used(mem, desc_index, len);
        }

        !used_desc_heads.is_empty()
    }
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> MutEventSubscriber
    for RngEpollHandler<AS, Q, R>
where
    AS: 'static + GuestAddressSpace + Send + Sync,
{
    fn init(&mut self, ops: &mut EventOps) {
        trace!(
            target: RNG_DRIVER_NAME,
            "{}: RngEpollHandler::init()",
            self.id
        );

        for (idx, queue) in self.config.queues.iter().enumerate() {
            let events = Events::with_data(queue.eventfd.as_ref(), idx as u32, EventSet::IN);
            if let Err(e) = ops.add(events) {
                error!("{}: failed to register queue event, {:?}", self.id, e);
            }
        }
    }

    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        trace!(
            target: RNG_DRIVER_NAME,
            "{}: RngEpollHandler::process()",
            self.id
        );

        let idx = events.data() as usize;
        if idx >= self.config.queues.len() {
            error!("{}: invalid queue index {}", self.id, idx);
            return;
        }

        if let Err(e) = self.config.queues[idx].consume_event() {
            error!("{}: failed to get queue event: {:?}", self.id, e);
        } else if self.process_queue(idx) {
            if let Err(e) = self.config.queues[idx].notify() {
                error!("{}: failed to signal used queue: {}", self.id, e);
            }
        } else {
            debug!("{}: no request processed", self.id);
        }
    }
}

/// Virtio device for exposing entropy to the guest OS through virtio.
pub struct Rng<AS: GuestAddressSpace> {
    pub(crate) device_info: VirtioDeviceInfo,
    pub(crate) random_file: Option<File>,
    pub(crate) subscriber_id: Option<SubscriberId>,
    pub(crate) id: String,
    pub(crate) phantom: PhantomData<AS>,
}

impl<AS: GuestAddressSpace> Rng<AS> {
    /// Create a new virtio-rng device that gets random data from the host.
    pub fn new(path: String, epoll_mgr: EpollManager) -> Result<Self> {
        trace!(target: RNG_DRIVER_NAME, "{}: Rng::new({})", RNG_DRIVER_NAME, path);

        let avail_features = 1u64 << VIRTIO_F_VERSION_1;
        let random_file = File::open(path)?;

        // The virtio-rng device has no device configuration space.
        let device_info = VirtioDeviceInfo::new(
            RNG_DRIVER_NAME.to_string(),
            avail_features,
            Arc::new(vec![QUEUE_SIZE; NUM_QUEUES]),
            Vec::new(),
            epoll_mgr,
        );
        let id = device_info.driver_name.clone();

        Ok(Rng {
            device_info,
            random_file: Some(random_file),
            subscriber_id: None,
            id,
            phantom: PhantomData,
        })
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for Rng<AS>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_RNG
    }

    fn queue_max_sizes(&self) -> &[u16] {
        QUEUE_SIZES
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(
            target: RNG_DRIVER_NAME,
            "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
            self.id,
            page,
            value
        );
        self.device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, _offset: u64, _data: &mut [u8]) -> ConfigResult {
        trace!("{RNG_DRIVER_NAME}: has no device configuration");
        Ok(())
    }

    fn write_config(&mut self, _offset: u64, _data: &[u8]) -> ConfigResult {
        trace!("{RNG_DRIVER_NAME}: has no device configuration");
        Ok(())
    }

    fn activate(&mut self, config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        trace!(
            target: RNG_DRIVER_NAME,
            "{}: VirtioDevice::activate()",
            self.id
        );

        self.device_info.check_queue_sizes(&config.queues)?;

        match self.random_file.as_ref() {
            Some(file) => {
                let random_file = file.try_clone().map_err(|e| {
                    error!("{}: failed to clone rng source, {}", self.id, e);
                    ActivateError::InternalError
                })?;
                let handler = Box::new(RngEpollHandler {
                    config,
                    random_file,
                    id: self.id.clone(),
                });

                self.subscriber_id = Some(self.device_info.register_event_handler(handler));

                Ok(())
            }
            None => Err(ActivateError::InternalError),
        }
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.device_info.queue_sizes.len() + 1) as u32,
            });
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::NoopNotifier;
    use dbs_utils::epoll_manager::SubscriberOps;
    use kvm_ioctls::Kvm;
    use test_utils::skip_if_kvm_unaccessable;
    use vm_memory::{GuestAddress, GuestMemoryMmap};
    use vmm_sys_util::eventfd::EventFd;
    use vmm_sys_util::tempfile::TempFile;

    use super::*;
    use crate::device::VirtioQueueConfig;
    use crate::tests::{create_address_space, VirtQueue, VIRTQ_DESC_F_WRITE};

    fn dummy_path(file: &TempFile) -> String {
        file.as_path()
            .to_owned()
            .into_os_string()
            .into_string()
            .unwrap()
    }

    fn create_rng_epoll_handler() -> RngEpollHandler<Arc<GuestMemoryMmap>> {
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0x0), 0x10000)]).unwrap());
        let queues = vec![VirtioQueueConfig::create(QUEUE_SIZE, 0).unwrap()];
        let resources = DeviceResources::new();
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let address_space = create_address_space();

        let config = VirtioDeviceConfig::new(
            mem,
            address_space,
            vm_fd,
            resources,
            queues,
            None,
            Arc::new(NoopNotifier::new()),
        );

        // Populate the entropy source with deterministic data so the device
        // has something to copy into the guest's write-only buffers.
        let random_file = TempFile::new().unwrap();
        std::fs::write(random_file.as_path(), vec![0xa5u8; 0x1000]).unwrap();
        let random_file = random_file.into_file();
        RngEpollHandler {
            config,
            random_file,
            id: RNG_DRIVER_NAME.to_string(),
        }
    }

    #[test]
    fn test_rng_virtio_device_normal() {
        let epoll_mgr = EpollManager::default();
        let dummy_file = TempFile::new().unwrap();

        let mut dev = Rng::<Arc<GuestMemoryMmap>>::new(dummy_path(&dummy_file), epoll_mgr).unwrap();

        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&dev),
            TYPE_RNG
        );
        let queue_size = [QUEUE_SIZE];
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::queue_max_sizes(
                &dev
            ),
            &queue_size[..]
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 0),
            dev.device_info.get_avail_features(0)
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 1),
            dev.device_info.get_avail_features(1)
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 2),
            dev.device_info.get_avail_features(2)
        );
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::set_acked_features(
            &mut dev, 2, 0,
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&dev, 2),
            0
        );

        // The device has no configuration space: writes are ignored and reads
        // leave the buffer untouched.
        let config: [u8; 8] = [0; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
            &mut dev, 0, &config,
        )
        .unwrap();
        let mut data: [u8; 8] = [1; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut dev, 0, &mut data,
        )
        .unwrap();
        assert_eq!(data, [1; 8]);
    }

    #[test]
    fn test_rng_virtio_device_activate() {
        skip_if_kvm_unaccessable!();
        let epoll_mgr = EpollManager::default();
        let dummy_file = TempFile::new().unwrap();

        // check queue sizes error
        {
            let mut dev =
                Rng::<Arc<GuestMemoryMmap>>::new(dummy_path(&dummy_file), epoll_mgr.clone())
                    .unwrap();
            let queues = vec![
                VirtioQueueConfig::<QueueSync>::create(16, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(16, 0).unwrap(),
            ];

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>>::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::new()),
            );
            assert!(matches!(
                dev.activate(config),
                Err(ActivateError::InvalidParam)
            ));
        }
        // success
        {
            let mut dev =
                Rng::<Arc<GuestMemoryMmap>>::new(dummy_path(&dummy_file), epoll_mgr).unwrap();
            let queues = vec![VirtioQueueConfig::<QueueSync>::create(QUEUE_SIZE, 0).unwrap()];

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>>::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::new()),
            );
            assert!(dev.activate(config).is_ok());
        }
    }

    #[test]
    fn test_rng_epoll_handler_handle_event() {
        skip_if_kvm_unaccessable!();
        let handler = create_rng_epoll_handler();
        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_rng_epoll_handler();

        // invalid queue index
        let events = Events::with_data(&event_fd, 1024, event_set);
        handler.process(events, &mut event_op);

        // valid queue index
        let events = Events::with_data(&event_fd, 0, event_set);
        handler.config.queues[0].generate_event().unwrap();
        handler.process(events, &mut event_op);
    }

    #[test]
    fn test_rng_epoll_handler_process_queue() {
        skip_if_kvm_unaccessable!();
        let mut handler = create_rng_epoll_handler();
        let m = &handler.config.vm_as.clone();

        let vq = VirtQueue::new(GuestAddress(0), m, 16);
        vq.avail.ring(0).store(0);
        vq.avail.idx().store(1);
        let q = vq.create_queue();
        // The virtio-rng driver hands the device a single write-only buffer to
        // be filled with entropy from the host source.
        vq.dtable(0).set(0x1000, 0x100, VIRTQ_DESC_F_WRITE, 0);
        handler.config.queues = vec![VirtioQueueConfig::new(
            q,
            Arc::new(EventFd::new(0).unwrap()),
            Arc::new(NoopNotifier::new()),
            0,
        )];
        assert!(handler.process_queue(0));

        // The device consumed the descriptor and reported the number of entropy
        // bytes written into the buffer through the used ring.
        let used_elem = vq.used.ring(0).load();
        assert_eq!(used_elem.id, 0);
        assert_eq!(used_elem.len, 0x100);
    }
}
