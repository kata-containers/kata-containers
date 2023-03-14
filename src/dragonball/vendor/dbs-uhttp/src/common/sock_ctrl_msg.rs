// Copyright 2022 Ant Group. All rights reserved.
// Copyright 2020 Alibaba Cloud. All Rights Reserved.
//
// Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

/* Copied from the crosvm Project, commit 186eb8b */

use std::fs::File;
use std::mem::size_of;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::ptr;

#[cfg(target_os = "linux")]
use libc::MSG_NOSIGNAL;
use libc::{self, c_void, cmsghdr, iovec, msghdr, recvmsg, sendmsg, SCM_RIGHTS, SOL_SOCKET};

use crate::common::{SysError, SysResult};

// Each of the following macros performs the same function as their C counterparts. They are each
// macros because they are used to size statically allocated arrays.

macro_rules! CMSG_ALIGN {
    ($len:expr) => {
        (($len) as usize + ::std::mem::size_of::<libc::c_long>() - 1)
            & !(::std::mem::size_of::<libc::c_long>() - 1)
    };
}

macro_rules! CMSG_SPACE {
    ($len:expr) => {
        ::std::mem::size_of::<cmsghdr>() + CMSG_ALIGN!($len)
    };
}

macro_rules! FD_LENGTH {
    ($count:expr) => {
        std::mem::size_of::<RawFd>() * $count
    };
}

// This function (macro in the C version) is not used in any compile time constant slots, so is just
// an ordinary function. The returned pointer is hard coded to be RawFd because that's all that this
// module supports.
#[allow(non_snake_case)]
#[inline(always)]
fn CMSG_DATA(cmsg_buffer: *mut libc::cmsghdr) -> *mut RawFd {
    // Essentially returns a pointer to just past the header.
    cmsg_buffer.wrapping_offset(1) as *mut RawFd
}

#[cfg(not(target_env = "musl"))]
fn new_msghdr(iovecs: &mut [libc::iovec]) -> libc::msghdr {
    libc::msghdr {
        msg_name: ptr::null_mut(),
        msg_namelen: 0,
        msg_iov: iovecs.as_mut_ptr(),
        msg_iovlen: iovecs.len() as _,
        msg_control: ptr::null_mut(),
        msg_controllen: 0,
        msg_flags: 0,
    }
}

#[cfg(target_env = "musl")]
fn new_msghdr(iovecs: &mut [iovec]) -> msghdr {
    assert!(iovecs.len() <= (std::i32::MAX as usize));
    let mut msg: msghdr = unsafe { std::mem::zeroed() };
    msg.msg_name = ptr::null_mut();
    msg.msg_iov = iovecs.as_mut_ptr();
    msg.msg_iovlen = iovecs.len() as i32;
    msg.msg_control = ptr::null_mut();
    msg
}

#[cfg(not(target_env = "musl"))]
fn set_msg_controllen(msg: &mut msghdr, cmsg_capacity: usize) {
    msg.msg_controllen = cmsg_capacity as _;
}

#[cfg(target_env = "musl")]
fn set_msg_controllen(msg: &mut msghdr, cmsg_capacity: usize) {
    assert!(cmsg_capacity <= (std::u32::MAX as usize));
    msg.msg_controllen = cmsg_capacity as u32;
}

// This function is like CMSG_NEXT, but safer because it reads only from references, although it
// does some pointer arithmetic on cmsg_ptr.
// #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_ptr_alignment))]
// fn get_next_cmsg(msghdr: &msghdr, cmsg: &cmsghdr, cmsg_ptr: *mut cmsghdr) -> *mut cmsghdr {
//     let next_cmsg = (cmsg_ptr as *mut u8).wrapping_add(CMSG_ALIGN!(cmsg.cmsg_len)) as *mut cmsghdr;
//     if next_cmsg
//         .wrapping_offset(1)
//         .wrapping_sub(msghdr.msg_control as usize) as usize
//         > msghdr.msg_controllen as usize
//     {
//         null_mut()
//     } else {
//         next_cmsg
//     }
// }

const CMSG_BUFFER_INLINE_CAPACITY: usize = CMSG_SPACE!(size_of::<RawFd>() * 32);

impl CmsgBuffer {
    fn with_capacity(capacity: usize) -> CmsgBuffer {
        let cap_in_cmsghdr_units =
            (capacity.checked_add(size_of::<cmsghdr>()).unwrap() - 1) / size_of::<cmsghdr>();
        if capacity <= CMSG_BUFFER_INLINE_CAPACITY {
            CmsgBuffer::Inline([0u64; (CMSG_BUFFER_INLINE_CAPACITY + 7) / 8])
        } else {
            CmsgBuffer::Heap(
                vec![
                    cmsghdr {
                        cmsg_len: 0,
                        cmsg_level: 0,
                        cmsg_type: 0,
                        #[cfg(all(target_env = "musl", target_pointer_width = "64"))]
                        __pad1: 0,
                    };
                    cap_in_cmsghdr_units
                ]
                .into_boxed_slice(),
            )
        }
    }

    fn as_mut_ptr(&mut self) -> *mut libc::cmsghdr {
        match self {
            CmsgBuffer::Inline(a) => a.as_mut_ptr() as *mut cmsghdr,
            CmsgBuffer::Heap(a) => a.as_mut_ptr(),
        }
    }
}

enum CmsgBuffer {
    Inline([u64; (CMSG_BUFFER_INLINE_CAPACITY + 7) / 8]),
    Heap(Box<[cmsghdr]>),
}

fn raw_sendmsg<D: IntoIovec>(fd: RawFd, out_data: &[D], out_fds: &[RawFd]) -> SysResult<usize> {
    unsafe {
        let fd_len = FD_LENGTH!(out_fds.len());
        // let cmsg_capacity2 = CMSG_SPACE!(fd_len);
        let cmsg_capacity = libc::CMSG_SPACE(fd_len as _);
        let mut cmsg_buffer = CmsgBuffer::with_capacity(cmsg_capacity as _);
        let mut iovecs = Vec::with_capacity(out_data.len());
        for data in out_data {
            iovecs.push(iovec {
                iov_base: data.as_ptr() as *mut c_void,
                iov_len: data.size(),
            });
        }

        let mut msghdr = new_msghdr(&mut iovecs);
        if !out_fds.is_empty() {
            let cmsg = libc::cmsghdr {
                cmsg_len: libc::CMSG_LEN(fd_len as u32) as _,
                cmsg_level: SOL_SOCKET,
                cmsg_type: SCM_RIGHTS,
                #[cfg(all(target_env = "musl", target_pointer_width = "64"))]
                __pad1: 0,
            };
            // Safe because cmsg_buffer was allocated to be large enough to contain cmsghdr.
            ptr::write_unaligned(cmsg_buffer.as_mut_ptr(), cmsg);
            // Safe because the cmsg_buffer was allocated to be large enough to hold out_fds.len()
            // file descriptors.
            ptr::copy_nonoverlapping(
                out_fds.as_ptr(),
                libc::CMSG_DATA(cmsg_buffer.as_mut_ptr()) as *mut _,
                out_fds.len(),
            );

            msghdr.msg_control = cmsg_buffer.as_mut_ptr() as *mut _;
            set_msg_controllen(&mut msghdr, cmsg_capacity as _);
        }

        // Safe because the msghdr was properly constructed from valid (or null) pointers of the
        // indicated length and we check the return value.

        #[cfg(target_os = "linux")]
        let write_count = sendmsg(fd, &msghdr, MSG_NOSIGNAL);
        #[cfg(target_os = "macos")]
        let write_count = sendmsg(fd, &msghdr, 0);

        if write_count == -1 {
            Err(SysError::last())
        } else {
            Ok(write_count as usize)
        }
    }
}

// fn raw_sendmsg<D: IntoIovec>(fd: RawFd, out_data: &[D], out_fds: &[RawFd]) -> SysResult<usize> {
//     let cmsg_capacity = CMSG_SPACE!(size_of::<RawFd>() * out_fds.len());
//     let mut cmsg_buffer = CmsgBuffer::with_capacity(cmsg_capacity);

//     let mut iovecs = Vec::with_capacity(out_data.len());
//     for data in out_data {
//         iovecs.push(iovec {
//             iov_base: data.as_ptr() as *mut c_void,
//             iov_len: data.size(),
//         });
//     }

//     let mut msg = new_msghdr(&mut iovecs);

//     if !out_fds.is_empty() {
//         let cmsg = cmsghdr {
//             cmsg_len: unsafe { libc::CMSG_LEN(out_fds.len() as u32) as _ }, //CMSG_LEN!(size_of::<RawFd>() * out_fds.len()) as _,
//             cmsg_level: SOL_SOCKET,
//             cmsg_type: SCM_RIGHTS,
//             #[cfg(all(target_env = "musl", target_pointer_width = "64"))]
//             __pad1: 0,
//         };
//         unsafe {
//             // Safe because cmsg_buffer was allocated to be large enough to contain cmsghdr.
//             write_unaligned(cmsg_buffer.as_mut_ptr() as *mut cmsghdr, cmsg);
//             // Safe because the cmsg_buffer was allocated to be large enough to hold out_fds.len()
//             // file descriptors.

//             copy_nonoverlapping(
//                 out_fds.as_ptr(),
//                 CMSG_DATA(cmsg_buffer.as_mut_ptr()),
//                 out_fds.len(),
//             );
//         }

//         msg.msg_control = cmsg_buffer.as_mut_ptr() as *mut c_void;
//         set_msg_controllen(&mut msg, cmsg_capacity);
//     }

//     // Safe because the msghdr was properly constructed from valid (or null) pointers of the
//     // indicated length and we check the return value.
//     #[cfg(target_os = "linux")]
//     let write_count = unsafe { sendmsg(fd, &msg, MSG_NOSIGNAL) };
//     #[cfg(target_os = "macos")]
//     let write_count = unsafe { sendmsg(fd, &msg, 0) };

//     if write_count == -1 {
//         Err(SysError::last())
//     } else {
//         Ok(write_count as usize)
//     }
// }

unsafe fn raw_recvmsg(
    fd: RawFd,
    iovecs: &mut [iovec],
    in_fds: &mut [RawFd],
) -> SysResult<(usize, usize)> {
    let fd_length = FD_LENGTH!(in_fds.len());
    let cmsg_capacity = libc::CMSG_SPACE(fd_length as _) as usize;
    let mut cmsg_buffer = CmsgBuffer::with_capacity(cmsg_capacity);
    let mut msg = new_msghdr(iovecs);

    if !in_fds.is_empty() {
        // MSG control len is size_of(cmsghdr) + size_of(RawFd) * in_fds.len().
        msg.msg_control = cmsg_buffer.as_mut_ptr() as *mut c_void;
        set_msg_controllen(&mut msg, cmsg_capacity);
    }

    // Safe because the msghdr was properly constructed from valid (or null) pointers of the
    // indicated length and we check the return value.
    // TODO: Should we handle MSG_TRUNC in a specific way?
    let total_read = recvmsg(fd, &mut msg, 0);
    if total_read == -1 {
        return Err(SysError::last());
    }

    if total_read == 0 && (msg.msg_controllen as usize) < size_of::<cmsghdr>() {
        return Ok((0, 0));
    }

    // Reference to a memory area with a CmsgBuffer, which contains a `cmsghdr` struct followed
    // by a sequence of `in_fds.len()` count RawFds.
    let mut cmsg_ptr = msg.msg_control as *mut cmsghdr;
    let mut copied_fds_count = 0;
    // If the control data was truncated, then this might be a sign of incorrect communication
    // protocol. If MSG_CTRUNC was set we must close the fds from the control data.
    let mut teardown_control_data = msg.msg_flags & libc::MSG_CTRUNC != 0;

    while !cmsg_ptr.is_null() {
        // Safe because we checked that cmsg_ptr was non-null, and the loop is constructed such
        // that it only happens when there is at least sizeof(cmsghdr) space after the pointer to
        // read.
        let cmsg = (cmsg_ptr as *mut cmsghdr).read_unaligned();
        if cmsg.cmsg_level == SOL_SOCKET && cmsg.cmsg_type == SCM_RIGHTS {
            // the type cmsg_len for musl target_env is u32 while usize for gnu taget_env, so to
            // make it easier to handle the type conversion, we will convert cmsg_len to usize in
            // all target
            let fds_count =
                (cmsg.cmsg_len as usize - libc::CMSG_LEN(0) as usize) / size_of::<RawFd>();
            // The sender can transmit more data than we can buffer. If a message is too long to
            // fit in the supplied buffer, excess bytes may be discarded depending on the type of
            // socket the message is received from.
            let fds_to_be_copied_count = std::cmp::min(in_fds.len() - copied_fds_count, fds_count);
            teardown_control_data |= fds_count > fds_to_be_copied_count;
            if teardown_control_data {
                // Allocating space for cmesg buffer might provide extra space for fds, due to
                // alignment. If these fds can not be stored in `in_fds` buffer, then all the control
                // data must be dropped to insufficient buffer space for returning them to outer
                // scope. This might be a sign of incorrect protocol communication.
                for fd_offset in 0..fds_count {
                    let raw_fds_ptr = CMSG_DATA(cmsg_ptr);
                    // The cmsg_ptr is valid here because is checked at the beginning of the
                    // loop and it is assured to have `fds_count` fds available.
                    let raw_fd = *(raw_fds_ptr.wrapping_add(fd_offset)) as libc::c_int;
                    libc::close(raw_fd);
                }
            } else {
                // Safe because `cmsg_ptr` is checked against null and we copy from `cmesg_buffer` to
                // `in_fds` according to their current capacity.
                ptr::copy_nonoverlapping(
                    CMSG_DATA(cmsg_ptr),
                    in_fds[copied_fds_count..(copied_fds_count + fds_to_be_copied_count)]
                        .as_mut_ptr(),
                    fds_to_be_copied_count,
                );

                copied_fds_count += fds_to_be_copied_count;
            }
        }

        // Remove the previously copied fds.
        if teardown_control_data {
            for fd in in_fds.iter().take(copied_fds_count) {
                // This is safe because we close only the previously copied fds. We do not care
                // about `close` return code.
                libc::close(*fd);
            }

            return Err(SysError::new(libc::ENOBUFS));
        }

        cmsg_ptr = libc::CMSG_NXTHDR(&msg, cmsg_ptr); //get_next_cmsg(&msg, &cmsg, cmsg_ptr);
    }

    Ok((total_read as usize, copied_fds_count))
}

impl ScmSocket for UnixDatagram {
    fn socket_fd(&self) -> RawFd {
        self.as_raw_fd()
    }
}

impl ScmSocket for UnixStream {
    fn socket_fd(&self) -> RawFd {
        self.as_raw_fd()
    }
}

/// Trait for types that can be converted into an `iovec` that can be referenced by a syscall for
/// the lifetime of this object.
///
/// # Safety
///
/// This is marked unsafe because the implementation must ensure that the returned pointer and size
/// is valid and that the lifetime of the returned pointer is at least that of the trait object.
pub unsafe trait IntoIovec {
    /// Gets the base pointer of this `iovec`.
    fn as_ptr(&self) -> *const libc::c_void;

    /// Gets the size in bytes of this `iovec`.
    fn size(&self) -> usize;
}

// Safe because this slice can not have another mutable reference and it's pointer and size are
// guaranteed to be valid.
unsafe impl<'a> IntoIovec for &'a [u8] {
    // Clippy false positive: https://github.com/rust-lang/rust-clippy/issues/3480
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::useless_asref))]
    fn as_ptr(&self) -> *const libc::c_void {
        self.as_ref().as_ptr() as *const libc::c_void
    }

    fn size(&self) -> usize {
        self.len()
    }
}

/// Trait for file descriptors can send and receive socket control messages via `sendmsg` and
/// `recvmsg`.
pub trait ScmSocket {
    /// Gets the file descriptor of this socket.
    fn socket_fd(&self) -> RawFd;

    /// Sends the given data and file descriptor over the socket.
    ///
    /// On success, returns the number of bytes sent.
    ///
    /// # Arguments
    ///
    /// * `buf` - A buffer of data to send on the `socket`.
    /// * `fd` - A file descriptors to be sent.
    fn send_with_fd<D: IntoIovec>(&self, buf: D, fd: RawFd) -> SysResult<usize> {
        self.send_with_fds(&[buf], &[fd])
    }

    /// Sends the given data and file descriptors over the socket.
    ///
    /// On success, returns the number of bytes sent.
    ///
    /// # Arguments
    ///
    /// * `bufs` - A list of data buffer to send on the `socket`.
    /// * `fds` - A list of file descriptors to be sent.
    fn send_with_fds<D: IntoIovec>(&self, bufs: &[D], fds: &[RawFd]) -> SysResult<usize> {
        raw_sendmsg(self.socket_fd(), bufs, fds)
    }

    /// Receives data and potentially a file descriptor from the socket.
    ///
    /// On success, returns the number of bytes and an optional file descriptor.
    ///
    /// # Arguments
    ///
    /// * `buf` - A buffer to receive data from the socket.
    fn recv_with_fd(&self, buf: &mut [u8]) -> SysResult<(usize, Option<File>)> {
        let mut fd = [0];
        let mut iovecs = [libc::iovec {
            iov_base: buf.as_mut_ptr() as *mut libc::c_void,
            iov_len: buf.len(),
        }];

        // Safe because we have mutably borrowed buf and it's safe to write arbitrary data
        // to a slice.
        let (read_count, fd_count) = unsafe { self.recv_with_fds(&mut iovecs[..], &mut fd)? };
        let file = if fd_count == 0 {
            None
        } else {
            // Safe because the first fd from recv_with_fds is owned by us and valid because this
            // branch was taken.
            Some(unsafe { File::from_raw_fd(fd[0]) })
        };
        Ok((read_count, file))
    }

    /// Receives data and file descriptors from the socket.
    ///
    /// On success, returns the number of bytes and file descriptors received as a tuple
    /// `(bytes count, files count)`.
    ///
    /// # Arguments
    ///
    /// * `iovecs` - A list of iovec to receive data from the socket.
    /// * `fds` - A slice of `RawFd`s to put the received file descriptors into. On success, the
    ///           number of valid file descriptors is indicated by the second element of the
    ///           returned tuple. The caller owns these file descriptors, but they will not be
    ///           closed on drop like a `File`-like type would be. It is recommended that each valid
    ///           file descriptor gets wrapped in a drop type that closes it after this returns.
    ///
    /// # Safety
    ///
    /// It is the callers responsibility to ensure it is safe for arbitrary data to be
    /// written to the iovec pointers.
    unsafe fn recv_with_fds(
        &self,
        iovecs: &mut [libc::iovec],
        fds: &mut [RawFd],
    ) -> SysResult<(usize, usize)> {
        raw_recvmsg(self.socket_fd(), iovecs, fds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::{Read, Seek, SeekFrom, Write};
    use std::mem::size_of;
    use std::os::raw::c_long;
    use std::os::unix::net::UnixDatagram;

    use libc::cmsghdr;
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn buffer_len() {
        assert_eq!(CMSG_SPACE!(0), size_of::<cmsghdr>());
        assert_eq!(
            CMSG_SPACE!(size_of::<RawFd>()),
            size_of::<cmsghdr>() + size_of::<c_long>()
        );
        if size_of::<RawFd>() == 4 {
            assert_eq!(
                CMSG_SPACE!(2 * size_of::<RawFd>()),
                size_of::<cmsghdr>() + size_of::<c_long>()
            );
            assert_eq!(
                CMSG_SPACE!(3 * size_of::<RawFd>()),
                size_of::<cmsghdr>() + size_of::<c_long>() * 2
            );
            assert_eq!(
                CMSG_SPACE!(4 * size_of::<RawFd>()),
                size_of::<cmsghdr>() + size_of::<c_long>() * 2
            );
        } else if size_of::<RawFd>() == 8 {
            assert_eq!(
                CMSG_SPACE!(2 * size_of::<RawFd>()),
                size_of::<cmsghdr>() + size_of::<c_long>() * 2
            );
            assert_eq!(
                CMSG_SPACE!(3 * size_of::<RawFd>()),
                size_of::<cmsghdr>() + size_of::<c_long>() * 3
            );
            assert_eq!(
                CMSG_SPACE!(4 * size_of::<RawFd>()),
                size_of::<cmsghdr>() + size_of::<c_long>() * 4
            );
        }
    }

    #[test]
    fn send_recv_no_fd() {
        let (s1, s2) = UnixDatagram::pair().expect("failed to create socket pair");

        let write_count = s1
            .send_with_fds(&[[1u8, 1, 2].as_ref(), [21u8, 34, 55].as_ref()], &[])
            .expect("failed to send data");

        assert_eq!(write_count, 6);

        let mut buf = [0u8; 6];
        let mut files = [0; 1];
        let mut iovecs = [iovec {
            iov_base: buf.as_mut_ptr() as *mut c_void,
            iov_len: buf.len(),
        }];
        let (read_count, file_count) = unsafe {
            s2.recv_with_fds(&mut iovecs[..], &mut files)
                .expect("failed to recv data")
        };

        assert_eq!(read_count, 6);
        assert_eq!(file_count, 0);
        assert_eq!(buf, [1, 1, 2, 21, 34, 55]);
    }

    #[test]
    fn send_recv_only_fd() {
        let (s1, s2) = UnixDatagram::pair().expect("failed to create socket pair");

        let mut file1 = TempFile::new().unwrap().into_file();
        file1.write_all(b"foo").unwrap();
        file1.seek(SeekFrom::Start(0)).unwrap();

        let write_count = s1
            .send_with_fd([].as_ref(), file1.as_raw_fd())
            .expect("failed to send fd");

        assert_eq!(write_count, 0);

        let (read_count, file_opt) = s2.recv_with_fd(&mut []).expect("failed to recv fd");

        let mut file = file_opt.unwrap();

        assert_eq!(read_count, 0);
        assert!(file.as_raw_fd() >= 0);
        assert_ne!(file.as_raw_fd(), s1.as_raw_fd());
        assert_ne!(file.as_raw_fd(), s2.as_raw_fd());
        assert_ne!(file.as_raw_fd(), file1.as_raw_fd());

        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();
        assert_eq!("foo".to_string(), buf);
    }

    #[test]
    fn send_recv_with_fd() {
        let (s1, s2) = UnixDatagram::pair().expect("failed to create socket pair");

        let mut file1 = TempFile::new().unwrap().into_file();
        file1.write_all(b"foo").unwrap();
        file1.seek(SeekFrom::Start(0)).unwrap();

        let write_count = s1
            .send_with_fds(&[[237].as_ref()], &[file1.as_raw_fd()])
            .expect("failed to send fd");

        assert_eq!(write_count, 1);

        let mut files = [0; 2];
        let mut buf = [0u8];
        let mut iovecs = [iovec {
            iov_base: buf.as_mut_ptr() as *mut c_void,
            iov_len: buf.len(),
        }];
        let (read_count, file_count) = unsafe {
            s2.recv_with_fds(&mut iovecs[..], &mut files)
                .expect("failed to recv fd")
        };

        assert_eq!(read_count, 1);
        assert_eq!(buf[0], 237);
        assert_eq!(file_count, 1);
        assert!(files[0] >= 0);
        assert_ne!(files[0], s1.as_raw_fd());
        assert_ne!(files[0], s2.as_raw_fd());
        assert_ne!(files[0], file1.as_raw_fd());

        let mut file = unsafe { File::from_raw_fd(files[0]) };
        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();
        assert_eq!("foo".to_string(), buf);
        assert_ne!("bar".to_string(), buf);
    }

    #[test]
    // Exercise the code paths that activate the issue of receiving the all the ancillary data,
    // but missing to provide enough buffer space to store it.
    fn send_more_recv_less1() {
        let (s1, s2) = UnixDatagram::pair().expect("failed to create socket pair");

        let mut file1 = TempFile::new().unwrap().into_file();
        let mut file2 = TempFile::new().unwrap().into_file();
        let mut file3 = TempFile::new().unwrap().into_file();
        let mut file4 = TempFile::new().unwrap().into_file();
        file1.write_all(b"foo").unwrap();
        file1.seek(SeekFrom::Start(0)).unwrap();
        file2.write_all(b"bar").unwrap();
        file2.seek(SeekFrom::Start(0)).unwrap();
        file3.write_all(b"foobar").unwrap();
        file3.seek(SeekFrom::Start(0)).unwrap();
        file4.write_all(b"foobarfoo").unwrap();
        file4.seek(SeekFrom::Start(0)).unwrap();
        let write_count = s1
            .send_with_fds(
                &[[237].as_ref()],
                &[
                    file1.as_raw_fd(),
                    file2.as_raw_fd(),
                    file3.as_raw_fd(),
                    file4.as_raw_fd(),
                ],
            )
            .expect("failed to send fd");

        assert_eq!(write_count, 1);

        let mut files = [0; 2];
        let mut buf = [0u8];
        let mut iovecs = [iovec {
            iov_base: buf.as_mut_ptr() as *mut c_void,
            iov_len: buf.len(),
        }];
        assert!(unsafe { s2.recv_with_fds(&mut iovecs[..], &mut files).is_err() });
    }

    // Exercise the code paths that activate the issue of receiving part of the sent ancillary
    // data due to insufficient buffer space, activating `msg_flags` `MSG_CTRUNC` flag.
    #[test]
    fn send_more_recv_less2() {
        let (s1, s2) = UnixDatagram::pair().expect("failed to create socket pair");

        let mut file1 = TempFile::new().unwrap().into_file();
        let mut file2 = TempFile::new().unwrap().into_file();
        let mut file3 = TempFile::new().unwrap().into_file();
        let mut file4 = TempFile::new().unwrap().into_file();
        file1.write_all(b"foo").unwrap();
        file1.seek(SeekFrom::Start(0)).unwrap();
        file2.write_all(b"bar").unwrap();
        file2.seek(SeekFrom::Start(0)).unwrap();
        file3.write_all(b"foobar").unwrap();
        file3.seek(SeekFrom::Start(0)).unwrap();
        file4.write_all(b"foobarfoo").unwrap();
        file4.seek(SeekFrom::Start(0)).unwrap();
        let write_count = s1
            .send_with_fds(
                &[[237].as_ref()],
                &[
                    file1.as_raw_fd(),
                    file2.as_raw_fd(),
                    file3.as_raw_fd(),
                    file4.as_raw_fd(),
                ],
            )
            .expect("failed to send fd");

        assert_eq!(write_count, 1);

        let mut files = [0; 1];
        let mut buf = [0u8];
        let mut iovecs = [iovec {
            iov_base: buf.as_mut_ptr() as *mut c_void,
            iov_len: buf.len(),
        }];
        assert!(unsafe { s2.recv_with_fds(&mut iovecs[..], &mut files).is_err() });
    }
}
