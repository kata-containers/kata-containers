// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

//! Traits and Structs to implement the virtiofs transport driver.
//!
//! Virtio-fs is a shared file system that lets virtual machines access a directory tree
//! on the host. Unlike existing approaches, it is designed to offer local file system
//! semantics and performance. Virtualization allows multiple virtual machines (VMs) to
//! run on a single physical host. Although VMs are isolated and run separate operating
//! system instances, their proximity on the physical host allows for fast shared memory
//! access.
//!
//! Virtio-fs uses FUSE as the foundation. FUSE has no dependencies on a networking stack
//! and exposes a rich native Linux file system interface that allows virtio-fs to act
//! like a local file system. Both the semantics and the performance of communication of
//! co-located VMs are different from the networking model for which remote file systems
//! were designed.
//!
//! Unlike traditional FUSE where the file system daemon runs in userspace, the virtio-fs
//! daemon runs on the host. A VIRTIO device carries FUSE messages and provides extensions
//! for advanced features not available in traditional FUSE.
//! The main extension to the FUSE protocol is the virtio-fs DAX Window, which supports
//! memory mapping the contents of files. The virtio-fs VIRTIO device implements this
//! as a shared memory region exposed through a PCI/MMIO BAR. This feature is
//! virtualization-specific and is not available outside of virtio-fs.
//!
//! Although virtio-fs uses FUSE as the protocol, it does not function as a new transport
//! for existing FUSE applications. It is not possible to run existing FUSE file systems
//! unmodified because virtio-fs has a different security model and extends the FUSE protocol.
//! Existing FUSE file systems trust the client because it is the kernel. There would be no
//! reason for the kernel to attack the file system since the kernel already has full control
//! of the host. In virtio-fs the client is the untrusted VM and the file system daemon must
//! not trust it. Therefore, virtio-fs server uses a hardened FUSE implementation that does
//! not trust the client.

use std::cmp;
use std::collections::VecDeque;
use std::io::{self, IoSlice, Write};
use std::ops::Deref;
use std::ptr::copy_nonoverlapping;

use virtio_queue::DescriptorChain;
use vm_memory::bitmap::{BitmapSlice, MS};
use vm_memory::{
    Address, ByteValued, GuestMemory, GuestMemoryRegion, MemoryRegionAddress, VolatileSlice,
};

use super::{Error, FileReadWriteVolatile, FileVolatileSlice, IoBuffers, Reader, Result, Writer};

impl<S: BitmapSlice> IoBuffers<'_, S> {
    /// Consumes for write.
    fn consume_for_write<F>(&mut self, count: usize, f: F) -> io::Result<usize>
    where
        F: FnOnce(&[FileVolatileSlice]) -> io::Result<usize>,
    {
        self.consume(true, count, f)
    }
}

impl<'a> Reader<'a> {
    /// Construct a new Reader wrapper over `desc_chain`.
    pub fn from_descriptor_chain<M>(
        mem: &'a M::Target,
        desc_chain: DescriptorChain<M>,
    ) -> Result<Reader<'a, MS<'a, M::Target>>>
    where
        M: Deref,
        M::Target: GuestMemory + Sized,
    {
        let mut total_len: usize = 0;
        let buffers = desc_chain
            .readable()
            .map(|desc| {
                // Verify that summing the descriptor sizes does not overflow.
                // This can happen if a driver tricks a device into reading more data than
                // fits in a `usize`.
                total_len = total_len
                    .checked_add(desc.len() as usize)
                    .ok_or(Error::DescriptorChainOverflow)?;

                let region = mem
                    .find_region(desc.addr())
                    .ok_or(Error::FindMemoryRegion)?;
                let offset = desc
                    .addr()
                    .checked_sub(region.start_addr().raw_value())
                    .unwrap();
                region
                    .get_slice(MemoryRegionAddress(offset.raw_value()), desc.len() as usize)
                    .map_err(Error::GuestMemoryError)
            })
            .collect::<Result<VecDeque<VolatileSlice<'a, MS<M::Target>>>>>()?;

        Ok(Reader {
            buffers: IoBuffers {
                buffers,
                bytes_consumed: 0,
            },
        })
    }
}

/// Provide high-level interface over the sequence of memory regions
/// defined by writable descriptors in the Virtio descriptor chain.
///
/// Note that virtio spec requires driver to place any device-writable
/// descriptors after any device-readable descriptors (2.6.4.2 in Virtio Spec v1.1).
/// Writer will start iterating the descriptors from the first writable one and will
/// assume that all following descriptors are writable.
#[derive(Clone)]
pub struct VirtioFsWriter<'a, S = ()> {
    buffers: IoBuffers<'a, S>,
}

impl<'a> VirtioFsWriter<'a> {
    /// Construct a new [Writer] wrapper over `desc_chain`.
    pub fn new<M>(
        mem: &'a M::Target,
        desc_chain: DescriptorChain<M>,
    ) -> Result<VirtioFsWriter<'a, MS<'a, M::Target>>>
    where
        M: Deref,
        M::Target: GuestMemory + Sized,
    {
        let mut total_len: usize = 0;
        let buffers = desc_chain
            .writable()
            .map(|desc| {
                // Verify that summing the descriptor sizes does not overflow.
                // This can happen if a driver tricks a device into writing more data than
                // fits in a `usize`.
                total_len = total_len
                    .checked_add(desc.len() as usize)
                    .ok_or(Error::DescriptorChainOverflow)?;

                let region = mem
                    .find_region(desc.addr())
                    .ok_or(Error::FindMemoryRegion)?;
                let offset = desc
                    .addr()
                    .checked_sub(region.start_addr().raw_value())
                    .unwrap();
                region
                    .get_slice(MemoryRegionAddress(offset.raw_value()), desc.len() as usize)
                    .map_err(Error::GuestMemoryError)
            })
            .collect::<Result<VecDeque<VolatileSlice<'a, MS<M::Target>>>>>()?;

        Ok(VirtioFsWriter {
            buffers: IoBuffers {
                buffers,
                bytes_consumed: 0,
            },
        })
    }
}

impl<'a, S: BitmapSlice> VirtioFsWriter<'a, S> {
    /// Write an object to the descriptor chain buffer.
    pub fn write_obj<T: ByteValued>(&mut self, val: T) -> io::Result<()> {
        self.write_all(val.as_slice())
    }

    /// Write data to the descriptor chain buffer from a file descriptor.
    ///
    /// Return the number of bytes written to the descriptor chain buffer.
    pub fn write_from<F: FileReadWriteVolatile>(
        &mut self,
        mut src: F,
        count: usize,
    ) -> io::Result<usize> {
        self.check_available_space(count, 0, 0)?;
        self.buffers
            .consume_for_write(count, |bufs| src.read_vectored_volatile(bufs))
    }

    /// Write data to the descriptor chain buffer from a File at offset `off`.
    ///
    /// Return the number of bytes written to the descriptor chain buffer.
    pub fn write_from_at<F: FileReadWriteVolatile>(
        &mut self,
        mut src: F,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.check_available_space(count, 0, 0)?;
        self.buffers
            .consume_for_write(count, |bufs| src.read_vectored_at_volatile(bufs, off))
    }

    /// Write all data to the descriptor chain buffer from a file descriptor.
    pub fn write_all_from<F: FileReadWriteVolatile>(
        &mut self,
        mut src: F,
        mut count: usize,
    ) -> io::Result<()> {
        self.check_available_space(count, 0, 0)?;
        while count > 0 {
            match self.write_from(&mut src, count) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ))
                }
                Ok(n) => count -= n,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    /// Return number of bytes available for writing.
    ///
    /// May return an error if the combined lengths of all the buffers in the DescriptorChain would
    /// cause an overflow.
    pub fn available_bytes(&self) -> usize {
        self.buffers.available_bytes()
    }

    /// Return number of bytes already written to the descriptor chain buffer.
    pub fn bytes_written(&self) -> usize {
        self.buffers.bytes_consumed()
    }

    /// Split this `Writer` into two at the given offset in the `DescriptorChain` buffer.
    /// After the split, `self` will be able to write up to `offset` bytes while the returned
    /// `Writer` can write up to `available_bytes() - offset` bytes.  Returns an error if
    /// `offset > self.available_bytes()`.
    pub fn split_at(&mut self, offset: usize) -> Result<Self> {
        self.buffers
            .split_at(offset)
            .map(|buffers| VirtioFsWriter { buffers })
    }

    /// Commit all internal buffers of self and others
    ///
    /// This is provided just to be compatible with fusedev
    pub fn commit(&mut self, _other: Option<&Writer<'a, S>>) -> io::Result<usize> {
        Ok(0)
    }

    fn check_available_space(&self, len1: usize, len2: usize, len3: usize) -> io::Result<()> {
        let len = len1
            .checked_add(len2)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "buffer size is too big"))?;
        let len = len
            .checked_add(len3)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "buffer size is too big"))?;
        if len > self.available_bytes() {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "data out of range, available {} requested {}",
                    self.available_bytes(),
                    len
                ),
            ))
        } else {
            Ok(())
        }
    }
}

impl<'a, S: BitmapSlice> io::Write for VirtioFsWriter<'a, S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.check_available_space(buf.len(), 0, 0)?;

        self.buffers.consume_for_write(buf.len(), |bufs| {
            let mut rem = buf;
            let mut total = 0;
            for buf in bufs {
                let copy_len = cmp::min(rem.len(), buf.len());

                // Safe because we have already verified that `buf` points to valid memory.
                unsafe {
                    copy_nonoverlapping(rem.as_ptr(), buf.as_ptr(), copy_len);
                }
                rem = &rem[copy_len..];
                total += copy_len;
            }
            Ok(total)
        })
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.check_available_space(bufs.iter().fold(0, |acc, x| acc + x.len()), 0, 0)?;

        let mut count = 0;
        for buf in bufs.iter().filter(|b| !b.is_empty()) {
            count += self.write(buf)?;
        }
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Nothing to flush since the writes go straight into the buffer.
        Ok(())
    }
}

// For Virtio-fs, the output is written to memory buffer, so no need for async io at all.
// Just relay the operation to corresponding sync io handler.
#[cfg(feature = "async-io")]
mod async_io {
    use super::*;
    use crate::transport::AsyncFileReadWriteVolatile;

    impl<'a, S: BitmapSlice> VirtioFsWriter<'a, S> {
        /// Write data from a buffer into this writer in asynchronous mode.
        pub async fn async_write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.write(data)
        }

        /// Write data from two buffers into this writer in asynchronous mode.
        pub async fn async_write2(&mut self, data: &[u8], data2: &[u8]) -> io::Result<usize> {
            self.check_available_space(data.len(), data2.len(), 0)?;
            let mut cnt = self.write(data)?;
            cnt += self.write(data2)?;

            Ok(cnt)
        }

        /// Write data from three buffers into this writer in asynchronous mode.
        pub async fn async_write3(
            &mut self,
            data: &[u8],
            data2: &[u8],
            data3: &[u8],
        ) -> io::Result<usize> {
            self.check_available_space(data.len(), data2.len(), data3.len())?;
            let mut cnt = self.write(data)?;
            cnt += self.write(data2)?;
            cnt += self.write(data3)?;

            Ok(cnt)
        }

        /// Attempts to write an entire buffer into this writer in asynchronous mode.
        pub async fn async_write_all(&mut self, buf: &[u8]) -> io::Result<()> {
            self.write_all(buf)
        }

        /// Writes data to the descriptor chain buffer from a File at offset `off`.
        /// Returns the number of bytes written to the descriptor chain buffer.
        pub async fn async_write_from_at<F: AsyncFileReadWriteVolatile>(
            &mut self,
            src: &F,
            count: usize,
            off: u64,
        ) -> io::Result<usize> {
            self.check_available_space(count, 0, 0)?;
            // Safe because `bufs` doesn't out-live `self`.
            let bufs = unsafe { self.buffers.prepare_mut_io_buf(count) };
            if bufs.is_empty() {
                Ok(0)
            } else {
                let (res, _) = src.async_read_vectored_at_volatile(bufs, off).await;
                match res {
                    Ok(cnt) => {
                        self.buffers.mark_dirty(cnt);
                        self.buffers.mark_used(cnt)?;
                        Ok(cnt)
                    }
                    Err(e) => Err(e),
                }
            }
        }

        /// Commit all internal buffers of self and others
        /// We need this because the lifetime of others is usually shorter than self.
        pub async fn async_commit(&mut self, other: Option<&Writer<'a, S>>) -> io::Result<usize> {
            self.commit(other)
        }
    }
}

/// Disabled since vm-virtio doesn't export any DescriptorChain constructors.
/// Should re-enable once it does.
#[cfg(testff)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use vm_memory::{Address, ByteValued, Bytes, GuestAddress, GuestMemoryMmap, Le16, Le32, Le64};
    use vmm_sys_util::tempfile::TempFile;

    const VIRTQ_DESC_F_NEXT: u16 = 0x1;
    const VIRTQ_DESC_F_WRITE: u16 = 0x2;

    #[derive(Copy, Clone, PartialEq, Eq)]
    pub enum DescriptorType {
        Readable,
        Writable,
    }

    #[derive(Copy, Clone, Debug, Default)]
    #[repr(C)]
    struct virtq_desc {
        addr: Le64,
        len: Le32,
        flags: Le16,
        next: Le16,
    }

    // Safe because it only has data and has no implicit padding.
    unsafe impl ByteValued for virtq_desc {}

    /// Test utility function to create a descriptor chain in guest memory.
    pub fn create_descriptor_chain(
        memory: &GuestMemoryMmap,
        descriptor_array_addr: GuestAddress,
        mut buffers_start_addr: GuestAddress,
        descriptors: Vec<(DescriptorType, u32)>,
        spaces_between_regions: u32,
    ) -> Result<DescriptorChain<GuestMemoryMmap>> {
        let descriptors_len = descriptors.len();
        for (index, (type_, size)) in descriptors.into_iter().enumerate() {
            let mut flags = 0;
            if let DescriptorType::Writable = type_ {
                flags |= VIRTQ_DESC_F_WRITE;
            }
            if index + 1 < descriptors_len {
                flags |= VIRTQ_DESC_F_NEXT;
            }

            let index = index as u16;
            let desc = virtq_desc {
                addr: buffers_start_addr.raw_value().into(),
                len: size.into(),
                flags: flags.into(),
                next: (index + 1).into(),
            };

            let offset = size + spaces_between_regions;
            buffers_start_addr = buffers_start_addr
                .checked_add(u64::from(offset))
                .ok_or(Error::InvalidChain)?;

            let _ = memory.write_obj(
                desc,
                descriptor_array_addr
                    .checked_add(u64::from(index) * std::mem::size_of::<virtq_desc>() as u64)
                    .ok_or(Error::InvalidChain)?,
            );
        }

        DescriptorChain::<&GuestMemoryMmap>::new(memory, descriptor_array_addr, 0x100, 0)
            .ok_or(Error::InvalidChain)
    }

    #[test]
    fn reader_test_simple_chain() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (Readable, 8),
                (Readable, 16),
                (Readable, 18),
                (Readable, 64),
            ],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");
        assert_eq!(reader.available_bytes(), 106);
        assert_eq!(reader.bytes_read(), 0);

        let mut buffer = [0 as u8; 64];
        if let Err(_) = reader.read_exact(&mut buffer) {
            panic!("read_exact should not fail here");
        }

        assert_eq!(reader.available_bytes(), 42);
        assert_eq!(reader.bytes_read(), 64);

        match reader.read(&mut buffer) {
            Err(_) => panic!("read should not fail here"),
            Ok(length) => assert_eq!(length, 42),
        }

        assert_eq!(reader.available_bytes(), 0);
        assert_eq!(reader.bytes_read(), 106);
    }

    #[test]
    fn writer_test_simple_chain() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (Writable, 8),
                (Writable, 16),
                (Writable, 18),
                (Writable, 64),
            ],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut writer = VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");
        assert_eq!(writer.available_bytes(), 106);
        assert_eq!(writer.bytes_written(), 0);

        let mut buffer = [0 as u8; 64];
        if let Err(_) = writer.write_all(&mut buffer) {
            panic!("write_all should not fail here");
        }

        assert_eq!(writer.available_bytes(), 42);
        assert_eq!(writer.bytes_written(), 64);

        let mut buffer = [0 as u8; 42];
        match writer.write(&mut buffer) {
            Err(_) => panic!("write should not fail here"),
            Ok(length) => assert_eq!(length, 42),
        }

        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 106);
    }

    #[test]
    fn reader_test_incompatible_chain() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Writable, 8)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");
        assert_eq!(reader.available_bytes(), 0);
        assert_eq!(reader.bytes_read(), 0);

        assert!(reader.read_obj::<u8>().is_err());

        assert_eq!(reader.available_bytes(), 0);
        assert_eq!(reader.bytes_read(), 0);
    }

    #[test]
    fn writer_test_incompatible_chain() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Readable, 8)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut writer = VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");
        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 0);

        assert!(writer.write_obj(0u8).is_err());

        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 0);
    }

    #[test]
    fn reader_writer_shared_chain() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (Readable, 16),
                (Readable, 16),
                (Readable, 96),
                (Writable, 64),
                (Writable, 1),
                (Writable, 3),
            ],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain.clone()).expect("failed to create Reader");
        let mut writer = VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");

        assert_eq!(reader.bytes_read(), 0);
        assert_eq!(writer.bytes_written(), 0);

        let mut buffer = Vec::with_capacity(200);

        assert_eq!(
            reader
                .read_to_end(&mut buffer)
                .expect("read should not fail here"),
            128
        );

        // The writable descriptors are only 68 bytes long.
        writer
            .write_all(&buffer[..68])
            .expect("write should not fail here");

        assert_eq!(reader.available_bytes(), 0);
        assert_eq!(reader.bytes_read(), 128);
        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 68);
    }

    #[test]
    fn reader_writer_shattered_object() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let secret: Le32 = 0x12345678.into();

        // Create a descriptor chain with memory regions that are properly separated.
        let chain_writer = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Writable, 1), (Writable, 1), (Writable, 1), (Writable, 1)],
            123,
        )
        .expect("create_descriptor_chain failed");
        let mut writer =
            VirtioFsWriter::new(&memory, chain_writer).expect("failed to create Writer");
        assert!(writer.flush().is_ok());
        if let Err(_) = writer.write_obj(secret) {
            panic!("write_obj should not fail here");
        }
        assert!(writer.flush().is_ok());

        // Now create new descriptor chain pointing to the same memory and try to read it.
        let chain_reader = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Readable, 1), (Readable, 1), (Readable, 1), (Readable, 1)],
            123,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain_reader).expect("failed to create Reader");
        match reader.read_obj::<Le32>() {
            Err(_) => panic!("read_obj should not fail here"),
            Ok(read_secret) => assert_eq!(read_secret, secret),
        }
    }

    #[test]
    fn reader_unexpected_eof() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Readable, 256), (Readable, 256)],
            0,
        )
        .expect("create_descriptor_chain failed");

        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");

        let mut buf = Vec::with_capacity(1024);
        buf.resize(1024, 0);

        assert_eq!(
            reader
                .read_exact(&mut buf[..])
                .expect_err("read more bytes than available")
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn split_border() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (Readable, 16),
                (Readable, 16),
                (Readable, 96),
                (Writable, 64),
                (Writable, 1),
                (Writable, 3),
            ],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");

        let other = reader.split_at(32).expect("failed to split Reader");
        assert_eq!(reader.available_bytes(), 32);
        assert_eq!(other.available_bytes(), 96);
    }

    #[test]
    fn split_middle() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (Readable, 16),
                (Readable, 16),
                (Readable, 96),
                (Writable, 64),
                (Writable, 1),
                (Writable, 3),
            ],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");

        let other = reader.split_at(24).expect("failed to split Reader");
        assert_eq!(reader.available_bytes(), 24);
        assert_eq!(other.available_bytes(), 104);
    }

    #[test]
    fn split_end() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (Readable, 16),
                (Readable, 16),
                (Readable, 96),
                (Writable, 64),
                (Writable, 1),
                (Writable, 3),
            ],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");

        let other = reader.split_at(128).expect("failed to split Reader");
        assert_eq!(reader.available_bytes(), 128);
        assert_eq!(other.available_bytes(), 0);
    }

    #[test]
    fn split_beginning() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (Readable, 16),
                (Readable, 16),
                (Readable, 96),
                (Writable, 64),
                (Writable, 1),
                (Writable, 3),
            ],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");

        let other = reader.split_at(0).expect("failed to split Reader");
        assert_eq!(reader.available_bytes(), 0);
        assert_eq!(other.available_bytes(), 128);
    }

    #[test]
    fn split_outofbounds() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (Readable, 16),
                (Readable, 16),
                (Readable, 96),
                (Writable, 64),
                (Writable, 1),
                (Writable, 3),
            ],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");

        if let Ok(_) = reader.split_at(256) {
            panic!("successfully split Reader with out of bounds offset");
        }
    }

    #[test]
    fn read_full() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Readable, 16), (Readable, 16), (Readable, 16)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Reader");

        let mut buf = vec![0u8; 64];
        assert_eq!(
            reader.read(&mut buf[..]).expect("failed to read to buffer"),
            48
        );
    }

    #[test]
    fn write_full() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Writable, 16), (Writable, 16), (Writable, 16)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut writer = VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");

        let buf = vec![0xdeu8; 40];
        assert_eq!(
            writer.write(&buf[..]).expect("failed to write from buffer"),
            40
        );
        assert_eq!(writer.available_bytes(), 8);
        assert_eq!(writer.bytes_written(), 40);

        // Write more data than capacity
        writer.write(&buf[..]).unwrap_err();
        assert_eq!(writer.available_bytes(), 8);
        assert_eq!(writer.bytes_written(), 40);
    }

    #[test]
    fn write_vectored() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Writable, 16), (Writable, 16), (Writable, 16)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut writer = VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");

        let buf = vec![0xdeu8; 48];
        let slices = [
            IoSlice::new(&buf[..32]),
            IoSlice::new(&buf[32..40]),
            IoSlice::new(&buf[40..]),
        ];
        assert_eq!(
            writer
                .write_vectored(&slices)
                .expect("failed to write from buffer"),
            48
        );
        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 48);

        // Write more data than capacity
        let buf = vec![0xdeu8; 40];
        writer.write(&buf[..]).unwrap_err();
        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 48);
    }

    #[test]
    fn read_exact_to() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Readable, 16), (Readable, 16), (Readable, 16)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Writer");

        let mut file = TempFile::new().unwrap().into_file();
        reader
            .read_exact_to(&mut file, 47)
            .expect("failed to read to file");

        assert_eq!(reader.available_bytes(), 1);
        assert_eq!(reader.bytes_read(), 47);
    }

    #[test]
    fn read_to_at() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Readable, 16), (Readable, 16), (Readable, 16)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut reader = Reader::new(&memory, chain).expect("failed to create Writer");

        let mut file = TempFile::new().unwrap().into_file();
        assert_eq!(
            reader
                .read_to_at(&mut file, 48, 16)
                .expect("failed to read to file"),
            48
        );

        assert_eq!(reader.available_bytes(), 0);
        assert_eq!(reader.bytes_read(), 48);
    }

    #[test]
    fn write_all_from() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Writable, 16), (Writable, 16), (Writable, 16)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut writer = VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");

        let mut file = TempFile::new().unwrap().into_file();
        let buf = vec![0xdeu8; 64];
        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        writer
            .write_all_from(&mut file, 47)
            .expect("failed to write from buffer");

        assert_eq!(writer.available_bytes(), 1);
        assert_eq!(writer.bytes_written(), 47);
    }

    #[test]
    fn write_from_at() {
        use DescriptorType::*;

        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![(Writable, 16), (Writable, 16), (Writable, 16)],
            0,
        )
        .expect("create_descriptor_chain failed");
        let mut writer = VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");

        let mut file = TempFile::new().unwrap().into_file();
        let buf = vec![0xdeu8; 64];
        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(
            writer
                .write_from_at(&mut file, 48, 16)
                .expect("failed to write from buffer"),
            48
        );

        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 48);
    }

    #[cfg(feature = "async-io")]
    mod async_io {
        use futures::executor::{block_on, ThreadPool};
        use futures::task::SpawnExt;
        use ringbahn::drive::demo::DemoDriver;
        use std::os::unix::io::AsRawFd;

        use super::*;

        #[test]
        fn async_read_to_at() {
            let file = TempFile::new().unwrap().into_file();
            let fd = file.as_raw_fd();
            let executor = ThreadPool::new().unwrap();

            let handle = executor
                .spawn_with_handle(async move {
                    use DescriptorType::*;

                    let memory_start_addr = GuestAddress(0x0);
                    let memory =
                        GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

                    let chain = create_descriptor_chain(
                        &memory,
                        GuestAddress(0x0),
                        GuestAddress(0x100),
                        vec![(Readable, 16), (Readable, 16), (Readable, 16)],
                        0,
                    )
                    .expect("create_descriptor_chain failed");
                    let mut reader = Reader::new(&memory, chain).expect("failed to create Writer");
                    let drive = DemoDriver::default();

                    reader.async_read_to_at(drive, fd, 48, 16).await
                })
                .unwrap();
            assert_eq!(block_on(handle).unwrap(), 48);
        }

        #[test]
        fn async_write() {
            let executor = ThreadPool::new().unwrap();
            let handle = executor
                .spawn_with_handle(async move {
                    use DescriptorType::*;

                    let memory_start_addr = GuestAddress(0x0);
                    let memory =
                        GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

                    let chain = create_descriptor_chain(
                        &memory,
                        GuestAddress(0x0),
                        GuestAddress(0x100),
                        vec![(Writable, 16), (Writable, 16), (Writable, 16)],
                        0,
                    )
                    .expect("create_descriptor_chain failed");
                    let mut writer =
                        VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");
                    let drive = DemoDriver::default();
                    let buf = vec![0xdeu8; 64];

                    writer.async_write(drive, &buf[..]).await
                })
                .unwrap();
            // expect errors
            block_on(handle).unwrap_err();

            let handle = executor
                .spawn_with_handle(async move {
                    use DescriptorType::*;

                    let memory_start_addr = GuestAddress(0x0);
                    let memory =
                        GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

                    let chain = create_descriptor_chain(
                        &memory,
                        GuestAddress(0x0),
                        GuestAddress(0x100),
                        vec![(Writable, 16), (Writable, 16), (Writable, 16)],
                        0,
                    )
                    .expect("create_descriptor_chain failed");
                    let mut writer =
                        VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");
                    let drive = DemoDriver::default();

                    let buf = vec![0xdeu8; 48];
                    writer.async_write(drive, &buf[..]).await
                })
                .unwrap();

            assert_eq!(block_on(handle).unwrap(), 48);
        }

        #[test]
        fn async_write2() {
            let executor = ThreadPool::new().unwrap();
            let handle = executor
                .spawn_with_handle(async move {
                    use DescriptorType::*;

                    let memory_start_addr = GuestAddress(0x0);
                    let memory =
                        GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

                    let chain = create_descriptor_chain(
                        &memory,
                        GuestAddress(0x0),
                        GuestAddress(0x100),
                        vec![(Writable, 16), (Writable, 16), (Writable, 16)],
                        0,
                    )
                    .expect("create_descriptor_chain failed");
                    let mut writer =
                        VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");
                    let drive = DemoDriver::default();
                    let buf = vec![0xdeu8; 48];

                    writer.async_write2(drive, &buf[..32], &buf[32..]).await
                })
                .unwrap();

            assert_eq!(block_on(handle).unwrap(), 48);
        }

        #[test]
        fn async_write3() {
            let executor = ThreadPool::new().unwrap();
            let handle = executor
                .spawn_with_handle(async move {
                    use DescriptorType::*;

                    let memory_start_addr = GuestAddress(0x0);
                    let memory =
                        GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

                    let chain = create_descriptor_chain(
                        &memory,
                        GuestAddress(0x0),
                        GuestAddress(0x100),
                        vec![(Writable, 16), (Writable, 16), (Writable, 16)],
                        0,
                    )
                    .expect("create_descriptor_chain failed");
                    let mut writer =
                        VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");
                    let drive = DemoDriver::default();
                    let buf = vec![0xdeu8; 48];

                    writer
                        .async_write3(drive, &buf[..32], &buf[32..40], &buf[40..])
                        .await
                })
                .unwrap();

            assert_eq!(block_on(handle).unwrap(), 48);
        }

        #[test]
        fn async_write_from_at() {
            let mut file = TempFile::new().unwrap().into_file();
            let fd = file.as_raw_fd();
            let buf = vec![0xdeu8; 64];

            file.write_all(&buf).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();

            let executor = ThreadPool::new().unwrap();
            let handle = executor
                .spawn_with_handle(async move {
                    use DescriptorType::*;

                    let memory_start_addr = GuestAddress(0x0);
                    let memory =
                        GuestMemoryMmap::from_ranges(&vec![(memory_start_addr, 0x10000)]).unwrap();

                    let chain = create_descriptor_chain(
                        &memory,
                        GuestAddress(0x0),
                        GuestAddress(0x100),
                        vec![(Writable, 16), (Writable, 16), (Writable, 16)],
                        0,
                    )
                    .expect("create_descriptor_chain failed");
                    let mut writer =
                        VirtioFsWriter::new(&memory, chain).expect("failed to create Writer");
                    let drive = DemoDriver::default();

                    writer.async_write_from_at(drive, fd, 40, 16).await
                })
                .unwrap();

            assert_eq!(block_on(handle).unwrap(), 40);
        }
    }
}
