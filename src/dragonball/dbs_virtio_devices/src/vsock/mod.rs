// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

pub mod backend;
pub mod csm;
mod device;
mod epoll_handler;
pub mod muxer;
mod packet;

use std::os::unix::io::AsRawFd;

use vm_memory::GuestMemoryError;

pub use self::defs::{NUM_QUEUES, QUEUE_SIZES};
pub use self::device::Vsock;
use self::muxer::Error as MuxerError;
pub use self::muxer::VsockMuxer;
use self::packet::VsockPacket;

mod defs {
    /// RX queue event: the driver added available buffers to the RX queue.
    pub const RXQ_EVENT: u32 = 0;
    /// TX queue event: the driver added available buffers to the RX queue.
    pub const TXQ_EVENT: u32 = 1;
    /// Event queue event: the driver added available buffers to the event
    /// queue.
    pub const EVQ_EVENT: u32 = 2;
    /// Backend event: the backend needs a kick.
    pub const BACKEND_EVENT: u32 = 3;

    /// Number of virtio queues.
    pub const NUM_QUEUES: usize = 3;
    /// Virtio queue sizes, in number of descriptor chain heads.
    ///
    /// There are 3 queues for a virtio device (in this order): RX, TX, Event
    pub const QUEUE_SIZES: &[u16] = &[256; NUM_QUEUES];

    /// Max vsock packet data/buffer size.
    pub const MAX_PKT_BUF_SIZE: usize = 64 * 1024;

    pub mod uapi {
        /// Virtio feature flags.
        ///
        /// Defined in `/include/uapi/linux/virtio_config.h`.
        ///
        /// The device processes available buffers in the same order in which
        /// the device offers them.
        pub const VIRTIO_F_IN_ORDER: usize = 35;
        /// The device conforms to the virtio spec version 1.0.
        pub const VIRTIO_F_VERSION_1: u32 = 32;

        /// Virtio vsock device ID.
        ///
        /// Defined in `include/uapi/linux/virtio_ids.h`.
        pub const VIRTIO_ID_VSOCK: u32 = 19;

        /// Vsock packet operation IDs.
        ///
        /// Defined in `/include/uapi/linux/virtio_vsock.h`.
        ///
        /// Connection request.
        pub const VSOCK_OP_REQUEST: u16 = 1;
        /// Connection response.
        pub const VSOCK_OP_RESPONSE: u16 = 2;
        /// Connection reset.
        pub const VSOCK_OP_RST: u16 = 3;
        /// Connection clean shutdown.
        pub const VSOCK_OP_SHUTDOWN: u16 = 4;
        /// Connection data (read/write).
        pub const VSOCK_OP_RW: u16 = 5;
        /// Flow control credit update.
        pub const VSOCK_OP_CREDIT_UPDATE: u16 = 6;
        /// Flow control credit update request.
        pub const VSOCK_OP_CREDIT_REQUEST: u16 = 7;

        /// Vsock packet flags. Defined in `/include/uapi/linux/virtio_vsock.h`.
        ///
        /// Valid with a VSOCK_OP_SHUTDOWN packet: the packet sender will
        /// receive no more data.
        pub const VSOCK_FLAGS_SHUTDOWN_RCV: u32 = 1;
        /// Valid with a VSOCK_OP_SHUTDOWN packet: the packet sender will send
        /// no more data.
        pub const VSOCK_FLAGS_SHUTDOWN_SEND: u32 = 2;

        /// Vsock packet type.
        /// Defined in `/include/uapi/linux/virtio_vsock.h`.
        ///
        /// Stream / connection-oriented packet (the only currently valid type).
        pub const VSOCK_TYPE_STREAM: u16 = 1;

        /// Well known vsock CID for host system.
        pub const VSOCK_HOST_CID: u64 = 2;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VsockError {
    /// vsock backend error
    #[error("Vsock backend error: {0}")]
    Backend(#[source] std::io::Error),
    /// The vsock data/buffer virtio descriptor is expected, but missing.
    #[error("The vsock data/buffer virtio descriptor is expected, but missing")]
    BufDescMissing,
    /// The vsock data/buffer virtio descriptor length is smaller than expected.
    #[error("The vsock data/buffer virtio descriptor length is smaller than expected")]
    BufDescTooSmall,
    /// Chained GuestMemory error.
    #[error("Chained GuestMemory error: {0}")]
    GuestMemory(#[source] GuestMemoryError),
    /// Bounds check failed on guest memory pointer.
    #[error("Bounds check failed on guest memory pointer, addr: {0}, size: {1}")]
    GuestMemoryBounds(u64, usize),
    /// The vsock header descriptor length is too small.
    #[error("The vsock header descriptor length {0} is too small")]
    HdrDescTooSmall(u32),
    /// The vsock header `len` field holds an invalid value.
    #[error("The vsock header `len` field holds an invalid value {0}")]
    InvalidPktLen(u32),
    /// vsock muxer error
    #[error("Vsock muxer error: {0}")]
    Muxer(#[source] MuxerError),
    /// A data fetch was attempted when no data was available.
    #[error("A data fetch was attempted when no data was available")]
    NoData,
    /// A data buffer was expected for the provided packet, but it is missing.
    #[error("A data buffer was expected for the provided packet, but it is missing")]
    PktBufMissing,
    /// Encountered an unexpected write-only virtio descriptor.
    #[error("Encountered an unexpected write-only virtio descriptor")]
    UnreadableDescriptor,
    /// Encountered an unexpected read-only virtio descriptor.
    #[error("Encountered an unexpected read-only virtio descriptor")]
    UnwritableDescriptor,
}

type Result<T> = std::result::Result<T, VsockError>;

/// A passive, event-driven object, that needs to be notified whenever an
/// epoll-able event occurs. An event-polling control loop will use
/// `get_polled_fd()` and `get_polled_evset()` to query the listener for the
/// file descriptor and the set of events it's interested in. When such an event
/// occurs, the control loop will route the event to the listener via
/// `notify()`.
pub trait VsockEpollListener: AsRawFd {
    /// Get the set of events for which the listener wants to be notified.
    fn get_polled_evset(&self) -> epoll::Events;

    /// Notify the listener that one ore more events have occured.
    fn notify(&mut self, evset: epoll::Events);
}

/// Any channel that handles vsock packet traffic: sending and receiving
/// packets. Since we're implementing the device model here, our responsibility
/// is to always process the sending of packets (i.e. the TX queue). So, any
/// locally generated data, addressed to the driver (e.g. a connection response
/// or RST), will have to be queued, until we get to processing the RX queue.
///
/// Note: `recv_pkt()` and `send_pkt()` are named analogous to `Read::read()`
///       and `Write::write()`, respectively. I.e. - `recv_pkt()` will read data
///       from the channel, and place it into a packet; and - `send_pkt()` will
///       fetch data from a packet, and place it into the channel.
pub trait VsockChannel {
    /// Read/receive an incoming packet from the channel.
    fn recv_pkt(&mut self, pkt: &mut VsockPacket) -> Result<()>;

    /// Write/send a packet through the channel.
    fn send_pkt(&mut self, pkt: &VsockPacket) -> Result<()>;

    /// Checks weather there is pending incoming data inside the channel,
    /// meaning that a subsequent call to `recv_pkt()` won't fail.
    fn has_pending_rx(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;
    use std::os::unix::io::{AsRawFd, RawFd};
    use std::sync::Arc;

    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::NoopNotifier;
    use dbs_utils::epoll_manager::EpollManager;
    use kvm_ioctls::Kvm;
    use virtio_queue::QueueSync;
    use vm_memory::{GuestAddress, GuestAddressSpace, GuestMemoryMmap, GuestRegionMmap};
    use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};

    use super::backend::VsockBackend;
    use super::defs::{EVQ_EVENT, RXQ_EVENT, TXQ_EVENT};
    use super::epoll_handler::VsockEpollHandler;
    use super::muxer::{Result as MuxerResult, VsockGenericMuxer};
    use super::packet::{VsockPacket, VSOCK_PKT_HDR_SIZE};
    use super::*;
    use crate::device::VirtioDeviceConfig;
    use crate::tests::{
        create_address_space, VirtQueue as GuestQ, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE,
    };
    use crate::Result as VirtioResult;
    use crate::VirtioQueueConfig;

    pub fn test_bytes(src: &[u8], dst: &[u8]) {
        let min_len = std::cmp::min(src.len(), dst.len());
        assert_eq!(src[0..min_len], dst[0..min_len])
    }

    type Result<T> = std::result::Result<T, VsockError>;

    pub struct TestMuxer {
        pub evfd: EventFd,
        pub rx_err: Option<VsockError>,
        pub tx_err: Option<VsockError>,
        pub pending_rx: bool,
        pub rx_ok_cnt: usize,
        pub tx_ok_cnt: usize,
        pub evset: Option<epoll::Events>,
    }

    impl TestMuxer {
        pub fn new() -> Self {
            Self {
                evfd: EventFd::new(EFD_NONBLOCK).unwrap(),
                rx_err: None,
                tx_err: None,
                pending_rx: false,
                rx_ok_cnt: 0,
                tx_ok_cnt: 0,
                evset: None,
            }
        }

        pub fn set_rx_err(&mut self, err: Option<VsockError>) {
            self.rx_err = err;
        }
        pub fn set_tx_err(&mut self, err: Option<VsockError>) {
            self.tx_err = err;
        }
        pub fn set_pending_rx(&mut self, prx: bool) {
            self.pending_rx = prx;
        }
    }

    impl Default for TestMuxer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl VsockChannel for TestMuxer {
        fn recv_pkt(&mut self, _pkt: &mut VsockPacket) -> Result<()> {
            let cool_buf = [0xDu8, 0xE, 0xA, 0xD, 0xB, 0xE, 0xE, 0xF];
            match self.rx_err.take() {
                None => {
                    if let Some(buf) = _pkt.buf_mut() {
                        for i in 0..buf.len() {
                            buf[i] = cool_buf[i % cool_buf.len()];
                        }
                    }
                    self.rx_ok_cnt += 1;
                    Ok(())
                }
                Some(e) => Err(e),
            }
        }

        fn send_pkt(&mut self, _pkt: &VsockPacket) -> Result<()> {
            match self.tx_err.take() {
                None => {
                    self.tx_ok_cnt += 1;
                    Ok(())
                }
                Some(e) => Err(e),
            }
        }

        fn has_pending_rx(&self) -> bool {
            self.pending_rx
        }
    }

    impl AsRawFd for TestMuxer {
        fn as_raw_fd(&self) -> RawFd {
            self.evfd.as_raw_fd()
        }
    }

    impl VsockEpollListener for TestMuxer {
        fn get_polled_evset(&self) -> epoll::Events {
            epoll::Events::EPOLLIN
        }
        fn notify(&mut self, evset: epoll::Events) {
            self.evset = Some(evset);
        }
    }

    impl VsockGenericMuxer for TestMuxer {
        fn add_backend(
            &mut self,
            _backend: Box<dyn VsockBackend>,
            _is_peer_backend: bool,
        ) -> MuxerResult<()> {
            Ok(())
        }
    }

    pub struct TestContext {
        pub cid: u64,
        pub mem: GuestMemoryMmap,
        pub mem_size: usize,
        pub epoll_manager: EpollManager,
        pub device: Vsock<Arc<GuestMemoryMmap>, TestMuxer>,
    }

    impl TestContext {
        pub fn new() -> Self {
            const CID: u64 = 52;
            const MEM_SIZE: usize = 1024 * 1024 * 128;
            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), MEM_SIZE)]).unwrap();
            let epoll_manager = EpollManager::default();
            Self {
                cid: CID,
                mem,
                mem_size: MEM_SIZE,
                epoll_manager: epoll_manager.clone(),
                device: Vsock::new_with_muxer(
                    CID,
                    Arc::new(defs::QUEUE_SIZES.to_vec()),
                    epoll_manager,
                    TestMuxer::new(),
                )
                .unwrap(),
            }
        }

        pub fn create_event_handler_context(&self) -> EventHandlerContext {
            const QSIZE: u16 = 256;

            let guest_rxvq = GuestQ::new(GuestAddress(0x0010_0000), &self.mem, QSIZE);
            let guest_txvq = GuestQ::new(GuestAddress(0x0020_0000), &self.mem, QSIZE);
            let guest_evvq = GuestQ::new(GuestAddress(0x0030_0000), &self.mem, QSIZE);
            let rxvq = guest_rxvq.create_queue();
            let txvq = guest_txvq.create_queue();
            let evvq = guest_evvq.create_queue();

            let rxvq_config = VirtioQueueConfig::new(
                rxvq,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                RXQ_EVENT as u16,
            );
            let txvq_config = VirtioQueueConfig::new(
                txvq,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                TXQ_EVENT as u16,
            );
            let evvq_config = VirtioQueueConfig::new(
                evvq,
                Arc::new(EventFd::new(0).unwrap()),
                Arc::new(NoopNotifier::new()),
                EVQ_EVENT as u16,
            );

            // Set up one available descriptor in the RX queue.
            guest_rxvq.dtable(0).set(
                0x0040_0000,
                VSOCK_PKT_HDR_SIZE as u32,
                VIRTQ_DESC_F_WRITE | VIRTQ_DESC_F_NEXT,
                1,
            );
            guest_rxvq
                .dtable(1)
                .set(0x0040_1000, 4096, VIRTQ_DESC_F_WRITE, 0);

            guest_rxvq.avail.ring(0).store(0);
            guest_rxvq.avail.idx().store(1);

            // Set up one available descriptor in the TX queue.
            guest_txvq
                .dtable(0)
                .set(0x0050_0000, VSOCK_PKT_HDR_SIZE as u32, VIRTQ_DESC_F_NEXT, 1);
            guest_txvq.dtable(1).set(0x0050_1000, 4096, 0, 0);
            guest_txvq.avail.ring(0).store(0);
            guest_txvq.avail.idx().store(1);

            let queues = vec![rxvq_config, txvq_config, evvq_config];
            EventHandlerContext {
                guest_rxvq,
                guest_txvq,
                guest_evvq,
                queues,
                epoll_handler: None,
                device: Vsock::new_with_muxer(
                    self.cid,
                    Arc::new(defs::QUEUE_SIZES.to_vec()),
                    EpollManager::default(),
                    TestMuxer::new(),
                )
                .unwrap(),
                mem: Arc::new(self.mem.clone()),
            }
        }
    }

    impl Default for TestContext {
        fn default() -> Self {
            Self::new()
        }
    }

    pub struct EventHandlerContext<'a> {
        pub device: Vsock<Arc<GuestMemoryMmap>, TestMuxer>,
        pub epoll_handler:
            Option<VsockEpollHandler<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap, TestMuxer>>,
        pub queues: Vec<VirtioQueueConfig<QueueSync>>,
        pub guest_rxvq: GuestQ<'a>,
        pub guest_txvq: GuestQ<'a>,
        pub guest_evvq: GuestQ<'a>,
        pub mem: Arc<GuestMemoryMmap>,
    }

    impl<'a> EventHandlerContext<'a> {
        // Artificially activate the device.
        pub fn arti_activate(&mut self, mem: &GuestMemoryMmap) {
            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::<Arc<GuestMemoryMmap<()>>, QueueSync>::new(
                Arc::new(mem.clone()),
                address_space,
                vm_fd,
                resources,
                self.queues.drain(..).collect(),
                None,
                Arc::new(NoopNotifier::new()),
            );

            let epoll_handler = self.device.mock_activate(config).unwrap();
            self.epoll_handler = Some(epoll_handler);
        }

        pub fn handle_txq_event(&mut self, mem: &GuestMemoryMmap) {
            if let Some(epoll_handler) = &mut self.epoll_handler {
                epoll_handler.config.queues[TXQ_EVENT as usize]
                    .generate_event()
                    .unwrap();
                epoll_handler.handle_txq_event(mem);
            }
        }

        pub fn handle_rxq_event(&mut self, mem: &GuestMemoryMmap) {
            if let Some(epoll_handler) = &mut self.epoll_handler {
                epoll_handler.config.queues[TXQ_EVENT as usize]
                    .generate_event()
                    .unwrap();
                epoll_handler.handle_rxq_event(mem);
            }
        }

        pub fn signal_txq_event(&mut self) {
            if let Some(epoll_handler) = &mut self.epoll_handler {
                epoll_handler.config.queues[TXQ_EVENT as usize]
                    .generate_event()
                    .unwrap();
            }
            let mem_guard = self.mem.memory();
            let mem = mem_guard.deref();
            self.handle_txq_event(mem);
        }

        pub fn signal_rxq_event(&mut self) {
            if let Some(epoll_handler) = &mut self.epoll_handler {
                epoll_handler.config.queues[RXQ_EVENT as usize]
                    .generate_event()
                    .unwrap();
            }
            let mem_guard = self.mem.memory();
            let mem = mem_guard.deref();
            self.handle_rxq_event(mem);
        }

        pub fn signal_used_queue(&mut self, idx: usize) -> VirtioResult<()> {
            if let Some(epoll_handler) = &mut self.epoll_handler {
                epoll_handler.config.queues[RXQ_EVENT as usize]
                    .generate_event()
                    .unwrap();
                epoll_handler.signal_used_queue(idx).unwrap();
            }

            Ok(())
        }
    }
}
