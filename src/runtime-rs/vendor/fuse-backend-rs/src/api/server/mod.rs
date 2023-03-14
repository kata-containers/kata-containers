// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.

//! Fuse API Server to interconnect transport layers with filesystem drivers.
//!
//! The Fuse API server is a adapter layer between transport layers and file system drivers.
//! The main functionalities of the Fuse API server is:
//! * Support different types of transport layers, fusedev, virtio-fs or vhost-user-fs.
//! * Hide different transport layers details from file system drivers.
//! * Parse transport messages according to the Fuse ABI to avoid duplicated message decoding
//!   in every file system driver.
//! * Invoke file system driver handler to serve each request and send the reply.
//!
//! The Fuse API server is performance critical, so it's designed to support multi-threading by
//! adopting interior-mutability. And the arcswap crate is used to implement interior-mutability.

use std::ffi::CStr;
use std::io::{self, Read};
use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::abi::fuse_abi::*;
use crate::api::filesystem::{Context, FileSystem, ZeroCopyReader, ZeroCopyWriter};
use crate::file_traits::FileReadWriteVolatile;
use crate::transport::{Reader, Writer};
use crate::{bytes_to_cstr, BitmapSlice, Error, Result};

#[cfg(feature = "async-io")]
mod async_io;
mod sync_io;

/// Maximum buffer size of FUSE requests.
#[cfg(target_os = "linux")]
pub const MAX_BUFFER_SIZE: u32 = 1 << 20;
/// Maximum buffer size of FUSE requests.
#[cfg(target_os = "macos")]
pub const MAX_BUFFER_SIZE: u32 = 1 << 25;
const MIN_READ_BUFFER: u32 = 8192;
const BUFFER_HEADER_SIZE: u32 = 0x1000;
const DIRENT_PADDING: [u8; 8] = [0; 8];

/// Maximum number of pages required for FUSE requests.
pub const MAX_REQ_PAGES: u16 = 256; // 1MB

/// Fuse Server to handle requests from the Fuse client and vhost user master.
pub struct Server<F: FileSystem + Sync> {
    fs: F,
    vers: ArcSwap<ServerVersion>,
}

impl<F: FileSystem + Sync> Server<F> {
    /// Create a Server instance from a filesystem driver object.
    pub fn new(fs: F) -> Server<F> {
        Server {
            fs,
            vers: ArcSwap::new(Arc::new(ServerVersion {
                major: KERNEL_VERSION,
                minor: KERNEL_MINOR_VERSION,
            })),
        }
    }
}

struct ZcReader<'a, S: BitmapSlice = ()>(Reader<'a, S>);

impl<'a, S: BitmapSlice> ZeroCopyReader for ZcReader<'a, S> {
    fn read_to(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.0.read_to_at(f, count, off)
    }
}

impl<'a, S: BitmapSlice> io::Read for ZcReader<'a, S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

struct ZcWriter<'a, S: BitmapSlice = ()>(Writer<'a, S>);

impl<'a, S: BitmapSlice> ZeroCopyWriter for ZcWriter<'a, S> {
    fn write_from(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.0.write_from_at(f, count, off)
    }
}

impl<'a, S: BitmapSlice> io::Write for ZcWriter<'a, S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[allow(dead_code)]
struct ServerVersion {
    major: u32,
    minor: u32,
}

struct ServerUtil();

impl ServerUtil {
    fn get_message_body<S: BitmapSlice>(
        r: &mut Reader<'_, S>,
        in_header: &InHeader,
        sub_hdr_sz: usize,
    ) -> Result<Vec<u8>> {
        let len = (in_header.len as usize)
            .checked_sub(size_of::<InHeader>())
            .and_then(|l| l.checked_sub(sub_hdr_sz))
            .ok_or(Error::InvalidHeaderLength)?;

        // Allocate buffer without zeroing out the content for performance.
        let mut buf = Vec::<u8>::with_capacity(len);
        // It's safe because read_exact() is called to fill all the allocated buffer.
        #[allow(clippy::uninit_vec)]
        unsafe {
            buf.set_len(len)
        };
        r.read_exact(&mut buf).map_err(Error::DecodeMessage)?;

        Ok(buf)
    }

    fn extract_two_cstrs(buf: &[u8]) -> Result<(&CStr, &CStr)> {
        if let Some(mut pos) = buf.iter().position(|x| *x == 0) {
            let first = CStr::from_bytes_with_nul(&buf[0..=pos]).map_err(Error::InvalidCString)?;
            pos += 1;
            if pos < buf.len() {
                return Ok((first, bytes_to_cstr(&buf[pos..])?));
            }
        }

        Err(Error::DecodeMessage(std::io::Error::from_raw_os_error(
            libc::EINVAL,
        )))
    }
}

/// Provide concrete backend filesystem a way to catch information/metrics from fuse.
pub trait MetricsHook {
    /// `collect()` will be invoked before the real request is processed
    fn collect(&self, ih: &InHeader);
    /// `release()` will be invoked after the real request is processed
    fn release(&self, oh: Option<&OutHeader>);
}

struct SrvContext<'a, F, S: BitmapSlice = ()> {
    in_header: InHeader,
    context: Context,
    r: Reader<'a, S>,
    w: Writer<'a, S>,
    phantom: PhantomData<F>,
    phantom2: PhantomData<S>,
}

impl<'a, F: FileSystem, S: BitmapSlice> SrvContext<'a, F, S> {
    fn new(in_header: InHeader, r: Reader<'a, S>, w: Writer<'a, S>) -> Self {
        let context = Context::from(&in_header);

        SrvContext {
            in_header,
            context,
            r,
            w,
            phantom: PhantomData,
            phantom2: PhantomData,
        }
    }

    fn context(&self) -> &Context {
        &self.context
    }

    fn unique(&self) -> u64 {
        self.in_header.unique
    }

    fn nodeid(&self) -> F::Inode {
        self.in_header.nodeid.into()
    }

    fn take_reader(&mut self) -> Reader<'a, S> {
        let mut reader = Reader::default();

        std::mem::swap(&mut self.r, &mut reader);

        reader
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cstrs() {
        assert_eq!(
            ServerUtil::extract_two_cstrs(&[0x1u8, 0x2u8, 0x0, 0x3, 0x0]).unwrap(),
            (
                CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap(),
                CStr::from_bytes_with_nul(&[0x3u8, 0x0]).unwrap(),
            )
        );
        assert_eq!(
            ServerUtil::extract_two_cstrs(&[0x1u8, 0x2u8, 0x0, 0x3, 0x0, 0x0]).unwrap(),
            (
                CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap(),
                CStr::from_bytes_with_nul(&[0x3u8, 0x0]).unwrap(),
            )
        );
        assert_eq!(
            ServerUtil::extract_two_cstrs(&[0x1u8, 0x2u8, 0x0, 0x3, 0x0, 0x4]).unwrap(),
            (
                CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap(),
                CStr::from_bytes_with_nul(&[0x3u8, 0x0]).unwrap(),
            )
        );
        assert_eq!(
            ServerUtil::extract_two_cstrs(&[0x1u8, 0x2u8, 0x0, 0x0, 0x4]).unwrap(),
            (
                CStr::from_bytes_with_nul(&[0x1u8, 0x2u8, 0x0]).unwrap(),
                CStr::from_bytes_with_nul(&[0x0]).unwrap(),
            )
        );

        ServerUtil::extract_two_cstrs(&[0x1u8, 0x2u8, 0x0, 0x3]).unwrap_err();
        ServerUtil::extract_two_cstrs(&[0x1u8, 0x2u8, 0x0]).unwrap_err();
        ServerUtil::extract_two_cstrs(&[0x1u8, 0x2u8]).unwrap_err();
    }
}
