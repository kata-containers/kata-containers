// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use dbs_device::resources::ResourceConstraint;
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use dbs_utils::metric::IncMetric;
use dbs_utils::net::{net_gen, MacAddr, Tap};
use log::{debug, error, info, trace, warn};
#[cfg(not(test))]
use vhost_rs::net::VhostNet as VhostNetTrait;
#[cfg(not(test))]
use vhost_rs::vhost_kern::net::Net as VhostNet;
use vhost_rs::vhost_user::message::VhostUserVringAddrFlags;
#[cfg(not(test))]
use vhost_rs::VhostBackend;
use vhost_rs::{VhostUserMemoryRegionInfo, VringConfigData};
use virtio_bindings::bindings::virtio_net::*;
use virtio_bindings::bindings::virtio_ring::*;
use virtio_queue::{DescriptorChain, QueueT};
use vm_memory::{Address, GuestMemory, GuestMemoryRegion, MemoryRegionAddress};

use crate::vhost::net::{virtio_handle_ctrl_mq, virtio_handle_ctrl_status, FromNetCtrl};
#[cfg(test)]
use crate::vhost::vhost_kern::test_utils::{
    MockVhostBackend as VhostBackend, MockVhostNet as VhostNet,
};
use crate::{
    setup_config_space, vnet_hdr_len, ActivateError, ConfigResult, DbsGuestAddressSpace,
    Error as VirtioError, NetDeviceMetrics, Result as VirtioResult, TapError, VirtioDevice,
    VirtioDeviceConfig, VirtioDeviceInfo, DEFAULT_MTU, TYPE_NET,
};

const NET_DRIVER_NAME: &str = "vhost-net";
// Epoll token for control queue
const CTRL_SLOT: u32 = 0;
// Control queue size
const CTRL_QUEUE_SIZE: u16 = 64;

/// Error for vhost-net devices to handle requests from guests.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tap device operation error: {0:?}")]
    TapError(#[source] TapError),
    #[error("vhost error: {0}")]
    VhostError(#[source] vhost_rs::Error),
}

/// Vhost-net device implementation
pub struct Net<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    taps: Vec<Tap>,
    handles: Vec<VhostNet<AS>>,
    device_info: VirtioDeviceInfo,
    queue_sizes: Arc<Vec<u16>>,
    ctrl_queue_size: u16,
    subscriber_id: Option<SubscriberId>,
    id: String,
    kernel_vring_bases: Option<Vec<(u32, u32)>>,
    metrics: Arc<NetDeviceMetrics>,

    // Type-Q placeholder
    _mark_q: PhantomData<Q>,
    // Type-R placeholder
    _mark_r: PhantomData<R>,
}

/// Ensure that the tap interface has the correct flags and sets the
/// offload and VNET header size to the appropriate values.
fn validate_and_configure_tap(tap: &Tap, vq_pairs: usize) -> VirtioResult<()> {
    // Check if there are missing flags。
    let flags = tap.if_flags();
    let mut required_flags = vec![
        (net_gen::IFF_TAP, "IFF_TAP"),
        (net_gen::IFF_NO_PI, "IFF_NO_PI"),
        (net_gen::IFF_VNET_HDR, "IFF_VNET_HDR"),
    ];
    if vq_pairs > 1 {
        required_flags.push((net_gen::IFF_MULTI_QUEUE, "IFF_MULTI_QUEUE"));
    }
    let missing_flags = required_flags
        .iter()
        .filter_map(
            |(value, name)| {
                if value & flags == 0 {
                    Some(name)
                } else {
                    None
                }
            },
        )
        .collect::<Vec<_>>();
    if !missing_flags.is_empty() {
        return Err(VirtioError::VhostNet(Error::TapError(
            TapError::MissingFlags(
                missing_flags
                    .into_iter()
                    .copied()
                    .collect::<Vec<&str>>()
                    .join(", "),
            ),
        )));
    }

    let vnet_hdr_size = vnet_hdr_len() as i32;
    tap.set_vnet_hdr_size(vnet_hdr_size)
        .map_err(|err| VirtioError::VhostNet(Error::TapError(TapError::SetVnetHdrSize(err))))?;

    Ok(())
}

impl<AS, Q, R> Net<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    /// Create a new vhost-net device with a given tap interface.
    pub fn new_with_tap(
        tap: Tap,
        guest_mac: Option<&MacAddr>,
        queue_sizes: Arc<Vec<u16>>,
        event_mgr: EpollManager,
    ) -> VirtioResult<Self> {
        trace!(target: "vhost-net", "{}: Net::new_with_tap()", NET_DRIVER_NAME);

        let vq_pairs = queue_sizes.len() / 2;

        let taps = tap
            .into_mq_taps(vq_pairs)
            .map_err(|err| VirtioError::VhostNet(Error::TapError(TapError::Open(err))))?;
        for tap in taps.iter() {
            validate_and_configure_tap(tap, vq_pairs)?;
        }

        let mut avail_features = 1u64 << VIRTIO_NET_F_GUEST_CSUM
            | 1u64 << VIRTIO_NET_F_CSUM
            | 1u64 << VIRTIO_NET_F_GUEST_TSO4
            | 1u64 << VIRTIO_NET_F_GUEST_UFO
            | 1u64 << VIRTIO_NET_F_HOST_TSO4
            | 1u64 << VIRTIO_NET_F_HOST_UFO
            | 1u64 << VIRTIO_NET_F_MRG_RXBUF
            | 1u64 << VIRTIO_RING_F_INDIRECT_DESC
            | 1u64 << VIRTIO_RING_F_EVENT_IDX
            | 1u64 << VIRTIO_F_NOTIFY_ON_EMPTY
            | 1u64 << VIRTIO_F_VERSION_1;

        if vq_pairs > 1 {
            avail_features |= (1 << VIRTIO_NET_F_MQ | 1 << VIRTIO_NET_F_CTRL_VQ) as u64;
        }

        let config_space = setup_config_space(
            NET_DRIVER_NAME,
            &guest_mac,
            &mut avail_features,
            vq_pairs as u16,
            DEFAULT_MTU,
        )?;

        let device_info = VirtioDeviceInfo::new(
            NET_DRIVER_NAME.to_owned(),
            avail_features,
            queue_sizes.clone(),
            config_space,
            event_mgr,
        );
        let id = device_info.driver_name.clone();

        Ok(Net {
            taps,
            handles: Vec::new(),
            device_info,
            queue_sizes,
            subscriber_id: None,
            ctrl_queue_size: {
                if vq_pairs > 1 {
                    CTRL_QUEUE_SIZE
                } else {
                    0
                }
            },
            id,
            kernel_vring_bases: None,
            _mark_r: PhantomData,
            _mark_q: PhantomData,
            metrics: Arc::new(NetDeviceMetrics::default()),
        })
    }

    /// Create a vhost network with the Tap name
    pub fn new(
        host_dev_name: String,
        guest_mac: Option<&MacAddr>,
        queue_sizes: Arc<Vec<u16>>,
        event_mgr: EpollManager,
    ) -> VirtioResult<Self> {
        let vq_pairs = queue_sizes.len() / 2;

        // Open a TAP interface
        let tap = Tap::open_named(&host_dev_name, vq_pairs > 1)
            .map_err(|err| VirtioError::VhostNet(Error::TapError(TapError::Open(err))))?;
        tap.enable()
            .map_err(|err| VirtioError::VhostNet(Error::TapError(TapError::Enable(err))))?;

        Self::new_with_tap(tap, guest_mac, queue_sizes, event_mgr)
    }

    fn do_device_activate(
        &mut self,
        config: &VirtioDeviceConfig<AS, Q, R>,
        vq_pairs: usize,
    ) -> VirtioResult<()>
    where
        Q: QueueT + Send + 'static,
        R: GuestMemoryRegion + Sync + Send + 'static,
    {
        let guard = config.lock_guest_memory();
        let mem = guard.deref();

        if self.handles.is_empty() {
            for _ in 0..vq_pairs {
                self.handles.push(
                    VhostNet::<AS>::new(config.vm_as.clone())
                        .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?,
                );
            }
        }

        if self.kernel_vring_bases.is_none() {
            self.setup_vhost_backend(config, mem)?
        }

        Ok(())
    }

    fn setup_vhost_backend(
        &mut self,
        config: &VirtioDeviceConfig<AS, Q, R>,
        mem: &AS::M,
    ) -> VirtioResult<()>
    where
        Q: QueueT + Send + 'static,
        R: GuestMemoryRegion + Sync + Send + 'static,
    {
        let vq_pairs = self.queue_sizes.len() / 2;
        trace!(target: "vhost-net", "{}: Net::setup_vhost_backend(vq_pairs: {})", NET_DRIVER_NAME, vq_pairs);

        if vq_pairs < 1 {
            error!(
                "{}: Invalid virtio queue pairs, expected a value greater than 0, but got {}",
                NET_DRIVER_NAME, vq_pairs
            );
            return Err(VirtioError::ActivateError(Box::new(
                ActivateError::InvalidParam,
            )));
        }

        if self.handles.len() != vq_pairs || self.taps.len() != vq_pairs {
            error!("{}: Invalid handlers or taps, handlers length {}, taps length {}, virtio queue pairs = {}",
                NET_DRIVER_NAME,
                self.handles.len(),
                self.taps.len(),
                vq_pairs);
            return Err(VirtioError::ActivateError(Box::new(
                ActivateError::InternalError,
            )));
        }

        for idx in 0..vq_pairs {
            self.init_vhost_dev(idx, config, mem)?;
        }

        self.kernel_vring_bases = None;

        Ok(())
    }

    fn init_vhost_dev(
        &mut self,
        pair_index: usize,
        config: &VirtioDeviceConfig<AS, Q, R>,
        mem: &AS::M,
    ) -> VirtioResult<()>
    where
        Q: QueueT + Send + 'static,
        R: GuestMemoryRegion + Sync + Send + 'static,
    {
        trace!(target: "vhost-net", "{}: Net::init_vhost_dev(pair_index: {})", NET_DRIVER_NAME, pair_index);

        let handle = &mut self.handles[pair_index];
        handle
            .set_owner()
            .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;

        let avail_features = handle
            .get_features()
            .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;
        let features = self.device_info.acked_features() & avail_features;
        handle
            .set_features(features)
            .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;

        let tap = &self.taps[pair_index];
        tap.set_offload(Self::virtio_features_to_tap_offload(
            self.device_info.acked_features(),
        ))
        .map_err(|err| VirtioError::VhostNet(Error::TapError(TapError::SetOffload(err))))?;

        self.setup_mem_table(pair_index, mem)?;

        self.init_vhost_queues(pair_index, config)?;

        Ok(())
    }

    fn setup_mem_table(&mut self, pair_index: usize, mem: &AS::M) -> VirtioResult<()> {
        let handle = &mut self.handles[pair_index];

        let mut regions = Vec::new();
        for region in mem.iter() {
            let guest_phys_addr = region.start_addr();

            let userspace_addr = region
                .get_host_address(MemoryRegionAddress(0))
                .map_err(|_| VirtioError::InvalidGuestAddress(guest_phys_addr))?;

            regions.push(VhostUserMemoryRegionInfo {
                guest_phys_addr: guest_phys_addr.raw_value(),
                memory_size: region.len(),
                userspace_addr: userspace_addr as *const u8 as u64,
                mmap_offset: 0,
                mmap_handle: -1,
            });
        }

        handle
            .set_mem_table(&regions)
            .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;

        Ok(())
    }

    fn init_vhost_queues(
        &mut self,
        pair_index: usize,
        config: &VirtioDeviceConfig<AS, Q, R>,
    ) -> VirtioResult<()>
    where
        Q: QueueT + Send + 'static,
        R: GuestMemoryRegion + Sync + Send + 'static,
    {
        trace!(target: "vhost-net", "{}: Net::init_vhost_queues(pair_index: {})", NET_DRIVER_NAME, pair_index);

        let handle = &mut self.handles[pair_index];
        let tap = &self.taps[pair_index];

        let intr_evts = config.get_queue_interrupt_eventfds();
        assert_eq!(config.queues.len(), intr_evts.len());

        let vq_pair = [
            &config.queues[2 * pair_index],
            &config.queues[2 * pair_index + 1],
        ];

        for queue_cfg in vq_pair.iter() {
            let queue = &queue_cfg.queue;
            let queue_index = queue_cfg.index() as usize;
            let vq_index = queue_index % 2;

            handle
                .set_vring_num(vq_index, queue_cfg.queue.size())
                .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;

            if let Some(vring_base) = &self.kernel_vring_bases {
                let base = if vq_index == 0 {
                    vring_base[pair_index].0
                } else {
                    vring_base[pair_index].1
                };
                handle
                    .set_vring_base(vq_index, base as u16)
                    .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;
            } else {
                handle
                    .set_vring_base(vq_index, 0)
                    .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;
            }

            let config_data = &VringConfigData {
                queue_max_size: queue.max_size(),
                queue_size: queue.size(),
                flags: VhostUserVringAddrFlags::empty().bits(),
                desc_table_addr: queue.desc_table(),
                used_ring_addr: queue.used_ring(),
                avail_ring_addr: queue.avail_ring(),
                log_addr: None,
            };

            handle
                .set_vring_addr(vq_index, config_data)
                .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;

            handle
                .set_vring_call(vq_index, intr_evts[queue_index])
                .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;

            handle
                .set_vring_kick(vq_index, &queue_cfg.eventfd)
                .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;

            handle
                .set_backend(vq_index, Some(&tap.tap_file))
                .map_err(|err| VirtioError::VhostNet(Error::VhostError(err)))?;
        }

        Ok(())
    }

    fn virtio_features_to_tap_offload(features: u64) -> u32 {
        let mut tap_offloads: u32 = 0;

        if features & (1 << VIRTIO_NET_F_GUEST_CSUM) != 0 {
            tap_offloads |= net_gen::TUN_F_CSUM;
        }
        if features & (1 << VIRTIO_NET_F_GUEST_TSO4) != 0 {
            tap_offloads |= net_gen::TUN_F_TSO4;
        }
        if features & (1 << VIRTIO_NET_F_GUEST_TSO6) != 0 {
            tap_offloads |= net_gen::TUN_F_TSO6;
        }
        if features & (1 << VIRTIO_NET_F_GUEST_ECN) != 0 {
            tap_offloads |= net_gen::TUN_F_TSO_ECN;
        }
        if features & (1 << VIRTIO_NET_F_GUEST_UFO) != 0 {
            tap_offloads |= net_gen::TUN_F_UFO;
        }

        tap_offloads
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for Net<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_NET
    }

    fn queue_max_sizes(&self) -> &[u16] {
        &self.queue_sizes
    }

    fn ctrl_queue_max_sizes(&self) -> u16 {
        self.ctrl_queue_size
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(target: "vhost-net", "{}: Net::set_acked_features({}, 0x{:x})",
            self.id, page, value);
        self.device_info.set_acked_features(page, value);
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        trace!(target: "vhost-net", "{}: Net::read_config(0x{:x}, {:?})",
            self.id, offset, data);
        self.device_info.read_config(offset, data).map_err(|e| {
            self.metrics.cfg_fails.inc();
            e
        })
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        trace!(target: "vhost-net", "{}: Net::write_config(0x{:x}, {:?})",
               self.id, offset, data);
        self.device_info.write_config(offset, data).map_err(|e| {
            self.metrics.cfg_fails.inc();
            e
        })
    }

    fn activate(&mut self, config: crate::VirtioDeviceConfig<AS, Q, R>) -> crate::ActivateResult {
        trace!(target: "vhost-net", "{}: Net::activate()", self.id);

        // Do not support control queue and multi-queue.
        let vq_pairs = config.queues.len() / 2;
        if config.queues.len() % 2 != 0 || self.taps.len() != vq_pairs {
            self.metrics.activate_fails.inc();
            return Err(crate::ActivateError::InvalidParam);
        }

        self.device_info
            .check_queue_sizes(&config.queues)
            .map_err(|err| {
                self.metrics.activate_fails.inc();
                err
            })?;

        if let Err(err) = self.do_device_activate(&config, vq_pairs) {
            error!(target: "vhost-net", "device {:?} activate failed: {:?}", self.id, err);
            panic!("vhost-net device {:?} activate failed: {:?}", self.id, err);
        }

        let handler = Box::new(NetEpollHandler {
            config,
            id: self.id.clone(),
        });
        self.subscriber_id = Some(self.device_info.register_event_handler(handler));

        Ok(())
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<dbs_device::resources::ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        trace!(target: "vhost-net", "{}: Net::get_resource_requirements()", self.id);

        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.queue_sizes.len() + 1) as u32,
            });
        }
    }

    fn remove(&mut self) {
        self.taps.clear();

        let subscriber_id = self.subscriber_id.take();
        if let Some(subscriber_id) = subscriber_id {
            match self.device_info.remove_event_handler(subscriber_id) {
                Ok(_) => debug!("vhost-net: removed subscriber_id {:?}", self.subscriber_id),
                Err(err) => warn!("vhost-net: failed to remove event handler: {:?}", err),
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub(crate) struct NetEpollHandler<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    pub(crate) config: VirtioDeviceConfig<AS, Q, R>,
    id: String,
}

impl<AS, Q, R> NetEpollHandler<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn process_ctrl_request(&mut self) -> VirtioResult<()> {
        let guard = self.config.lock_guest_memory();
        let mem = guard.deref();
        // It is safty to unwrap here as the value of ctrl_queue is
        // confirmed in `CTRL_SLOT`.
        let cvq = self.config.ctrl_queue.as_mut().unwrap();

        while let Some(mut desc_chain) = cvq.get_next_descriptor(mem)? {
            let len = match Self::process_ctrl_desc(&mut desc_chain, mem) {
                Ok(len) => {
                    debug!("{}: process ctrl desc succeed!", self.id);
                    len
                }
                Err(err) => {
                    debug!(
                        "{}: failed to process control queue request, {}",
                        self.id, err
                    );
                    0
                }
            };
            cvq.add_used(mem, desc_chain.head_index(), len);
        }

        Ok(())
    }

    fn process_ctrl_desc(
        desc_chain: &mut DescriptorChain<&AS::M>,
        mem: &AS::M,
    ) -> VirtioResult<u32> {
        if let Some(header) = desc_chain.next() {
            let ctrl_hdr = virtio_net_ctrl_hdr::from_net_ctrl_st(mem, &header)?;
            match ctrl_hdr.class as u32 {
                VIRTIO_NET_CTRL_MQ => {
                    virtio_handle_ctrl_mq::<AS, _>(desc_chain, ctrl_hdr.cmd, mem, |curr_queues| {
                        info!("{}: vq pairs: {}", NET_DRIVER_NAME, curr_queues);
                        Ok(())
                    })?;
                    return virtio_handle_ctrl_status::<AS>(
                        NET_DRIVER_NAME,
                        desc_chain,
                        VIRTIO_NET_OK as u8,
                        mem,
                    );
                }
                _ => error!(
                    "{}: unknown net control request class: 0x{:x}",
                    NET_DRIVER_NAME, ctrl_hdr.class
                ),
            }
        }
        Ok(0)
    }
}

impl<AS, Q, R> MutEventSubscriber for NetEpollHandler<AS, Q, R>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
{
    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        match events.data() {
            CTRL_SLOT => {
                if let Some(config) = self.config.ctrl_queue.as_ref() {
                    if let Err(err) = config.consume_event() {
                        error!("{}: failed to read eventfd, {:?}", self.id, err);
                    } else if let Err(err) = self.process_ctrl_request() {
                        error!(
                            "{}: failed to handle control queue request, {:?}",
                            self.id, err
                        );
                    }
                }
            }
            _ => error!("{}: unknown epoll event slot {}", self.id, events.data()),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!(target: "vhost-net", "{}: NetEpollHandler::init()", self.id);

        if let Some(config) = self.config.ctrl_queue.as_ref() {
            let event = Events::with_data(&config.eventfd, CTRL_SLOT, EventSet::IN);
            if let Err(err) = ops.add(event) {
                error!(
                    "{}: failed to register epoll event for control queue, {:?}",
                    self.id, err
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::{
        InterruptIndex, InterruptManager, InterruptSourceType, InterruptStatusRegister32,
        NoopNotifier,
    };
    use dbs_utils::epoll_manager::SubscriberOps;
    use kvm_ioctls::Kvm;
    use virtio_queue::{Queue, QueueSync};
    use vm_memory::{GuestAddress, GuestMemoryMmap, GuestRegionMmap};
    use vmm_sys_util::eventfd::EventFd;

    use super::*;
    use crate::tests::{create_address_space, create_vm_and_irq_manager};
    use crate::{create_queue_notifier, VirtioQueueConfig};

    fn create_vhost_kern_net_epoll_handler(
        id: String,
    ) -> NetEpollHandler<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> {
        let queues = vec![
            VirtioQueueConfig::create(128, 0).unwrap(),
            VirtioQueueConfig::create(128, 0).unwrap(),
        ];
        let ctrl_queue = VirtioQueueConfig::create(128, 0).unwrap();
        let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let kvm = Arc::new(Kvm::new().unwrap());
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let resources = DeviceResources::new();
        let address_space = create_address_space();
        let config = VirtioDeviceConfig::new(
            Arc::new(mem),
            address_space,
            vm_fd,
            resources,
            queues,
            Some(ctrl_queue),
            Arc::new(NoopNotifier::default()),
        );

        NetEpollHandler { config, id }
    }

    #[test]
    fn test_vhost_kern_net_virtio_normal() {
        let guest_mac_str = "11:22:33:44:55:66";
        let guest_mac = MacAddr::parse_str(guest_mac_str).unwrap();
        let queue_sizes = Arc::new(vec![128]);
        let epoll_mgr = EpollManager::default();
        let mut dev: Net<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> = Net::new(
            String::from("test_vhosttap"),
            Some(&guest_mac),
            queue_sizes,
            epoll_mgr,
        )
        .unwrap();

        assert_eq!(dev.device_type(), TYPE_NET);

        let queue_size = vec![128];
        assert_eq!(dev.queue_max_sizes(), &queue_size[..]);
        assert_eq!(
            dev.get_avail_features(0),
            dev.device_info.get_avail_features(0)
        );
        assert_eq!(
            dev.get_avail_features(1),
            dev.device_info.get_avail_features(1)
        );
        assert_eq!(
            dev.get_avail_features(2),
            dev.device_info.get_avail_features(2)
        );
        dev.set_acked_features(2, 0);
        assert_eq!(dev.get_avail_features(2), 0);
        let config: [u8; 8] = [0; 8];
        dev.write_config(0, &config).unwrap();
        let mut data: [u8; 8] = [1; 8];
        dev.read_config(0, &mut data).unwrap();
        assert_eq!(config, data);
    }

    #[test]
    fn test_vhost_kern_net_virtio_activate() {
        let guest_mac_str = "11:22:33:44:55:66";
        let guest_mac = MacAddr::parse_str(guest_mac_str).unwrap();
        // Invalid queue sizes
        {
            let queue_sizes = Arc::new(vec![128]);
            let epoll_mgr = EpollManager::default();
            let mut dev: Net<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> = Net::new(
                String::from("test_vhosttap"),
                Some(&guest_mac),
                queue_sizes,
                epoll_mgr,
            )
            .unwrap();

            let queues = vec![
                VirtioQueueConfig::create(128, 0).unwrap(),
                VirtioQueueConfig::create(128, 0).unwrap(),
            ];

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::default()),
            );

            assert!(dev.activate(config).is_err());
        }
        // Success
        {
            let (_vmfd, irq_manager) = create_vm_and_irq_manager();
            let group = irq_manager
                .create_group(InterruptSourceType::LegacyIrq, 0, 1)
                .unwrap();
            let status = Arc::new(InterruptStatusRegister32::new());
            let notifier = create_queue_notifier(group, status, 0u32 as InterruptIndex);
            let queue: Queue = Queue::new(1024).unwrap();
            let queue2 = Queue::new(1024).unwrap();
            let queue_eventfd = Arc::new(EventFd::new(0).unwrap());
            let queue_eventfd2 = Arc::new(EventFd::new(0).unwrap());
            let queue_sizes = Arc::new(vec![128, 128]);
            let epoll_mgr = EpollManager::default();
            let mut dev: Net<Arc<GuestMemoryMmap>, Queue, GuestRegionMmap> = Net::new(
                String::from("test_vhosttap"),
                Some(&guest_mac),
                queue_sizes,
                epoll_mgr,
            )
            .unwrap();

            let queues = vec![
                VirtioQueueConfig::new(queue, queue_eventfd, notifier.clone(), 1),
                VirtioQueueConfig::new(queue2, queue_eventfd2, notifier, 1),
            ];

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::default()),
            );

            assert!(dev.activate(config).is_ok());
        }
    }

    #[test]
    fn test_vhost_kern_net_epoll_handler_handle_event() {
        let handler = create_vhost_kern_net_epoll_handler("test_1".to_string());
        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_vhost_kern_net_epoll_handler("test_2".to_string());

        // test for CTRL_SLOT
        let events = Events::with_data(&event_fd, CTRL_SLOT, event_set);
        handler.process(events, &mut event_op);
        handler.config.queues[0].generate_event().unwrap();
        handler.process(events, &mut event_op);

        // test for unknown event
        let events = Events::with_data(&event_fd, CTRL_SLOT + 1, event_set);
        handler.process(events, &mut event_op);
    }
}
