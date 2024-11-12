// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

/// `VsockPacket` provides a thin wrapper over the buffers exchanged via virtio
/// queues. There are two components to a vsock packet, each using its own
/// descriptor in a virtio queue:
/// - the packet header; and
/// - the packet data/buffer.
///
/// There is a 1:1 relation between descriptor chains and packets: the first
/// (chain head) holds the header, and an optional second descriptor holds the
/// data. The second descriptor is only present for data packets (VSOCK_OP_RW).
///
/// `VsockPacket` wraps these two buffers and provides direct access to the data
/// stored in guest memory. This is done to avoid unnecessarily copying data
/// from guest memory to temporary buffers, before passing it on to the vsock
/// backend.
use std::ops::{Deref, DerefMut};

use virtio_queue::{Descriptor, DescriptorChain};
use vm_memory::GuestMemory;

use super::defs;
use super::{Result, VsockError};

/// The vsock packet header.
//
// The vsock packet header is defined by the C struct:
//
// ```C
// struct virtio_vsock_hdr {
//     le64 src_cid;
//     le64 dst_cid;
//     le32 src_port;
//     le32 dst_port;
//     le32 len;
//     le16 type;
//     le16 op;
//     le32 flags;
//     le32 buf_alloc;
//     le32 fwd_cnt;
// } __attribute__((packed));
// ```
//
// NOTE: this needs to be marked as repr(C), so we can predict its exact layout
// in memory, since we'll be using guest-provided pointers for access. The Linux
// UAPI headers define this struct as packed, but, in this particular case,
// packing only eliminates 4 trailing padding bytes. Declaring this struct as
// packed would also reduce its alignment to 1, which gets the Rust compiler all
// fidgety. Little does it know, the guest driver already aligned the structure
// properly, so we don't need to worry about alignment. That said, we'll be
// going with only repr(C) (no packing), and hard-coding the struct size as
// `VSOCK_PKT_HDR_SIZE`, since, given this particular layout, the first
// `VSOCK_PKT_HDR_SIZE` bytes are the same in both the packed and unpacked
// layouts.
//
// All fields use the little-endian byte order. Since we're only thinly wrapping
// a pointer to where the guest driver stored the packet header, let's restrict
// this to little-endian targets.
#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct VsockPacketHdr {
    /// Source CID.
    pub src_cid: u64,
    /// Destination CID.
    pub dst_cid: u64,
    /// Source port.
    pub src_port: u32,
    /// Destination port.
    pub dst_port: u32,
    /// Data length (in bytes) - may be 0, if there is now data buffer.
    pub len: u32,
    /// Socket type. Currently, only connection-oriented streams are defined by
    /// the vsock protocol.
    pub type_: u16,
    /// Operation ID - one of the VSOCK_OP_* values; e.g.
    /// - VSOCK_OP_RW: a data packet;
    /// - VSOCK_OP_REQUEST: connection request;
    /// - VSOCK_OP_RST: forcefull connection termination;
    /// etc (see `super::defs::uapi` for the full list).
    pub op: u16,
    /// Additional options (flags) associated with the current operation (`op`).
    /// Currently, only used with shutdown requests (VSOCK_OP_SHUTDOWN).
    pub flags: u32,
    /// Size (in bytes) of the packet sender receive buffer (for the connection
    /// to which this packet belongs).
    pub buf_alloc: u32,
    /// Number of bytes the sender has received and consumed (for the connection
    /// to which this packet belongs). For instance, for our Unix backend, this
    /// counter would be the total number of bytes we have successfully written
    /// to a backing Unix socket.
    pub fwd_cnt: u32,
}

/// The size (in bytes) of the above packet header struct, as present in a
/// virtio queue buffer. See the explanation above on why we are hard-coding
/// this value here.
pub const VSOCK_PKT_HDR_SIZE: usize = 44;

/// A thin wrapper over a `VsockPacketHdr` pointer. This is useful because
/// packet headers are provided by the guest via virtio descriptors (so,
/// basically, pointers). We never need to create header structs - only access
/// them. Access to specific members of the wrapped struct is provided via
/// `Deref` and `DerefMut` impls.
pub struct HdrWrapper {
    ptr: *mut VsockPacketHdr,
}

impl HdrWrapper {
    /// Create the wrapper from a virtio queue descriptor (a pointer), performing some sanity checks
    /// in the process.
    pub fn from_virtq_desc<M: GuestMemory>(desc: &Descriptor, mem: &M) -> Result<Self> {
        if desc.len() < VSOCK_PKT_HDR_SIZE as u32 {
            return Err(VsockError::HdrDescTooSmall(desc.len()));
        }
        // TODO: check buffer alignment

        mem.checked_offset(desc.addr(), VSOCK_PKT_HDR_SIZE)
            .ok_or_else(|| VsockError::GuestMemoryBounds(desc.addr().0, VSOCK_PKT_HDR_SIZE))?;

        // It's safe to create the wrapper from this pointer, as:
        // - the guest driver aligned the data; and
        // - `GuestMemory` is page-aligned.
        Ok(Self::from_ptr_unchecked(
            mem.get_host_address(desc.addr())
                .map_err(VsockError::GuestMemory)?,
        ))
    }

    /// Create the wrapper from a raw pointer.
    ///
    /// Warning: the pointer needs to follow proper alignment for
    /// `VsockPacketHdr`. This is not a problem for virtq buffers, since the
    /// guest driver already handled alignment, and `GuestMemory` is
    /// page-aligned.
    fn from_ptr_unchecked(ptr: *const u8) -> Self {
        #[allow(clippy::cast_ptr_alignment)]
        Self {
            ptr: ptr as *mut VsockPacketHdr,
        }
    }

    /// Provide byte-wise access to the data stored inside the header, via a
    /// slice / fat-pointer.
    pub fn as_slice(&self) -> &[u8] {
        // This is safe, since `Self::from_virtq_head()` already performed all the bound checks.
        //
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, VSOCK_PKT_HDR_SIZE) }
    }

    /// Provide byte-wise mutable access to the data stored inside the header,
    /// via a slice / fat-pointer.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // This is safe, since `Self::from_virtq_head()` already performed all
        // the bound checks.
        unsafe { std::slice::from_raw_parts_mut(self.ptr as *mut u8, VSOCK_PKT_HDR_SIZE) }
    }
}

/// `Deref` implementation for `HdrWrapper`, allowing access to `VsockPacketHdr`
/// individual members.
impl Deref for HdrWrapper {
    type Target = VsockPacketHdr;

    fn deref(&self) -> &VsockPacketHdr {
        // Dereferencing this pointer is safe, because it was already validated
        // by the `HdrWrapper` constructor.
        unsafe { &*self.ptr }
    }
}

/// `DerefMut` implementation for `HdrWrapper`, allowing mutable access to
/// `VsockPacketHdr` individual members.
impl DerefMut for HdrWrapper {
    fn deref_mut(&mut self) -> &mut VsockPacketHdr {
        // Dereferencing this pointer is safe, because it was already validated
        // by the `HdrWrapper` constructor.
        unsafe { &mut *self.ptr }
    }
}

/// A thin wrapper over a vsock data pointer in guest memory. The wrapper is
/// meant to be constructed from a guest-provided virtq descriptor, and provides
/// byte-slice-like access.
pub struct BufWrapper {
    ptr: *mut u8,
    len: usize,
}

impl BufWrapper {
    /// Create the data wrapper from a virtq descriptor.
    pub fn from_virtq_desc<M: GuestMemory>(desc: &Descriptor, mem: &M) -> Result<Self> {
        // Check the guest provided pointer and data size.
        mem.checked_offset(desc.addr(), desc.len() as usize)
            .ok_or_else(|| VsockError::GuestMemoryBounds(desc.addr().0, desc.len() as usize))?;

        Ok(Self::from_fat_ptr_unchecked(
            mem.get_host_address(desc.addr())
                .map_err(VsockError::GuestMemory)?,
            desc.len() as usize,
        ))
    }

    /// Create the data wrapper from a pointer and size.
    ///
    /// Warning: Both `ptr` and `len` must be insured as valid by the caller.
    fn from_fat_ptr_unchecked(ptr: *const u8, len: usize) -> Self {
        Self {
            ptr: ptr as *mut u8,
            len,
        }
    }

    /// Provide access to the data buffer, as a byte slice.
    pub fn as_slice(&self) -> &[u8] {
        // This is safe since bound checks have already been performed when
        // creating the buffer from the virtq descriptor.
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }

    /// Provide mutable access to the data buffer, as a byte slice.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // This is safe since bound checks have already been performed when
        // creating the buffer from the virtq descriptor.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

/// The vsock packet, implemented as a wrapper over a virtq descriptor chain:
/// - the chain head, holding the packet header; and
/// - (an optional) data/buffer descriptor, only present for data packets
///   (VSOCK_OP_RW).
pub struct VsockPacket {
    hdr: HdrWrapper,
    buf: Option<BufWrapper>,
}

impl VsockPacket {
    /// Create the packet wrapper from a TX virtq chain head.
    ///
    /// The chain head is expected to hold valid packet header data. A following
    /// packet buffer descriptor can optionally end the chain. Bounds and
    /// pointer checks are performed when creating the wrapper.
    pub fn from_tx_virtq_head<M: GuestMemory>(
        desc_chain: &mut DescriptorChain<&M>,
    ) -> Result<Self> {
        let desc = desc_chain.next().ok_or(VsockError::BufDescMissing)?;

        // All buffers in the TX queue must be readable.
        if desc.is_write_only() {
            return Err(VsockError::UnreadableDescriptor);
        }

        let hdr = HdrWrapper::from_virtq_desc(&desc, desc_chain.memory())?;

        // Reject weirdly-sized packets.
        if hdr.len > defs::MAX_PKT_BUF_SIZE as u32 {
            return Err(VsockError::InvalidPktLen(hdr.len));
        }

        // Don't bother to look for the data descriptor, if the header says
        // there's no data.
        if hdr.len == 0 {
            return Ok(Self { hdr, buf: None });
        }

        let buf_desc = desc_chain.next().ok_or(VsockError::BufDescMissing)?;

        // All TX buffers must be readable.
        if buf_desc.is_write_only() {
            return Err(VsockError::UnreadableDescriptor);
        }

        // The data descriptor should be large enough to hold the data length
        // indicated by the header.
        if buf_desc.len() < hdr.len {
            return Err(VsockError::BufDescTooSmall);
        }

        Ok(Self {
            hdr,
            buf: Some(BufWrapper::from_virtq_desc(&buf_desc, desc_chain.memory())?),
        })
    }

    /// Create the packet wrapper from an RX virtq chain head.
    ///
    /// There must be two descriptors in the chain, both writable: a header
    /// descriptor and a data descriptor. Bounds and pointer checks are
    /// performed when creating the wrapper.
    pub fn from_rx_virtq_head<M: GuestMemory>(
        desc_chain: &mut DescriptorChain<&M>,
    ) -> Result<Self> {
        let desc = desc_chain.next().ok_or(VsockError::BufDescMissing)?;

        // All RX buffers must be writable.
        if !desc.is_write_only() {
            return Err(VsockError::UnwritableDescriptor);
        }

        let hdr = HdrWrapper::from_virtq_desc(&desc, desc_chain.memory())?;

        let buf_desc = desc_chain.next().ok_or(VsockError::BufDescMissing)?;
        if !buf_desc.is_write_only() {
            return Err(VsockError::UnwritableDescriptor);
        }

        Ok(Self {
            hdr,
            buf: Some(BufWrapper::from_virtq_desc(&buf_desc, desc_chain.memory())?),
        })
    }

    /// Provides in-place, byte-slice, access to the vsock packet header.
    pub fn hdr(&self) -> &[u8] {
        self.hdr.as_slice()
    }

    /// Provides in-place, byte-slice, mutable access to the vsock packet
    /// header.
    pub fn hdr_mut(&mut self) -> &mut [u8] {
        self.hdr.as_mut_slice()
    }

    // Provides in-place, byte-slice access to the vsock packet data buffer.
    ///
    /// Note: control packets (e.g. connection request or reset) have no data
    ///       buffer associated. For those packets, this method will return
    ///       `None`. Also note: calling `len()` on the returned slice will
    ///       yield the buffer size, which may be (and often is) larger than the
    ///       length of the packet data. The packet data length is stored in the
    ///       packet header, and accessible via `VsockPacket::len()`.
    pub fn buf(&self) -> Option<&[u8]> {
        self.buf.as_ref().map(|buf| buf.as_slice())
    }

    /// Provides in-place, byte-slice, mutable access to the vsock packet data
    /// buffer.
    ///
    /// Note: control packets (e.g. connection request or reset) have no data
    ///       buffer associated. For those packets, this method will return
    ///       `None`. Also note: calling `len()` on the returned slice will
    ///       yield the buffer size, which may be (and often is) larger than the
    ///       length of the packet data. The packet data length is stored in the
    ///       packet header, and accessible via `VsockPacket::len()`.
    pub fn buf_mut(&mut self) -> Option<&mut [u8]> {
        self.buf.as_mut().map(|buf| buf.as_mut_slice())
    }

    pub fn src_cid(&self) -> u64 {
        self.hdr.src_cid
    }

    pub fn set_src_cid(&mut self, cid: u64) -> &mut Self {
        self.hdr.src_cid = cid;
        self
    }

    pub fn dst_cid(&self) -> u64 {
        self.hdr.dst_cid
    }

    pub fn set_dst_cid(&mut self, cid: u64) -> &mut Self {
        self.hdr.dst_cid = cid;
        self
    }

    pub fn src_port(&self) -> u32 {
        self.hdr.src_port
    }

    pub fn set_src_port(&mut self, port: u32) -> &mut Self {
        self.hdr.src_port = port;
        self
    }

    pub fn dst_port(&self) -> u32 {
        self.hdr.dst_port
    }

    pub fn set_dst_port(&mut self, port: u32) -> &mut Self {
        self.hdr.dst_port = port;
        self
    }

    pub fn len(&self) -> u32 {
        self.hdr.len
    }

    pub fn set_len(&mut self, len: u32) -> &mut Self {
        self.hdr.len = len;
        self
    }

    pub fn type_(&self) -> u16 {
        self.hdr.type_
    }

    pub fn set_type(&mut self, type_: u16) -> &mut Self {
        self.hdr.type_ = type_;
        self
    }

    pub fn op(&self) -> u16 {
        self.hdr.op
    }

    pub fn set_op(&mut self, op: u16) -> &mut Self {
        self.hdr.op = op;
        self
    }

    pub fn flags(&self) -> u32 {
        self.hdr.flags
    }

    pub fn set_flags(&mut self, flags: u32) -> &mut Self {
        self.hdr.flags = flags;
        self
    }

    pub fn set_flag(&mut self, flag: u32) -> &mut Self {
        self.set_flags(self.flags() | flag);
        self
    }

    pub fn buf_alloc(&self) -> u32 {
        self.hdr.buf_alloc
    }

    pub fn set_buf_alloc(&mut self, buf_alloc: u32) -> &mut Self {
        self.hdr.buf_alloc = buf_alloc;
        self
    }

    pub fn fwd_cnt(&self) -> u32 {
        self.hdr.fwd_cnt
    }

    pub fn set_fwd_cnt(&mut self, fwd_cnt: u32) -> &mut Self {
        self.hdr.fwd_cnt = fwd_cnt;
        self
    }
}

#[cfg(test)]
mod tests {
    use virtio_queue::QueueT;
    use vm_memory::{GuestAddress, GuestMemoryMmap};

    use super::super::defs::MAX_PKT_BUF_SIZE;
    use super::super::tests::{test_bytes, TestContext};
    use super::defs::{RXQ_EVENT, TXQ_EVENT};
    use super::*;
    use crate::tests::{VirtqDesc as GuestQDesc, VIRTQ_DESC_F_WRITE};

    const HDROFF_SRC_CID: usize = 0;
    const HDROFF_DST_CID: usize = 8;
    const HDROFF_SRC_PORT: usize = 16;
    const HDROFF_DST_PORT: usize = 20;
    const HDROFF_LEN: usize = 24;
    const HDROFF_TYPE: usize = 28;
    const HDROFF_OP: usize = 30;
    const HDROFF_FLAGS: usize = 32;
    const HDROFF_BUF_ALLOC: usize = 36;
    const HDROFF_FWD_CNT: usize = 40;

    macro_rules! create_context {
        ($test_ctx:ident, $handler_ctx:ident) => {
            let $test_ctx = TestContext::new();
            let mut $handler_ctx = $test_ctx.create_event_handler_context();
            // For TX packets, hdr.len should be set to a valid value.
            set_pkt_len(1024, &$handler_ctx.guest_txvq.dtable(0), &$test_ctx.mem);
        };
    }

    macro_rules! expect_asm_error {
        (tx, $test_ctx:expr, $handler_ctx:expr, $err:pat) => {
            expect_asm_error!($test_ctx, $handler_ctx, $err, from_tx_virtq_head, TXQ_EVENT);
        };
        (rx, $test_ctx:expr, $handler_ctx:expr, $err:pat) => {
            expect_asm_error!($test_ctx, $handler_ctx, $err, from_rx_virtq_head, RXQ_EVENT);
        };
        ($test_ctx:expr, $handler_ctx:expr, $err:pat, $ctor:ident, $vq_index:ident) => {
            match VsockPacket::$ctor(
                &mut $handler_ctx.queues[$vq_index as usize]
                    .queue_mut()
                    .pop_descriptor_chain(&$test_ctx.mem)
                    .unwrap(),
            ) {
                Err($err) => (),
                Ok(_) => panic!("Packet assembly should've failed!"),
                Err(other) => panic!("Packet assembly failed with: {:?}", other),
            }
        };
    }

    fn set_pkt_len(len: u32, guest_desc: &GuestQDesc, mem: &GuestMemoryMmap) {
        let hdr_gpa = guest_desc.addr();
        let hdr_ptr = mem.get_host_address(GuestAddress(hdr_gpa.load())).unwrap();
        let len_ptr = unsafe { hdr_ptr.add(HDROFF_LEN) };

        unsafe { std::slice::from_raw_parts_mut(len_ptr, 4).copy_from_slice(&len.to_le_bytes()) };
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_tx_packet_assembly() {
        // Test case: successful TX packet assembly.
        {
            create_context!(test_ctx, handler_ctx);
            let pkt = VsockPacket::from_tx_virtq_head(
                &mut handler_ctx.queues[TXQ_EVENT as usize]
                    .queue_mut()
                    .pop_descriptor_chain(&test_ctx.mem)
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(pkt.hdr().len(), VSOCK_PKT_HDR_SIZE);
            assert_eq!(
                pkt.buf().unwrap().len(),
                handler_ctx.guest_txvq.dtable(1).len().load() as usize
            );
        }

        // Test case: error on write-only hdr descriptor.
        {
            create_context!(test_ctx, handler_ctx);
            handler_ctx
                .guest_txvq
                .dtable(0)
                .flags()
                .store(VIRTQ_DESC_F_WRITE);
            expect_asm_error!(tx, test_ctx, handler_ctx, VsockError::UnreadableDescriptor);
        }

        // Test case: header descriptor has insufficient space to hold the packet header.
        {
            create_context!(test_ctx, handler_ctx);
            handler_ctx
                .guest_txvq
                .dtable(0)
                .len()
                .store(VSOCK_PKT_HDR_SIZE as u32 - 1);
            expect_asm_error!(tx, test_ctx, handler_ctx, VsockError::HdrDescTooSmall(_));
        }

        // Test case: zero-length TX packet.
        {
            create_context!(test_ctx, handler_ctx);
            set_pkt_len(0, &handler_ctx.guest_txvq.dtable(0), &test_ctx.mem);
            let mut pkt = VsockPacket::from_tx_virtq_head(
                &mut handler_ctx.queues[TXQ_EVENT as usize]
                    .queue_mut()
                    .pop_descriptor_chain(&test_ctx.mem)
                    .unwrap(),
            )
            .unwrap();
            assert!(pkt.buf().is_none());
            assert!(pkt.buf_mut().is_none());
        }

        // Test case: TX packet has more data than we can handle.
        {
            create_context!(test_ctx, handler_ctx);
            set_pkt_len(
                MAX_PKT_BUF_SIZE as u32 + 1,
                &handler_ctx.guest_txvq.dtable(0),
                &test_ctx.mem,
            );
            expect_asm_error!(tx, test_ctx, handler_ctx, VsockError::InvalidPktLen(_));
        }

        // Test case:
        // - packet header advertises some data length; and
        // - the data descriptor is missing.
        {
            create_context!(test_ctx, handler_ctx);
            set_pkt_len(1024, &handler_ctx.guest_txvq.dtable(0), &test_ctx.mem);
            handler_ctx.guest_txvq.dtable(0).flags().store(0);
            expect_asm_error!(tx, test_ctx, handler_ctx, VsockError::BufDescMissing);
        }

        // Test case: error on write-only buf descriptor.
        {
            create_context!(test_ctx, handler_ctx);
            handler_ctx
                .guest_txvq
                .dtable(1)
                .flags()
                .store(VIRTQ_DESC_F_WRITE);
            expect_asm_error!(tx, test_ctx, handler_ctx, VsockError::UnreadableDescriptor);
        }

        // Test case: the buffer descriptor cannot fit all the data advertised by the the
        // packet header `len` field.
        {
            create_context!(test_ctx, handler_ctx);
            set_pkt_len(8 * 1024, &handler_ctx.guest_txvq.dtable(0), &test_ctx.mem);
            handler_ctx.guest_txvq.dtable(1).len().store(4 * 1024);
            expect_asm_error!(tx, test_ctx, handler_ctx, VsockError::BufDescTooSmall);
        }
    }

    #[test]
    fn test_rx_packet_assembly() {
        // Test case: successful RX packet assembly.
        {
            create_context!(test_ctx, handler_ctx);
            let pkt = VsockPacket::from_rx_virtq_head(
                &mut handler_ctx.queues[RXQ_EVENT as usize]
                    .queue_mut()
                    .pop_descriptor_chain(&test_ctx.mem)
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(pkt.hdr().len(), VSOCK_PKT_HDR_SIZE);
            assert_eq!(
                pkt.buf().unwrap().len(),
                handler_ctx.guest_rxvq.dtable(1).len().load() as usize
            );
        }

        // Test case: read-only RX packet header.
        {
            create_context!(test_ctx, handler_ctx);
            handler_ctx.guest_rxvq.dtable(0).flags().store(0);
            expect_asm_error!(rx, test_ctx, handler_ctx, VsockError::UnwritableDescriptor);
        }

        // Test case: RX descriptor head cannot fit the entire packet header.
        {
            create_context!(test_ctx, handler_ctx);
            handler_ctx
                .guest_rxvq
                .dtable(0)
                .len()
                .store(VSOCK_PKT_HDR_SIZE as u32 - 1);
            expect_asm_error!(rx, test_ctx, handler_ctx, VsockError::HdrDescTooSmall(_));
        }

        // Test case: RX descriptor chain is missing the packet buffer descriptor.
        {
            create_context!(test_ctx, handler_ctx);
            handler_ctx
                .guest_rxvq
                .dtable(0)
                .flags()
                .store(VIRTQ_DESC_F_WRITE);
            expect_asm_error!(rx, test_ctx, handler_ctx, VsockError::BufDescMissing);
        }
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_packet_hdr_accessors() {
        const SRC_CID: u64 = 1;
        const DST_CID: u64 = 2;
        const SRC_PORT: u32 = 3;
        const DST_PORT: u32 = 4;
        const LEN: u32 = 5;
        const TYPE: u16 = 6;
        const OP: u16 = 7;
        const FLAGS: u32 = 8;
        const BUF_ALLOC: u32 = 9;
        const FWD_CNT: u32 = 10;

        create_context!(test_ctx, handler_ctx);
        let mut pkt = VsockPacket::from_rx_virtq_head(
            &mut handler_ctx.queues[RXQ_EVENT as usize]
                .queue_mut()
                .pop_descriptor_chain(&test_ctx.mem)
                .unwrap(),
        )
        .unwrap();

        // Test field accessors.
        pkt.set_src_cid(SRC_CID)
            .set_dst_cid(DST_CID)
            .set_src_port(SRC_PORT)
            .set_dst_port(DST_PORT)
            .set_len(LEN)
            .set_type(TYPE)
            .set_op(OP)
            .set_flags(FLAGS)
            .set_buf_alloc(BUF_ALLOC)
            .set_fwd_cnt(FWD_CNT);

        assert_eq!(pkt.src_cid(), SRC_CID);
        assert_eq!(pkt.dst_cid(), DST_CID);
        assert_eq!(pkt.src_port(), SRC_PORT);
        assert_eq!(pkt.dst_port(), DST_PORT);
        assert_eq!(pkt.len(), LEN);
        assert_eq!(pkt.type_(), TYPE);
        assert_eq!(pkt.op(), OP);
        assert_eq!(pkt.flags(), FLAGS);
        assert_eq!(pkt.buf_alloc(), BUF_ALLOC);
        assert_eq!(pkt.fwd_cnt(), FWD_CNT);

        // Test individual flag setting.
        let flags = pkt.flags() | 0b1000;
        pkt.set_flag(0b1000);
        assert_eq!(pkt.flags(), flags);

        // Test packet header as-slice access.
        //

        assert_eq!(pkt.hdr().len(), VSOCK_PKT_HDR_SIZE);

        test_bytes(&SRC_CID.to_le_bytes(), &pkt.hdr()[HDROFF_SRC_CID..]);
        test_bytes(&DST_CID.to_le_bytes(), &pkt.hdr()[HDROFF_DST_CID..]);
        test_bytes(&SRC_PORT.to_le_bytes(), &pkt.hdr()[HDROFF_SRC_PORT..]);
        test_bytes(&DST_PORT.to_le_bytes(), &pkt.hdr()[HDROFF_DST_PORT..]);
        test_bytes(&LEN.to_le_bytes(), &pkt.hdr()[HDROFF_LEN..]);
        test_bytes(&TYPE.to_le_bytes(), &pkt.hdr()[HDROFF_TYPE..]);
        test_bytes(&OP.to_le_bytes(), &pkt.hdr()[HDROFF_OP..]);
        test_bytes(&FLAGS.to_le_bytes(), &pkt.hdr()[HDROFF_FLAGS..]);
        test_bytes(&BUF_ALLOC.to_le_bytes(), &pkt.hdr()[HDROFF_BUF_ALLOC..]);
        test_bytes(&FWD_CNT.to_le_bytes(), &pkt.hdr()[HDROFF_FWD_CNT..]);

        assert_eq!(pkt.hdr_mut().len(), VSOCK_PKT_HDR_SIZE);
        for b in pkt.hdr_mut() {
            *b = 0;
        }
        assert_eq!(pkt.src_cid(), 0);
        assert_eq!(pkt.dst_cid(), 0);
        assert_eq!(pkt.src_port(), 0);
        assert_eq!(pkt.dst_port(), 0);
        assert_eq!(pkt.len(), 0);
        assert_eq!(pkt.type_(), 0);
        assert_eq!(pkt.op(), 0);
        assert_eq!(pkt.flags(), 0);
        assert_eq!(pkt.buf_alloc(), 0);
        assert_eq!(pkt.fwd_cnt(), 0);
    }

    #[test]
    fn test_packet_buf() {
        create_context!(test_ctx, handler_ctx);
        let mut pkt = VsockPacket::from_rx_virtq_head(
            &mut handler_ctx.queues[RXQ_EVENT as usize]
                .queue_mut()
                .pop_descriptor_chain(&test_ctx.mem)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(
            pkt.buf().unwrap().len(),
            handler_ctx.guest_rxvq.dtable(1).len().load() as usize
        );
        assert_eq!(
            pkt.buf_mut().unwrap().len(),
            handler_ctx.guest_rxvq.dtable(1).len().load() as usize
        );

        for i in 0..pkt.buf().unwrap().len() {
            pkt.buf_mut().unwrap()[i] = (i % 0x100) as u8;
            assert_eq!(pkt.buf().unwrap()[i], (i % 0x100) as u8);
        }
    }
}
