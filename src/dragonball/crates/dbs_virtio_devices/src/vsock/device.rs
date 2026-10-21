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
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use dbs_device::resources::ResourceConstraint;
use dbs_utils::epoll_manager::{EpollManager, SubscriberId};
use libc::EFD_NONBLOCK;
use log::debug;
use log::info;
use log::trace;
use log::warn;
use serde::{Deserialize, Serialize};
use virtio_queue::{QueueOwnedT, QueueT};
use vm_memory::Bytes;
use vm_memory::GuestAddressSpace;
use vm_memory::GuestMemoryRegion;
use vmm_sys_util::eventfd::EventFd;

use super::backend::VsockBackend;
use super::defs::uapi;
use super::epoll_handler::VsockEpollHandler;
use super::muxer::{Error as MuxerError, VsockGenericMuxer, VsockMuxer};
use super::{Result, VsockError};
use crate::device::{VirtioDeviceConfig, VirtioDeviceInfo};
use crate::{ActivateResult, ConfigResult, DbsGuestAddressSpace, VirtioDevice, VirtioQueueConfig};

const VSOCK_DRIVER_NAME: &str = "virtio-vsock";
const VSOCK_CONFIG_SPACE_SIZE: usize = 8;
const VSOCK_EVENT_TRANSPORT_RESET: [u8; 4] = 0u32.to_le_bytes();
const HANDLER_REGISTRATION_PENDING: u8 = 0;
const HANDLER_REGISTRATION_READY: u8 = 1;
const HANDLER_REGISTRATION_FAILED: u8 = 2;
const VSOCK_AVAIL_FEATURES: u64 =
    (1u64 << uapi::VIRTIO_F_VERSION_1) | (1u64 << uapi::VIRTIO_F_IN_ORDER);

fn transport_reset_acked(current_avail_idx: u16, baseline_avail_idx: u16) -> bool {
    current_avail_idx != baseline_avail_idx
}

/// Persisted vsock device state.
///
/// Keep the original `VirtioDeviceInfoState` fields flattened so snapshots
/// created before snapshot-boundary reset support remain readable.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct VsockState {
    #[serde(flatten)]
    pub device_info: crate::persist::VirtioDeviceInfoState,
    #[serde(default)]
    pub snapshot_reset_pending: bool,
    #[serde(default)]
    pub reset_ack_avail_idx: u16,
    /// Last host-side port allocated before the snapshot. Live connections are
    /// deliberately not restored; only allocator progress crosses restore.
    #[serde(default)]
    pub local_port_last: Option<u32>,
}

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
    muxer: Option<M>,
    local_port_watermark: Arc<AtomicU32>,
    snapshot_control: Arc<SnapshotControl>,
    phantom: PhantomData<AS>,
}

type SnapshotPrepare = Box<dyn Fn() -> crate::Result<u16> + Send + Sync>;
type SnapshotAckProbe = Box<dyn Fn(u16) -> crate::Result<bool> + Send + Sync>;

pub(super) struct SnapshotControl {
    pub(super) pending_event_ack: AtomicBool,
    reset_on_restore: AtomicBool,
    reset_ack_avail_idx: AtomicU16,
    reset_generation: AtomicU64,
    handler_registration: AtomicU8,
    pub(super) handler_ready_event: Arc<EventFd>,
    reset_transition: Mutex<()>,
    prepare: Mutex<Option<SnapshotPrepare>>,
    ack_probe: Mutex<Option<SnapshotAckProbe>>,
}

impl SnapshotControl {
    fn new() -> std::io::Result<Self> {
        Ok(Self {
            pending_event_ack: AtomicBool::new(false),
            reset_on_restore: AtomicBool::new(false),
            reset_ack_avail_idx: AtomicU16::new(0),
            reset_generation: AtomicU64::new(0),
            handler_registration: AtomicU8::new(HANDLER_REGISTRATION_PENDING),
            handler_ready_event: Arc::new(EventFd::new(EFD_NONBLOCK)?),
            reset_transition: Mutex::new(()),
            prepare: Mutex::new(None),
            ack_probe: Mutex::new(None),
        })
    }

    pub(super) fn begin_handler_registration(&self) {
        self.handler_registration
            .store(HANDLER_REGISTRATION_PENDING, Ordering::Release);
    }

    pub(super) fn complete_handler_registration(&self, ready: bool) {
        self.handler_registration.store(
            if ready {
                HANDLER_REGISTRATION_READY
            } else {
                HANDLER_REGISTRATION_FAILED
            },
            Ordering::Release,
        );
        info!(
            "vsock-handler event={}",
            if ready { "ready" } else { "failed" }
        );
    }

    pub(super) fn schedule_reset(&self) {
        self.reset_on_restore.store(true, Ordering::Release);
    }

    pub(super) fn request_reset(&self) -> crate::Result<bool> {
        let _transition = self.reset_transition.lock().unwrap();
        if !self.reset_on_restore.swap(false, Ordering::AcqRel) {
            return Ok(false);
        }
        let prepare = self.prepare.lock().unwrap();
        let Some(prepare) = prepare.as_ref() else {
            self.reset_on_restore.store(true, Ordering::Release);
            return Ok(false);
        };
        let ack_avail_idx = match prepare() {
            Ok(ack_avail_idx) => ack_avail_idx,
            Err(err) => {
                // Publishing did not complete, so retain the request for a
                // later retry instead of silently consuming the armed state.
                self.reset_on_restore.store(true, Ordering::Release);
                return Err(err);
            }
        };
        self.reset_ack_avail_idx
            .store(ack_avail_idx, Ordering::Release);
        let generation = self.reset_generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.pending_event_ack.store(true, Ordering::Release);
        info!("vsock-reset event=issued reset_generation={generation}");
        Ok(true)
    }

    fn queue_snapshot_reset(&self) -> crate::Result<u16> {
        if self.pending_event_ack.load(Ordering::Acquire) {
            return Ok(self.reset_ack_avail_idx.load(Ordering::Acquire));
        }

        self.schedule_reset();
        if !self.request_reset()? {
            return Err(crate::Error::InvalidInput);
        }
        Ok(self.reset_ack_avail_idx.load(Ordering::Acquire))
    }

    fn restore_snapshot_reset(&self, pending: bool, baseline: u16, fallback_reset: bool) {
        self.reset_ack_avail_idx.store(baseline, Ordering::Release);
        self.pending_event_ack.store(pending, Ordering::Release);
        self.reset_on_restore
            .store(!pending && fallback_reset, Ordering::Release);
        if pending {
            self.reset_generation.store(1, Ordering::Release);
        }
    }

    pub(super) fn poll_reset_ack(&self) -> crate::Result<bool> {
        // Serialize acknowledgement with reset publication. Otherwise an ACK
        // observed for generation N can clear the pending bit after generation
        // N+1 has reused it (a boolean ABA race).
        let _transition = self.reset_transition.lock().unwrap();
        if !self.pending_event_ack.load(Ordering::Acquire) {
            return Ok(true);
        }
        let reset_acked = self
            .ack_probe
            .lock()
            .unwrap()
            .as_ref()
            .map(|probe| probe(self.reset_ack_avail_idx.load(Ordering::Acquire)))
            .transpose()?
            .unwrap_or(false);
        if reset_acked {
            if self
                .pending_event_ack
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Wake the handler so RX deferred behind the ACK is drained.
                self.handler_ready_event.write(1)?;
            }
            info!(
                "vsock-reset event=ack reset_generation={}",
                self.reset_generation.load(Ordering::Acquire)
            );
        }
        Ok(reset_acked)
    }

    fn wait_for_reset_ack(&self, timeout: Duration) -> crate::Result<bool> {
        const POLL_INTERVAL: Duration = Duration::from_millis(1);
        let deadline = Instant::now() + timeout;
        loop {
            if self.poll_reset_ack()? {
                return Ok(true);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(false);
            }
            thread::sleep(POLL_INTERVAL.min(deadline - now));
        }
    }
}

pub(super) fn publish_transport_reset<AS, Q>(
    vm_as: &AS,
    eventq: &mut VirtioQueueConfig<Q>,
) -> crate::Result<u16>
where
    AS: GuestAddressSpace,
    Q: QueueT,
{
    let guard = vm_as.memory();
    let mem = guard.deref();
    let mut queue = eventq.queue.lock();
    let mut iter = queue.iter(mem)?;
    let mut chain = iter.next().ok_or(crate::Error::InvalidInput)?;
    let desc = chain.next().ok_or(crate::Error::DescriptorChainTooShort)?;
    if !desc.is_write_only() {
        return Err(crate::Error::UnexpectedReadOnlyDescriptor);
    }
    if desc.len() < 4 {
        return Err(crate::Error::DescriptorLengthTooSmall);
    }
    mem.write_slice(&VSOCK_EVENT_TRANSPORT_RESET, desc.addr())
        .map_err(crate::Error::GuestMemory)?;
    // Record guest producer progress before making the reset visible in the
    // used ring. Descriptors already pre-posted by the guest must not count as
    // an acknowledgement; the guest has to advance avail.idx after this
    // reset is published.
    let ack_avail_idx = queue.avail_idx(mem, Ordering::Acquire)?.0;
    queue.add_used(mem, chain.head_index(), 4)?;
    queue.enable_notification(mem)?;
    drop(queue);
    eventq.notify()?;
    Ok(ack_avail_idx)
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
        mut muxer: M,
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

        let local_port_watermark = Arc::new(AtomicU32::new(muxer.local_port_watermark()));
        muxer.set_local_port_watermark_observer(local_port_watermark.clone());

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
            muxer: Some(muxer),
            local_port_watermark,
            snapshot_control: Arc::new(
                SnapshotControl::new().map_err(VsockError::HandlerReadyEvent)?,
            ),
            phantom: PhantomData,
        })
    }

    pub fn handler_ready(&self) -> bool {
        self.snapshot_control
            .handler_registration
            .load(Ordering::Acquire)
            == HANDLER_REGISTRATION_READY
    }

    pub fn request_transport_reset(&self) -> crate::Result<bool> {
        self.snapshot_control.request_reset()
    }

    pub fn schedule_transport_reset(&self) {
        self.snapshot_control.schedule_reset();
    }

    pub fn transport_reset_pending(&self) -> bool {
        self.snapshot_control
            .pending_event_ack
            .load(Ordering::Acquire)
    }

    pub fn poll_transport_reset_ack(&self) -> crate::Result<bool> {
        self.snapshot_control.poll_reset_ack()
    }

    pub fn wait_for_transport_reset_ack(&self, timeout: Duration) -> crate::Result<bool> {
        self.snapshot_control.wait_for_reset_ack(timeout)
    }

    fn id(&self) -> &str {
        &self.device_info.driver_name
    }

    /// add backend for vsock muxer
    // NOTE: Backend is not allowed to add when vsock device is activated.
    pub fn add_backend(&mut self, backend: Box<dyn VsockBackend>, is_default: bool) -> Result<()> {
        if let Some(muxer) = self.muxer.as_mut() {
            muxer
                .add_backend(backend, is_default)
                .map_err(VsockError::Muxer)
        } else {
            Err(VsockError::Muxer(MuxerError::BackendAddAfterActivated))
        }
    }
}

impl<'a, AS: GuestAddressSpace, M: VsockGenericMuxer> crate::persist::VirtioDevicePersist<'a>
    for Vsock<AS, M>
{
    type State = VsockState;
    type SaveArgs = bool;
    type RestoreArgs = bool;
    type Error = crate::Error;

    /// Capture the guest-negotiated state of this device.
    ///
    /// Live vsock connection state is not captured: a snapshot must be taken
    /// at a clean quiesce point with no active connections.
    fn save_state(&mut self, reset_transport: bool) -> crate::Result<Self::State> {
        if reset_transport {
            self.snapshot_control.queue_snapshot_reset()?;
        }
        let snapshot_reset_pending = self
            .snapshot_control
            .pending_event_ack
            .load(Ordering::Acquire);
        let reset_ack_avail_idx = self
            .snapshot_control
            .reset_ack_avail_idx
            .load(Ordering::Acquire);
        Ok(VsockState {
            device_info: self.device_info.save_state(),
            snapshot_reset_pending,
            reset_ack_avail_idx,
            local_port_last: Some(self.local_port_watermark.load(Ordering::Acquire)),
        })
    }

    /// Restore the guest-negotiated state of this device.
    ///
    /// The device must have been re-created with the same configuration and
    /// must not have been activated yet.
    fn restore_state(&mut self, state: &Self::State, reset_pending: bool) -> crate::Result<()> {
        self.device_info.restore_state(&state.device_info)?;
        if let Some(watermark) = state.local_port_last {
            self.local_port_watermark
                .store(watermark, Ordering::Release);
            if let Some(muxer) = self.muxer.as_mut() {
                muxer.restore_local_port_watermark(watermark);
            }
            info!("vsock-restore event=local-port-watermark-restored value={watermark}");
        }
        self.snapshot_control.restore_snapshot_reset(
            state.snapshot_reset_pending,
            state.reset_ack_avail_idx,
            reset_pending,
        );
        Ok(())
    }
}

impl<AS, Q, R, M> VirtioDevice<AS, Q, R> for Vsock<AS, M>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Clone + Send + Sync + 'static,
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
        let reset_vm_as = config.vm_as.clone();
        let reset_queue = Arc::new(Mutex::new(
            config.queues[super::epoll_handler::QUEUE_CFG].clone(),
        ));
        let prepare_queue = reset_queue.clone();
        *self.snapshot_control.prepare.lock().unwrap() = Some(Box::new(move || {
            publish_transport_reset(&reset_vm_as, &mut prepare_queue.lock().unwrap())
        }));
        let ack_vm_as = config.vm_as.clone();
        *self.snapshot_control.ack_probe.lock().unwrap() = Some(Box::new(move |baseline| {
            let guard = ack_vm_as.memory();
            let mem = guard.deref();
            let mut queue_config = reset_queue.lock().unwrap();
            let queue = queue_config.queue.lock();
            let avail_idx = queue.avail_idx(mem, Ordering::Acquire)?;
            Ok(transport_reset_acked(avail_idx.0, baseline))
        }));
        let handler: VsockEpollHandler<AS, Q, R, M> = VsockEpollHandler::new(
            config,
            self.id().to_owned(),
            self.cid,
            // safe to unwrap, because we create muxer using New()
            self.muxer.take().unwrap(),
            self.snapshot_control.clone(),
        );

        self.snapshot_control.begin_handler_registration();
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
        } else {
            self.muxer.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::NoopNotifier;
    use kvm_ioctls::Kvm;
    use test_utils::skip_if_kvm_unaccessable;
    use virtio_queue::QueueSync;
    use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap, GuestRegionMmap};

    use super::super::defs::uapi;
    use super::super::epoll_handler::QUEUE_CFG;
    use super::super::tests::{test_bytes, TestContext};
    use super::*;
    use crate::device::VirtioDeviceConfig;
    use crate::tests::create_address_space;
    use crate::tests::VIRTQ_DESC_F_WRITE;
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
            let reset_vm_as = config.vm_as.clone();
            let reset_queue = Arc::new(Mutex::new(config.queues[QUEUE_CFG].clone()));
            let prepare_queue = reset_queue.clone();
            *self.snapshot_control.prepare.lock().unwrap() = Some(Box::new(move || {
                publish_transport_reset(&reset_vm_as, &mut prepare_queue.lock().unwrap())
            }));
            let ack_vm_as = config.vm_as.clone();
            *self.snapshot_control.ack_probe.lock().unwrap() = Some(Box::new(move |baseline| {
                let guard = ack_vm_as.memory();
                let mem = guard.deref();
                let mut queue_config = reset_queue.lock().unwrap();
                let queue = queue_config.queue.lock();
                let avail_idx = queue.avail_idx(mem, Ordering::Acquire)?;
                Ok(transport_reset_acked(avail_idx.0, baseline))
            }));
            let handler: VsockEpollHandler<AS, QueueSync, GuestRegionMmap, M> =
                VsockEpollHandler::new(
                    config,
                    self.id().to_owned(),
                    self.cid,
                    // safe to unwrap, because we create muxer using New()
                    self.muxer.take().unwrap(),
                    self.snapshot_control.clone(),
                );

            Ok(handler)
        }
    }

    #[test]
    fn test_poll_transport_reset_ack_without_queue_kick() {
        let control = SnapshotControl::new().unwrap();
        control.pending_event_ack.store(true, Ordering::Release);
        control.reset_ack_avail_idx.store(7, Ordering::Release);
        *control.ack_probe.lock().unwrap() = Some(Box::new(|baseline| Ok(baseline == 7)));

        assert!(control.poll_reset_ack().unwrap());
        assert!(!control.pending_event_ack.load(Ordering::Acquire));
        assert_eq!(control.handler_ready_event.read().unwrap(), 1);
    }

    #[test]
    fn test_save_preserves_an_already_pending_reset() {
        let mut test_ctx = TestContext::new();
        test_ctx
            .device
            .snapshot_control
            .pending_event_ack
            .store(true, Ordering::Release);
        test_ctx
            .device
            .snapshot_control
            .reset_ack_avail_idx
            .store(41, Ordering::Release);

        let state =
            crate::persist::VirtioDevicePersist::save_state(&mut test_ctx.device, false).unwrap();
        assert!(state.snapshot_reset_pending);
        assert_eq!(state.reset_ack_avail_idx, 41);
    }

    #[test]
    fn test_transport_reset_ack_avail_idx_progress() {
        assert!(!transport_reset_acked(7, 7));
        assert!(transport_reset_acked(8, 7));
        assert!(transport_reset_acked(0, u16::MAX));
    }

    #[test]
    fn test_vsock_state_reads_legacy_json_and_roundtrips_reset_state() {
        let legacy = serde_json::json!({
            "avail_features": 7,
            "acked_features": 3,
            "config_space": [1, 2, 3]
        });
        let legacy_state: VsockState = serde_json::from_value(legacy).unwrap();
        assert_eq!(legacy_state.device_info.avail_features, 7);
        assert!(!legacy_state.snapshot_reset_pending);
        assert_eq!(legacy_state.reset_ack_avail_idx, 0);
        assert_eq!(legacy_state.local_port_last, None);

        let state = VsockState {
            device_info: legacy_state.device_info,
            snapshot_reset_pending: true,
            reset_ack_avail_idx: 19,
            local_port_last: Some((1 << 30) + 7),
        };
        let roundtrip: VsockState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(roundtrip, state);
    }

    #[test]
    fn test_snapshot_reset_publish_failure_is_retryable_and_pending_is_idempotent() {
        use std::sync::atomic::AtomicUsize;

        let control = SnapshotControl::new().unwrap();
        assert!(control.queue_snapshot_reset().is_err());
        assert!(!control.pending_event_ack.load(Ordering::Acquire));
        assert!(control.reset_on_restore.load(Ordering::Acquire));

        let publishes = Arc::new(AtomicUsize::new(0));
        let publishes_for_prepare = publishes.clone();
        *control.prepare.lock().unwrap() = Some(Box::new(move || {
            publishes_for_prepare.fetch_add(1, Ordering::AcqRel);
            Ok(23)
        }));
        // The failed attempt retained the arm, so retry does not need to arm
        // the reset again.
        assert!(control.request_reset().unwrap());
        assert_eq!(control.queue_snapshot_reset().unwrap(), 23);
        assert_eq!(publishes.load(Ordering::Acquire), 1);

        let failed_publish = SnapshotControl::new().unwrap();
        *failed_publish.prepare.lock().unwrap() =
            Some(Box::new(|| Err(crate::Error::InvalidInput)));
        failed_publish.schedule_reset();
        assert!(failed_publish.request_reset().is_err());
        assert!(failed_publish.reset_on_restore.load(Ordering::Acquire));
        *failed_publish.prepare.lock().unwrap() = Some(Box::new(|| Ok(31)));
        assert!(failed_publish.request_reset().unwrap());
        assert!(failed_publish.pending_event_ack.load(Ordering::Acquire));
    }

    #[test]
    fn test_ack_cannot_clear_a_new_reset_generation() {
        use std::sync::Barrier;

        let control = Arc::new(SnapshotControl::new().unwrap());
        *control.prepare.lock().unwrap() = Some(Box::new(|| Ok(17)));
        control.schedule_reset();
        assert!(control.request_reset().unwrap());

        let probe_entered = Arc::new(Barrier::new(2));
        let release_probe = Arc::new(Barrier::new(2));
        let probe_entered_cb = probe_entered.clone();
        let release_probe_cb = release_probe.clone();
        *control.ack_probe.lock().unwrap() = Some(Box::new(move |_| {
            probe_entered_cb.wait();
            release_probe_cb.wait();
            Ok(true)
        }));

        let poll_control = control.clone();
        let poll = std::thread::spawn(move || poll_control.poll_reset_ack().unwrap());
        probe_entered.wait();
        assert!(control.reset_transition.try_lock().is_err());

        control.schedule_reset();
        let publish_started = Arc::new(Barrier::new(2));
        let publish_started_thread = publish_started.clone();
        let publish_control = control.clone();
        let publish = std::thread::spawn(move || {
            publish_started_thread.wait();
            publish_control.request_reset().unwrap()
        });
        publish_started.wait();

        release_probe.wait();
        assert!(poll.join().unwrap());
        assert!(publish.join().unwrap());
        assert!(control.pending_event_ack.load(Ordering::Acquire));
        assert_eq!(control.reset_generation.load(Ordering::Acquire), 2);
    }

    #[test]
    fn test_restore_snapshot_reset_truth_table() {
        let new_snapshot = SnapshotControl::new().unwrap();
        new_snapshot.restore_snapshot_reset(true, 29, true);
        assert!(new_snapshot.pending_event_ack.load(Ordering::Acquire));
        assert!(!new_snapshot.reset_on_restore.load(Ordering::Acquire));
        assert_eq!(new_snapshot.reset_ack_avail_idx.load(Ordering::Acquire), 29);

        let legacy_snapshot = SnapshotControl::new().unwrap();
        legacy_snapshot.restore_snapshot_reset(false, 0, true);
        assert!(!legacy_snapshot.pending_event_ack.load(Ordering::Acquire));
        assert!(legacy_snapshot.reset_on_restore.load(Ordering::Acquire));

        let no_reset = SnapshotControl::new().unwrap();
        no_reset.restore_snapshot_reset(false, 0, false);
        assert!(!no_reset.pending_event_ack.load(Ordering::Acquire));
        assert!(!no_reset.reset_on_restore.load(Ordering::Acquire));
    }

    #[test]
    fn test_publish_transport_reset() {
        skip_if_kvm_unaccessable!();
        const EVENT_ADDR: u64 = 0x0060_0000;

        let test_ctx = TestContext::new();
        let mut ctx = test_ctx.create_event_handler_context();
        ctx._guest_evvq
            .dtable(0)
            .set(EVENT_ADDR, 4, VIRTQ_DESC_F_WRITE, 0);
        ctx._guest_evvq.avail.ring(0).store(0);
        ctx._guest_evvq.avail.idx().store(1);
        ctx.arti_activate(&test_ctx.mem);

        let handler = ctx.epoll_handler.as_mut().unwrap();
        let vm_as = handler.config.vm_as.clone();
        let ack_avail_idx =
            publish_transport_reset(&vm_as, &mut handler.config.queues[QUEUE_CFG]).unwrap();

        assert_eq!(ack_avail_idx, 1);
        assert_eq!(ctx._guest_evvq.used.idx().load(), 1);
        assert_eq!(
            test_ctx
                .mem
                .read_obj::<[u8; 4]>(GuestAddress(EVENT_ADDR))
                .unwrap(),
            VSOCK_EVENT_TRANSPORT_RESET
        );
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
}
