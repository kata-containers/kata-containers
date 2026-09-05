// Copyright 2022 Alibaba Cloud. All Rights Reserved.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
use std::any::Any;
use std::collections::HashSet;
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
use super::persist::VsockState;
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
    /// that `save_state()` can still read the live connection state. The
    /// handler locks it per packet; contention is between the device thread
    /// and the rare snapshot, so the lock is effectively uncontended.
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
    /// panicked while mutating the muxer, so its connection state may be
    /// incomplete -- and this method's callers are the snapshot paths. A
    /// snapshot that under-reports live connections restores a guest holding
    /// sockets whose peers are gone, which is the exact failure
    /// reset-on-restore exists to prevent, so refusing to save is the safe
    /// answer. Taking the VMM down with a second panic is not.
    fn muxer(&self) -> Result<std::sync::MutexGuard<'_, M>> {
        self.muxer
            .as_ref()
            .ok_or(VsockError::MuxerUnavailable)?
            .lock()
            .map_err(|_| VsockError::MuxerLockPoisoned)
    }
}

// Snapshot support is implemented for the concrete muxer rather than for any
// `VsockGenericMuxer`: capturing connection identity is a property of the real
// muxer, not of the channel abstraction, and a fake muxer could only mirror it
// -- which is the kind of parallel state this device state exists to avoid.
impl<'a, AS: GuestAddressSpace> crate::persist::VirtioDevicePersist<'a> for Vsock<AS, VsockMuxer> {
    type State = VsockState;
    type SaveArgs = ();
    type RestoreArgs = ();
    type Error = crate::Error;

    /// Capture the guest-negotiated state of this device, together with the
    /// identity of every connection that is live right now.
    ///
    /// The host half of a live connection cannot be captured -- it is a file
    /// descriptor, an epoll registration and a peer process -- so the
    /// snapshot records only each connection's port tuple, which is enough
    /// for [`restore_state`](Self::restore_state) to tell the restored guest
    /// that the connection is gone.
    ///
    /// The tuples are read from the live muxer, which the device shares with
    /// its epoll handler, so they cannot disagree with it.
    fn save_state(&mut self, _args: ()) -> crate::Result<Self::State> {
        let reset_connections = self.muxer()?.connections_to_reset();
        Ok(VsockState {
            device_info: self.device_info.save_state(),
            reset_connections,
        })
    }

    /// Restore the guest-negotiated state of this device and queue one
    /// `VSOCK_OP_RST` per connection the snapshot recorded.
    ///
    /// The device must have been re-created with the same configuration and
    /// must not have been activated yet: the resets are queued on the muxer
    /// the device still owns, and delivered by the RX pass that
    /// [`activate`](VirtioDevice::activate) runs before the vCPUs resume.
    /// No connection object is created -- there is nothing to connect to.
    fn restore_state(&mut self, state: &Self::State, _args: ()) -> crate::Result<()> {
        // Check the reset list for self-consistency first, so a malformed
        // snapshot is refused before anything is touched.
        let mut seen = HashSet::with_capacity(state.reset_connections.len());
        for id in &state.reset_connections {
            if !seen.insert(*id) {
                warn!(
                    "virtio-vsock: duplicate reset tuple in snapshot (lp={}, pp={})",
                    id.local_port, id.peer_port
                );
                return Err(crate::Error::InvalidInput);
            }
        }

        // Then the guest-negotiated state, which is the check most likely to
        // reject a mismatched snapshot: it refuses a feature set the guest
        // never negotiated against, and does so before mutating anything, so
        // a device rebuilt from the wrong configuration keeps its muxer
        // untouched.
        self.device_info.restore_state(&state.device_info)?;

        // Queue the resets last. This validates the list against the muxer's
        // own bound before extending the queue, so it too cannot apply half
        // of a rejected list.
        self.muxer()?
            .queue_restore_resets(&state.reset_connections)
            .map_err(|err| {
                warn!("virtio-vsock: failed to queue restore resets: {err:?}");
                crate::Error::InvalidInput
            })
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
        // The device keeps its own handle: sharing the muxer, rather than
        // handing it over, is what lets `save_state()` read live connection
        // state after activation.
        let muxer = self
            .muxer
            .as_ref()
            .ok_or(VsockError::MuxerUnavailable)
            .map_err(crate::Error::from)?
            .clone();
        let mut handler: VsockEpollHandler<AS, Q, R, M> =
            VsockEpollHandler::new(config, self.id().to_owned(), self.cid, muxer);

        // On the restore path the muxer already holds a reset for every
        // connection that was live when the snapshot was taken, and nothing
        // else would deliver them: `init()` only registers epoll listeners,
        // and `process_rx()` otherwise runs only on a queue or backend event.
        // The transport has replayed the queue addresses and MSI
        // configuration by now, so this pass sees the descriptors the guest
        // posted before the snapshot and can raise the interrupt before the
        // vCPUs resume. On cold boot a fresh muxer has nothing pending and
        // this is a no-op.
        handler.flush_pending_rx();

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
    use super::super::persist::VsockConnectionId;
    use super::super::tests::{test_bytes, TestContext};
    use super::super::VsockChannel;
    use super::*;
    use crate::device::VirtioDeviceConfig;
    use crate::persist::VirtioDevicePersist;
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
                    // safe to unwrap, because we create muxer using New()
                    self.muxer.take().unwrap(),
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

    fn conn_id(local_port: u32, peer_port: u32) -> VsockConnectionId {
        VsockConnectionId {
            local_port,
            peer_port,
        }
    }

    /// Build a device config from the shared harness. `ctx.queues` is
    /// muxer-agnostic, so a real-muxer device can be activated with it.
    fn config_from(
        test_ctx: &TestContext,
        ctx: &mut super::super::tests::EventHandlerContext<'_>,
    ) -> VirtioDeviceConfig<Arc<GuestMemoryMmap<()>>, QueueSync> {
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>, QueueSync>::new(
            Arc::new(test_ctx.mem.clone()),
            create_address_space(),
            vm_fd,
            DeviceResources::new(),
            ctx.queues.drain(..).collect(),
            None,
            Arc::new(NoopNotifier::new()),
        )
    }

    /// A device with a real `VsockMuxer`, for the checks that exercise the
    /// muxer's own limits rather than the mock's bookkeeping.
    fn real_muxer_device() -> Vsock<Arc<GuestMemoryMmap<()>>> {
        Vsock::new(
            52,
            Arc::new(super::super::defs::QUEUE_SIZES.to_vec()),
            EpollManager::default(),
            false,
        )
        .unwrap()
    }

    /// Make a thread panic while holding `lock`, poisoning it.
    fn poison(lock: Arc<Mutex<VsockMuxer>>) {
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
    fn test_poisoned_muxer_lock_is_reported_not_panicked() {
        let mut device = real_muxer_device();
        poison(device.muxer.as_ref().unwrap().clone());

        // A thread panicked mid-mutation, so the muxer's state may be
        // incomplete. Report that rather than panicking a second thread.
        let backend = Box::new(super::super::backend::VsockInnerBackend::new().unwrap());
        assert!(matches!(
            device.add_backend(backend, false),
            Err(VsockError::MuxerLockPoisoned)
        ));
        assert!(matches!(
            device.save_state(()),
            Err(crate::Error::VirtioVsockError(
                VsockError::MuxerLockPoisoned
            ))
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

        let config = config_from(&test_ctx, &mut ctx);
        assert!(ctx.device.activate(config).is_err());
    }

    #[test]
    fn test_save_state_captures_what_the_muxer_reports() {
        let mut device = real_muxer_device();

        // A cold-booted device with no connections has nothing to reset --
        // and neither does one that was merely paused and resumed, since
        // neither touches the muxer's connection state.
        let state = device.save_state(()).unwrap();
        assert!(state.reset_connections.is_empty());
        assert_eq!(
            state.device_info,
            device.device_info.save_state(),
            "the guest-negotiated state must be captured unchanged"
        );

        // Whatever the live muxer owes the guest is what gets captured. The
        // union itself is the muxer's business and is tested there; here the
        // point is only that `save_state()` reads the real thing.
        let stale = vec![conn_id(1024, 7), conn_id(1025, 9)];
        device
            .muxer()
            .unwrap()
            .queue_restore_resets(&stale)
            .unwrap();
        assert_eq!(device.save_state(()).unwrap().reset_connections, stale);
    }

    #[test]
    fn test_restore_state_queues_one_reset_per_connection() {
        let mut device = real_muxer_device();
        let stale = vec![conn_id(1024, 7), conn_id(1025, 9)];
        let state = VsockState {
            device_info: device.device_info.save_state(),
            reset_connections: stale.clone(),
        };

        device.restore_state(&state, ()).unwrap();

        let muxer = device.muxer().unwrap();
        assert!(muxer.has_pending_rx());
        // No connection object is created; the resets are still owed, so a
        // snapshot taken now carries them forward rather than losing them.
        assert_eq!(muxer.connections_to_reset(), stale);
    }

    #[test]
    fn test_restore_state_without_connections_queues_nothing() {
        let mut device = real_muxer_device();
        let state = VsockState {
            device_info: device.device_info.save_state(),
            reset_connections: Vec::new(),
        };

        device.restore_state(&state, ()).unwrap();
        assert!(!device.muxer().unwrap().has_pending_rx());
    }

    #[test]
    fn test_restore_state_refuses_malformed_reset_state() {
        // A repeated tuple would reset the same guest socket twice, and says
        // the state was not produced by `save_state()`.
        let mut device = real_muxer_device();
        let state = VsockState {
            device_info: device.device_info.save_state(),
            reset_connections: vec![conn_id(1024, 7), conn_id(1024, 7)],
        };
        assert!(matches!(
            device.restore_state(&state, ()),
            Err(crate::Error::InvalidInput)
        ));
        assert!(!device.muxer().unwrap().has_pending_rx());

        // More tuples than any snapshot could legitimately carry.
        let mut device = real_muxer_device();
        let excess = super::super::muxer::defs::MAX_RESTORE_RESETS + 1;
        let state = VsockState {
            device_info: device.device_info.save_state(),
            reset_connections: (0..excess as u32).map(|i| conn_id(1024 + i, 7)).collect(),
        };
        assert!(matches!(
            device.restore_state(&state, ()),
            Err(crate::Error::InvalidInput)
        ));
        assert!(!device.muxer().unwrap().has_pending_rx());

        // A feature set the guest never negotiated against. This is checked
        // before the resets are queued, so a device rebuilt from the wrong
        // configuration keeps its muxer untouched.
        let mut device = real_muxer_device();
        let mut device_info = device.device_info.save_state();
        device_info.avail_features ^= 1;
        let state = VsockState {
            device_info,
            reset_connections: vec![conn_id(1024, 7)],
        };
        assert!(matches!(
            device.restore_state(&state, ()),
            Err(crate::Error::InvalidInput)
        ));
        assert!(!device.muxer().unwrap().has_pending_rx());
    }

    #[test]
    fn test_activate_delivers_restored_resets() {
        skip_if_kvm_unaccessable!();

        // Queueing a reset is not delivering it: `init()` only registers
        // epoll listeners, and `process_rx()` otherwise runs only on a queue
        // or backend event. Activation must make the pass itself, so the
        // guest sees the resets before its vCPUs resume.
        let test_ctx = TestContext::new();
        let mut ctx = test_ctx.create_event_handler_context();
        let mut device = real_muxer_device();
        let stale = conn_id(1024, 7);
        device
            .restore_state(
                &VsockState {
                    device_info: device.device_info.save_state(),
                    reset_connections: vec![stale],
                },
                (),
            )
            .unwrap();

        let config = config_from(&test_ctx, &mut ctx);
        device.activate(config).unwrap();

        // The descriptor the guest posted before the snapshot was used, and
        // the muxer now owes it nothing -- the reset is what consumed it.
        // That the reset is well formed is the muxer's contract, asserted
        // against the real builder in `muxer_impl`'s tests.
        assert_eq!(ctx.guest_rxvq.used.idx().load(), 1);
        assert!(device.muxer().unwrap().connections_to_reset().is_empty());
    }

    #[test]
    fn test_activate_on_cold_boot_touches_no_descriptor() {
        skip_if_kvm_unaccessable!();

        let test_ctx = TestContext::new();
        let mut ctx = test_ctx.create_event_handler_context();
        let mut device = real_muxer_device();

        let config = config_from(&test_ctx, &mut ctx);
        device.activate(config).unwrap();

        // A fresh muxer has nothing pending, so activation is a no-op.
        assert_eq!(ctx.guest_rxvq.used.idx().load(), 0);
    }

    #[test]
    fn test_device_reaches_the_muxer_the_handler_uses() {
        skip_if_kvm_unaccessable!();

        // The point of sharing the muxer: after activation has handed it to
        // the epoll handler, the device still sees the handler's view of it,
        // so a snapshot cannot disagree with the running device.
        let test_ctx = TestContext::new();
        let mut ctx = test_ctx.create_event_handler_context();
        let mut device = real_muxer_device();
        let shared = device.muxer.as_ref().unwrap().clone();
        let config = config_from(&test_ctx, &mut ctx);
        device.activate(config).unwrap();

        let stale = conn_id(1024, 7);
        shared
            .lock()
            .unwrap()
            .queue_restore_resets(&[stale])
            .unwrap();
        assert_eq!(
            device.save_state(()).unwrap().reset_connections,
            vec![stale]
        );
    }
}
