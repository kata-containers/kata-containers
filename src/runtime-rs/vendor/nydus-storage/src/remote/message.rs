// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Define communication messages for the remote blob manager.

#![allow(dead_code)]

use std::fmt::Debug;

use vm_memory::ByteValued;

pub(crate) const MAX_MSG_SIZE: usize = 0x1000;
pub(crate) const MAX_ATTACHED_FD_ENTRIES: usize = 4;

pub(crate) trait Req:
    Clone + Copy + Debug + PartialEq + Eq + PartialOrd + Ord + Send + Sync + Into<u32>
{
    fn is_valid(&self) -> bool;
}

/// Type of requests sending from clients to servers.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestCode {
    /// Null operation.
    Noop = 0,
    /// Get a reference to a blob from the blob manager.
    GetBlob = 1,
    /// Ask the blob manager to fetch a range of data.
    FetchRange = 2,
    /// Upper bound of valid commands.
    MaxCommand = 3,
}

impl From<RequestCode> for u32 {
    fn from(req: RequestCode) -> u32 {
        req as u32
    }
}

impl Req for RequestCode {
    fn is_valid(&self) -> bool {
        (*self >= RequestCode::Noop) && (*self < RequestCode::MaxCommand)
    }
}

/// Vhost message Validator.
pub trait MsgValidator {
    /// Validate message syntax only.
    /// It doesn't validate message semantics such as protocol version number and dependency
    /// on feature flags etc.
    fn is_valid(&self) -> bool {
        true
    }
}

// Bit mask for common message flags.
bitflags! {
    /// Common message flags for blob manager requests and replies.
    pub struct HeaderFlag: u32 {
        /// Bits[0..2] is message version number.
        const VERSION = 0x1;
        /// Mark message as reply.
        const REPLY = 0x4;
        /// Sender anticipates a reply message from the peer.
        const NEED_REPLY = 0x8;
        /// All valid bits.
        const ALL_FLAGS = 0xc;
        /// All reserved bits.
        const RESERVED_BITS = !0xf;
    }
}

/// Common message header for blob manager.
#[repr(C, packed)]
#[derive(Copy)]
pub(crate) struct MsgHeader {
    tag: u64,
    request: u32,
    flags: u32,
    size: u32,
}

impl Debug for MsgHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MsgHeader")
            .field("tag", &{ self.tag })
            .field("request", &{ self.request })
            .field("flags", &{ self.flags })
            .field("size", &{ self.size })
            .finish()
    }
}

impl Clone for MsgHeader {
    fn clone(&self) -> MsgHeader {
        *self
    }
}

impl PartialEq for MsgHeader {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag
            && self.request == other.request
            && self.flags == other.flags
            && self.size == other.size
    }
}

impl MsgHeader {
    /// Create a new instance of `MsgHeader`.
    pub fn new(tag: u64, request: RequestCode, flags: u32, size: u32) -> Self {
        // Default to protocol version 1
        let fl = (flags & HeaderFlag::ALL_FLAGS.bits()) | 0x1;
        MsgHeader {
            tag,
            request: request.into(),
            flags: fl,
            size,
        }
    }

    /// Get message tag.
    pub fn get_tag(&self) -> u64 {
        self.tag
    }

    /// Set message tag.
    pub fn set_tag(&mut self, tag: u64) {
        self.tag = tag;
    }

    /// Get message type.
    pub fn get_code(&self) -> RequestCode {
        // It's safe because R is marked as repr(u32).
        unsafe { std::mem::transmute_copy::<u32, RequestCode>(&{ self.request }) }
    }

    /// Set message type.
    pub fn set_code(&mut self, request: RequestCode) {
        self.request = request.into();
    }

    /// Get message version number.
    pub fn get_version(&self) -> u32 {
        self.flags & 0x3
    }

    /// Set message version number.
    pub fn set_version(&mut self, ver: u32) {
        self.flags &= !0x3;
        self.flags |= ver & 0x3;
    }

    /// Check whether it's a reply message.
    pub fn is_reply(&self) -> bool {
        (self.flags & HeaderFlag::REPLY.bits()) != 0
    }

    /// Mark message as reply.
    pub fn set_reply(&mut self, is_reply: bool) {
        if is_reply {
            self.flags |= HeaderFlag::REPLY.bits();
        } else {
            self.flags &= !HeaderFlag::REPLY.bits();
        }
    }

    /// Check whether reply for this message is requested.
    pub fn is_need_reply(&self) -> bool {
        (self.flags & HeaderFlag::NEED_REPLY.bits()) != 0
    }

    /// Mark that reply for this message is needed.
    pub fn set_need_reply(&mut self, need_reply: bool) {
        if need_reply {
            self.flags |= HeaderFlag::NEED_REPLY.bits();
        } else {
            self.flags &= !HeaderFlag::NEED_REPLY.bits();
        }
    }

    /// Check whether it's the reply message for the request `req`.
    pub fn is_reply_for(&self, req: &MsgHeader) -> bool {
        self.is_reply()
            && !req.is_reply()
            && self.get_code() == req.get_code()
            && req.tag == self.tag
    }

    /// Get message size.
    pub fn get_size(&self) -> u32 {
        self.size
    }

    /// Set message size.
    pub fn set_size(&mut self, size: u32) {
        self.size = size;
    }
}

impl Default for MsgHeader {
    fn default() -> Self {
        MsgHeader {
            tag: 0,
            request: 0,
            flags: 0x1,
            size: 0,
        }
    }
}

unsafe impl ByteValued for MsgHeader {}

impl MsgValidator for MsgHeader {
    #[allow(clippy::if_same_then_else)]
    fn is_valid(&self) -> bool {
        if !self.get_code().is_valid() {
            return false;
        } else if self.tag == 0 {
            return false;
        } else if self.size as usize > MAX_MSG_SIZE {
            return false;
        } else if self.get_version() != 0x1 {
            return false;
        } else if (self.flags & HeaderFlag::RESERVED_BITS.bits()) != 0 {
            return false;
        }
        true
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct GetBlobRequest {
    pub generation: u32,
    pub id: [u8; 256],
}

impl Default for GetBlobRequest {
    fn default() -> Self {
        Self {
            generation: 0,
            id: [0u8; 256],
        }
    }
}

impl GetBlobRequest {
    /// Create a new instance.
    pub fn new(generation: u32, id: &str) -> Self {
        debug_assert!(id.len() < 256);
        let mut buf = [0x0u8; 256];

        buf.copy_from_slice(id.as_bytes());

        GetBlobRequest {
            generation,
            id: buf,
        }
    }
}

unsafe impl ByteValued for GetBlobRequest {}

impl MsgValidator for GetBlobRequest {
    fn is_valid(&self) -> bool {
        self.id.contains(&0u8)
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub(crate) struct GetBlobReply {
    pub token: u64,
    pub base: u64,
    pub result: u32,
}

impl GetBlobReply {
    pub fn new(token: u64, base: u64, result: u32) -> Self {
        Self {
            token,
            base,
            result,
        }
    }
}

unsafe impl ByteValued for GetBlobReply {}

impl MsgValidator for GetBlobReply {
    fn is_valid(&self) -> bool {
        self.token != 0 || self.result != 0
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub(crate) struct FetchRangeRequest {
    pub token: u64,
    pub start: u64,
    pub count: u64,
}

impl FetchRangeRequest {
    /// Create a new instance.
    pub fn new(token: u64, start: u64, count: u64) -> Self {
        FetchRangeRequest {
            token,
            start,
            count,
        }
    }
}

unsafe impl ByteValued for FetchRangeRequest {}

impl MsgValidator for FetchRangeRequest {}

#[repr(u32)]
pub enum FetchRangeResult {
    Success = 0,
    Failure = 1,
    GenerationMismatch = 2,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub(crate) struct FetchRangeReply {
    pub token: u64,
    pub count: u64,
    pub result: u32,
}

impl FetchRangeReply {
    /// Create a new instance.
    pub fn new(token: u64, count: u64, result: u32) -> Self {
        FetchRangeReply {
            token,
            count,
            result,
        }
    }
}

unsafe impl ByteValued for FetchRangeReply {}

impl MsgValidator for FetchRangeReply {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn check_master_request_code() {
        let code = RequestCode::Noop;
        assert!(code.is_valid());
        let code = RequestCode::MaxCommand;
        assert!(!code.is_valid());
        assert!(code > RequestCode::Noop);
        let code = RequestCode::GetBlob;
        assert!(code.is_valid());
        let code = RequestCode::FetchRange;
        assert!(code.is_valid());
        assert_eq!(code, code.clone());
        let code: RequestCode = unsafe { std::mem::transmute::<u32, RequestCode>(10000u32) };
        assert!(!code.is_valid());
    }

    #[test]
    fn msg_header_ops() {
        let mut hdr = MsgHeader::new(2, RequestCode::GetBlob, 0, 0x100);
        assert_eq!(hdr.get_code(), RequestCode::GetBlob);
        hdr.set_code(RequestCode::FetchRange);
        assert_eq!(hdr.get_code(), RequestCode::FetchRange);

        assert_eq!(hdr.get_version(), 0x1);

        assert!(!hdr.is_reply());
        hdr.set_reply(true);
        assert!(hdr.is_reply());
        hdr.set_reply(false);

        assert!(!hdr.is_need_reply());
        hdr.set_need_reply(true);
        assert!(hdr.is_need_reply());
        hdr.set_need_reply(false);

        assert_eq!(hdr.get_size(), 0x100);
        hdr.set_size(0x200);
        assert_eq!(hdr.get_size(), 0x200);

        assert!(!hdr.is_need_reply());
        assert!(!hdr.is_reply());
        assert_eq!(hdr.get_version(), 0x1);

        // Check message length
        assert!(hdr.is_valid());
        hdr.set_size(0x2000);
        assert!(!hdr.is_valid());
        hdr.set_size(0x100);
        assert_eq!(hdr.get_size(), 0x100);
        assert!(hdr.is_valid());
        hdr.set_size((MAX_MSG_SIZE - mem::size_of::<MsgHeader>()) as u32);
        assert!(hdr.is_valid());
        hdr.set_size(0x0);
        assert!(hdr.is_valid());

        // Check version
        hdr.set_version(0x0);
        assert!(!hdr.is_valid());
        hdr.set_version(0x2);
        assert!(!hdr.is_valid());
        hdr.set_version(0x1);
        assert!(hdr.is_valid());

        assert_eq!(hdr.get_tag(), 2);
        hdr.set_tag(200);
        assert_eq!(hdr.get_tag(), 200);

        // Test Debug, Clone, PartiaEq trait
        assert_eq!(hdr, hdr.clone());
        assert_eq!(hdr.clone().get_code(), hdr.get_code());
        assert_eq!(format!("{:?}", hdr.clone()), format!("{:?}", hdr));
    }
}
