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
use virtio_queue::{desc::split::Descriptor, DescriptorChain};
use vm_memory::{Address, GuestMemory};

use super::defs;
use super::{Result, VsockError};

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
// The header lives in guest memory and is only guaranteed to be valid for its
// 44 wire bytes, at whatever address the guest driver chose. A `repr(C)` Rust
// struct with the same fields is 48 bytes long and 8-byte aligned, so it must
// not be used to reference the header. Fields are accessed at their wire
// offsets instead, in little-endian byte order.

/// The size (in bytes) of the vsock packet header, as present in a virtio
/// queue buffer.
pub const VSOCK_PKT_HDR_SIZE: usize = 44;
const VSOCK_PKT_HDR_SRC_CID_OFFSET: usize = 0;
const VSOCK_PKT_HDR_DST_CID_OFFSET: usize = 8;
const VSOCK_PKT_HDR_SRC_PORT_OFFSET: usize = 16;
const VSOCK_PKT_HDR_DST_PORT_OFFSET: usize = 20;
const VSOCK_PKT_HDR_LEN_OFFSET: usize = 24;
const VSOCK_PKT_HDR_TYPE_OFFSET: usize = 28;
const VSOCK_PKT_HDR_OP_OFFSET: usize = 30;
const VSOCK_PKT_HDR_FLAGS_OFFSET: usize = 32;
const VSOCK_PKT_HDR_BUF_ALLOC_OFFSET: usize = 36;
const VSOCK_PKT_HDR_FWD_CNT_OFFSET: usize = 40;

/// A thin wrapper over the 44-byte packed vsock wire header provided through a
/// virtio descriptor.
///
/// The guest-provided header has no alignment guarantee, so fields are read
/// and written individually with unaligned scalar operations.
pub struct HdrWrapper {
    ptr: *mut u8,
}

impl HdrWrapper {
    /// Create the wrapper from a virtio queue descriptor (a pointer), performing some sanity checks
    /// in the process.
    pub fn from_virtq_desc<M: GuestMemory>(desc: &Descriptor, mem: &M) -> Result<Self> {
        if desc.len() < VSOCK_PKT_HDR_SIZE as u32 {
            return Err(VsockError::HdrDescTooSmall(desc.len()));
        }

        // Validate the ENTIRE [desc.addr(), desc.addr() + VSOCK_PKT_HDR_SIZE) range is a
        // single contiguous guest-memory mapping before building the raw header pointer.
        // `get_host_address()` only validates the start address, so without this check a
        // compromised guest could place the header at the end of a guest-memory region and
        // have the 44-byte header access cross the region boundary into unrelated host
        // memory (out-of-bounds read/write in the shim).
        let slice = mem
            .get_slice(desc.addr(), VSOCK_PKT_HDR_SIZE)
            .map_err(VsockError::GuestMemory)?;

        Ok(Self::from_ptr_unchecked(slice.ptr_guard().as_ptr()))
    }

    /// Create the wrapper from a raw pointer.
    ///
    /// The caller must ensure that the pointer remains valid for all 44 header
    /// bytes and that mutable operations are permitted when setters are used.
    fn from_ptr_unchecked(ptr: *const u8) -> Self {
        Self {
            ptr: ptr as *mut u8,
        }
    }

    #[inline]
    fn read_u16(&self, offset: usize) -> u16 {
        // SAFETY: the constructor validated the complete 44-byte mapping. All
        // offsets are compile-time wire-layout constants within that mapping.
        u16::from_le(unsafe { (self.ptr.add(offset) as *const u16).read_unaligned() })
    }

    #[inline]
    fn read_u32(&self, offset: usize) -> u32 {
        // SAFETY: see `read_u16`.
        u32::from_le(unsafe { (self.ptr.add(offset) as *const u32).read_unaligned() })
    }

    #[inline]
    fn read_u64(&self, offset: usize) -> u64 {
        // SAFETY: see `read_u16`.
        u64::from_le(unsafe { (self.ptr.add(offset) as *const u64).read_unaligned() })
    }

    #[inline]
    fn write_u16(&mut self, offset: usize, value: u16) {
        // SAFETY: the constructor validated the complete 44-byte mapping and
        // callers only use this through a mutable packet.
        unsafe {
            (self.ptr.add(offset) as *mut u16).write_unaligned(value.to_le());
        }
    }

    #[inline]
    fn write_u32(&mut self, offset: usize, value: u32) {
        // SAFETY: see `write_u16`.
        unsafe {
            (self.ptr.add(offset) as *mut u32).write_unaligned(value.to_le());
        }
    }

    #[inline]
    fn write_u64(&mut self, offset: usize, value: u64) {
        // SAFETY: see `write_u16`.
        unsafe {
            (self.ptr.add(offset) as *mut u64).write_unaligned(value.to_le());
        }
    }

    /// Provide byte-wise access to the data stored inside the header, via a
    /// slice / fat-pointer.
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: the constructor validated the complete 44-byte mapping.
        unsafe { std::slice::from_raw_parts(self.ptr, VSOCK_PKT_HDR_SIZE) }
    }

    /// Provide byte-wise mutable access to the data stored inside the header,
    /// via a slice / fat-pointer.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: the constructor validated the complete 44-byte mapping and
        // the mutable borrow prevents concurrent access through this wrapper.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, VSOCK_PKT_HDR_SIZE) }
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
    /// Add offset and size, to support different forms of descriptor.
    pub fn from_virtq_desc<M: GuestMemory>(
        desc: &Descriptor,
        mem: &M,
        offset: usize,
        size: usize,
    ) -> Result<Self> {
        let base = desc
            .addr()
            .checked_add(offset as u64)
            .ok_or(VsockError::Overflow(desc.addr(), offset))?;

        // Validate the ENTIRE [base, base + size) range actually accessed by the data
        // buffer is a single contiguous guest-memory mapping. `get_host_address()` only
        // validates the start address, so without this check a compromised guest could
        // place the buffer at the end of a guest-memory region and have the `size`-byte
        // access cross the region boundary into unrelated host memory (out-of-bounds
        // read/write in the shim).
        let slice = mem.get_slice(base, size).map_err(VsockError::GuestMemory)?;

        Ok(Self::from_fat_ptr_unchecked(
            slice.ptr_guard().as_ptr(),
            size,
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
        let len = hdr.read_u32(VSOCK_PKT_HDR_LEN_OFFSET);

        // Reject weirdly-sized packets.
        if len > defs::MAX_PKT_BUF_SIZE as u32 {
            return Err(VsockError::InvalidPktLen(len));
        }

        // Don't bother to look for the data descriptor, if the header says
        // there's no data.
        if len == 0 {
            return Ok(Self { hdr, buf: None });
        }

        if desc.has_next() {
            let buf_desc = desc_chain.next().ok_or(VsockError::BufDescMissing)?;

            // All TX buffers must be readable.
            if buf_desc.is_write_only() {
                return Err(VsockError::UnreadableDescriptor);
            }

            // The data descriptor should be large enough to hold the data length
            // indicated by the header.
            if buf_desc.len() < len {
                return Err(VsockError::BufDescTooSmall);
            }

            Ok(Self {
                hdr,
                buf: Some(BufWrapper::from_virtq_desc(
                    &buf_desc,
                    desc_chain.memory(),
                    0,
                    buf_desc.len() as usize,
                )?),
            })
        } else {
            let buf_size = desc.len() as usize - VSOCK_PKT_HDR_SIZE;

            // The data descriptor should be large enough to hold the data length
            // indicated by the header.
            if buf_size < len as usize {
                return Err(VsockError::BufDescTooSmall);
            }

            Ok(Self {
                hdr,
                buf: Some(BufWrapper::from_virtq_desc(
                    &desc,
                    desc_chain.memory(),
                    VSOCK_PKT_HDR_SIZE,
                    buf_size,
                )?),
            })
        }
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

        if desc.has_next() {
            let buf_desc = desc_chain.next().ok_or(VsockError::BufDescMissing)?;
            if !buf_desc.is_write_only() {
                return Err(VsockError::UnwritableDescriptor);
            }

            Ok(Self {
                hdr,
                buf: Some(BufWrapper::from_virtq_desc(
                    &buf_desc,
                    desc_chain.memory(),
                    0,
                    buf_desc.len() as usize,
                )?),
            })
        } else {
            let buf_size = desc.len() as usize - VSOCK_PKT_HDR_SIZE;

            Ok(Self {
                hdr,
                buf: Some(BufWrapper::from_virtq_desc(
                    &desc,
                    desc_chain.memory(),
                    VSOCK_PKT_HDR_SIZE,
                    buf_size,
                )?),
            })
        }
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
        self.hdr.read_u64(VSOCK_PKT_HDR_SRC_CID_OFFSET)
    }

    pub fn set_src_cid(&mut self, cid: u64) -> &mut Self {
        self.hdr.write_u64(VSOCK_PKT_HDR_SRC_CID_OFFSET, cid);
        self
    }

    pub fn dst_cid(&self) -> u64 {
        self.hdr.read_u64(VSOCK_PKT_HDR_DST_CID_OFFSET)
    }

    pub fn set_dst_cid(&mut self, cid: u64) -> &mut Self {
        self.hdr.write_u64(VSOCK_PKT_HDR_DST_CID_OFFSET, cid);
        self
    }

    pub fn src_port(&self) -> u32 {
        self.hdr.read_u32(VSOCK_PKT_HDR_SRC_PORT_OFFSET)
    }

    pub fn set_src_port(&mut self, port: u32) -> &mut Self {
        self.hdr.write_u32(VSOCK_PKT_HDR_SRC_PORT_OFFSET, port);
        self
    }

    pub fn dst_port(&self) -> u32 {
        self.hdr.read_u32(VSOCK_PKT_HDR_DST_PORT_OFFSET)
    }

    pub fn set_dst_port(&mut self, port: u32) -> &mut Self {
        self.hdr.write_u32(VSOCK_PKT_HDR_DST_PORT_OFFSET, port);
        self
    }

    pub fn len(&self) -> u32 {
        self.hdr.read_u32(VSOCK_PKT_HDR_LEN_OFFSET)
    }

    pub fn set_len(&mut self, len: u32) -> &mut Self {
        self.hdr.write_u32(VSOCK_PKT_HDR_LEN_OFFSET, len);
        self
    }

    pub fn type_(&self) -> u16 {
        self.hdr.read_u16(VSOCK_PKT_HDR_TYPE_OFFSET)
    }

    pub fn set_type(&mut self, type_: u16) -> &mut Self {
        self.hdr.write_u16(VSOCK_PKT_HDR_TYPE_OFFSET, type_);
        self
    }

    pub fn op(&self) -> u16 {
        self.hdr.read_u16(VSOCK_PKT_HDR_OP_OFFSET)
    }

    pub fn set_op(&mut self, op: u16) -> &mut Self {
        self.hdr.write_u16(VSOCK_PKT_HDR_OP_OFFSET, op);
        self
    }

    pub fn flags(&self) -> u32 {
        self.hdr.read_u32(VSOCK_PKT_HDR_FLAGS_OFFSET)
    }

    pub fn set_flags(&mut self, flags: u32) -> &mut Self {
        self.hdr.write_u32(VSOCK_PKT_HDR_FLAGS_OFFSET, flags);
        self
    }

    pub fn set_flag(&mut self, flag: u32) -> &mut Self {
        self.set_flags(self.flags() | flag);
        self
    }

    pub fn buf_alloc(&self) -> u32 {
        self.hdr.read_u32(VSOCK_PKT_HDR_BUF_ALLOC_OFFSET)
    }

    pub fn set_buf_alloc(&mut self, buf_alloc: u32) -> &mut Self {
        self.hdr
            .write_u32(VSOCK_PKT_HDR_BUF_ALLOC_OFFSET, buf_alloc);
        self
    }

    pub fn fwd_cnt(&self) -> u32 {
        self.hdr.read_u32(VSOCK_PKT_HDR_FWD_CNT_OFFSET)
    }

    pub fn set_fwd_cnt(&mut self, fwd_cnt: u32) -> &mut Self {
        self.hdr.write_u32(VSOCK_PKT_HDR_FWD_CNT_OFFSET, fwd_cnt);
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
        // This will be treated as single descriptor now.
        {
            create_context!(test_ctx, handler_ctx);
            set_pkt_len(1024, &handler_ctx.guest_txvq.dtable(0), &test_ctx.mem);
            handler_ctx.guest_txvq.dtable(0).flags().store(0);
            expect_asm_error!(tx, test_ctx, handler_ctx, VsockError::BufDescTooSmall);
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
        // This will be treated as single descriptor now.
        {
            create_context!(test_ctx, handler_ctx);
            handler_ctx
                .guest_rxvq
                .dtable(0)
                .flags()
                .store(VIRTQ_DESC_F_WRITE);
            let pkt = VsockPacket::from_rx_virtq_head(
                &mut handler_ctx.queues[RXQ_EVENT as usize]
                    .queue_mut()
                    .pop_descriptor_chain(&test_ctx.mem)
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(pkt.buf(), Some(vec![]).as_deref());
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

    #[repr(align(8))]
    struct AlignedHeaderBytes<const N: usize>([u8; N]);

    fn assert_header_round_trip(ptr: *mut u8) {
        let mut packet = VsockPacket {
            hdr: HdrWrapper::from_ptr_unchecked(ptr),
            buf: None,
        };

        packet
            .set_src_cid(1)
            .set_dst_cid(2)
            .set_src_port(3)
            .set_dst_port(4)
            .set_len(5)
            .set_type(6)
            .set_op(7)
            .set_flags(8)
            .set_buf_alloc(9)
            .set_fwd_cnt(10);

        assert_eq!(packet.src_cid(), 1);
        assert_eq!(packet.dst_cid(), 2);
        assert_eq!(packet.src_port(), 3);
        assert_eq!(packet.dst_port(), 4);
        assert_eq!(packet.len(), 5);
        assert_eq!(packet.type_(), 6);
        assert_eq!(packet.op(), 7);
        assert_eq!(packet.flags(), 8);
        assert_eq!(packet.buf_alloc(), 9);
        assert_eq!(packet.fwd_cnt(), 10);
        assert_eq!(packet.hdr().len(), VSOCK_PKT_HDR_SIZE);
    }

    #[test]
    fn test_hdr_wrapper_exact_wire_allocation() {
        let layout =
            std::alloc::Layout::from_size_align(VSOCK_PKT_HDR_SIZE, std::mem::align_of::<u64>())
                .unwrap();
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        assert!(!ptr.is_null());

        assert_header_round_trip(ptr);
        unsafe { std::alloc::dealloc(ptr, layout) };
    }

    #[test]
    fn test_hdr_wrapper_unaligned_wire_address() {
        let mut bytes = AlignedHeaderBytes([0u8; VSOCK_PKT_HDR_SIZE + 1]);
        let ptr = unsafe { bytes.0.as_mut_ptr().add(1) };

        assert_header_round_trip(ptr);
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
