// vim: tw=80
//! POSIX Asynchronous I/O
//!
//! The POSIX AIO interface is used for asynchronous I/O on files and disk-like
//! devices.  It supports [`read`](struct.AioCb.html#method.read),
//! [`write`](struct.AioCb.html#method.write), and
//! [`fsync`](struct.AioCb.html#method.fsync) operations.  Completion
//! notifications can optionally be delivered via
//! [signals](../signal/enum.SigevNotify.html#variant.SigevSignal), via the
//! [`aio_suspend`](fn.aio_suspend.html) function, or via polling.  Some
//! platforms support other completion
//! notifications, such as
//! [kevent](../signal/enum.SigevNotify.html#variant.SigevKevent).
//!
//! Multiple operations may be submitted in a batch with
//! [`lio_listio`](fn.lio_listio.html), though the standard does not guarantee
//! that they will be executed atomically.
//!
//! Outstanding operations may be cancelled with
//! [`cancel`](struct.AioCb.html#method.cancel) or
//! [`aio_cancel_all`](fn.aio_cancel_all.html), though the operating system may
//! not support this for all filesystems and devices.

use crate::{Error, Result};
use crate::errno::Errno;
use std::os::unix::io::RawFd;
use libc::{c_void, off_t, size_t};
use std::borrow::{Borrow, BorrowMut};
use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem;
use std::ptr::{null, null_mut};
use crate::sys::signal::*;
use std::thread;
use crate::sys::time::TimeSpec;

libc_enum! {
    /// Mode for `AioCb::fsync`.  Controls whether only data or both data and
    /// metadata are synced.
    #[repr(i32)]
    pub enum AioFsyncMode {
        /// do it like `fsync`
        O_SYNC,
        /// on supported operating systems only, do it like `fdatasync`
        #[cfg(any(target_os = "ios",
                  target_os = "linux",
                  target_os = "macos",
                  target_os = "netbsd",
                  target_os = "openbsd"))]
        O_DSYNC
    }
}

libc_enum! {
    /// When used with [`lio_listio`](fn.lio_listio.html), determines whether a
    /// given `aiocb` should be used for a read operation, a write operation, or
    /// ignored.  Has no effect for any other aio functions.
    #[repr(i32)]
    pub enum LioOpcode {
        LIO_NOP,
        LIO_WRITE,
        LIO_READ,
    }
}

libc_enum! {
    /// Mode for [`lio_listio`](fn.lio_listio.html)
    #[repr(i32)]
    pub enum LioMode {
        /// Requests that [`lio_listio`](fn.lio_listio.html) block until all
        /// requested operations have been completed
        LIO_WAIT,
        /// Requests that [`lio_listio`](fn.lio_listio.html) return immediately
        LIO_NOWAIT,
    }
}

/// Return values for [`AioCb::cancel`](struct.AioCb.html#method.cancel) and
/// [`aio_cancel_all`](fn.aio_cancel_all.html)
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AioCancelStat {
    /// All outstanding requests were canceled
    AioCanceled = libc::AIO_CANCELED,
    /// Some requests were not canceled.  Their status should be checked with
    /// `AioCb::error`
    AioNotCanceled = libc::AIO_NOTCANCELED,
    /// All of the requests have already finished
    AioAllDone = libc::AIO_ALLDONE,
}

/// Owns (uniquely or shared) a memory buffer to keep it from `Drop`ing while
/// the kernel has a pointer to it.
pub enum Buffer<'a> {
    /// No buffer to own.
    ///
    /// Used for operations like `aio_fsync` that have no data, or for unsafe
    /// operations that work with raw pointers.
    None,
    /// Keeps a reference to a slice
    Phantom(PhantomData<&'a mut [u8]>),
    /// Generic thing that keeps a buffer from dropping
    BoxedSlice(Box<dyn Borrow<[u8]>>),
    /// Generic thing that keeps a mutable buffer from dropping
    BoxedMutSlice(Box<dyn BorrowMut<[u8]>>),
}

impl<'a> Debug for Buffer<'a> {
    // Note: someday it may be possible to Derive Debug for a trait object, but
    // not today.
    // https://github.com/rust-lang/rust/issues/1563
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Buffer::None => write!(fmt, "None"),
            Buffer::Phantom(p) => p.fmt(fmt),
            Buffer::BoxedSlice(ref bs) => {
                let borrowed : &dyn Borrow<[u8]> = bs.borrow();
                write!(fmt, "BoxedSlice({:?})",
                    borrowed as *const dyn Borrow<[u8]>)
            },
            Buffer::BoxedMutSlice(ref bms) => {
                let borrowed : &dyn BorrowMut<[u8]> = bms.borrow();
                write!(fmt, "BoxedMutSlice({:?})",
                    borrowed as *const dyn BorrowMut<[u8]>)
            }
        }
    }
}

/// AIO Control Block.
///
/// The basic structure used by all aio functions.  Each `AioCb` represents one
/// I/O request.
pub struct AioCb<'a> {
    aiocb: libc::aiocb,
    /// Tracks whether the buffer pointed to by `libc::aiocb.aio_buf` is mutable
    mutable: bool,
    /// Could this `AioCb` potentially have any in-kernel state?
    in_progress: bool,
    /// Optionally keeps a reference to the data.
    ///
    /// Used to keep buffers from `Drop`'ing, and may be returned once the
    /// `AioCb` is completed by [`buffer`](#method.buffer).
    buffer: Buffer<'a>
}

impl<'a> AioCb<'a> {
    /// Remove the inner `Buffer` and return it
    ///
    /// It is an error to call this method while the `AioCb` is still in
    /// progress.
    pub fn buffer(&mut self) -> Buffer<'a> {
        assert!(!self.in_progress);
        let mut x = Buffer::None;
        mem::swap(&mut self.buffer, &mut x);
        x
    }

    /// Remove the inner boxed slice, if any, and return it.
    ///
    /// The returned value will be the argument that was passed to
    /// `from_boxed_slice` when this `AioCb` was created.
    ///
    /// It is an error to call this method while the `AioCb` is still in
    /// progress.
    pub fn boxed_slice(&mut self) -> Option<Box<dyn Borrow<[u8]>>> {
        assert!(!self.in_progress, "Can't remove the buffer from an AioCb that's still in-progress.  Did you forget to call aio_return?");
        if let Buffer::BoxedSlice(_) = self.buffer {
            let mut oldbuffer = Buffer::None;
            mem::swap(&mut self.buffer, &mut oldbuffer);
            if let Buffer::BoxedSlice(inner) = oldbuffer {
                Some(inner)
            } else {
                unreachable!();
            }
        } else {
            None
        }
    }

    /// Remove the inner boxed mutable slice, if any, and return it.
    ///
    /// The returned value will be the argument that was passed to
    /// `from_boxed_mut_slice` when this `AioCb` was created.
    ///
    /// It is an error to call this method while the `AioCb` is still in
    /// progress.
    pub fn boxed_mut_slice(&mut self) -> Option<Box<dyn BorrowMut<[u8]>>> {
        assert!(!self.in_progress, "Can't remove the buffer from an AioCb that's still in-progress.  Did you forget to call aio_return?");
        if let Buffer::BoxedMutSlice(_) = self.buffer {
            let mut oldbuffer = Buffer::None;
            mem::swap(&mut self.buffer, &mut oldbuffer);
            if let Buffer::BoxedMutSlice(inner) = oldbuffer {
                Some(inner)
            } else {
                unreachable!();
            }
        } else {
            None
        }
    }

    /// Returns the underlying file descriptor associated with the `AioCb`
    pub fn fd(&self) -> RawFd {
        self.aiocb.aio_fildes
    }

    /// Constructs a new `AioCb` with no associated buffer.
    ///
    /// The resulting `AioCb` structure is suitable for use with `AioCb::fsync`.
    ///
    /// # Parameters
    ///
    /// * `fd`:           File descriptor.  Required for all aio functions.
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`.
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                    completion.
    ///
    /// # Examples
    ///
    /// Create an `AioCb` from a raw file descriptor and use it for an
    /// [`fsync`](#method.fsync) operation.
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify::SigevNone;
    /// # use std::{thread, time};
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// let f = tempfile().unwrap();
    /// let mut aiocb = AioCb::from_fd( f.as_raw_fd(), 0, SigevNone);
    /// aiocb.fsync(AioFsyncMode::O_SYNC).expect("aio_fsync failed early");
    /// while (aiocb.error() == Err(Error::from(Errno::EINPROGRESS))) {
    ///     thread::sleep(time::Duration::from_millis(10));
    /// }
    /// aiocb.aio_return().expect("aio_fsync failed late");
    /// # }
    /// ```
    pub fn from_fd(fd: RawFd, prio: libc::c_int,
                    sigev_notify: SigevNotify) -> AioCb<'a> {
        let mut a = AioCb::common_init(fd, prio, sigev_notify);
        a.aio_offset = 0;
        a.aio_nbytes = 0;
        a.aio_buf = null_mut();

        AioCb {
            aiocb: a,
            mutable: false,
            in_progress: false,
            buffer: Buffer::None
        }
    }

    /// Constructs a new `AioCb` from a mutable slice.
    ///
    /// The resulting `AioCb` will be suitable for both read and write
    /// operations, but only if the borrow checker can guarantee that the slice
    /// will outlive the `AioCb`.  That will usually be the case if the `AioCb`
    /// is stack-allocated.  If the borrow checker gives you trouble, try using
    /// [`from_boxed_mut_slice`](#method.from_boxed_mut_slice) instead.
    ///
    /// # Parameters
    ///
    /// * `fd`:           File descriptor.  Required for all aio functions.
    /// * `offs`:         File offset
    /// * `buf`:          A memory buffer
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    /// * `opcode`:       This field is only used for `lio_listio`.  It
    ///                   determines which operation to use for this individual
    ///                   aiocb
    ///
    /// # Examples
    ///
    /// Create an `AioCb` from a mutable slice and read into it.
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::{thread, time};
    /// # use std::io::Write;
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// const INITIAL: &[u8] = b"abcdef123456";
    /// const LEN: usize = 4;
    /// let mut rbuf = vec![0; LEN];
    /// let mut f = tempfile().unwrap();
    /// f.write_all(INITIAL).unwrap();
    /// {
    ///     let mut aiocb = AioCb::from_mut_slice( f.as_raw_fd(),
    ///         2,   //offset
    ///         &mut rbuf,
    ///         0,   //priority
    ///         SigevNotify::SigevNone,
    ///         LioOpcode::LIO_NOP);
    ///     aiocb.read().unwrap();
    ///     while (aiocb.error() == Err(Error::from(Errno::EINPROGRESS))) {
    ///         thread::sleep(time::Duration::from_millis(10));
    ///     }
    ///     assert_eq!(aiocb.aio_return().unwrap() as usize, LEN);
    /// }
    /// assert_eq!(rbuf, b"cdef");
    /// # }
    /// ```
    pub fn from_mut_slice(fd: RawFd, offs: off_t, buf: &'a mut [u8],
                          prio: libc::c_int, sigev_notify: SigevNotify,
                          opcode: LioOpcode) -> AioCb<'a> {
        let mut a = AioCb::common_init(fd, prio, sigev_notify);
        a.aio_offset = offs;
        a.aio_nbytes = buf.len() as size_t;
        a.aio_buf = buf.as_ptr() as *mut c_void;
        a.aio_lio_opcode = opcode as libc::c_int;

        AioCb {
            aiocb: a,
            mutable: true,
            in_progress: false,
            buffer: Buffer::Phantom(PhantomData),
        }
    }

    /// The safest and most flexible way to create an `AioCb`.
    ///
    /// Unlike [`from_slice`], this method returns a structure suitable for
    /// placement on the heap.  It may be used for write operations, but not
    /// read operations.  Unlike `from_ptr`, this method will ensure that the
    /// buffer doesn't `drop` while the kernel is still processing it.  Any
    /// object that can be borrowed as a boxed slice will work.
    ///
    /// # Parameters
    ///
    /// * `fd`:           File descriptor.  Required for all aio functions.
    /// * `offs`:         File offset
    /// * `buf`:          A boxed slice-like object
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    /// * `opcode`:       This field is only used for `lio_listio`.  It
    ///                   determines which operation to use for this individual
    ///                   aiocb
    ///
    /// # Examples
    ///
    /// Create an `AioCb` from a Vector and use it for writing
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::{thread, time};
    /// # use std::io::Write;
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// let wbuf = Box::new(Vec::from("CDEF"));
    /// let expected_len = wbuf.len();
    /// let mut f = tempfile().unwrap();
    /// let mut aiocb = AioCb::from_boxed_slice( f.as_raw_fd(),
    ///     2,   //offset
    ///     wbuf,
    ///     0,   //priority
    ///     SigevNotify::SigevNone,
    ///     LioOpcode::LIO_NOP);
    /// aiocb.write().unwrap();
    /// while (aiocb.error() == Err(Error::from(Errno::EINPROGRESS))) {
    ///     thread::sleep(time::Duration::from_millis(10));
    /// }
    /// assert_eq!(aiocb.aio_return().unwrap() as usize, expected_len);
    /// # }
    /// ```
    ///
    /// Create an `AioCb` from a `Bytes` object
    ///
    /// ```
    /// # use bytes::Bytes;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// let wbuf = Box::new(Bytes::from(&b"CDEF"[..]));
    /// let mut f = tempfile().unwrap();
    /// let mut aiocb = AioCb::from_boxed_slice( f.as_raw_fd(),
    ///     2,   //offset
    ///     wbuf,
    ///     0,   //priority
    ///     SigevNotify::SigevNone,
    ///     LioOpcode::LIO_NOP);
    /// # }
    /// ```
    ///
    /// If a library needs to work with buffers that aren't `Box`ed, it can
    /// create a `Box`ed container for use with this method.  Here's an example
    /// using an un`Box`ed `Bytes` object.
    ///
    /// ```
    /// # use bytes::Bytes;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::borrow::Borrow;
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// struct BytesContainer(Bytes);
    /// impl Borrow<[u8]> for BytesContainer {
    ///     fn borrow(&self) -> &[u8] {
    ///         self.0.as_ref()
    ///     }
    /// }
    /// fn main() {
    ///     let wbuf = Bytes::from(&b"CDEF"[..]);
    ///     let boxed_wbuf = Box::new(BytesContainer(wbuf));
    ///     let mut f = tempfile().unwrap();
    ///     let mut aiocb = AioCb::from_boxed_slice( f.as_raw_fd(),
    ///         2,   //offset
    ///         boxed_wbuf,
    ///         0,   //priority
    ///         SigevNotify::SigevNone,
    ///         LioOpcode::LIO_NOP);
    /// }
    /// ```
    ///
    /// [`from_slice`]: #method.from_slice
    pub fn from_boxed_slice(fd: RawFd, offs: off_t, buf: Box<dyn Borrow<[u8]>>,
                      prio: libc::c_int, sigev_notify: SigevNotify,
                      opcode: LioOpcode) -> AioCb<'a> {
        let mut a = AioCb::common_init(fd, prio, sigev_notify);
        {
            let borrowed : &dyn Borrow<[u8]> = buf.borrow();
            let slice : &[u8] = borrowed.borrow();
            a.aio_nbytes = slice.len() as size_t;
            a.aio_buf = slice.as_ptr() as *mut c_void;
        }
        a.aio_offset = offs;
        a.aio_lio_opcode = opcode as libc::c_int;

        AioCb {
            aiocb: a,
            mutable: false,
            in_progress: false,
            buffer: Buffer::BoxedSlice(buf),
        }
    }

    /// The safest and most flexible way to create an `AioCb` for reading.
    ///
    /// Like [`from_boxed_slice`], but the slice is a mutable one.  More
    /// flexible than [`from_mut_slice`], because a wide range of objects can be
    /// used.
    ///
    /// # Examples
    ///
    /// Create an `AioCb` from a Vector and use it for reading
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::{thread, time};
    /// # use std::io::Write;
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// const INITIAL: &[u8] = b"abcdef123456";
    /// const LEN: usize = 4;
    /// let rbuf = Box::new(vec![0; LEN]);
    /// let mut f = tempfile().unwrap();
    /// f.write_all(INITIAL).unwrap();
    /// let mut aiocb = AioCb::from_boxed_mut_slice( f.as_raw_fd(),
    ///     2,   //offset
    ///     rbuf,
    ///     0,   //priority
    ///     SigevNotify::SigevNone,
    ///     LioOpcode::LIO_NOP);
    /// aiocb.read().unwrap();
    /// while (aiocb.error() == Err(Error::from(Errno::EINPROGRESS))) {
    ///     thread::sleep(time::Duration::from_millis(10));
    /// }
    /// assert_eq!(aiocb.aio_return().unwrap() as usize, LEN);
    /// let mut buffer = aiocb.boxed_mut_slice().unwrap();
    /// const EXPECT: &[u8] = b"cdef";
    /// assert_eq!(buffer.borrow_mut(), EXPECT);
    /// # }
    /// ```
    ///
    /// [`from_boxed_slice`]: #method.from_boxed_slice
    /// [`from_mut_slice`]: #method.from_mut_slice
    pub fn from_boxed_mut_slice(fd: RawFd, offs: off_t,
                                mut buf: Box<dyn BorrowMut<[u8]>>,
                                prio: libc::c_int, sigev_notify: SigevNotify,
                                opcode: LioOpcode) -> AioCb<'a> {
        let mut a = AioCb::common_init(fd, prio, sigev_notify);
        {
            let borrowed : &mut dyn BorrowMut<[u8]> = buf.borrow_mut();
            let slice : &mut [u8] = borrowed.borrow_mut();
            a.aio_nbytes = slice.len() as size_t;
            a.aio_buf = slice.as_mut_ptr() as *mut c_void;
        }
        a.aio_offset = offs;
        a.aio_lio_opcode = opcode as libc::c_int;

        AioCb {
            aiocb: a,
            mutable: true,
            in_progress: false,
            buffer: Buffer::BoxedMutSlice(buf),
        }
    }

    /// Constructs a new `AioCb` from a mutable raw pointer
    ///
    /// Unlike `from_mut_slice`, this method returns a structure suitable for
    /// placement on the heap.  It may be used for both reads and writes.  Due
    /// to its unsafety, this method is not recommended.  It is most useful when
    /// heap allocation is required but for some reason the data cannot be
    /// wrapped in a `struct` that implements `BorrowMut<[u8]>`
    ///
    /// # Parameters
    ///
    /// * `fd`:           File descriptor.  Required for all aio functions.
    /// * `offs`:         File offset
    /// * `buf`:          Pointer to the memory buffer
    /// * `len`:          Length of the buffer pointed to by `buf`
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    /// * `opcode`:       This field is only used for `lio_listio`.  It
    ///                   determines which operation to use for this individual
    ///                   aiocb
    ///
    /// # Safety
    ///
    /// The caller must ensure that the storage pointed to by `buf` outlives the
    /// `AioCb`.  The lifetime checker can't help here.
    pub unsafe fn from_mut_ptr(fd: RawFd, offs: off_t,
                           buf: *mut c_void, len: usize,
                           prio: libc::c_int, sigev_notify: SigevNotify,
                           opcode: LioOpcode) -> AioCb<'a> {
        let mut a = AioCb::common_init(fd, prio, sigev_notify);
        a.aio_offset = offs;
        a.aio_nbytes = len;
        a.aio_buf = buf;
        a.aio_lio_opcode = opcode as libc::c_int;

        AioCb {
            aiocb: a,
            mutable: true,
            in_progress: false,
            buffer: Buffer::None
        }
    }

    /// Constructs a new `AioCb` from a raw pointer.
    ///
    /// Unlike `from_slice`, this method returns a structure suitable for
    /// placement on the heap.  Due to its unsafety, this method is not
    /// recommended.  It is most useful when heap allocation is required but for
    /// some reason the data cannot be wrapped in a `struct` that implements
    /// `Borrow<[u8]>`
    ///
    /// # Parameters
    ///
    /// * `fd`:           File descriptor.  Required for all aio functions.
    /// * `offs`:         File offset
    /// * `buf`:          Pointer to the memory buffer
    /// * `len`:          Length of the buffer pointed to by `buf`
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    /// * `opcode`:       This field is only used for `lio_listio`.  It
    ///                   determines which operation to use for this individual
    ///                   aiocb
    ///
    /// # Safety
    ///
    /// The caller must ensure that the storage pointed to by `buf` outlives the
    /// `AioCb`.  The lifetime checker can't help here.
    pub unsafe fn from_ptr(fd: RawFd, offs: off_t,
                           buf: *const c_void, len: usize,
                           prio: libc::c_int, sigev_notify: SigevNotify,
                           opcode: LioOpcode) -> AioCb<'a> {
        let mut a = AioCb::common_init(fd, prio, sigev_notify);
        a.aio_offset = offs;
        a.aio_nbytes = len;
        // casting a const ptr to a mutable ptr here is ok, because we set the
        // AioCb's mutable field to false
        a.aio_buf = buf as *mut c_void;
        a.aio_lio_opcode = opcode as libc::c_int;

        AioCb {
            aiocb: a,
            mutable: false,
            in_progress: false,
            buffer: Buffer::None
        }
    }

    /// Like `from_mut_slice`, but works on constant slices rather than
    /// mutable slices.
    ///
    /// An `AioCb` created this way cannot be used with `read`, and its
    /// `LioOpcode` cannot be set to `LIO_READ`.  This method is useful when
    /// writing a const buffer with `AioCb::write`, since `from_mut_slice` can't
    /// work with const buffers.
    ///
    /// # Examples
    ///
    /// Construct an `AioCb` from a slice and use it for writing.
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::{thread, time};
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// const WBUF: &[u8] = b"abcdef123456";
    /// let mut f = tempfile().unwrap();
    /// let mut aiocb = AioCb::from_slice( f.as_raw_fd(),
    ///     2,   //offset
    ///     WBUF,
    ///     0,   //priority
    ///     SigevNotify::SigevNone,
    ///     LioOpcode::LIO_NOP);
    /// aiocb.write().unwrap();
    /// while (aiocb.error() == Err(Error::from(Errno::EINPROGRESS))) {
    ///     thread::sleep(time::Duration::from_millis(10));
    /// }
    /// assert_eq!(aiocb.aio_return().unwrap() as usize, WBUF.len());
    /// # }
    /// ```
    // Note: another solution to the problem of writing const buffers would be
    // to genericize AioCb for both &mut [u8] and &[u8] buffers.  AioCb::read
    // could take the former and AioCb::write could take the latter.  However,
    // then lio_listio wouldn't work, because that function needs a slice of
    // AioCb, and they must all be of the same type.
    pub fn from_slice(fd: RawFd, offs: off_t, buf: &'a [u8],
                      prio: libc::c_int, sigev_notify: SigevNotify,
                      opcode: LioOpcode) -> AioCb {
        let mut a = AioCb::common_init(fd, prio, sigev_notify);
        a.aio_offset = offs;
        a.aio_nbytes = buf.len() as size_t;
        // casting an immutable buffer to a mutable pointer looks unsafe,
        // but technically its only unsafe to dereference it, not to create
        // it.
        a.aio_buf = buf.as_ptr() as *mut c_void;
        assert!(opcode != LioOpcode::LIO_READ, "Can't read into an immutable buffer");
        a.aio_lio_opcode = opcode as libc::c_int;

        AioCb {
            aiocb: a,
            mutable: false,
            in_progress: false,
            buffer: Buffer::None,
        }
    }

    fn common_init(fd: RawFd, prio: libc::c_int,
                   sigev_notify: SigevNotify) -> libc::aiocb {
        // Use mem::zeroed instead of explicitly zeroing each field, because the
        // number and name of reserved fields is OS-dependent.  On some OSes,
        // some reserved fields are used the kernel for state, and must be
        // explicitly zeroed when allocated.
        let mut a = unsafe { mem::zeroed::<libc::aiocb>()};
        a.aio_fildes = fd;
        a.aio_reqprio = prio;
        a.aio_sigevent = SigEvent::new(sigev_notify).sigevent();
        a
    }

    /// Update the notification settings for an existing `aiocb`
    pub fn set_sigev_notify(&mut self, sigev_notify: SigevNotify) {
        self.aiocb.aio_sigevent = SigEvent::new(sigev_notify).sigevent();
    }

    /// Cancels an outstanding AIO request.
    ///
    /// The operating system is not required to implement cancellation for all
    /// file and device types.  Even if it does, there is no guarantee that the
    /// operation has not already completed.  So the caller must check the
    /// result and handle operations that were not canceled or that have already
    /// completed.
    ///
    /// # Examples
    ///
    /// Cancel an outstanding aio operation.  Note that we must still call
    /// `aio_return` to free resources, even though we don't care about the
    /// result.
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::{thread, time};
    /// # use std::io::Write;
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// let wbuf = b"CDEF";
    /// let mut f = tempfile().unwrap();
    /// let mut aiocb = AioCb::from_slice( f.as_raw_fd(),
    ///     2,   //offset
    ///     &wbuf[..],
    ///     0,   //priority
    ///     SigevNotify::SigevNone,
    ///     LioOpcode::LIO_NOP);
    /// aiocb.write().unwrap();
    /// let cs = aiocb.cancel().unwrap();
    /// if cs == AioCancelStat::AioNotCanceled {
    ///     while (aiocb.error() == Err(Error::from(Errno::EINPROGRESS))) {
    ///         thread::sleep(time::Duration::from_millis(10));
    ///     }
    /// }
    /// // Must call `aio_return`, but ignore the result
    /// let _ = aiocb.aio_return();
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// [aio_cancel](http://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_cancel.html)
    pub fn cancel(&mut self) -> Result<AioCancelStat> {
        match unsafe { libc::aio_cancel(self.aiocb.aio_fildes, &mut self.aiocb) } {
            libc::AIO_CANCELED => Ok(AioCancelStat::AioCanceled),
            libc::AIO_NOTCANCELED => Ok(AioCancelStat::AioNotCanceled),
            libc::AIO_ALLDONE => Ok(AioCancelStat::AioAllDone),
            -1 => Err(Error::last()),
            _ => panic!("unknown aio_cancel return value")
        }
    }

    /// Retrieve error status of an asynchronous operation.
    ///
    /// If the request has not yet completed, returns `EINPROGRESS`.  Otherwise,
    /// returns `Ok` or any other error.
    ///
    /// # Examples
    ///
    /// Issue an aio operation and use `error` to poll for completion.  Polling
    /// is an alternative to `aio_suspend`, used by most of the other examples.
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::{thread, time};
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// const WBUF: &[u8] = b"abcdef123456";
    /// let mut f = tempfile().unwrap();
    /// let mut aiocb = AioCb::from_slice( f.as_raw_fd(),
    ///     2,   //offset
    ///     WBUF,
    ///     0,   //priority
    ///     SigevNotify::SigevNone,
    ///     LioOpcode::LIO_NOP);
    /// aiocb.write().unwrap();
    /// while (aiocb.error() == Err(Error::from(Errno::EINPROGRESS))) {
    ///     thread::sleep(time::Duration::from_millis(10));
    /// }
    /// assert_eq!(aiocb.aio_return().unwrap() as usize, WBUF.len());
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// [aio_error](http://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_error.html)
    pub fn error(&mut self) -> Result<()> {
        match unsafe { libc::aio_error(&mut self.aiocb as *mut libc::aiocb) } {
            0 => Ok(()),
            num if num > 0 => Err(Error::from_errno(Errno::from_i32(num))),
            -1 => Err(Error::last()),
            num => panic!("unknown aio_error return value {:?}", num)
        }
    }

    /// An asynchronous version of `fsync(2)`.
    ///
    /// # References
    ///
    /// [aio_fsync](http://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_fsync.html)
    pub fn fsync(&mut self, mode: AioFsyncMode) -> Result<()> {
        let p: *mut libc::aiocb = &mut self.aiocb;
        Errno::result(unsafe {
                libc::aio_fsync(mode as libc::c_int, p)
        }).map(|_| {
            self.in_progress = true;
        })
    }

    /// Returns the `aiocb`'s `LioOpcode` field
    ///
    /// If the value cannot be represented as an `LioOpcode`, returns `None`
    /// instead.
    pub fn lio_opcode(&self) -> Option<LioOpcode> {
        match self.aiocb.aio_lio_opcode {
            libc::LIO_READ => Some(LioOpcode::LIO_READ),
            libc::LIO_WRITE => Some(LioOpcode::LIO_WRITE),
            libc::LIO_NOP => Some(LioOpcode::LIO_NOP),
            _ => None
        }
    }

    /// Returns the requested length of the aio operation in bytes
    ///
    /// This method returns the *requested* length of the operation.  To get the
    /// number of bytes actually read or written by a completed operation, use
    /// `aio_return` instead.
    pub fn nbytes(&self) -> usize {
        self.aiocb.aio_nbytes
    }

    /// Returns the file offset stored in the `AioCb`
    pub fn offset(&self) -> off_t {
        self.aiocb.aio_offset
    }

    /// Returns the priority of the `AioCb`
    pub fn priority(&self) -> libc::c_int {
        self.aiocb.aio_reqprio
    }

    /// Asynchronously reads from a file descriptor into a buffer
    ///
    /// # References
    ///
    /// [aio_read](http://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_read.html)
    pub fn read(&mut self) -> Result<()> {
        assert!(self.mutable, "Can't read into an immutable buffer");
        let p: *mut libc::aiocb = &mut self.aiocb;
        Errno::result(unsafe {
            libc::aio_read(p)
        }).map(|_| {
            self.in_progress = true;
        })
    }

    /// Returns the `SigEvent` stored in the `AioCb`
    pub fn sigevent(&self) -> SigEvent {
        SigEvent::from(&self.aiocb.aio_sigevent)
    }

    /// Retrieve return status of an asynchronous operation.
    ///
    /// Should only be called once for each `AioCb`, after `AioCb::error`
    /// indicates that it has completed.  The result is the same as for the
    /// synchronous `read(2)`, `write(2)`, of `fsync(2)` functions.
    ///
    /// # References
    ///
    /// [aio_return](http://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_return.html)
    // Note: this should be just `return`, but that's a reserved word
    pub fn aio_return(&mut self) -> Result<isize> {
        let p: *mut libc::aiocb = &mut self.aiocb;
        self.in_progress = false;
        Errno::result(unsafe { libc::aio_return(p) })
    }

    /// Asynchronously writes from a buffer to a file descriptor
    ///
    /// # References
    ///
    /// [aio_write](http://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_write.html)
    pub fn write(&mut self) -> Result<()> {
        let p: *mut libc::aiocb = &mut self.aiocb;
        Errno::result(unsafe {
            libc::aio_write(p)
        }).map(|_| {
            self.in_progress = true;
        })
    }

}

/// Cancels outstanding AIO requests for a given file descriptor.
///
/// # Examples
///
/// Issue an aio operation, then cancel all outstanding operations on that file
/// descriptor.
///
/// ```
/// # use nix::errno::Errno;
/// # use nix::Error;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use std::{thread, time};
/// # use std::io::Write;
/// # use std::os::unix::io::AsRawFd;
/// # use tempfile::tempfile;
/// # fn main() {
/// let wbuf = b"CDEF";
/// let mut f = tempfile().unwrap();
/// let mut aiocb = AioCb::from_slice( f.as_raw_fd(),
///     2,   //offset
///     &wbuf[..],
///     0,   //priority
///     SigevNotify::SigevNone,
///     LioOpcode::LIO_NOP);
/// aiocb.write().unwrap();
/// let cs = aio_cancel_all(f.as_raw_fd()).unwrap();
/// if cs == AioCancelStat::AioNotCanceled {
///     while (aiocb.error() == Err(Error::from(Errno::EINPROGRESS))) {
///         thread::sleep(time::Duration::from_millis(10));
///     }
/// }
/// // Must call `aio_return`, but ignore the result
/// let _ = aiocb.aio_return();
/// # }
/// ```
///
/// # References
///
/// [`aio_cancel`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_cancel.html)
pub fn aio_cancel_all(fd: RawFd) -> Result<AioCancelStat> {
    match unsafe { libc::aio_cancel(fd, null_mut()) } {
        libc::AIO_CANCELED => Ok(AioCancelStat::AioCanceled),
        libc::AIO_NOTCANCELED => Ok(AioCancelStat::AioNotCanceled),
        libc::AIO_ALLDONE => Ok(AioCancelStat::AioAllDone),
        -1 => Err(Error::last()),
        _ => panic!("unknown aio_cancel return value")
    }
}

/// Suspends the calling process until at least one of the specified `AioCb`s
/// has completed, a signal is delivered, or the timeout has passed.
///
/// If `timeout` is `None`, `aio_suspend` will block indefinitely.
///
/// # Examples
///
/// Use `aio_suspend` to block until an aio operation completes.
///
/// ```
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use std::os::unix::io::AsRawFd;
/// # use tempfile::tempfile;
/// # fn main() {
/// const WBUF: &[u8] = b"abcdef123456";
/// let mut f = tempfile().unwrap();
/// let mut aiocb = AioCb::from_slice( f.as_raw_fd(),
///     2,   //offset
///     WBUF,
///     0,   //priority
///     SigevNotify::SigevNone,
///     LioOpcode::LIO_NOP);
/// aiocb.write().unwrap();
/// aio_suspend(&[&aiocb], None).expect("aio_suspend failed");
/// assert_eq!(aiocb.aio_return().unwrap() as usize, WBUF.len());
/// # }
/// ```
/// # References
///
/// [`aio_suspend`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_suspend.html)
pub fn aio_suspend(list: &[&AioCb], timeout: Option<TimeSpec>) -> Result<()> {
    let plist = list as *const [&AioCb] as *const [*const libc::aiocb];
    let p = plist as *const *const libc::aiocb;
    let timep = match timeout {
        None    => null::<libc::timespec>(),
        Some(x) => x.as_ref() as *const libc::timespec
    };
    Errno::result(unsafe {
        libc::aio_suspend(p, list.len() as i32, timep)
    }).map(drop)
}

impl<'a> Debug for AioCb<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("AioCb")
            .field("aiocb", &self.aiocb)
            .field("mutable", &self.mutable)
            .field("in_progress", &self.in_progress)
            .finish()
    }
}

impl<'a> Drop for AioCb<'a> {
    /// If the `AioCb` has no remaining state in the kernel, just drop it.
    /// Otherwise, dropping constitutes a resource leak, which is an error
    fn drop(&mut self) {
        assert!(thread::panicking() || !self.in_progress,
                "Dropped an in-progress AioCb");
    }
}

/// LIO Control Block.
///
/// The basic structure used to issue multiple AIO operations simultaneously.
#[cfg(not(any(target_os = "ios", target_os = "macos")))]
pub struct LioCb<'a> {
    /// A collection of [`AioCb`]s.  All of these will be issued simultaneously
    /// by the [`listio`] method.
    ///
    /// [`AioCb`]: struct.AioCb.html
    /// [`listio`]: #method.listio
    pub aiocbs: Vec<AioCb<'a>>,

    /// The actual list passed to `libc::lio_listio`.
    ///
    /// It must live for as long as any of the operations are still being
    /// processesed, because the aio subsystem uses its address as a unique
    /// identifier.
    list: Vec<*mut libc::aiocb>,

    /// A partial set of results.  This field will get populated by
    /// `listio_resubmit` when an `LioCb` is resubmitted after an error
    results: Vec<Option<Result<isize>>>
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
impl<'a> LioCb<'a> {
    /// Initialize an empty `LioCb`
    pub fn with_capacity(capacity: usize) -> LioCb<'a> {
        LioCb {
            aiocbs: Vec::with_capacity(capacity),
            list: Vec::with_capacity(capacity),
            results: Vec::with_capacity(capacity)
        }
    }

    /// Submits multiple asynchronous I/O requests with a single system call.
    ///
    /// They are not guaranteed to complete atomically, and the order in which
    /// the requests are carried out is not specified.  Reads, writes, and
    /// fsyncs may be freely mixed.
    ///
    /// This function is useful for reducing the context-switch overhead of
    /// submitting many AIO operations.  It can also be used with
    /// `LioMode::LIO_WAIT` to block on the result of several independent
    /// operations.  Used that way, it is often useful in programs that
    /// otherwise make little use of AIO.
    ///
    /// # Examples
    ///
    /// Use `listio` to submit an aio operation and wait for its completion.  In
    /// this case, there is no need to use [`aio_suspend`] to wait or
    /// [`AioCb::error`] to poll.
    ///
    /// ```
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::os::unix::io::AsRawFd;
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// const WBUF: &[u8] = b"abcdef123456";
    /// let mut f = tempfile().unwrap();
    /// let mut liocb = LioCb::with_capacity(1);
    /// liocb.aiocbs.push(AioCb::from_slice( f.as_raw_fd(),
    ///     2,   //offset
    ///     WBUF,
    ///     0,   //priority
    ///     SigevNotify::SigevNone,
    ///     LioOpcode::LIO_WRITE));
    /// liocb.listio(LioMode::LIO_WAIT,
    ///              SigevNotify::SigevNone).unwrap();
    /// assert_eq!(liocb.aio_return(0).unwrap() as usize, WBUF.len());
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// [`lio_listio`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/lio_listio.html)
    ///
    /// [`aio_suspend`]: fn.aio_suspend.html
    /// [`AioCb::error`]: struct.AioCb.html#method.error
    pub fn listio(&mut self, mode: LioMode,
                  sigev_notify: SigevNotify) -> Result<()> {
        let sigev = SigEvent::new(sigev_notify);
        let sigevp = &mut sigev.sigevent() as *mut libc::sigevent;
        self.list.clear();
        for a in &mut self.aiocbs {
            a.in_progress = true;
            self.list.push(a as *mut AioCb<'a>
                             as *mut libc::aiocb);
        }
        let p = self.list.as_ptr();
        Errno::result(unsafe {
            libc::lio_listio(mode as i32, p, self.list.len() as i32, sigevp)
        }).map(drop)
    }

    /// Resubmits any incomplete operations with [`lio_listio`].
    ///
    /// Sometimes, due to system resource limitations, an `lio_listio` call will
    /// return `EIO`, or `EAGAIN`.  Or, if a signal is received, it may return
    /// `EINTR`.  In any of these cases, only a subset of its constituent
    /// operations will actually have been initiated.  `listio_resubmit` will
    /// resubmit any operations that are still uninitiated.
    ///
    /// After calling `listio_resubmit`, results should be collected by
    /// [`LioCb::aio_return`].
    ///
    /// # Examples
    /// ```no_run
    /// # use nix::Error;
    /// # use nix::errno::Errno;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::os::unix::io::AsRawFd;
    /// # use std::{thread, time};
    /// # use tempfile::tempfile;
    /// # fn main() {
    /// const WBUF: &[u8] = b"abcdef123456";
    /// let mut f = tempfile().unwrap();
    /// let mut liocb = LioCb::with_capacity(1);
    /// liocb.aiocbs.push(AioCb::from_slice( f.as_raw_fd(),
    ///     2,   //offset
    ///     WBUF,
    ///     0,   //priority
    ///     SigevNotify::SigevNone,
    ///     LioOpcode::LIO_WRITE));
    /// let mut err = liocb.listio(LioMode::LIO_WAIT, SigevNotify::SigevNone);
    /// while err == Err(Error::Sys(Errno::EIO)) ||
    ///       err == Err(Error::Sys(Errno::EAGAIN)) {
    ///     thread::sleep(time::Duration::from_millis(10));
    ///     err = liocb.listio_resubmit(LioMode::LIO_WAIT, SigevNotify::SigevNone);
    /// }
    /// assert_eq!(liocb.aio_return(0).unwrap() as usize, WBUF.len());
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// [`lio_listio`](http://pubs.opengroup.org/onlinepubs/9699919799/functions/lio_listio.html)
    ///
    /// [`lio_listio`]: http://pubs.opengroup.org/onlinepubs/9699919799/functions/lio_listio.html
    /// [`LioCb::aio_return`]: struct.LioCb.html#method.aio_return
    // Note: the addresses of any EINPROGRESS or EOK aiocbs _must_ not be
    // changed by this method, because the kernel relies on their addresses
    // being stable.
    // Note: aiocbs that are Ok(()) must be finalized by aio_return, or else the
    // sigev_notify will immediately refire.
    pub fn listio_resubmit(&mut self, mode:LioMode,
                           sigev_notify: SigevNotify) -> Result<()> {
        let sigev = SigEvent::new(sigev_notify);
        let sigevp = &mut sigev.sigevent() as *mut libc::sigevent;
        self.list.clear();

        while self.results.len() < self.aiocbs.len() {
            self.results.push(None);
        }

        for (i, a) in self.aiocbs.iter_mut().enumerate() {
            if self.results[i].is_some() {
                // Already collected final status for this operation
                continue;
            }
            match a.error() {
                Ok(()) => {
                    // aiocb is complete; collect its status and don't resubmit
                    self.results[i] = Some(a.aio_return());
                },
                Err(Error::Sys(Errno::EAGAIN)) => {
                    self.list.push(a as *mut AioCb<'a> as *mut libc::aiocb);
                },
                Err(Error::Sys(Errno::EINPROGRESS)) => {
                    // aiocb is was successfully queued; no need to do anything
                },
                Err(Error::Sys(Errno::EINVAL)) => panic!(
                    "AioCb was never submitted, or already finalized"),
                _ => unreachable!()
            }
        }
        let p = self.list.as_ptr();
        Errno::result(unsafe {
            libc::lio_listio(mode as i32, p, self.list.len() as i32, sigevp)
        }).map(drop)
    }

    /// Collect final status for an individual `AioCb` submitted as part of an
    /// `LioCb`.
    ///
    /// This is just like [`AioCb::aio_return`], except it takes into account
    /// operations that were restarted by [`LioCb::listio_resubmit`]
    ///
    /// [`AioCb::aio_return`]: struct.AioCb.html#method.aio_return
    /// [`LioCb::listio_resubmit`]: #method.listio_resubmit
    pub fn aio_return(&mut self, i: usize) -> Result<isize> {
        if i >= self.results.len() || self.results[i].is_none() {
            self.aiocbs[i].aio_return()
        } else {
            self.results[i].unwrap()
        }
    }

    /// Retrieve error status of an individual `AioCb` submitted as part of an
    /// `LioCb`.
    ///
    /// This is just like [`AioCb::error`], except it takes into account
    /// operations that were restarted by [`LioCb::listio_resubmit`]
    ///
    /// [`AioCb::error`]: struct.AioCb.html#method.error
    /// [`LioCb::listio_resubmit`]: #method.listio_resubmit
    pub fn error(&mut self, i: usize) -> Result<()> {
        if i >= self.results.len() || self.results[i].is_none() {
            self.aiocbs[i].error()
        } else {
            Ok(())
        }
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
impl<'a> Debug for LioCb<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("LioCb")
            .field("aiocbs", &self.aiocbs)
            .finish()
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
impl<'a> From<Vec<AioCb<'a>>> for LioCb<'a> {
    fn from(src: Vec<AioCb<'a>>) -> LioCb<'a> {
        LioCb {
            list: Vec::with_capacity(src.capacity()),
            results: Vec::with_capacity(src.capacity()),
            aiocbs: src,
        }
    }
}
