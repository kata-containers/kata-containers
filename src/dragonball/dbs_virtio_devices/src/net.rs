// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::any::Any;
use std::cmp;
use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::unix::io::AsRawFd;
use std::sync::{mpsc, Arc};

use dbs_device::resources::ResourceConstraint;
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use dbs_utils::metric::IncMetric;
use dbs_utils::net::{net_gen, MacAddr, Tap};
use dbs_utils::rate_limiter::{BucketUpdate, RateLimiter, TokenType};
use libc;
use log::{debug, error, info, trace, warn};
use virtio_bindings::bindings::virtio_net::*;
use virtio_queue::{QueueOwnedT, QueueSync, QueueT};
use vm_memory::{Bytes, GuestAddress, GuestAddressSpace, GuestMemoryRegion, GuestRegionMmap};
use vmm_sys_util::eventfd::EventFd;

use crate::device::{VirtioDeviceConfig, VirtioDeviceInfo};
use crate::{
    setup_config_space, vnet_hdr_len, ActivateError, ActivateResult, ConfigResult,
    DbsGuestAddressSpace, Error, NetDeviceMetrics, Result, TapError, VirtioDevice,
    VirtioQueueConfig, DEFAULT_MTU, TYPE_NET,
};

const NET_DRIVER_NAME: &str = "virtio-net";

/// The maximum buffer size when segmentation offload is enabled. This
/// includes the 12-byte virtio net header.
/// http://docs.oasis-open.org/virtio/virtio/v1.0/virtio-v1.0.html#x1-1740003
const MAX_BUFFER_SIZE: usize = 65562;

// A frame is available for reading from the tap device to receive in the guest.
const RX_TAP_EVENT: u32 = 0;
// The guest has made a buffer available to receive a frame into.
const RX_QUEUE_EVENT: u32 = 1;
// The transmit queue has a frame that is ready to send from the guest.
const TX_QUEUE_EVENT: u32 = 2;
// rx rate limiter budget is now available.
const RX_RATE_LIMITER_EVENT: u32 = 3;
// tx rate limiter budget is now available.
const TX_RATE_LIMITER_EVENT: u32 = 4;
// patch request of rate limiters has arrived
const PATCH_RATE_LIMITER_EVENT: u32 = 5;
// Number of DeviceEventT events supported by this implementation.
pub const NET_EVENTS_COUNT: u32 = 6;

/// Error for virtio-net devices to handle requests from guests.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("tap device operation error: {0:?}")]
    TapError(#[source] TapError),
}

struct TxVirtio<Q: QueueT> {
    queue: VirtioQueueConfig<Q>,
    rate_limiter: RateLimiter,
    iovec: Vec<(GuestAddress, usize)>,
    used_desc_heads: Vec<u16>,
    frame_buf: [u8; MAX_BUFFER_SIZE],
}

impl<Q: QueueT> TxVirtio<Q> {
    fn new(queue: VirtioQueueConfig<Q>, rate_limiter: RateLimiter) -> Self {
        let tx_queue_max_size = queue.max_size() as usize;

        TxVirtio {
            queue,
            rate_limiter,
            iovec: Vec::with_capacity(tx_queue_max_size),
            used_desc_heads: vec![0u16; tx_queue_max_size],
            frame_buf: [0u8; MAX_BUFFER_SIZE],
        }
    }
}

struct RxVirtio<Q: QueueT> {
    queue: VirtioQueueConfig<Q>,
    rate_limiter: RateLimiter,
    deferred_frame: bool,
    deferred_irqs: bool,
    bytes_read: usize,
    frame_buf: [u8; MAX_BUFFER_SIZE],
}

impl<Q: QueueT> RxVirtio<Q> {
    fn new(queue: VirtioQueueConfig<Q>, rate_limiter: RateLimiter) -> Self {
        RxVirtio {
            queue,
            rate_limiter,
            deferred_frame: false,
            deferred_irqs: false,
            bytes_read: 0,
            frame_buf: [0u8; MAX_BUFFER_SIZE],
        }
    }
}

#[allow(dead_code)]
pub(crate) struct NetEpollHandler<
    AS: GuestAddressSpace,
    Q: QueueT + Send = QueueSync,
    R: GuestMemoryRegion = GuestRegionMmap,
> {
    tap: Tap,
    rx: RxVirtio<Q>,
    tx: TxVirtio<Q>,
    config: VirtioDeviceConfig<AS, Q, R>,
    id: String,
    patch_rate_limiter_fd: EventFd,
    receiver: Option<mpsc::Receiver<(BucketUpdate, BucketUpdate, BucketUpdate, BucketUpdate)>>,
    metrics: Arc<NetDeviceMetrics>,
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> NetEpollHandler<AS, Q, R> {
    // Attempts to copy a single frame into the guest if there is enough rate limiting budget.
    // Returns true on successful frame delivery.
    fn rate_limited_rx_single_frame(&mut self, mem: &AS::M) -> bool {
        // If limiter.consume() fails it means there is no more TokenType::Ops
        // budget and rate limiting is in effect.
        if !self.rx.rate_limiter.consume(1, TokenType::Ops) {
            return false;
        }
        // If limiter.consume() fails it means there is no more TokenType::Bytes
        // budget and rate limiting is in effect.
        if !self
            .rx
            .rate_limiter
            .consume(self.rx.bytes_read as u64, TokenType::Bytes)
        {
            // revert the OPS consume()
            self.rx.rate_limiter.manual_replenish(1, TokenType::Ops);
            return false;
        }

        // Attempt frame delivery.
        let success = self.rx_single_frame(mem);

        // Undo the tokens consumption if guest delivery failed.
        if !success {
            self.rx.rate_limiter.manual_replenish(1, TokenType::Ops);
            self.rx
                .rate_limiter
                .manual_replenish(self.rx.bytes_read as u64, TokenType::Bytes);
        }

        success
    }

    // Copies a single frame from `self.rx.frame_buf` into the guest.
    //
    // Returns true if a buffer was used, and false if the frame must be deferred until a buffer
    // is made available by the driver.
    fn rx_single_frame(&mut self, mem: &AS::M) -> bool {
        let mut next_desc;
        let mut desc_chain;
        let mut write_count = 0;

        {
            let queue = &mut self.rx.queue.queue_mut().lock();
            let mut iter = match queue.iter(mem) {
                Err(e) => {
                    error!("{}: failed to process queue. {}", self.id, e);
                    return false;
                }
                Ok(iter) => iter,
            };
            desc_chain = match iter.next() {
                Some(v) => v,
                None => return false,
            };
            next_desc = desc_chain.next();

            // Copy from frame into buffer, which may span multiple descriptors.
            loop {
                match next_desc {
                    Some(desc) => {
                        if !desc.is_write_only() {
                            self.metrics.rx_fails.inc();
                            debug!("{}: receiving buffer is not write-only", self.id);
                            break;
                        }

                        let limit = cmp::min(write_count + desc.len() as usize, self.rx.bytes_read);
                        let source_slice = &self.rx.frame_buf[write_count..limit];
                        match mem.write(source_slice, desc.addr()) {
                            Ok(sz) => write_count += sz,
                            Err(e) => {
                                self.metrics.rx_fails.inc();
                                debug!("{}: failed to write guest memory slice, {:?}", self.id, e);
                                break;
                            }
                        };

                        if write_count >= self.rx.bytes_read {
                            break;
                        }
                        next_desc = desc_chain.next();
                    }
                    None => {
                        self.metrics.rx_fails.inc();
                        debug!("{}: receiving buffer is too small", self.id);
                        break;
                    }
                }
            }
        }
        self.rx
            .queue
            .add_used(mem, desc_chain.head_index(), write_count as u32);

        // Mark that we have at least one pending packet and we need to interrupt the guest.
        self.rx.deferred_irqs = true;

        // Current descriptor chain is too small, need a bigger one.
        if write_count < self.rx.bytes_read {
            return false;
        }

        self.metrics.rx_bytes_count.add(write_count);
        self.metrics.rx_packets_count.inc();
        true
    }

    // Sends frame to the host TAP.
    //
    // `frame_buf` should contain the frame bytes in a slice of exact length.
    // Returns whether MMDS consumed the frame.
    fn write_to_tap(frame_buf: &[u8], tap: &mut Tap, metrics: &Arc<NetDeviceMetrics>) {
        match tap.write(frame_buf) {
            Ok(_) => {
                metrics.tx_bytes_count.add(frame_buf.len());
                metrics.tx_packets_count.inc();
            }
            Err(e) => {
                metrics.tx_fails.inc();
                error!("{}: failed to write to tap, {:?}", NET_DRIVER_NAME, e);
            }
        }
    }

    // Read from regular network packets.
    fn read_from_tap(&mut self) -> io::Result<usize> {
        self.tap.read(&mut self.rx.frame_buf)
    }

    fn process_rx(&mut self, mem: &AS::M) -> Result<()> {
        // Read as many frames as possible.
        loop {
            match self.read_from_tap() {
                Ok(count) => {
                    self.rx.bytes_read = count;
                    if !self.rate_limited_rx_single_frame(mem) {
                        self.rx.deferred_frame = true;
                        break;
                    }
                }
                Err(e) => {
                    // The tap device is non-blocking, so any error aside from EAGAIN is unexpected.
                    match e.raw_os_error() {
                        Some(err) if err == libc::EAGAIN => (),
                        _ => {
                            self.metrics.rx_fails.inc();
                            error!("{}: failed to read tap: {:?}", self.id, e);
                            return Err(e.into());
                        }
                    };
                    break;
                }
            }
        }

        if self.rx.deferred_irqs {
            self.rx.deferred_irqs = false;
            self.rx.queue.notify()
        } else {
            Ok(())
        }
    }

    fn resume_rx(&mut self, mem: &AS::M) -> Result<()> {
        if self.rx.deferred_frame {
            if self.rate_limited_rx_single_frame(mem) {
                self.rx.deferred_frame = false;
                // process_rx() was interrupted possibly before consuming all
                // packets in the tap; try continuing now.
                self.process_rx(mem)
            } else if self.rx.deferred_irqs {
                self.rx.deferred_irqs = false;
                self.rx.queue.notify()
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn process_tx(&mut self, mem: &AS::M) -> Result<()> {
        let mut rate_limited = false;
        let mut used_count = 0;
        {
            let queue = &mut self.tx.queue.queue_mut().lock();

            let mut iter = match queue.iter(mem) {
                Err(e) => {
                    return Err(Error::VirtioQueueError(e));
                }
                Ok(iter) => iter,
            };

            for desc_chain in &mut iter {
                // If limiter.consume() fails it means there is no more TokenType::Ops
                // budget and rate limiting is in effect.
                if !self.tx.rate_limiter.consume(1, TokenType::Ops) {
                    rate_limited = true;
                    // Stop processing the queue.

                    break;
                }

                let mut read_count = 0;
                let header_index = desc_chain.head_index();
                self.tx.iovec.clear();

                for desc in desc_chain {
                    if desc.is_write_only() {
                        break;
                    }
                    self.tx.iovec.push((desc.addr(), desc.len() as usize));
                    read_count += desc.len() as usize;
                }

                // If limiter.consume() fails it means there is no more TokenType::Bytes
                // budget and rate limiting is in effect.
                if !self
                    .tx
                    .rate_limiter
                    .consume(read_count as u64, TokenType::Bytes)
                {
                    rate_limited = true;
                    // revert the OPS consume()
                    self.tx.rate_limiter.manual_replenish(1, TokenType::Ops);
                    // stop processing the queue
                    break;
                }

                read_count = 0;
                // Copy buffer from across multiple descriptors.
                // TODO(performance - Issue #420): change this to use `writev()` instead of `write()`
                // and get rid of the intermediate buffer.
                for (desc_addr, desc_len) in self.tx.iovec.drain(..) {
                    let limit = cmp::min(read_count + desc_len, self.tx.frame_buf.len());

                    let read_result =
                        mem.read(&mut self.tx.frame_buf[read_count..limit], desc_addr);
                    match read_result {
                        Ok(sz) => read_count += sz,
                        Err(e) => {
                            self.metrics.tx_fails.inc();
                            error!("{}: failed to read slice: {:?}", self.id, e);
                            break;
                        }
                    }
                }

                Self::write_to_tap(
                    &self.tx.frame_buf[..read_count],
                    &mut self.tap,
                    &self.metrics,
                );

                self.tx.used_desc_heads[used_count] = header_index;
                used_count += 1;
            }
            if rate_limited {
                // If rate limiting kicked in, queue had advanced one element that we aborted
                // processing; go back one element so it can be processed next time.
                iter.go_to_previous_position();
            }
        }
        if used_count != 0 {
            // TODO(performance - Issue #425): find a way around RUST mutability enforcements to
            // allow calling queue.add_used() inside the loop. This would lead to better distribution
            // of descriptor usage between the dragonball thread and the guest tx thread.
            // One option to do this is to call queue.add_used() from a static function.
            for &desc_index in &self.tx.used_desc_heads[..used_count] {
                self.tx.queue.add_used(mem, desc_index, 0);
            }

            if let Err(e) = self.tx.queue.notify() {
                error!("{}: failed to send tx interrupt to guest, {:?}", self.id, e);
            }
        }
        Ok(())
    }

    pub fn get_patch_rate_limiters(
        &mut self,
        rx_bytes: BucketUpdate,
        rx_ops: BucketUpdate,
        tx_bytes: BucketUpdate,
        tx_ops: BucketUpdate,
    ) {
        self.rx.rate_limiter.update_buckets(rx_bytes, rx_ops);
        self.tx.rate_limiter.update_buckets(tx_bytes, tx_ops);
        info!("{}: Update rate limiters", self.id);
    }
}

impl<AS: DbsGuestAddressSpace, Q: QueueT + Send, R: GuestMemoryRegion> MutEventSubscriber
    for NetEpollHandler<AS, Q, R>
{
    fn process(&mut self, events: Events, _ops: &mut EventOps) {
        let guard = self.config.lock_guest_memory();
        let mem = guard.deref();
        self.metrics.event_count.inc();
        match events.data() {
            RX_QUEUE_EVENT => {
                self.metrics.rx_queue_event_count.inc();
                if let Err(e) = self.rx.queue.consume_event() {
                    self.metrics.event_fails.inc();
                    error!("{}: failed to get rx queue event, {:?}", self.id, e);
                } else if !self.rx.rate_limiter.is_blocked() {
                    // If the limiter is not blocked, resume the receiving of bytes.
                    // There should be a buffer available now to receive the frame into.
                    if let Err(e) = self.resume_rx(mem) {
                        self.metrics.event_fails.inc();
                        error!("{}: failed to resume rx_queue event, {:?}", self.id, e);
                    }
                }
            }
            RX_TAP_EVENT => {
                self.metrics.rx_tap_event_count.inc();

                // While limiter is blocked, don't process any more incoming.
                if self.rx.rate_limiter.is_blocked() {
                    // TODO: this may cause busy loop when rate limiting.
                    // Process a deferred frame first if available. Don't read from tap again
                    // until we manage to receive this deferred frame.
                } else if self.rx.deferred_frame {
                    if self.rate_limited_rx_single_frame(mem) {
                        self.rx.deferred_frame = false;
                        // Process more packats from the tap device.
                        if let Err(e) = self.process_rx(mem) {
                            self.metrics.event_fails.inc();
                            error!("{}: failed to process rx queue, {:?}", self.id, e);
                        }
                    } else if self.rx.deferred_irqs {
                        self.rx.deferred_irqs = false;
                        if let Err(e) = self.rx.queue.notify() {
                            error!("{}: failed to send rx interrupt to guest, {:?}", self.id, e);
                        }
                    }
                } else if let Err(e) = self.process_rx(mem) {
                    error!("{}: failed to process rx queue, {:?}", self.id, e);
                }
            }
            TX_QUEUE_EVENT => {
                self.metrics.tx_queue_event_count.inc();
                if let Err(e) = self.tx.queue.consume_event() {
                    self.metrics.event_fails.inc();
                    error!("{}: failed to get tx queue event: {:?}", self.id, e);
                // If the limiter is not blocked, continue transmitting bytes.
                } else if !self.tx.rate_limiter.is_blocked() {
                    if let Err(e) = self.process_tx(mem) {
                        self.metrics.event_fails.inc();
                        error!("{}: failed to process tx queue, {:?}", self.id, e);
                    }
                }
            }
            RX_RATE_LIMITER_EVENT => {
                // Upon rate limiter event, call the rate limiter handler and restart processing
                // the rx queue.
                self.metrics.rx_event_rate_limiter_count.inc();
                match self.rx.rate_limiter.event_handler() {
                    // There might be enough budget now to receive the frame.
                    Ok(_) => {
                        if let Err(e) = self.resume_rx(mem) {
                            self.metrics.event_fails.inc();
                            error!("{}: failed to resume rx, {:?}", self.id, e);
                        }
                    }
                    Err(e) => {
                        self.metrics.event_fails.inc();
                        error!("{}: failed to get rx rate-limiter event: {:?}", self.id, e);
                    }
                }
            }
            TX_RATE_LIMITER_EVENT => {
                // Upon rate limiter event, call the rate limiter handler and restart processing
                // the tx queue.
                self.metrics.tx_rate_limiter_event_count.inc();
                match self.tx.rate_limiter.event_handler() {
                    // There might be enough budget now to send the frame.
                    Ok(_) => {
                        if let Err(e) = self.process_tx(mem) {
                            self.metrics.event_fails.inc();
                            error!("{}: failed to resume tx, {:?}", self.id, e);
                        }
                    }
                    Err(e) => {
                        self.metrics.event_fails.inc();
                        error!("{}: failed to get tx rate-limiter event, {:?}", self.id, e);
                    }
                }
            }
            PATCH_RATE_LIMITER_EVENT => {
                if let Some(receiver) = &self.receiver {
                    if let Ok((rx_bytes, rx_ops, tx_bytes, tx_ops)) = receiver.try_recv() {
                        self.get_patch_rate_limiters(rx_bytes, rx_ops, tx_bytes, tx_ops);
                        if let Err(e) = self.patch_rate_limiter_fd.read() {
                            error!("{}: failed to get patch event, {:?}", self.id, e);
                        }
                    }
                }
            }
            _ => error!("{}: unknown epoll event slot {}", self.id, events.data()),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        trace!(target: "virtio-net", "{}: NetEpollHandler::init()", self.id);

        let events = Events::with_data(&self.tap, RX_TAP_EVENT, EventSet::IN);
        if let Err(e) = ops.add(events) {
            error!("{}: failed to register TAP RX event, {:?}", self.id, e);
        }

        let events =
            Events::with_data(self.rx.queue.eventfd.as_ref(), RX_QUEUE_EVENT, EventSet::IN);
        if let Err(e) = ops.add(events) {
            error!("{}: failed to register RX queue event, {:?}", self.id, e);
        }

        let events =
            Events::with_data(self.tx.queue.eventfd.as_ref(), TX_QUEUE_EVENT, EventSet::IN);
        if let Err(e) = ops.add(events) {
            error!("{}: failed to register TX queue event, {:?}", self.id, e);
        }

        let rx_rate_limiter_fd = self.rx.rate_limiter.as_raw_fd();
        if rx_rate_limiter_fd >= 0 {
            let events =
                Events::with_data_raw(rx_rate_limiter_fd, RX_RATE_LIMITER_EVENT, EventSet::IN);
            if let Err(e) = ops.add(events) {
                error!(
                    "{}: failed to register RX rate limit event, {:?}",
                    self.id, e
                );
            }
        }

        let tx_rate_limiter_fd = self.tx.rate_limiter.as_raw_fd();
        if tx_rate_limiter_fd >= 0 {
            let events =
                Events::with_data_raw(tx_rate_limiter_fd, TX_RATE_LIMITER_EVENT, EventSet::IN);
            if let Err(e) = ops.add(events) {
                error!(
                    "{}: failed to register TX rate limit event, {:?}",
                    self.id, e
                );
            }
        }

        let events = Events::with_data(
            &self.patch_rate_limiter_fd,
            PATCH_RATE_LIMITER_EVENT,
            EventSet::IN,
        );
        if let Err(e) = ops.add(events) {
            error!(
                "{}: failed to register rate limiter patch event, {:?}",
                self.id, e
            );
        }
    }
}

pub struct Net<AS: GuestAddressSpace> {
    pub(crate) device_info: VirtioDeviceInfo,
    pub tap: Option<Tap>,
    pub queue_sizes: Arc<Vec<u16>>,
    pub rx_rate_limiter: Option<RateLimiter>,
    pub tx_rate_limiter: Option<RateLimiter>,
    pub subscriber_id: Option<SubscriberId>,
    id: String,
    phantom: PhantomData<AS>,
    patch_rate_limiter_fd: EventFd,
    sender: Option<mpsc::Sender<(BucketUpdate, BucketUpdate, BucketUpdate, BucketUpdate)>>,
    metrics: Arc<NetDeviceMetrics>,
}

impl<AS: GuestAddressSpace> Net<AS> {
    /// Create a new virtio network device with the given TAP interface.
    pub fn new_with_tap(
        tap: Tap,
        guest_mac: Option<&MacAddr>,
        queue_sizes: Arc<Vec<u16>>,
        event_mgr: EpollManager,
        rx_rate_limiter: Option<RateLimiter>,
        tx_rate_limiter: Option<RateLimiter>,
    ) -> Result<Self> {
        trace!(target: "virtio-net", "{}: Net::new_with_tap()", NET_DRIVER_NAME);

        // Set offload flags to match the virtio features below.
        tap.set_offload(
            net_gen::TUN_F_CSUM | net_gen::TUN_F_UFO | net_gen::TUN_F_TSO4 | net_gen::TUN_F_TSO6,
        )
        .map_err(|err| Error::VirtioNet(NetError::TapError(TapError::SetOffload(err))))?;

        let vnet_hdr_size = vnet_hdr_len() as i32;
        tap.set_vnet_hdr_size(vnet_hdr_size)
            .map_err(|err| Error::VirtioNet(NetError::TapError(TapError::SetVnetHdrSize(err))))?;
        info!("net tap set finished");

        let mut avail_features = 1u64 << VIRTIO_NET_F_GUEST_CSUM
            | 1u64 << VIRTIO_NET_F_CSUM
            | 1u64 << VIRTIO_NET_F_GUEST_TSO4
            | 1u64 << VIRTIO_NET_F_GUEST_UFO
            | 1u64 << VIRTIO_NET_F_HOST_TSO4
            | 1u64 << VIRTIO_NET_F_HOST_UFO
            | 1u64 << VIRTIO_F_VERSION_1;

        let config_space = setup_config_space(
            NET_DRIVER_NAME,
            &guest_mac,
            &mut avail_features,
            1,
            DEFAULT_MTU,
        )?;

        let device_info = VirtioDeviceInfo::new(
            NET_DRIVER_NAME.to_string(),
            avail_features,
            queue_sizes.clone(),
            config_space,
            event_mgr,
        );
        let id = device_info.driver_name.clone();
        Ok(Net {
            tap: Some(tap),
            device_info,
            queue_sizes,
            rx_rate_limiter,
            tx_rate_limiter,
            subscriber_id: None,
            id,
            phantom: PhantomData,
            patch_rate_limiter_fd: EventFd::new(0).unwrap(),
            sender: None,
            metrics: Arc::new(NetDeviceMetrics::default()),
        })
    }

    /// Create a new virtio network device with the given Host Device Name
    pub fn new(
        host_dev_name: String,
        guest_mac: Option<&MacAddr>,
        queue_sizes: Arc<Vec<u16>>,
        epoll_mgr: EpollManager,
        rx_rate_limiter: Option<RateLimiter>,
        tx_rate_limiter: Option<RateLimiter>,
    ) -> Result<Self> {
        info!("open net tap {}", host_dev_name);
        let tap = Tap::open_named(host_dev_name.as_str(), false)
            .map_err(|err| Error::VirtioNet(NetError::TapError(TapError::Open(err))))?;
        info!("net tap opened");

        Self::new_with_tap(
            tap,
            guest_mac,
            queue_sizes,
            epoll_mgr,
            rx_rate_limiter,
            tx_rate_limiter,
        )
    }

    pub fn metrics(&self) -> Arc<NetDeviceMetrics> {
        self.metrics.clone()
    }
}

impl<AS: GuestAddressSpace + 'static> Net<AS> {
    pub fn set_patch_rate_limiters(
        &self,
        rx_bytes: BucketUpdate,
        rx_ops: BucketUpdate,
        tx_bytes: BucketUpdate,
        tx_ops: BucketUpdate,
    ) -> Result<()> {
        if let Some(sender) = &self.sender {
            if sender.send((rx_bytes, rx_ops, tx_bytes, tx_ops)).is_ok() {
                if let Err(e) = self.patch_rate_limiter_fd.write(1) {
                    error!(
                        "virtio-net: failed to write rate-limiter patch event {:?}",
                        e
                    );
                    Err(Error::InternalError)
                } else {
                    Ok(())
                }
            } else {
                error!("virtio-net: failed to send rate-limiter patch data");
                Err(Error::InternalError)
            }
        } else {
            error!("virtio-net: failed to establish channel to send rate-limiter patch data");
            Err(Error::InternalError)
        }
    }
}

impl<AS, Q, R> VirtioDevice<AS, Q, R> for Net<AS>
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

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(target: "virtio-net", "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
               self.id, page, value);
        self.device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        trace!(target: "virtio-net", "{}: VirtioDevice::read_config(0x{:x}, {:?})",
               self.id, offset, data);
        self.device_info.read_config(offset, data).map_err(|e| {
            self.metrics.cfg_fails.inc();
            e
        })
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        trace!(target: "virtio-net", "{}: VirtioDevice::write_config(0x{:x}, {:?})",
               self.id, offset, data);
        self.device_info.write_config(offset, data).map_err(|e| {
            self.metrics.cfg_fails.inc();
            e
        })
    }

    fn activate(&mut self, mut config: VirtioDeviceConfig<AS, Q, R>) -> ActivateResult {
        trace!(target: "virtio-net", "{}: VirtioDevice::activate()", self.id);
        // Do not support control queue and multi queue.
        if config.queues.len() != 2 {
            self.metrics.activate_fails.inc();
            return Err(ActivateError::InvalidParam);
        }

        self.device_info
            .check_queue_sizes(&config.queues[..])
            .map_err(|e| {
                self.metrics.activate_fails.inc();
                e
            })?;
        let tap = self.tap.take().ok_or_else(|| {
            self.metrics.activate_fails.inc();
            ActivateError::InvalidParam
        })?;
        let (sender, receiver) = mpsc::channel();
        self.sender = Some(sender);
        let rx_queue = config.queues.remove(0);
        let tx_queue = config.queues.remove(0);
        let rx = RxVirtio::<Q>::new(rx_queue, self.rx_rate_limiter.take().unwrap_or_default());
        let tx = TxVirtio::<Q>::new(tx_queue, self.tx_rate_limiter.take().unwrap_or_default());
        let patch_rate_limiter_fd = self.patch_rate_limiter_fd.try_clone().unwrap();

        let handler = Box::new(NetEpollHandler {
            tap,
            rx,
            tx,
            config,
            id: self.id.clone(),
            patch_rate_limiter_fd,
            receiver: Some(receiver),
            metrics: self.metrics.clone(),
        });

        self.subscriber_id = Some(self.device_info.register_event_handler(handler));
        Ok(())
    }

    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        trace!(target: "virtio-net", "{}: VirtioDevice::get_resource_requirements()", self.id);
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
                Ok(_) => debug!("virtio-net: removed subscriber_id {:?}", subscriber_id),
                Err(err) => warn!("virtio-net: failed to remove event handler: {:?}", err),
            };
        } else {
            self.tap.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::NoopNotifier;
    use dbs_utils::epoll_manager::SubscriberOps;
    use dbs_utils::rate_limiter::TokenBucket;
    use kvm_ioctls::Kvm;
    use vm_memory::{GuestAddress, GuestMemoryMmap};

    use super::*;
    use crate::tests::{create_address_space, VirtQueue, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
    use crate::{ConfigError, CONFIG_SPACE_SIZE};

    static NEXT_IP: AtomicUsize = AtomicUsize::new(1);

    #[allow(dead_code)]
    const MAX_REQ_SIZE: u32 = 0x10000;

    fn create_net_epoll_handler(id: String) -> NetEpollHandler<Arc<GuestMemoryMmap>> {
        let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);
        let tap = Tap::open_named(&format!("tap{next_ip}"), false).unwrap();
        let rx = RxVirtio::new(
            VirtioQueueConfig::create(256, 0).unwrap(),
            RateLimiter::default(),
        );
        let tx = TxVirtio::new(
            VirtioQueueConfig::create(256, 0).unwrap(),
            RateLimiter::default(),
        );
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0x0), 0x10000)]).unwrap());
        let queues = vec![VirtioQueueConfig::create(256, 0).unwrap()];

        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let resources = DeviceResources::new();
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
        NetEpollHandler {
            tap,
            rx,
            tx,
            config,
            id,
            patch_rate_limiter_fd: EventFd::new(0).unwrap(),
            receiver: None,
            metrics: Arc::new(NetDeviceMetrics::default()),
        }
    }

    #[test]
    fn test_net_virtio_device_normal() {
        let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);
        let tap = Tap::open_named(&format!("tap{next_ip}"), false).unwrap();
        let epoll_mgr = EpollManager::default();

        let mut dev = Net::<Arc<GuestMemoryMmap>>::new_with_tap(
            tap,
            None,
            Arc::new(vec![128]),
            epoll_mgr,
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&dev),
            TYPE_NET
        );
        let queue_size = vec![128];
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
        // Config with correct size
        let config: [u8; CONFIG_SPACE_SIZE] = [0; CONFIG_SPACE_SIZE];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
            &mut dev, 0, &config,
        )
        .unwrap();
        // Config with invalid size
        let config: [u8; CONFIG_SPACE_SIZE + 1] = [0; CONFIG_SPACE_SIZE + 1];
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
                &mut dev, 0, &config,
            )
            .unwrap_err(),
            ConfigError::InvalidOffsetPlusDataLen(CONFIG_SPACE_SIZE as u64 + 1)
        );
    }

    #[test]
    fn test_net_virtio_device_active() {
        let epoll_mgr = EpollManager::default();
        {
            // config queue size is not 2
            let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);
            let tap = Tap::open_named(&format!("tap{next_ip}"), false).unwrap();
            let mut dev = Net::<Arc<GuestMemoryMmap>>::new_with_tap(
                tap,
                None,
                Arc::new(vec![128]),
                epoll_mgr.clone(),
                None,
                None,
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = Vec::new();

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config =
                VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::new(
                    Arc::new(mem),
                    address_space,
                    vm_fd,
                    resources,
                    queues,
                    None,
                    Arc::new(NoopNotifier::new()),
                );

            matches!(dev.activate(config), Err(ActivateError::InvalidParam));
        }
        {
            // check queue sizes error
            let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);
            let tap = Tap::open_named(&format!("tap{next_ip}"), false).unwrap();
            let mut dev = Net::<Arc<GuestMemoryMmap>>::new_with_tap(
                tap,
                None,
                Arc::new(vec![128]),
                epoll_mgr.clone(),
                None,
                None,
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![
                VirtioQueueConfig::create(2, 0).unwrap(),
                VirtioQueueConfig::create(2, 0).unwrap(),
            ];

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config =
                VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::new(
                    Arc::new(mem),
                    address_space,
                    vm_fd,
                    resources,
                    queues,
                    None,
                    Arc::new(NoopNotifier::new()),
                );

            matches!(dev.activate(config), Err(ActivateError::InvalidParam));
        }
        {
            // test no tap
            let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);
            let tap = Tap::open_named(&format!("tap{next_ip}"), false).unwrap();
            let mut dev = Net::<Arc<GuestMemoryMmap>>::new_with_tap(
                tap,
                None,
                Arc::new(vec![128, 128]),
                epoll_mgr.clone(),
                None,
                None,
            )
            .unwrap();
            dev.tap = None;
            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![
                VirtioQueueConfig::create(128, 0).unwrap(),
                VirtioQueueConfig::create(128, 0).unwrap(),
            ];
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config =
                VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::new(
                    Arc::new(mem),
                    address_space,
                    vm_fd,
                    resources,
                    queues,
                    None,
                    Arc::new(NoopNotifier::new()),
                );

            matches!(dev.activate(config), Err(ActivateError::InvalidParam));
        }
        {
            // Ok
            let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);
            let tap = Tap::open_named(&format!("tap{next_ip}"), false).unwrap();
            let mut dev = Net::<Arc<GuestMemoryMmap>>::new_with_tap(
                tap,
                None,
                Arc::new(vec![128, 128]),
                epoll_mgr,
                None,
                None,
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![
                VirtioQueueConfig::create(128, 0).unwrap(),
                VirtioQueueConfig::create(128, 0).unwrap(),
            ];

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config =
                VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::new(
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
    fn test_net_set_patch_rate_limiters() {
        let next_ip = NEXT_IP.fetch_add(1, Ordering::SeqCst);
        let tap = Tap::open_named(&format!("tap{next_ip}"), false).unwrap();
        let epoll_mgr = EpollManager::default();

        let mut dev = Net::<Arc<GuestMemoryMmap>>::new_with_tap(
            tap,
            None,
            Arc::new(vec![128]),
            epoll_mgr,
            None,
            None,
        )
        .unwrap();

        //No sender
        assert!(dev
            .set_patch_rate_limiters(
                BucketUpdate::None,
                BucketUpdate::None,
                BucketUpdate::None,
                BucketUpdate::None
            )
            .is_err());

        let (sender, _receiver) = mpsc::channel();
        dev.sender = Some(sender);
        assert!(dev
            .set_patch_rate_limiters(
                BucketUpdate::None,
                BucketUpdate::None,
                BucketUpdate::None,
                BucketUpdate::None
            )
            .is_ok());
    }

    #[test]
    fn test_net_get_patch_rate_limiters() {
        let mut handler = create_net_epoll_handler("test_1".to_string());
        let tokenbucket = TokenBucket::new(1, 1, 4);

        //update rx
        handler.get_patch_rate_limiters(
            BucketUpdate::None,
            BucketUpdate::Update(tokenbucket.clone()),
            BucketUpdate::None,
            BucketUpdate::None,
        );
        assert_eq!(handler.rx.rate_limiter.ops().unwrap(), &tokenbucket);

        //update tx
        handler.get_patch_rate_limiters(
            BucketUpdate::None,
            BucketUpdate::None,
            BucketUpdate::None,
            BucketUpdate::Update(tokenbucket.clone()),
        );
        assert_eq!(handler.tx.rate_limiter.ops().unwrap(), &tokenbucket);
    }

    #[test]
    fn test_net_epoll_handler_handle_event() {
        let handler = create_net_epoll_handler("test_1".to_string());
        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_net_epoll_handler("test_2".to_string());

        // test for RX_QUEUE_EVENT
        let events = Events::with_data(&event_fd, RX_QUEUE_EVENT, event_set);
        handler.process(events, &mut event_op);
        handler.config.queues[0].generate_event().unwrap();
        handler.process(events, &mut event_op);

        // test for TX_QUEUE_EVENT
        let events = Events::with_data(&event_fd, TX_QUEUE_EVENT, event_set);
        handler.process(events, &mut event_op);
        handler.config.queues[0].generate_event().unwrap();
        handler.process(events, &mut event_op);

        // test for RX_TAP_EVENT
        let events = Events::with_data(&event_fd, RX_TAP_EVENT, event_set);
        handler.process(events, &mut event_op);

        // test for RX&TX RATE_LIMITER_EVENT
        let events = Events::with_data(&event_fd, RX_RATE_LIMITER_EVENT, event_set);
        handler.process(events, &mut event_op);
        let events = Events::with_data(&event_fd, TX_RATE_LIMITER_EVENT, event_set);
        handler.process(events, &mut event_op);

        // test for PATCH_RATE_LIMITER_EVENT
        let events = Events::with_data(&event_fd, PATCH_RATE_LIMITER_EVENT, event_set);
        handler.process(events, &mut event_op);
    }

    #[test]
    fn test_net_epoll_handler_handle_unknown_event() {
        let handler = create_net_epoll_handler("test_1".to_string());
        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_net_epoll_handler("test_2".to_string());

        // test for unknown event
        let events = Events::with_data(&event_fd, NET_EVENTS_COUNT + 10, event_set);
        handler.process(events, &mut event_op);
    }

    #[test]
    fn test_net_epoll_handler_process_queue() {
        {
            let mut handler = create_net_epoll_handler("test_1".to_string());

            let m = &handler.config.vm_as.clone();
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            let q = vq.create_queue();
            vq.dtable(0).set(0x1000, 0x1000, VIRTQ_DESC_F_NEXT, 1);
            vq.dtable(1)
                .set(0x2000, 0x1000, VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, 2);
            vq.dtable(2).set(0x3000, 1, VIRTQ_DESC_F_WRITE, 1);

            handler.config.queues = vec![VirtioQueueConfig::new(
                q,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                0,
            )];
            assert!(handler.process_rx(m).is_ok());
        }
    }

    #[test]
    fn test_net_bandwidth_rate_limiter() {
        let handler = create_net_epoll_handler("test_1".to_string());

        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_net_epoll_handler("test_2".to_string());
        let m = &handler.config.vm_as.clone();

        // Test TX bandwidth rate limiting
        {
            // create bandwidth rate limiter
            let mut rl = RateLimiter::new(0x1000, 0, 100, 0, 0, 0).unwrap();
            // use up the budget
            assert!(rl.consume(0x1000, TokenType::Bytes));

            // set this tx rate limiter to be used
            handler.tx.rate_limiter = rl;
            // try doing TX
            let vq = VirtQueue::new(GuestAddress(0), m, 16);

            let q = vq.create_queue();

            vq.avail.idx().store(1);
            vq.avail.ring(0).store(0);
            vq.dtable(0).set(0x2000, 0x1000, 0, 0);
            handler.tx.queue.queue = q;

            let events = Events::with_data(&event_fd, TX_QUEUE_EVENT, event_set);
            assert!(handler.tx.queue.generate_event().is_ok());
            handler.process(events, &mut event_op);
            assert!(handler.tx.rate_limiter.is_blocked());

            thread::sleep(Duration::from_millis(200));

            let events = Events::with_data(&event_fd, TX_RATE_LIMITER_EVENT, event_set);
            handler.process(events, &mut event_op);
            assert!(!handler.tx.rate_limiter.is_blocked());
        }
        // Test RX bandwidth rate limiting
        {
            // create bandwidth rate limiter
            let mut rl = RateLimiter::new(0x1000, 0, 100, 0, 0, 0).unwrap();
            // use up the budget
            assert!(rl.consume(0x1000, TokenType::Bytes));

            // set this rx rate limiter to be used
            handler.rx.rate_limiter = rl;
            // try doing RX
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);
            vq.dtable(0).set(0x2000, 0x1000, VIRTQ_DESC_F_WRITE, 0);

            let q = vq.create_queue();
            handler.rx.queue.queue = q;

            handler.rx.deferred_frame = true;
            handler.rx.bytes_read = 0x1000;

            let events = Events::with_data(&event_fd, RX_QUEUE_EVENT, event_set);
            assert!(handler.rx.queue.generate_event().is_ok());
            handler.process(events, &mut event_op);
            assert!(handler.rx.rate_limiter.is_blocked());

            thread::sleep(Duration::from_millis(200));

            let events = Events::with_data(&event_fd, RX_RATE_LIMITER_EVENT, event_set);
            handler.process(events, &mut event_op);
            assert!(!handler.rx.rate_limiter.is_blocked());
        }
    }

    #[test]
    fn test_net_ops_rate_limiter() {
        let handler = create_net_epoll_handler("test_1".to_string());

        let event_fd = EventFd::new(0).unwrap();
        let mgr = EpollManager::default();
        let id = mgr.add_subscriber(Box::new(handler));
        let mut inner_mgr = mgr.mgr.lock().unwrap();
        let mut event_op = inner_mgr.event_ops(id).unwrap();
        let event_set = EventSet::EDGE_TRIGGERED;
        let mut handler = create_net_epoll_handler("test_2".to_string());
        let m = &handler.config.vm_as.clone();

        // Test TX ops rate limiting
        {
            // create ops rate limiter
            let mut rl = RateLimiter::new(0, 0, 0, 2, 0, 100).unwrap();
            // use up the budget
            assert!(rl.consume(2, TokenType::Ops));

            // set this tx rate limiter to be used
            handler.tx.rate_limiter = rl;
            // try doing TX
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);

            let q = vq.create_queue();
            handler.tx.queue.queue = q;

            let events = Events::with_data(&event_fd, TX_QUEUE_EVENT, event_set);
            assert!(handler.tx.queue.generate_event().is_ok());
            handler.process(events, &mut event_op);
            assert!(handler.tx.rate_limiter.is_blocked());

            thread::sleep(Duration::from_millis(100));

            let events = Events::with_data(&event_fd, TX_RATE_LIMITER_EVENT, event_set);
            handler.process(events, &mut event_op);
            assert!(!handler.tx.rate_limiter.is_blocked());
        }
        // Test RX ops rate limiting
        {
            // create ops rate limiter
            let mut rl = RateLimiter::new(0, 0, 0, 2, 0, 100).unwrap();
            // use up the budget
            assert!(rl.consume(2, TokenType::Ops));

            // set this rx rate limiter to be used
            handler.rx.rate_limiter = rl;
            // try doing RX
            let vq = VirtQueue::new(GuestAddress(0), m, 16);
            vq.avail.ring(0).store(0);
            vq.avail.idx().store(1);

            let q = vq.create_queue();
            handler.rx.queue.queue = q;

            handler.rx.deferred_frame = true;

            let events = Events::with_data(&event_fd, RX_QUEUE_EVENT, event_set);
            assert!(handler.rx.queue.generate_event().is_ok());
            handler.process(events, &mut event_op);
            assert!(handler.rx.rate_limiter.is_blocked());

            thread::sleep(Duration::from_millis(100));

            let events = Events::with_data(&event_fd, RX_RATE_LIMITER_EVENT, event_set);
            handler.process(events, &mut event_op);
            assert!(!handler.rx.rate_limiter.is_blocked());
        }
    }
}
