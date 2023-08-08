// Copyright 2019-2020 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io::{self, Seek, SeekFrom, Write};
use std::ops::Deref;
use std::result;

use log::error;
use virtio_bindings::bindings::virtio_blk::*;
use virtio_queue::{Descriptor, DescriptorChain};
use vm_memory::{ByteValued, Bytes, GuestAddress, GuestMemory, GuestMemoryError};

use crate::{
    block::{ufile::Ufile, SECTOR_SHIFT, SECTOR_SIZE},
    Error, Result,
};

/// Error executing request.
#[derive(Debug)]
pub(crate) enum ExecuteError {
    BadRequest(Error),
    Flush(io::Error),
    Read(GuestMemoryError),
    Seek(io::Error),
    Write(GuestMemoryError),
    GetDeviceID(GuestMemoryError),
    Unsupported(u32),
}

/// Type of request from driver to device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestType {
    /// Read request.
    In,
    /// Write request.
    Out,
    /// Flush request.
    Flush,
    /// Get device ID request.
    GetDeviceID,
    /// Unsupported request.
    Unsupported(u32),
}

impl From<u32> for RequestType {
    fn from(value: u32) -> Self {
        match value {
            VIRTIO_BLK_T_IN => RequestType::In,
            VIRTIO_BLK_T_OUT => RequestType::Out,
            VIRTIO_BLK_T_FLUSH => RequestType::Flush,
            VIRTIO_BLK_T_GET_ID => RequestType::GetDeviceID,
            t => RequestType::Unsupported(t),
        }
    }
}

/// The request header represents the mandatory fields of each block device request.
///
/// A request header contains the following fields:
///   * request_type: an u32 value mapping to a read, write or flush operation.
///   * reserved: 32 bits are reserved for future extensions of the Virtio Spec.
///   * sector: an u64 value representing the offset where a read/write is to occur.
///
/// The header simplifies reading the request from memory as all request follow
/// the same memory layout.
#[derive(Copy, Clone, Default)]
#[repr(C)]
struct RequestHeader {
    request_type: u32,
    _reserved: u32,
    sector: u64,
}

// Safe because RequestHeader only contains plain data.
unsafe impl ByteValued for RequestHeader {}

impl RequestHeader {
    /// Reads the request header from GuestMemory starting at `addr`.
    ///
    /// Virtio 1.0 specifies that the data is transmitted by the driver in little-endian
    /// format. Firecracker currently runs only on little endian platforms so we don't
    /// need to do an explicit little endian read as all reads are little endian by default.
    /// When running on a big endian platform, this code should not compile, and support
    /// for explicit little endian reads is required.
    #[cfg(target_endian = "little")]
    fn read_from<M: GuestMemory + ?Sized>(memory: &M, addr: GuestAddress) -> Result<Self> {
        memory.read_obj(addr).map_err(Error::GuestMemory)
    }
}

/// IO Data descriptor.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct IoDataDesc {
    pub data_addr: u64,
    pub data_len: usize,
}

/// The block request.
#[derive(Clone, Debug)]
pub struct Request {
    /// The type of the request.
    pub(crate) request_type: RequestType,
    /// The offset of the request.
    pub(crate) sector: u64,
    pub(crate) status_addr: GuestAddress,
    pub(crate) request_index: u16,
}

impl Request {
    /// Parses a `desc_chain` and returns the associated `Request`.
    pub(crate) fn parse<M>(
        desc_chain: &mut DescriptorChain<M>,
        data_descs: &mut Vec<IoDataDesc>,
        max_size: u32,
    ) -> Result<Self>
    where
        M: Deref,
        M::Target: GuestMemory,
    {
        let desc = desc_chain.next().ok_or(Error::DescriptorChainTooShort)?;
        // The head contains the request type which MUST be readable.
        if desc.is_write_only() {
            return Err(Error::UnexpectedWriteOnlyDescriptor);
        }

        let request_header = RequestHeader::read_from(desc_chain.memory(), desc.addr())?;
        let mut req = Request {
            request_type: RequestType::from(request_header.request_type),
            sector: request_header.sector,
            status_addr: GuestAddress(0),
            request_index: desc_chain.head_index(),
        };
        let status_desc;
        let mut desc = desc_chain
            .next()
            .ok_or(Error::DescriptorChainTooShort)
            .map_err(|e| {
                error!("virtio-blk: Request {:?} has only head descriptor", req);
                e
            })?;
        if !desc.has_next() {
            status_desc = desc;
            // Only flush requests are allowed to skip the data descriptor.
            if req.request_type != RequestType::Flush {
                error!("virtio-blk: Request {:?} need a data descriptor", req);
                return Err(Error::DescriptorChainTooShort);
            }
        } else {
            while desc.has_next() {
                req.check_request(desc, max_size)?;
                data_descs.push(IoDataDesc {
                    data_addr: desc.addr().0,
                    data_len: desc.len() as usize,
                });
                desc = desc_chain
                    .next()
                    .ok_or(Error::DescriptorChainTooShort)
                    .map_err(|e| {
                        error!("virtio-blk: descriptor chain corrupted");
                        e
                    })?;
            }
            status_desc = desc;
        }

        // The status MUST always be writable and the guest address must be accessible.
        if !status_desc.is_write_only() {
            return Err(Error::UnexpectedReadOnlyDescriptor);
        }
        if status_desc.len() < 1 {
            return Err(Error::DescriptorLengthTooSmall);
        }
        if !desc_chain.memory().address_in_range(status_desc.addr()) {
            return Err(Error::InvalidGuestAddress(status_desc.addr()));
        }
        req.status_addr = status_desc.addr();

        Ok(req)
    }

    pub(crate) fn check_request(&self, desc: Descriptor, max_size: u32) -> Result<()> {
        match self.request_type {
            RequestType::Out => {
                if desc.is_write_only() {
                    error!(
                        "virtio-blk: Request {:?} sees unexpected write-only descriptor",
                        self
                    );
                    return Err(Error::UnexpectedWriteOnlyDescriptor);
                } else if desc.len() > max_size {
                    error!(
                        "virtio-blk: Request {:?} size is greater than disk size ({} > {})",
                        self,
                        desc.len(),
                        max_size
                    );
                    return Err(Error::DescriptorLengthTooBig);
                }
            }
            RequestType::In => {
                if !desc.is_write_only() {
                    error!(
                        "virtio-blk: Request {:?} sees unexpected read-only descriptor for read",
                        self
                    );
                    return Err(Error::UnexpectedReadOnlyDescriptor);
                } else if desc.len() > max_size {
                    error!(
                        "virtio-blk: Request {:?} size is greater than disk size ({} > {})",
                        self,
                        desc.len(),
                        max_size
                    );
                    return Err(Error::DescriptorLengthTooBig);
                }
            }
            RequestType::GetDeviceID if !desc.is_write_only() => {
                error!(
                    "virtio-blk: Request {:?} sees unexpected read-only descriptor for GetDeviceID",
                    self
                );
                return Err(Error::UnexpectedReadOnlyDescriptor);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn execute<M: GuestMemory + ?Sized>(
        &self,
        disk: &mut Box<dyn Ufile>,
        mem: &M,
        data_descs: &[IoDataDesc],
        disk_id: &[u8],
    ) -> result::Result<u32, ExecuteError> {
        self.check_capacity(disk, data_descs)?;
        disk.seek(SeekFrom::Start(self.sector << SECTOR_SHIFT))
            .map_err(ExecuteError::Seek)?;
        let mut len = 0;
        for io in data_descs {
            match self.request_type {
                RequestType::In => {
                    mem.read_from(GuestAddress(io.data_addr), disk, io.data_len)
                        .map_err(ExecuteError::Read)?;
                    len += io.data_len;
                }
                RequestType::Out => {
                    mem.write_to(GuestAddress(io.data_addr), disk, io.data_len)
                        .map_err(ExecuteError::Write)?;
                }
                RequestType::Flush => match disk.flush() {
                    Ok(_) => {}
                    Err(e) => return Err(ExecuteError::Flush(e)),
                },
                RequestType::GetDeviceID => {
                    if io.data_len < disk_id.len() {
                        return Err(ExecuteError::BadRequest(Error::InvalidOffset));
                    }
                    mem.write_slice(disk_id, GuestAddress(io.data_addr))
                        .map_err(ExecuteError::GetDeviceID)?;
                    // TODO: dragonball returns 0 here, check which value to return?
                    return Ok(disk_id.len() as u32);
                }
                RequestType::Unsupported(t) => return Err(ExecuteError::Unsupported(t)),
            };
        }

        Ok(len as u32)
    }

    pub(crate) fn check_capacity(
        &self,
        disk: &mut Box<dyn Ufile>,
        data_descs: &[IoDataDesc],
    ) -> result::Result<(), ExecuteError> {
        for d in data_descs {
            let mut top = (d.data_len as u64 + SECTOR_SIZE - 1) & !(SECTOR_SIZE - 1u64);

            top = top
                .checked_add(self.sector << SECTOR_SHIFT)
                .ok_or(ExecuteError::BadRequest(Error::InvalidOffset))?;
            if top > disk.get_capacity() {
                return Err(ExecuteError::BadRequest(Error::InvalidOffset));
            }
        }

        Ok(())
    }

    pub(crate) fn update_status<M: GuestMemory + ?Sized>(&self, mem: &M, status: u32) {
        // Safe to unwrap because we have validated request.status_addr in parse()
        mem.write_obj(status as u8, self.status_addr).unwrap();
    }

    // Return total IO length of all segments. Assume the req has been checked and is valid.
    pub(crate) fn data_len(&self, data_descs: &[IoDataDesc]) -> u32 {
        let mut len = 0;
        for d in data_descs {
            len += d.data_len;
        }
        len as u32
    }
}
