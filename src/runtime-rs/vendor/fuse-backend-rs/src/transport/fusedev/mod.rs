// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Traits and Structs to implement the fusedev transport driver.
//!
//! With fusedev transport driver, requests received from `/dev/fuse` will be stored in an internal
//! buffer and the whole reply message must be written all at once.

use std::collections::VecDeque;

use std::io::{self, IoSlice, Write};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::os::unix::io::RawFd;

use nix::sys::uio::writev;
use nix::unistd::write;
use vm_memory::{ByteValued, VolatileMemory, VolatileSlice};

use super::{Error, FileReadWriteVolatile, IoBuffers, Reader, Result, Writer};
use crate::file_buf::FileVolatileSlice;
use crate::BitmapSlice;

#[cfg(target_os = "linux")]
mod linux_session;
#[cfg(target_os = "linux")]
pub use linux_session::*;

#[cfg(target_os = "macos")]
mod macos_session;
#[cfg(target_os = "macos")]
pub use macos_session::*;

/// A buffer reference wrapper for fuse requests.
#[derive(Debug)]
pub struct FuseBuf<'a> {
    mem: &'a mut [u8],
}

impl<'a> FuseBuf<'a> {
    /// Construct a new fuse request buffer wrapper.
    pub fn new(mem: &'a mut [u8]) -> FuseBuf<'a> {
        FuseBuf { mem }
    }
}

impl<'a, S: BitmapSlice + Default> Reader<'a, S> {
    /// Construct a new Reader wrapper over `desc_chain`.
    ///
    /// 'request`: Fuse request from clients read from /dev/fuse
    pub fn from_fuse_buffer(buf: FuseBuf<'a>) -> Result<Reader<'a, S>> {
        let mut buffers: VecDeque<VolatileSlice<'a, S>> = VecDeque::new();
        // Safe because Reader has the same lifetime with buf.
        buffers.push_back(unsafe {
            VolatileSlice::with_bitmap(buf.mem.as_mut_ptr(), buf.mem.len(), S::default())
        });

        Ok(Reader {
            buffers: IoBuffers {
                buffers,
                bytes_consumed: 0,
            },
        })
    }
}

/// Writer to send FUSE reply to the FUSE driver.
///
/// There are a few special properties to follow:
/// 1. A fuse device request MUST be written to the fuse device in one shot.
/// 2. If the writer is split, a final commit() MUST be called to issue the
///    device write operation.
/// 3. Concurrency, caller should not write to the writer concurrently.
#[derive(Debug, PartialEq, Eq)]
pub struct FuseDevWriter<'a, S: BitmapSlice = ()> {
    fd: RawFd,
    buffered: bool,
    buf: ManuallyDrop<Vec<u8>>,
    bitmapslice: S,
    phantom: PhantomData<&'a mut [S]>,
}

impl<'a, S: BitmapSlice + Default> FuseDevWriter<'a, S> {
    /// Construct a new [Writer].
    pub fn new(fd: RawFd, data_buf: &'a mut [u8]) -> Result<FuseDevWriter<'a, S>> {
        let buf = unsafe { Vec::from_raw_parts(data_buf.as_mut_ptr(), 0, data_buf.len()) };
        Ok(FuseDevWriter {
            fd,
            buffered: false,
            buf: ManuallyDrop::new(buf),
            bitmapslice: S::default(),
            phantom: PhantomData,
        })
    }
}

impl<'a, S: BitmapSlice> FuseDevWriter<'a, S> {
    /// Split the [Writer] at the given offset.
    ///
    /// After the split, `self` will be able to write up to `offset` bytes while the returned
    /// `Writer` can write up to `available_bytes() - offset` bytes.  Returns an error if
    /// `offset > self.available_bytes()`.
    pub fn split_at(&mut self, offset: usize) -> Result<FuseDevWriter<'a, S>> {
        if self.buf.capacity() < offset {
            return Err(Error::SplitOutOfBounds(offset));
        }

        let (len1, len2) = if self.buf.len() > offset {
            (offset, self.buf.len() - offset)
        } else {
            (self.buf.len(), 0)
        };
        let cap2 = self.buf.capacity() - offset;
        let ptr = self.buf.as_mut_ptr();

        // Safe because both buffers refer to different parts of the same underlying `data_buf`.
        self.buf = unsafe { ManuallyDrop::new(Vec::from_raw_parts(ptr, len1, offset)) };
        self.buffered = true;
        let buf = unsafe { ManuallyDrop::new(Vec::from_raw_parts(ptr.add(offset), len2, cap2)) };

        Ok(FuseDevWriter {
            fd: self.fd,
            buffered: true,
            buf,
            bitmapslice: self.bitmapslice.clone(),
            phantom: PhantomData,
        })
    }

    /// Compose the FUSE reply message and send the message to `/dev/fuse`.
    pub fn commit(&mut self, other: Option<&Writer<'a, S>>) -> io::Result<usize> {
        if !self.buffered {
            return Ok(0);
        }

        let o = match other {
            Some(Writer::FuseDev(w)) => w.buf.as_slice(),
            _ => &[],
        };
        let res = match (self.buf.len(), o.len()) {
            (0, 0) => Ok(0),
            (0, _) => write(self.fd, o),
            (_, 0) => write(self.fd, self.buf.as_slice()),
            (_, _) => {
                let bufs = [IoSlice::new(self.buf.as_slice()), IoSlice::new(o)];
                writev(self.fd, &bufs)
            }
        };

        res.map_err(|e| {
            error! {"fail to write to fuse device on commit: {}", e};
            io::Error::from_raw_os_error(e as i32)
        })
    }

    /// Return number of bytes already written to the internal buffer.
    pub fn bytes_written(&self) -> usize {
        self.buf.len()
    }

    /// Return number of bytes available for writing.
    pub fn available_bytes(&self) -> usize {
        self.buf.capacity() - self.buf.len()
    }

    fn account_written(&mut self, count: usize) {
        let new_len = self.buf.len() + count;
        // Safe because check_avail_space() ensures that `count` is valid.
        unsafe { self.buf.set_len(new_len) };
    }

    /// Write an object to the writer.
    pub fn write_obj<T: ByteValued>(&mut self, val: T) -> io::Result<()> {
        self.write_all(val.as_slice())
    }

    /// Write data to the writer from a file descriptor.
    ///
    /// Return the number of bytes written to the writer.
    pub fn write_from<F: FileReadWriteVolatile>(
        &mut self,
        mut src: F,
        count: usize,
    ) -> io::Result<usize> {
        self.check_available_space(count)?;

        let cnt = src.read_vectored_volatile(
            // Safe because we have made sure buf has at least count capacity above
            unsafe {
                &[FileVolatileSlice::from_raw_ptr(
                    self.buf.as_mut_ptr().add(self.buf.len()),
                    count,
                )]
            },
        )?;
        self.account_written(cnt);

        if self.buffered {
            Ok(cnt)
        } else {
            Self::do_write(self.fd, &self.buf[..cnt])
        }
    }

    /// Write data to the writer from a File at offset `off`.
    /// Return the number of bytes written to the writer.
    pub fn write_from_at<F: FileReadWriteVolatile>(
        &mut self,
        mut src: F,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.check_available_space(count)?;

        let cnt = src.read_vectored_at_volatile(
            // Safe because we have made sure buf has at least count capacity above
            unsafe {
                &[FileVolatileSlice::from_raw_ptr(
                    self.buf.as_mut_ptr().add(self.buf.len()),
                    count,
                )]
            },
            off,
        )?;
        self.account_written(cnt);

        if self.buffered {
            Ok(cnt)
        } else {
            Self::do_write(self.fd, &self.buf[..cnt])
        }
    }

    /// Write all data to the writer from a file descriptor.
    pub fn write_all_from<F: FileReadWriteVolatile>(
        &mut self,
        mut src: F,
        mut count: usize,
    ) -> io::Result<()> {
        self.check_available_space(count)?;

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

    fn check_available_space(&self, sz: usize) -> io::Result<()> {
        assert!(self.buffered || self.buf.len() == 0);
        if sz > self.available_bytes() {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "data out of range, available {} requested {}",
                    self.available_bytes(),
                    sz
                ),
            ))
        } else {
            Ok(())
        }
    }

    fn do_write(fd: RawFd, data: &[u8]) -> io::Result<usize> {
        write(fd, data).map_err(|e| {
            error! {"fail to write to fuse device fd {}: {}, {:?}", fd, e, data};
            io::Error::new(io::ErrorKind::Other, format!("{}", e))
        })
    }
}

impl<'a, S: BitmapSlice> io::Write for FuseDevWriter<'a, S> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.check_available_space(data.len())?;

        if self.buffered {
            self.buf.extend_from_slice(data);
            Ok(data.len())
        } else {
            Self::do_write(self.fd, data).map(|x| {
                self.account_written(x);
                x
            })
        }
    }

    // default write_vectored only writes the first non-empty IoSlice. Override it.
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.check_available_space(bufs.iter().fold(0, |acc, x| acc + x.len()))?;

        if self.buffered {
            let count = bufs.iter().filter(|b| !b.is_empty()).fold(0, |acc, b| {
                self.buf.extend_from_slice(b);
                acc + b.len()
            });
            Ok(count)
        } else {
            if bufs.is_empty() {
                return Ok(0);
            }
            writev(self.fd, bufs)
                .map(|x| {
                    self.account_written(x);
                    x
                })
                .map_err(|e| {
                    error! {"fail to write to fuse device on commit: {}", e};
                    io::Error::new(io::ErrorKind::Other, format!("{}", e))
                })
        }
    }

    /// As this writer can associate multiple writers by splitting, `flush()` can't
    /// flush them all. Disable it!
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Writer does not support flush buffer.",
        ))
    }
}

#[cfg(feature = "async-io")]
mod async_io {
    use super::*;
    use crate::file_buf::FileVolatileBuf;
    use crate::file_traits::AsyncFileReadWriteVolatile;

    impl<'a, S: BitmapSlice> FuseDevWriter<'a, S> {
        /// Write data from a buffer into this writer in asynchronous mode.
        ///
        /// Return the number of bytes written to the writer.
        pub async fn async_write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.check_available_space(data.len())?;

            if self.buffered {
                // write to internal buf
                self.buf.extend_from_slice(data);
                Ok(data.len())
            } else {
                nix::sys::uio::pwrite(self.fd, data, 0)
                    .map(|x| {
                        self.account_written(x);
                        x
                    })
                    .map_err(|e| {
                        error! {"fail to write to fuse device fd {}: {}", self.fd, e};
                        io::Error::new(io::ErrorKind::Other, format!("{}", e))
                    })
            }
        }

        /// Write data from two buffers into this writer in asynchronous mode.
        ///
        /// Return the number of bytes written to the writer.
        pub async fn async_write2(&mut self, data: &[u8], data2: &[u8]) -> io::Result<usize> {
            let len = data.len() + data2.len();
            self.check_available_space(len)?;

            if self.buffered {
                // write to internal buf
                self.buf.extend_from_slice(data);
                self.buf.extend_from_slice(data2);
                Ok(len)
            } else {
                let bufs = [std::io::IoSlice::new(data), std::io::IoSlice::new(data2)];
                writev(self.fd, &bufs)
                    .map(|x| {
                        self.account_written(x);
                        x
                    })
                    .map_err(|e| {
                        error! {"fail to write to fuse device fd {}: {}", self.fd, e};
                        io::Error::new(io::ErrorKind::Other, format!("{}", e))
                    })
            }
        }

        /// Write data from two buffers into this writer in asynchronous mode.
        ///
        /// Return the number of bytes written to the writer.
        pub async fn async_write3(
            &mut self,
            data: &[u8],
            data2: &[u8],
            data3: &[u8],
        ) -> io::Result<usize> {
            let len = data.len() + data2.len() + data3.len();
            self.check_available_space(len)?;

            if self.buffered {
                // write to internal buf
                self.buf.extend_from_slice(data);
                self.buf.extend_from_slice(data2);
                self.buf.extend_from_slice(data3);
                Ok(len)
            } else {
                let bufs = [
                    std::io::IoSlice::new(data),
                    std::io::IoSlice::new(data2),
                    std::io::IoSlice::new(data3),
                ];
                writev(self.fd, &bufs)
                    .map(|x| {
                        self.account_written(x);
                        x
                    })
                    .map_err(|e| {
                        error! {"fail to write to fuse device fd {}: {}", self.fd, e};
                        io::Error::new(io::ErrorKind::Other, format!("{}", e))
                    })
            }
        }

        /// Attempts to write an entire buffer into this writer in asynchronous mode.
        pub async fn async_write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
            while !buf.is_empty() {
                match self.async_write(buf).await {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to write whole buffer",
                        ));
                    }
                    Ok(n) => buf = &buf[n..],
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }

            Ok(())
        }

        /// Write data from a File at offset `off` to the writer in asynchronous mode.
        ///
        /// Return the number of bytes written to the writer.
        pub async fn async_write_from_at<F: AsyncFileReadWriteVolatile>(
            &mut self,
            src: &F,
            count: usize,
            off: u64,
        ) -> io::Result<usize> {
            self.check_available_space(count)?;

            let buf = unsafe { FileVolatileBuf::from_raw_ptr(self.buf.as_mut_ptr(), 0, count) };
            let (res, _) = src.async_read_at_volatile(buf, off).await;
            match res {
                Ok(cnt) => {
                    self.account_written(cnt);
                    if self.buffered {
                        Ok(cnt)
                    } else {
                        // write to fd, can only happen once per instance
                        nix::sys::uio::pwrite(self.fd, &self.buf[..cnt], 0).map_err(|e| {
                            error! {"fail to write to fuse device fd {}: {}", self.fd, e};
                            io::Error::new(io::ErrorKind::Other, format!("{}", e))
                        })
                    }
                }
                Err(e) => Err(e),
            }
        }

        /// Commit all internal buffers of the writer and others.
        ///
        /// We need this because the lifetime of others is usually shorter than self.
        pub async fn async_commit(&mut self, other: Option<&Writer<'a, S>>) -> io::Result<usize> {
            let o = match other {
                Some(Writer::FuseDev(w)) => w.buf.as_slice(),
                _ => &[],
            };

            let res = match (self.buf.len(), o.len()) {
                (0, 0) => Ok(0),
                (0, _) => nix::sys::uio::pwrite(self.fd, o, 0).map_err(|e| {
                    error! {"fail to write to fuse device fd {}: {}", self.fd, e};
                    io::Error::new(io::ErrorKind::Other, format!("{}", e))
                }),
                (_, 0) => nix::sys::uio::pwrite(self.fd, self.buf.as_slice(), 0).map_err(|e| {
                    error! {"fail to write to fuse device fd {}: {}", self.fd, e};
                    io::Error::new(io::ErrorKind::Other, format!("{}", e))
                }),
                (_, _) => {
                    let bufs = [
                        std::io::IoSlice::new(self.buf.as_slice()),
                        std::io::IoSlice::new(o),
                    ];
                    writev(self.fd, &bufs).map_err(|e| {
                        error! {"fail to write to fuse device fd {}: {}", self.fd, e};
                        io::Error::new(io::ErrorKind::Other, format!("{}", e))
                    })
                }
            };

            res.map_err(|e| {
                error! {"fail to write to fuse device on commit: {}", e};
                e
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::unix::io::AsRawFd;
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn reader_test_simple_chain() {
        let mut buf = [0u8; 106];
        let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf)).unwrap();

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
        let file = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 106];
        let mut writer = FuseDevWriter::<()>::new(file.as_raw_fd(), &mut buf).unwrap();

        writer.buffered = true;
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
    fn writer_test_split_chain() {
        let file = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 108];
        let mut writer = FuseDevWriter::<()>::new(file.as_raw_fd(), &mut buf).unwrap();
        let writer2 = writer.split_at(106).unwrap();

        assert_eq!(writer.available_bytes(), 106);
        assert_eq!(writer.bytes_written(), 0);
        assert_eq!(writer2.available_bytes(), 2);
        assert_eq!(writer2.bytes_written(), 0);

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
    fn reader_unexpected_eof() {
        let mut buf = [0u8; 106];
        let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf)).unwrap();

        let mut buf2 = Vec::with_capacity(1024);
        buf2.resize(1024, 0);

        assert_eq!(
            reader
                .read_exact(&mut buf2[..])
                .expect_err("read more bytes than available")
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn reader_split_border() {
        let mut buf = [0u8; 128];
        let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf)).unwrap();
        let other = reader.split_at(32).expect("failed to split Reader");

        assert_eq!(reader.available_bytes(), 32);
        assert_eq!(other.available_bytes(), 96);
    }

    #[test]
    fn reader_split_outofbounds() {
        let mut buf = [0u8; 128];
        let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf)).unwrap();

        if let Ok(_) = reader.split_at(256) {
            panic!("successfully split Reader with out of bounds offset");
        }
    }

    #[test]
    fn writer_simple_commit_header() {
        let file = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 106];
        let mut writer = FuseDevWriter::<()>::new(file.as_raw_fd(), &mut buf).unwrap();

        writer.buffered = true;
        assert_eq!(writer.available_bytes(), 106);

        writer.write(&[0x1u8; 4]).unwrap();
        assert_eq!(writer.available_bytes(), 102);
        assert_eq!(writer.bytes_written(), 4);

        let buf = vec![0xdeu8; 64];
        let slices = [
            IoSlice::new(&buf[..32]),
            IoSlice::new(&buf[32..48]),
            IoSlice::new(&buf[48..]),
        ];
        assert_eq!(
            writer
                .write_vectored(&slices)
                .expect("failed to write from buffer"),
            64
        );
        assert!(writer.flush().is_err());

        writer.commit(None).unwrap();
    }

    #[test]
    fn writer_split_commit_header() {
        let file = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 106];
        let mut writer = FuseDevWriter::<()>::new(file.as_raw_fd(), &mut buf).unwrap();
        let mut other = writer.split_at(4).expect("failed to split Writer");

        assert_eq!(writer.available_bytes(), 4);
        assert_eq!(other.available_bytes(), 102);

        writer.write(&[0x1u8; 4]).unwrap();
        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 4);

        let buf = vec![0xdeu8; 64];
        let slices = [
            IoSlice::new(&buf[..32]),
            IoSlice::new(&buf[32..48]),
            IoSlice::new(&buf[48..]),
        ];
        assert_eq!(
            other
                .write_vectored(&slices)
                .expect("failed to write from buffer"),
            64
        );
        assert!(writer.flush().is_err());

        writer.commit(None).unwrap();
    }

    #[test]
    fn writer_split_commit_all() {
        let file = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 106];
        let mut writer = FuseDevWriter::<()>::new(file.as_raw_fd(), &mut buf).unwrap();
        let mut other = writer.split_at(4).expect("failed to split Writer");

        assert_eq!(writer.available_bytes(), 4);
        assert_eq!(other.available_bytes(), 102);

        writer.write(&[0x1u8; 4]).unwrap();
        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 4);

        let buf = vec![0xdeu8; 64];
        let slices = [
            IoSlice::new(&buf[..32]),
            IoSlice::new(&buf[32..48]),
            IoSlice::new(&buf[48..]),
        ];
        assert_eq!(
            other
                .write_vectored(&slices)
                .expect("failed to write from buffer"),
            64
        );

        writer.commit(Some(&other.into())).unwrap();
    }

    #[test]
    fn read_full() {
        let mut buf2 = [0u8; 48];
        let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf2)).unwrap();
        let mut buf = vec![0u8; 64];

        assert_eq!(
            reader.read(&mut buf[..]).expect("failed to read to buffer"),
            48
        );
    }

    #[test]
    fn write_full() {
        let file = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 48];
        let mut writer = FuseDevWriter::<()>::new(file.as_raw_fd(), &mut buf).unwrap();

        let buf = vec![0xdeu8; 64];
        writer.write(&buf[..]).unwrap_err();

        let buf = vec![0xdeu8; 48];
        assert_eq!(
            writer.write(&buf[..]).expect("failed to write from buffer"),
            48
        );
    }

    #[test]
    fn write_vectored() {
        let file = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 48];
        let mut writer = FuseDevWriter::<()>::new(file.as_raw_fd(), &mut buf).unwrap();

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
    }

    #[test]
    fn read_obj() {
        let mut buf2 = [0u8; 9];
        let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf2)).unwrap();

        let _val: u64 = reader.read_obj().expect("failed to read to file");

        assert_eq!(reader.available_bytes(), 1);
        assert_eq!(reader.bytes_read(), 8);
        assert!(reader.read_obj::<u64>().is_err());
    }

    #[test]
    fn read_exact_to() {
        let mut buf2 = [0u8; 48];
        let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf2)).unwrap();
        let mut file = TempFile::new().unwrap().into_file();

        reader
            .read_exact_to(&mut file, 47)
            .expect("failed to read to file");

        assert_eq!(reader.available_bytes(), 1);
        assert_eq!(reader.bytes_read(), 47);
    }

    #[test]
    fn read_to_at() {
        let mut buf2 = [0u8; 48];
        let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf2)).unwrap();
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
    fn write_obj() {
        let file1 = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 48];
        let mut writer = FuseDevWriter::<()>::new(file1.as_raw_fd(), &mut buf).unwrap();
        let _writer2 = writer.split_at(40).unwrap();
        let val = 0x1u64;

        writer.write_obj(val).expect("failed to write from buffer");
        assert_eq!(writer.available_bytes(), 32);
    }

    #[test]
    fn write_all_from() {
        let file1 = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 48];
        let mut writer = FuseDevWriter::<()>::new(file1.as_raw_fd(), &mut buf).unwrap();
        let mut file = TempFile::new().unwrap().into_file();
        let buf = vec![0xdeu8; 64];

        writer.buffered = true;

        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        writer
            .write_all_from(&mut file, 47)
            .expect("failed to write from buffer");
        assert_eq!(writer.available_bytes(), 1);
        assert_eq!(writer.bytes_written(), 47);

        // Write more data than capacity
        writer.write_all_from(&mut file, 2).unwrap_err();
        assert_eq!(writer.available_bytes(), 1);
        assert_eq!(writer.bytes_written(), 47);
    }

    #[test]
    fn write_all_from_split() {
        let file1 = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 58];
        let mut writer = FuseDevWriter::<()>::new(file1.as_raw_fd(), &mut buf).unwrap();
        let _other = writer.split_at(48).unwrap();
        let mut file = TempFile::new().unwrap().into_file();
        let buf = vec![0xdeu8; 64];

        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        writer
            .write_all_from(&mut file, 47)
            .expect("failed to write from buffer");
        assert_eq!(writer.available_bytes(), 1);
        assert_eq!(writer.bytes_written(), 47);

        // Write more data than capacity
        writer.write_all_from(&mut file, 2).unwrap_err();
        assert_eq!(writer.available_bytes(), 1);
        assert_eq!(writer.bytes_written(), 47);
    }

    #[test]
    fn write_from_at() {
        let file1 = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 48];
        let mut writer = FuseDevWriter::<()>::new(file1.as_raw_fd(), &mut buf).unwrap();
        let mut file = TempFile::new().unwrap().into_file();
        let buf = vec![0xdeu8; 64];

        writer.buffered = true;

        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(
            writer
                .write_from_at(&mut file, 40, 16)
                .expect("failed to write from buffer"),
            40
        );
        assert_eq!(writer.available_bytes(), 8);
        assert_eq!(writer.bytes_written(), 40);

        // Write more data than capacity
        writer.write_from_at(&mut file, 40, 16).unwrap_err();
        assert_eq!(writer.available_bytes(), 8);
        assert_eq!(writer.bytes_written(), 40);
    }

    #[test]
    fn write_from_at_split() {
        let file1 = TempFile::new().unwrap().into_file();
        let mut buf = vec![0x0u8; 58];
        let mut writer = FuseDevWriter::<()>::new(file1.as_raw_fd(), &mut buf).unwrap();
        let _other = writer.split_at(48).unwrap();
        let mut file = TempFile::new().unwrap().into_file();
        let buf = vec![0xdeu8; 64];

        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(
            writer
                .write_from_at(&mut file, 40, 16)
                .expect("failed to write from buffer"),
            40
        );
        assert_eq!(writer.available_bytes(), 8);
        assert_eq!(writer.bytes_written(), 40);

        // Write more data than capacity
        writer.write_from_at(&mut file, 40, 16).unwrap_err();
        assert_eq!(writer.available_bytes(), 8);
        assert_eq!(writer.bytes_written(), 40);
    }

    #[cfg(feature = "async-io")]
    mod async_io {
        use vmm_sys_util::tempdir::TempDir;

        use crate::async_file::File;
        use crate::async_runtime;

        use super::*;

        #[test]
        fn async_read_to_at() {
            let dir = TempDir::new().unwrap();
            let path = dir.as_path().to_path_buf().join("test.txt");
            std::fs::write(&path, b"this is a test").unwrap();

            let mut buf2 = [0u8; 48];
            let mut reader = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut buf2)).unwrap();

            async_runtime::block_on(async {
                let file = File::async_open(&path, true, false).await.unwrap();
                let res = reader.async_read_to_at(&file, 48, 0).await.unwrap();
                assert_eq!(res, 48);
            })
        }

        #[test]
        fn async_write() {
            let dir = TempDir::new().unwrap();
            let path = dir.as_path().to_path_buf().join("test.txt");
            std::fs::write(&path, b"this is a test").unwrap();

            let file = TempFile::new().unwrap().into_file();
            let fd = file.as_raw_fd();
            let mut buf = vec![0x0u8; 48];
            let mut writer = FuseDevWriter::<()>::new(fd, &mut buf).unwrap();
            let buf = vec![0xdeu8; 64];
            let res = async_runtime::block_on(async { writer.async_write(&buf[..]).await });
            assert!(res.is_err());

            let fd = file.as_raw_fd();
            let mut buf = vec![0x0u8; 48];
            let mut writer2 = FuseDevWriter::<()>::new(fd, &mut buf).unwrap();
            let buf = vec![0xdeu8; 48];
            let res = async_runtime::block_on(async { writer2.async_write(&buf[..]).await });
            assert_eq!(res.unwrap(), 48);
        }

        #[test]
        fn async_write2() {
            let file = TempFile::new().unwrap().into_file();
            let fd = file.as_raw_fd();
            let mut buf = vec![0x0u8; 48];
            let mut writer = FuseDevWriter::<()>::new(fd, &mut buf).unwrap();
            let buf = vec![0xdeu8; 48];
            let res = async_runtime::block_on(async {
                writer.async_write2(&buf[..32], &buf[32..]).await
            });
            assert_eq!(res.unwrap(), 48);
        }

        #[test]
        fn async_write3() {
            let file = TempFile::new().unwrap().into_file();
            let fd = file.as_raw_fd();
            let mut buf = vec![0x0u8; 48];
            let mut writer = FuseDevWriter::<()>::new(fd, &mut buf).unwrap();
            let buf = vec![0xdeu8; 48];
            let res = async_runtime::block_on(async {
                writer
                    .async_write3(&buf[..32], &buf[32..40], &buf[40..])
                    .await
            });
            assert_eq!(res.unwrap(), 48);
        }

        #[test]
        fn async_write_from_at() {
            let file1 = TempFile::new().unwrap().into_file();
            let fd1 = file1.as_raw_fd();

            let buf = vec![0xdeu8; 64];
            let dir = TempDir::new().unwrap();
            let path = dir.as_path().to_path_buf().join("test.txt");
            std::fs::write(&path, &buf).unwrap();

            let mut buf = vec![0x0u8; 48];
            let mut writer = FuseDevWriter::<()>::new(fd1, &mut buf).unwrap();
            let res = async_runtime::block_on(async {
                let file = File::async_open(&path, true, false).await.unwrap();
                writer.async_write_from_at(&file, 40, 16).await
            });

            assert_eq!(res.unwrap(), 40);
        }

        #[test]
        fn async_writer_split_commit_all() {
            let file = TempFile::new().unwrap().into_file();
            let fd = file.as_raw_fd();
            let mut buf = vec![0x0u8; 106];
            let buf = unsafe { std::mem::transmute::<&mut [u8], &'static mut [u8]>(&mut buf) };
            let mut writer = FuseDevWriter::<()>::new(fd, buf).unwrap();
            let mut other = writer.split_at(4).expect("failed to split Writer");

            assert_eq!(writer.available_bytes(), 4);
            assert_eq!(other.available_bytes(), 102);

            writer.write(&[0x1u8; 4]).unwrap();
            assert_eq!(writer.available_bytes(), 0);
            assert_eq!(writer.bytes_written(), 4);

            let buf = vec![0xdeu8; 64];
            let slices = [
                IoSlice::new(&buf[..32]),
                IoSlice::new(&buf[32..48]),
                IoSlice::new(&buf[48..]),
            ];
            assert_eq!(
                other
                    .write_vectored(&slices)
                    .expect("failed to write from buffer"),
                64
            );

            let res =
                async_runtime::block_on(async { writer.async_commit(Some(&other.into())).await });
            let _ = res.unwrap();
        }
    }
}
