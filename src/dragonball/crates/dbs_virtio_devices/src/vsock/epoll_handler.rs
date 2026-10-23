// Copyright 2022 Alibaba Cloud. All Rights Reserved.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::ops::Deref;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use dbs_utils::epoll_manager::{EventOps, EventSet, Events, MutEventSubscriber};
use log::{error, info, trace, warn};
use virtio_queue::{QueueOwnedT, QueueSync, QueueT};
use vm_memory::{GuestMemoryRegion, GuestRegionMmap};

use super::defs;
use super::device::SnapshotControl;
use super::muxer::{VsockGenericMuxer, VsockMuxer};
use super::packet::VsockPacket;
use crate::device::VirtioDeviceConfig;
use crate::{DbsGuestAddressSpace, Result as VirtioResult};

const QUEUE_RX: usize = 0;
const QUEUE_TX: usize = 1;
pub(super) const QUEUE_CFG: usize = 2;

// TODO: Detect / handle queue deadlock:
// 1. If `self.backend.send_pkt()` errors out, TX queue processing will halt.
//    Try to process any pending backend RX, then try TX again. If it fails
//    again, we have a deadlock.
// 2. If the driver halts RX queue processing, we'll need to notify
//    `self.backend`, so that it can unregister any EPOLLIN listeners, since
//    otherwise it will keep spinning, unable to consume its EPOLLIN events.

/// The vsock `EpollHandler` implements the runtime logic of our vsock device:
/// 1. Respond to TX queue events by wrapping virtio buffers into
///    `VsockPacket`s, then sending those packets to the `VsockBackend`;
/// 2. Forward backend FD event notifications to the `VsockBackend`;
/// 3. Fetch incoming packets from the `VsockBackend` and place them into the
///    virtio RX queue;
/// 4. Whenever we have processed some virtio buffers (either TX or RX), let the
///    driver know by raising our assigned IRQ.
///
/// In a nutshell, the `EpollHandler` logic looks like this:
/// - on TX queue event:
///   - fetch all packets from the TX queue and send them to the backend; then
///   - if the backend has queued up any incoming packets, fetch them into any
///     available RX buffers.
/// - on RX queue event:
///   - fetch any incoming packets, queued up by the backend, into newly
///     available RX buffers.
/// - on backend event:
///   - forward the event to the backend; then
///   - again, attempt to fetch any incoming packets queued by the backend into
///     virtio RX buffers.
pub struct VsockEpollHandler<
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
    M: VsockGenericMuxer = VsockMuxer,
> {
    pub(crate) config: VirtioDeviceConfig<AS, Q, R>,
    id: String,
    pub(crate) muxer: M,
    _cid: u64,
    snapshot_control: Arc<SnapshotControl>,
}

impl<AS, Q, R, M> VsockEpollHandler<AS, Q, R, M>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send,
    R: GuestMemoryRegion,
    M: VsockGenericMuxer,
{
    pub(super) fn new(
        config: VirtioDeviceConfig<AS, Q, R>,
        id: String,
        cid: u64,
        muxer: M,
        snapshot_control: Arc<SnapshotControl>,
    ) -> Self {
        VsockEpollHandler {
            config,
            id,
            _cid: cid,
            muxer,
            snapshot_control,
        }
    }

    /// Signal the guest driver that we've used some virtio buffers that it had
    /// previously made available.
    pub(crate) fn signal_used_queue(&self, idx: usize) -> VirtioResult<()> {
        trace!("{}: raising IRQ", self.id);
        self.config.queues[idx].notify().map_err(|e| {
            error!("{}: failed to signal used queue {}, {:?}", self.id, idx, e);
            e
        })
    }

    /// Walk the driver-provided RX queue buffers and attempt to fill them up
    /// with any data that we have pending.
    fn process_rx(&mut self, mem: &AS::M) {
        trace!("{}: epoll_handler::process_rx()", self.id);
        if self
            .snapshot_control
            .pending_event_ack
            .load(Ordering::Acquire)
        {
            return;
        }
        let mut raise_irq = false;
        {
            let rxvq = &mut self.config.queues[QUEUE_RX].queue_mut().lock();
            loop {
                let mut iter = match rxvq.iter(mem) {
                    Err(e) => {
                        error!("{}: failed to process rx queue. {}", self.id, e);
                        return;
                    }
                    Ok(iter) => iter,
                };

                if let Some(mut desc_chain) = iter.next() {
                    let used_len = match VsockPacket::from_rx_virtq_head(&mut desc_chain) {
                        Ok(mut pkt) => {
                            if self.muxer.recv_pkt(&mut pkt).is_ok() {
                                pkt.hdr().len() as u32 + pkt.len()
                            } else {
                                // We are using a consuming iterator over the virtio buffers, so, if we
                                // can't fill in this buffer, we'll need to undo the last iterator step.
                                iter.go_to_previous_position();
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("{}: RX queue error: {:?}", self.id, e);
                            0
                        }
                    };

                    raise_irq = true;
                    let _ = rxvq.add_used(mem, desc_chain.head_index(), used_len);
                } else {
                    break;
                }
            }
        }
        if raise_irq {
            if let Err(e) = self.signal_used_queue(QUEUE_RX) {
                error!("{}: failed to notify guest for RX queue, {:?}", self.id, e);
            }
        }
    }

    /// Walk the dirver-provided TX queue buffers, package them up as vsock
    /// packets, and send them to the backend for processing.
    fn process_tx(&mut self, mem: &AS::M) {
        trace!("{}: epoll_handler::process_tx()", self.id);
        let mut have_used = false;

        {
            let txvq = &mut self.config.queues[QUEUE_TX].queue_mut().lock();

            loop {
                let mut iter = match txvq.iter(mem) {
                    Err(e) => {
                        error!("{}: failed to process tx queue. {}", self.id, e);
                        return;
                    }
                    Ok(iter) => iter,
                };

                if let Some(mut desc_chain) = iter.next() {
                    let pkt = match VsockPacket::from_tx_virtq_head(&mut desc_chain) {
                        Ok(pkt) => pkt,
                        Err(e) => {
                            error!("{}: error reading TX packet: {:?}", self.id, e);
                            have_used = true;
                            let _ = txvq.add_used(mem, desc_chain.head_index(), 0);
                            continue;
                        }
                    };

                    if self.muxer.send_pkt(&pkt).is_err() {
                        iter.go_to_previous_position();
                        break;
                    }

                    have_used = true;
                    let _ = txvq.add_used(mem, desc_chain.head_index(), 0);
                } else {
                    break;
                }
            }
        }
        if have_used {
            if let Err(e) = self.signal_used_queue(QUEUE_TX) {
                error!("{}: failed to notify guest for TX queue, {:?}", self.id, e);
            }
        }
    }

    pub(crate) fn handle_rxq_event(&mut self, mem: &AS::M) {
        trace!("{}: handle RX queue event", self.id);
        if let Err(e) = self.config.queues[QUEUE_RX].consume_event() {
            error!("{}: failed to consume rx queue event, {:?}", self.id, e);
        } else if self.muxer.has_pending_rx() {
            self.process_rx(mem);
        }
    }

    pub(crate) fn handle_txq_event(&mut self, mem: &AS::M) {
        trace!("{}: handle TX queue event", self.id);
        if let Err(e) = self.config.queues[QUEUE_TX].consume_event() {
            error!("{}: failed to consume tx queue event, {:?}", self.id, e);
        } else {
            self.process_tx(mem);
            // The backend may have queued up responses to the packets
            // we sent during TX queue processing. If that happened, we
            // need to fetch those responses and place them into RX
            // buffers.
            if self.muxer.has_pending_rx() {
                self.process_rx(mem);
            }
        }
    }

    fn handle_evq_event(&mut self, mem: &AS::M) {
        trace!("{}: handle event queue event", self.id);
        if let Err(e) = self.config.queues[QUEUE_CFG].consume_event() {
            error!("{}: failed to consume config queue event, {:?}", self.id, e);
            return;
        }

        let reset_pending = self
            .snapshot_control
            .pending_event_ack
            .load(Ordering::Acquire);
        let reset_acked = if reset_pending {
            match self.snapshot_control.poll_reset_ack() {
                Ok(acked) => acked,
                Err(e) => {
                    error!("{}: failed to poll transport reset ACK: {}", self.id, e);
                    false
                }
            }
        } else {
            false
        };
        if (reset_acked || !reset_pending) && self.muxer.has_pending_rx() {
            self.process_rx(mem);
        }
    }

    pub(crate) fn notify_backend_event(&mut self, events: &Events, mem: &AS::M) {
        trace!("{}: backend event", self.id);
        let events = epoll::Events::from_bits(events.event_set().bits()).unwrap();
        self.muxer.notify(events);
        // After the backend has been kicked, it might've freed up some
        // resources, so we can attempt to send it more data to process. In
        // particular, if `self.backend.send_pkt()` halted the TX queue
        // processing (by reurning an error) at some point in the past, now is
        // the time to try walking the TX queue again.
        self.process_tx(mem);
        // This event may have caused some packets to be queued up by the
        // backend. Make sure they are processed.
        if self.muxer.has_pending_rx() {
            self.process_rx(mem);
        }
    }
}

impl<AS, Q, R, M> MutEventSubscriber for VsockEpollHandler<AS, Q, R, M>
where
    AS: DbsGuestAddressSpace,
    Q: QueueT + Send,
    R: GuestMemoryRegion,
    M: VsockGenericMuxer + 'static,
{
    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        let guard = self.config.lock_guest_memory();
        let mem = guard.deref();

        match events.data() {
            defs::RXQ_EVENT => self.handle_rxq_event(mem),
            defs::TXQ_EVENT => self.handle_txq_event(mem),
            defs::EVQ_EVENT => self.handle_evq_event(mem),
            defs::BACKEND_EVENT => self.notify_backend_event(&events, mem),
            defs::HANDLER_READY_EVENT => {
                let ready = self.snapshot_control.handler_ready_event.read().is_ok();
                self.snapshot_control.complete_handler_registration(ready);
                if ready && self.muxer.has_pending_rx() {
                    self.process_rx(mem);
                }
            }
            _ => error!("{}: unknown epoll event slot {}", self.id, events.data()),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!("{}: VsockEpollHandler::init()", self.id);
        let mut registration_ready = true;

        let events = Events::with_data(
            self.config.queues[QUEUE_RX].eventfd.as_ref(),
            defs::RXQ_EVENT,
            EventSet::IN,
        );
        if let Err(e) = ops.add(events) {
            registration_ready = false;
            error!(
                "{}: failed to register epoll event for RX queue, {:?}.",
                self.id, e
            );
        }

        let events = Events::with_data(
            self.config.queues[QUEUE_TX].eventfd.as_ref(),
            defs::TXQ_EVENT,
            EventSet::IN,
        );
        if let Err(e) = ops.add(events) {
            registration_ready = false;
            error!(
                "{}: failed to register epoll event for TX queue, {:?}.",
                self.id, e
            );
        }

        let events = Events::with_data(
            self.config.queues[QUEUE_CFG].eventfd.as_ref(),
            defs::EVQ_EVENT,
            EventSet::IN,
        );
        if let Err(e) = ops.add(events) {
            registration_ready = false;
            error!(
                "{}: failed to register epoll event for config queue, {:?}.",
                self.id, e
            );
        }

        let be_fd = self.muxer.as_raw_fd();
        let be_evset = EventSet::from_bits(self.muxer.get_polled_evset().bits()).unwrap();
        let events = Events::with_data_raw(be_fd, defs::BACKEND_EVENT, be_evset);
        if let Err(e) = ops.add(events) {
            registration_ready = false;
            error!(
                "{}: failed to register epoll event for backend fd: {:?}, {:?}.",
                self.id, be_fd, e
            );
        }

        if !registration_ready {
            self.snapshot_control.complete_handler_registration(false);
            return;
        }

        let events = Events::with_data(
            self.snapshot_control.handler_ready_event.as_ref(),
            defs::HANDLER_READY_EVENT,
            EventSet::IN,
        );
        if let Err(e) = ops.add(events) {
            error!(
                "{}: failed to register vsock readiness barrier: {:?}",
                self.id, e
            );
            self.snapshot_control.complete_handler_registration(false);
            return;
        }
        if let Err(e) = self.snapshot_control.handler_ready_event.write(1) {
            error!(
                "{}: failed to signal vsock readiness barrier: {:?}",
                self.id, e
            );
            self.snapshot_control.complete_handler_registration(false);
            return;
        }

        if self
            .snapshot_control
            .pending_event_ack
            .load(Ordering::Acquire)
        {
            if let Err(e) = self.signal_used_queue(QUEUE_CFG) {
                error!(
                    "{}: failed to replay transport reset interrupt: {:?}",
                    self.id, e
                );
            } else {
                info!("vsock-reset event=snapshot_irq_replayed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    use dbs_interrupt::InterruptNotifier;
    use test_utils::skip_if_kvm_unaccessable;
    use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};
    use vmm_sys_util::epoll::EventSet;
    use vmm_sys_util::eventfd::EventFd;

    use super::super::packet::VSOCK_PKT_HDR_SIZE;
    use super::super::tests::TestContext;
    use super::super::{VsockChannel, VsockError};
    use super::*;
    use crate::tests::VIRTQ_DESC_F_WRITE;

    #[derive(Clone)]
    struct CountingNotifier(Arc<AtomicUsize>);

    impl InterruptNotifier for CountingNotifier {
        fn notify(&self) -> std::io::Result<()> {
            self.0.fetch_add(1, Ordering::AcqRel);
            Ok(())
        }

        fn notifier(&self) -> Option<&EventFd> {
            None
        }

        fn clone_boxed(&self) -> Box<dyn InterruptNotifier> {
            Box::new(self.clone())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn test_restore_reset_irq_replayed_once_without_consuming_queue() {
        skip_if_kvm_unaccessable!();

        for (pending, expected_notifications) in [(true, 1), (false, 0)] {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            let notifications = Arc::new(AtomicUsize::new(0));
            let eventq = ctx.queues.remove(QUEUE_CFG);
            ctx.queues.insert(
                QUEUE_CFG,
                crate::VirtioQueueConfig::new(
                    eventq.queue,
                    eventq.eventfd,
                    Arc::new(CountingNotifier(notifications.clone())),
                    QUEUE_CFG as u16,
                ),
            );
            let used_before = ctx._guest_evvq.used.idx().load();
            ctx.arti_activate(&test_ctx.mem);
            let handler = ctx.epoll_handler.take().unwrap();
            handler
                .snapshot_control
                .pending_event_ack
                .store(pending, Ordering::Release);
            let control = handler.snapshot_control.clone();

            let manager = dbs_utils::epoll_manager::EpollManager::default();
            let id = manager.add_subscriber(Box::new(handler));

            assert_eq!(
                notifications.load(Ordering::Acquire),
                expected_notifications
            );
            assert_eq!(ctx._guest_evvq.used.idx().load(), used_before);
            assert_eq!(control.pending_event_ack.load(Ordering::Acquire), pending);
            manager.clone().remove_subscriber(id).unwrap();
        }
    }

    #[test]
    fn test_irq() {
        skip_if_kvm_unaccessable!();
        let test_ctx = TestContext::new();
        let mut ctx = test_ctx.create_event_handler_context();
        ctx.arti_activate(&test_ctx.mem);

        assert!(ctx.signal_used_queue(0).is_ok());
    }

    #[test]
    fn test_transport_reset_ack_requires_replenished_descriptor() {
        skip_if_kvm_unaccessable!();
        const EVENT_ADDR: u64 = 0x0060_0000;

        let test_ctx = TestContext::new();
        let mut ctx = test_ctx.create_event_handler_context();
        ctx._guest_evvq
            .dtable(0)
            .set(EVENT_ADDR, 4, VIRTQ_DESC_F_WRITE, 0);
        ctx._guest_evvq
            .dtable(1)
            .set(EVENT_ADDR + 4, 4, VIRTQ_DESC_F_WRITE, 0);
        ctx._guest_evvq
            .dtable(2)
            .set(EVENT_ADDR + 8, 4, VIRTQ_DESC_F_WRITE, 0);
        ctx._guest_evvq.avail.ring(0).store(0);
        ctx._guest_evvq.avail.ring(1).store(1);
        ctx._guest_evvq.avail.idx().store(2);
        ctx.arti_activate(&test_ctx.mem);

        let handler = ctx.epoll_handler.as_mut().unwrap();
        handler.snapshot_control.schedule_reset();
        assert!(handler.snapshot_control.request_reset().unwrap());
        handler.muxer.set_pending_rx(true);
        let rx_ok_before = handler.muxer.rx_ok_cnt;

        assert!(!handler.snapshot_control.poll_reset_ack().unwrap());

        handler.config.queues[QUEUE_CFG].generate_event().unwrap();
        handler.handle_evq_event(&test_ctx.mem);
        assert!(handler
            .snapshot_control
            .pending_event_ack
            .load(Ordering::Acquire));
        assert!(handler.muxer.has_pending_rx());

        // Replenishing with a different valid descriptor is sufficient. ACK
        // tracks guest producer progress, not descriptor identity.
        ctx._guest_evvq.avail.ring(2).store(2);
        ctx._guest_evvq.avail.idx().store(3);
        handler.config.queues[QUEUE_CFG].generate_event().unwrap();
        handler.handle_evq_event(&test_ctx.mem);
        assert!(!handler
            .snapshot_control
            .pending_event_ack
            .load(Ordering::Acquire));
        assert!(handler.muxer.rx_ok_cnt > rx_ok_before);
    }

    #[test]
    fn test_txq_event() {
        skip_if_kvm_unaccessable!();
        // Test case:
        // - the driver has something to send (there's data in the TX queue);
        //   and
        // - the backend has no pending RX data.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                epoll_handler.muxer.set_pending_rx(false);
            }
            ctx.signal_txq_event();

            // The available TX descriptor should have been used.
            assert_eq!(ctx.guest_txvq.used.idx().load(), 1);
            // The available RX descriptor should be untouched.
            assert_eq!(ctx.guest_rxvq.used.idx().load(), 0);
        }

        // Test case:
        // - the driver has something to send (there's data in the TX queue);
        //   and
        // - the backend also has some pending RX data.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                epoll_handler.muxer.set_pending_rx(true);
            }
            ctx.signal_txq_event();

            // Both available RX and TX descriptors should have been used.
            assert_eq!(ctx.guest_txvq.used.idx().load(), 1);
            assert_eq!(ctx.guest_rxvq.used.idx().load(), 1);
        }

        // Test case:
        // - the driver has something to send (there's data in the TX queue);
        //   and
        // - the backend errors out and cannot process the TX queue.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                epoll_handler.muxer.set_pending_rx(false);
                epoll_handler.muxer.set_tx_err(Some(VsockError::NoData));
            }
            ctx.signal_txq_event();

            // Both RX and TX queues should be untouched.
            assert_eq!(ctx.guest_txvq.used.idx().load(), 0);
            assert_eq!(ctx.guest_rxvq.used.idx().load(), 0);
        }

        // Test case:
        // - the driver supplied a malformed TX buffer.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            // Invalidate the packet header descriptor, by setting its length to
            // 0.
            ctx.guest_txvq.dtable(0).len().store(0);
            ctx.signal_txq_event();

            // The available descriptor should have been consumed, but no packet
            // should have reached the backend.
            assert_eq!(ctx.guest_txvq.used.idx().load(), 1);
            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                assert_eq!(epoll_handler.muxer.tx_ok_cnt, 0);
            }
        }
    }

    #[test]
    fn test_rxq_event() {
        skip_if_kvm_unaccessable!();
        // Test case:
        // - there is pending RX data in the backend; and
        // - the driver makes RX buffers available; and
        // - the backend successfully places its RX data into the queue.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                epoll_handler.muxer.set_pending_rx(true);
                epoll_handler.muxer.set_rx_err(Some(VsockError::NoData));
            }
            ctx.signal_rxq_event();

            // The available RX buffer should've been left untouched.
            assert_eq!(ctx.guest_rxvq.used.idx().load(), 0);
        }

        // Test case:
        // - there is pending RX data in the backend; and
        // - the driver makes RX buffers available; and
        // - the backend errors out, when attempting to receive data.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                epoll_handler.muxer.set_pending_rx(true);
            }
            ctx.signal_rxq_event();

            // The available RX buffer should have been used.
            assert_eq!(ctx.guest_rxvq.used.idx().load(), 1);
        }

        // Test case: the driver provided a malformed RX descriptor chain.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            // Invalidate the packet header descriptor, by setting its length to 0.
            ctx.guest_rxvq.dtable(0).len().store(0);

            // The chain should've been processed, without employing the backend.
            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                epoll_handler.process_rx(&test_ctx.mem);
                assert_eq!(ctx.guest_rxvq.used.idx().load(), 1);
                assert_eq!(epoll_handler.muxer.rx_ok_cnt, 0);
            }
        }
    }

    #[test]
    fn test_backend_event() {
        skip_if_kvm_unaccessable!();
        // Test case:
        // - a backend event is received; and
        // - the backend has pending RX data.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                epoll_handler.muxer.set_pending_rx(true);
                epoll_handler
                    .notify_backend_event(&Events::new_raw(0, EventSet::IN), &test_ctx.mem);

                // The backend should've received this event
                assert_eq!(epoll_handler.muxer.evset, Some(epoll::Events::EPOLLIN));
            }

            // TX queue processing should've been triggered.
            assert_eq!(ctx.guest_txvq.used.idx().load(), 1);
            // RX queue processing should've been triggered.
            assert_eq!(ctx.guest_rxvq.used.idx().load(), 1);
        }

        // Test case:
        // - a backend event is received; and
        // - the backend doesn't have any pending RX data.
        {
            let test_ctx = TestContext::new();
            let mut ctx = test_ctx.create_event_handler_context();
            ctx.arti_activate(&test_ctx.mem);

            if let Some(epoll_handler) = &mut ctx.epoll_handler {
                epoll_handler.muxer.set_pending_rx(false);
                epoll_handler
                    .notify_backend_event(&Events::new_raw(0, EventSet::IN), &test_ctx.mem);

                // The backend should've received this event.
                assert_eq!(epoll_handler.muxer.evset, Some(epoll::Events::EPOLLIN));
            }
            // TX queue processing should've been triggered.
            assert_eq!(ctx.guest_txvq.used.idx().load(), 1);
            // The RX queue should've been left untouched.
            assert_eq!(ctx.guest_rxvq.used.idx().load(), 0);
        }
    }

    // Creates an epoll handler context and attempts to assemble a VsockPkt from
    // the descriptor chains available on the rx and tx virtqueues, but first it
    // will set the addr and len of the descriptor specified by desc_idx to the
    // provided values. We are only using this function for testing error cases,
    // so the asserts always expect is_err() to be true. When desc_idx = 0 we
    // are altering the header (first descriptor in the chain), and when
    // desc_idx = 1 we are altering the packet buffer.
    fn vsock_bof_helper(test_ctx: &mut TestContext, desc_idx: usize, addr: u64, len: u32) {
        assert!(desc_idx <= 1);

        {
            // should error here, but it works
            // let mut ctx = test_ctx.create_event_handler_context();
            // ctx.guest_rxvq.dtable(desc_idx as u16).addr().store(addr);
            // ctx.guest_rxvq.dtable(desc_idx as u16).len().store(len);
            // // If the descriptor chain is already declared invalid, there's no
            // // reason to assemble a packet.
            // if let Some(mut rx_desc) = ctx.queues[defs::RXQ_EVENT as usize]
            //     .iter(&mut test_ctx.mem)
            //     .next()
            // {
            //     assert!(VsockPacket::from_rx_virtq_head(&mut rx_desc).is_err());
            // }
        }

        {
            let mut ctx = test_ctx.create_event_handler_context();

            // When modifiyng the buffer descriptor, make sure the len field is altered in the
            // vsock packet header descriptor as well.
            if desc_idx == 1 {
                // The vsock packet len field has offset 24 in the header.
                let hdr_len_addr = GuestAddress(ctx.guest_txvq.dtable(0).addr().load() + 24);
                test_ctx
                    .mem
                    .write_obj(len.to_le_bytes(), hdr_len_addr)
                    .unwrap();
            }

            ctx.guest_txvq.dtable(desc_idx as u16).addr().store(addr);
            ctx.guest_txvq.dtable(desc_idx as u16).len().store(len);

            if let Some(mut tx_desc) = ctx.queues[defs::TXQ_EVENT as usize]
                .queue_mut()
                .pop_descriptor_chain(&test_ctx.mem)
            {
                assert!(VsockPacket::from_tx_virtq_head(&mut tx_desc).is_err());
            }
        }
    }

    #[test]
    fn test_vsock_bof() {
        skip_if_kvm_unaccessable!();
        const GAP_SIZE: usize = 768 << 20;
        const FIRST_AFTER_GAP: usize = 1 << 32;
        const GAP_START_ADDR: usize = FIRST_AFTER_GAP - GAP_SIZE;
        const MIB: usize = 1 << 20;

        let mut test_ctx = TestContext::new();
        test_ctx.mem = GuestMemoryMmap::from_ranges(&[
            (GuestAddress(0), 8 * MIB),
            (GuestAddress((GAP_START_ADDR - MIB) as u64), MIB),
            (GuestAddress(FIRST_AFTER_GAP as u64), MIB),
        ])
        .unwrap();

        // The default configured descriptor chains are valid.
        {
            let mut ctx = test_ctx.create_event_handler_context();
            let mut rx_desc = ctx.queues[defs::RXQ_EVENT as usize]
                .queue_mut()
                .pop_descriptor_chain(&test_ctx.mem)
                .unwrap();
            assert!(VsockPacket::from_rx_virtq_head(&mut rx_desc).is_ok());
        }

        {
            let mut ctx = test_ctx.create_event_handler_context();
            let mut tx_desc = ctx.queues[defs::TXQ_EVENT as usize]
                .queue_mut()
                .pop_descriptor_chain(&test_ctx.mem)
                .unwrap();
            assert!(VsockPacket::from_tx_virtq_head(&mut tx_desc).is_ok());
        }

        // Let's check what happens when the header descriptor is right before
        // the gap.
        vsock_bof_helper(
            &mut test_ctx,
            0,
            GAP_START_ADDR as u64 - 1,
            VSOCK_PKT_HDR_SIZE as u32,
        );

        // Let's check what happens when the buffer descriptor crosses into the
        // gap, but does not go past its right edge.
        vsock_bof_helper(
            &mut test_ctx,
            1,
            GAP_START_ADDR as u64 - 4,
            GAP_SIZE as u32 + 4,
        );

        // Let's modify the buffer descriptor addr and len such that it crosses
        // over the MMIO gap, and check we cannot assemble the VsockPkts.
        vsock_bof_helper(
            &mut test_ctx,
            1,
            GAP_START_ADDR as u64 - 4,
            GAP_SIZE as u32 + 100,
        );
    }
}
