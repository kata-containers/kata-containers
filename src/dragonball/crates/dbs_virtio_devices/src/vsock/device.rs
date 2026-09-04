// Copyright 2022 Alibaba Cloud. All Rights Reserved.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
use std::any::Any;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use dbs_device::resources::ResourceConstraint;
use dbs_utils::epoll_manager::{EpollManager, SubscriberId};
use log::debug;
use log::trace;
use log::warn;
use virtio_queue::QueueT;
use vm_memory::GuestAddressSpace;
use vm_memory::GuestMemoryRegion;

use super::backend::VsockBackend;
use super::defs::uapi;
use super::epoll_handler::VsockEpollHandler;
use super::muxer::{Error as MuxerError, VsockGenericMuxer, VsockMuxer};
use super::{Result, VsockError};
use crate::device::{VirtioDeviceConfig, VirtioDeviceInfo};
use crate::{ActivateResult, ConfigResult, DbsGuestAddressSpace, VirtioDevice};

const VSOCK_DRIVER_NAME: &str = "virtio-vsock";
const VSOCK_CONFIG_SPACE_SIZE: usize = 8;
const VSOCK_AVAIL_FEATURES: u64 =
    (1u64 << uapi::VIRTIO_F_VERSION_1) | (1u64 << uapi::VIRTIO_F_IN_ORDER);

/// This is the `VirtioDevice` implementation for our vsock device. It handles
/// the virtio-level device logic: feature negociation, device configuration,
/// and device activation. The run-time device logic (i.e. event-driven data
/// handling) is implemented by `super::epoll_handler::EpollHandler`.
///
/// The vsock device has two input parameters: a CID to identify the device, and
/// a `VsockBackend` to use for offloading vsock traffic.
///
/// Upon its activation, the vsock device creates its `EpollHandler`, passes it
/// the event-interested file descriptors, and registers these descriptors with
/// the VMM `EpollContext`. Going forward, the `EpollHandler` will get notified
/// whenever an event occurs on the just-registered FDs:
/// - an RX queue FD;
/// - a TX queue FD;
/// - an event queue FD; and
/// - a backend FD.
pub struct Vsock<AS: GuestAddressSpace, M: VsockGenericMuxer = VsockMuxer> {
    cid: u64,
    queue_sizes: Arc<Vec<u16>>,
    device_info: VirtioDeviceInfo,
    subscriber_id: Option<SubscriberId>,
    /// Shared with the activated epoll handler rather than moved into it, so
    /// that the device can still reach the muxer once the handler owns it.
    /// The handler locks it per packet; the only other user is the device
    /// thread, so the lock is effectively uncontended.
    muxer: Option<Arc<Mutex<M>>>,
    phantom: PhantomData<AS>,
}

// Default muxer implementation of Vsock
impl<AS: GuestAddressSpace> Vsock<AS> {
    /// Create a new virtio-vsock device with the given VM CID and vsock
    /// backend.
    pub fn new(
        cid: u64,
        queue_sizes: Arc<Vec<u16>>,
        epoll_mgr: EpollManager,
        f_access_platform: bool,
    ) -> Result<Self> {
        let muxer = VsockMuxer::new(cid).map_err(VsockError::Muxer)?;
        Self::new_with_muxer(cid, queue_sizes, epoll_mgr, muxer, f_access_platform)
    }
}

impl<AS: GuestAddressSpace, M: VsockGenericMuxer> Vsock<AS, M> {
    pub(crate) fn new_with_muxer(
        cid: u64,
        queue_sizes: Arc<Vec<u16>>,
        epoll_mgr: EpollManager,
        muxer: M,
        f_access_platform: bool,
    ) -> Result<Self> {
        let mut config_space = Vec::with_capacity(VSOCK_CONFIG_SPACE_SIZE);
        for i in 0..VSOCK_CONFIG_SPACE_SIZE {
            config_space.push((cid >> (8 * i as u64)) as u8);
        }

        let mut avail_features = VSOCK_AVAIL_FEATURES;

        if f_access_platform {
            avail_features |= 1u64 << uapi::VIRTIO_F_ACCESS_PLATFORM;
        }

        Ok(Vsock {
            cid,
            queue_sizes: queue_sizes.clone(),
            device_info: VirtioDeviceInfo::new(
                VSOCK_DRIVER_NAME.to_string(),
                avail_features,
                queue_sizes,
                config_space,
                epoll_mgr,
            ),
            subscriber_id: None,
            muxer: Some(Arc::new(Mutex::new(muxer))),
            phantom: PhantomData,
        })
    }

    fn id(&self) -> &str {
        &self.device_info.driver_name
    }

    /// add backend for vsock muxer
    // NOTE: Backend is not allowed to add when vsock device is activated.
    pub fn add_backend(&mut self, backend: Box<dyn VsockBackend>, is_default: bool) -> Result<()> {
        // The muxer outlives activation now, so a missing one no longer says
        // the device was activated; the subscriber id does.
        if self.subscriber_id.is_some() {
            return Err(VsockError::Muxer(MuxerError::BackendAddAfterActivated));
        }
        self.muxer()?
            .add_backend(backend, is_default)
            .map_err(VsockError::Muxer)
    }

    /// Borrow the muxer, which the device shares with its epoll handler.
    ///
    /// A poisoned lock is reported, not panicked on. It means some thread
    /// panicked while mutating the muxer, so its state may be incomplete;
    /// refusing the operation is the safe answer, and taking the VMM down
    /// with a second panic is not.
    fn muxer(&self) -> Result<std::sync::MutexGuard<'_, M>> {
        self.muxer
            .as_ref()
            .ok_or(VsockError::MuxerUnavailable)?
            .lock()
            .map_err(|_| VsockError::MuxerLockPoisoned)
    }
}

impl<'a, AS: GuestAddressSpace, M: VsockGenericMuxer> crate::persist::VirtioDevicePersist<'a>
    for Vsock<AS, M>
{
    type State = crate::persist::VirtioDeviceInfoState;
    type SaveArgs = ();
    type RestoreArgs = ();
    type Error = crate::Error;

    /// Capture the guest-negotiated state of this device.
    ///
    /// Live vsock connection state is not captured: a snapshot must be taken
    /// at a clean quiesce point with no active connections.
    fn save_state(&mut self, _args: ()) -> crate::Result<Self::State> {
        Ok(self.device_info.save_state())
    }

    /// Restore the guest-negotiated state of this device.
    ///
    /// The device must have been re-created with the same configuration and
    /// must not have been activated yet.
    fn restore_state(&mut self, state: &Self::State, _args: ()) -> crate::Result<()> {
        self.device_info.restore_state(state)
    }
}

impl<AS, Q, R, M> VirtioDevice<AS, Q, R> for Vsock<AS, M>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send + 'static,
    R: GuestMemoryRegion + Sync + Send + 'static,
    M: VsockGenericMuxer + 'static,
{
    fn device_type(&self) -> u32 {
        uapi::VIRTIO_ID_VSOCK
    }

    fn queue_max_sizes(&self) -> &[u16] {
        &self.queue_sizes
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(target: "virtio-vsock", "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
            self.id(), page, value
        );
        self.device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        trace!(target: "virtio-vsock", "{}: VirtioDevice::read_config(0x{:x}, {:?})",
            self.id(), offset, data);
        self.device_info.read_config(offset, data)
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        trace!(target: "virtio-vsock", "{}: VirtioDevice::write_config(0x{:x}, {:?})",
        self.id(), offset, data);
        self.device_info.write_config(offset, data)
    }

    fn activate(&mut self, config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        trace!(target: "virtio-vsock", "{}: VirtioDevice::activate()", self.id());

        self.device_info.check_queue_sizes(&config.queues[..])?;
        // The device keeps its own handle rather than handing the muxer over.
        let muxer = self
            .muxer
            .as_ref()
            .ok_or(VsockError::MuxerUnavailable)
            .map_err(crate::Error::from)?
            .clone();
        let handler: VsockEpollHandler<AS, Q, R, M> =
            VsockEpollHandler::new(config, self.id().to_owned(), self.cid, muxer);

        self.subscriber_id = Some(self.device_info.register_event_handler(Box::new(handler)));

        Ok(())
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        trace!(target: "virtio-vsock", "{}: VirtioDevice::get_resource_requirements()", self.id());

        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.queue_sizes.len() + 1) as u32,
            });
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn remove(&mut self) {
        let subscriber_id = self.subscriber_id.take();
        if let Some(subscriber_id) = subscriber_id {
            match self.device_info.remove_event_handler(subscriber_id) {
                Ok(_) => debug!("virtio-vsock: removed subscriber_id {subscriber_id:?}"),
                Err(err) => warn!("virtio-vsock: failed to remove event handler: {err:?}"),
            };
        }
        // Drop the device's handle either way. Before activation this is the
        // only one, so the muxer's epoll FD and backend sockets are released
        // here; afterwards the removed handler drops the last one.
        self.muxer.take();
    }
}

#[cfg(test)]
mod tests {
    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::NoopNotifier;
    use kvm_ioctls::Kvm;
    use test_utils::skip_if_kvm_unaccessable;
    use virtio_queue::QueueSync;
    use vm_memory::{GuestAddress, GuestMemoryMmap, GuestRegionMmap};

    use super::super::defs::uapi;
    use super::super::tests::{test_bytes, TestContext, TestMuxer};
    use super::super::VsockChannel;
    use super::*;
    use crate::device::VirtioDeviceConfig;
    use crate::tests::create_address_space;
    use crate::VirtioQueueConfig;

    impl<AS: DbsGuestAddressSpace, M: VsockGenericMuxer + 'static> Vsock<AS, M> {
        pub fn mock_activate(
            &mut self,
            config: VirtioDeviceConfig<AS, QueueSync, GuestRegionMmap>,
        ) -> Result<VsockEpollHandler<AS, QueueSync, GuestRegionMmap, M>> {
            trace!(target: "virtio-vsock", "{}: VirtioDevice::activate_re()", self.id());

            self.device_info
                .check_queue_sizes(&config.queues[..])
                .unwrap();
            let handler: VsockEpollHandler<AS, QueueSync, GuestRegionMmap, M> =
                VsockEpollHandler::new(
                    config,
                    self.id().to_owned(),
                    self.cid,
                    self.muxer.as_ref().unwrap().clone(),
                );

            Ok(handler)
        }
    }

    #[test]
    fn test_virtio_device() {
        skip_if_kvm_unaccessable!();
        let mut ctx = TestContext::new();
        let device_features = VSOCK_AVAIL_FEATURES;
        let driver_features: u64 = VSOCK_AVAIL_FEATURES | 1 | (1 << 32);
        let device_pages = [
            (device_features & 0xffff_ffff) as u32,
            (device_features >> 32) as u32,
        ];
        let driver_pages = [
            (driver_features & 0xffff_ffff) as u32,
            (driver_features >> 32) as u32,
        ];
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(
                &ctx.device
            ),
            uapi::VIRTIO_ID_VSOCK
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(
                &ctx.device, 0
            ),
            device_pages[0]
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(
                &ctx.device, 1
            ),
            device_pages[1]
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(
                &ctx.device, 2
            ),
            0
        );

        // Ack device features, page 0.
        ctx.device
            .device_info
            .set_acked_features(0, driver_pages[0]);
        // Ack device features, page 1.
        ctx.device
            .device_info
            .set_acked_features(1, driver_pages[1]);
        // Ack some bogus page (i.e. 2). This should have no side effect.
        ctx.device.device_info.set_acked_features(2, 0);
        // Attempt to un-ack the first feature page. This should have no side effect.
        ctx.device
            .device_info
            .set_acked_features(0, !driver_pages[0]);
        // Check that no side effect are present, and that the acked features are exactly the same
        // as the device features.
        assert_eq!(
            ctx.device.device_info.acked_features(),
            device_features & driver_features
        );

        // Test reading 32-bit chunks.
        let mut data = [0u8; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut ctx.device,
            0,
            &mut data[..4],
        )
        .unwrap();
        test_bytes(&data[..], &(ctx.cid & 0xffff_ffff).to_le_bytes());
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut ctx.device,
            4,
            &mut data[4..],
        )
        .unwrap();
        test_bytes(&data[4..], &((ctx.cid >> 32) & 0xffff_ffff).to_le_bytes());

        // Test reading 64-bit.
        let mut data = [0u8; 8];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut ctx.device,
            0,
            &mut data,
        )
        .unwrap();
        test_bytes(&data, &ctx.cid.to_le_bytes());

        // Check out-of-bounds reading.
        let mut data = [0u8, 1, 2, 3, 4, 5, 6, 7];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut ctx.device,
            2,
            &mut data,
        )
        .unwrap();
        assert_eq!(data, [0u8, 0, 0, 0, 0, 0, 6, 7]);

        // Just covering lines here, since the vsock device has no writable config.
        // A warning is, however, logged, if the guest driver attempts to write any config data.
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
            &mut ctx.device,
            0,
            &data[..4],
        )
        .unwrap();

        let mem = GuestMemoryMmap::<()>::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
        let queues = vec![
            VirtioQueueConfig::<QueueSync>::create(2, 0).unwrap(),
            VirtioQueueConfig::<QueueSync>::create(2, 0).unwrap(),
            VirtioQueueConfig::<QueueSync>::create(2, 0).unwrap(),
        ];
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

        // Test activation.
        ctx.device.activate(config).unwrap();
    }

    /// Make a thread panic while holding `lock`, poisoning it.
    fn poison(lock: Arc<Mutex<TestMuxer>>) {
        let previous = std::panic::take_hook();
        // The panic below is the point of the test; don't print its backtrace.
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::thread::spawn(move || {
            let _guard = lock.lock().unwrap();
            panic!("poison the muxer lock");
        })
        .join();
        std::panic::set_hook(previous);
        assert!(result.is_err());
    }

    #[test]
    fn test_device_reaches_the_muxer_the_handler_uses() {
        skip_if_kvm_unaccessable!();

        // The point of sharing: after activation has handed the muxer to the
        // epoll handler, the device still sees the handler's view of it.
        let test_ctx = TestContext::new();
        let mut ctx = test_ctx.create_event_handler_context();

        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let config = VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>, QueueSync>::new(
            Arc::new(test_ctx.mem.clone()),
            create_address_space(),
            vm_fd,
            DeviceResources::new(),
            ctx.queues.drain(..).collect(),
            None,
            Arc::new(NoopNotifier::new()),
        );
        let shared = ctx.device.muxer.as_ref().unwrap().clone();
        ctx.device.activate(config).unwrap();

        shared.lock().unwrap().set_pending_rx(true);
        assert!(ctx.device.muxer().unwrap().has_pending_rx());
    }

    #[test]
    fn test_poisoned_muxer_lock_is_reported_not_panicked() {
        let mut ctx = TestContext::new();
        poison(ctx.device.muxer.as_ref().unwrap().clone());

        // A thread panicked mid-mutation, so the muxer's state may be
        // incomplete. Report that rather than panicking a second thread.
        let backend = Box::new(super::super::backend::VsockInnerBackend::new().unwrap());
        assert!(matches!(
            ctx.device.add_backend(backend, false),
            Err(VsockError::MuxerLockPoisoned)
        ));
    }

    #[test]
    fn test_activate_without_a_muxer_is_reported_not_panicked() {
        skip_if_kvm_unaccessable!();

        let test_ctx = TestContext::new();
        let mut ctx = test_ctx.create_event_handler_context();
        // `remove()` drops the device's muxer handle.
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::remove(
            &mut ctx.device,
        );

        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let config = VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>, QueueSync>::new(
            Arc::new(test_ctx.mem.clone()),
            create_address_space(),
            vm_fd,
            DeviceResources::new(),
            ctx.queues.drain(..).collect(),
            None,
            Arc::new(NoopNotifier::new()),
        );

        assert!(ctx.device.activate(config).is_err());
    }
}
