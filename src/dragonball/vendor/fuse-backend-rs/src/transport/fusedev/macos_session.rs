// Copyright 2020-2022 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! FUSE session management.
//!
//! A FUSE channel is a FUSE request handling context that takes care of handling FUSE requests
//! sequentially. A FUSE session is a connection from a FUSE mountpoint to a FUSE server daemon.
//! A FUSE session can have multiple FUSE channels so that FUSE requests are handled in parallel.

use core_foundation_sys::base::{CFAllocatorRef, CFIndex, CFRelease};
use core_foundation_sys::string::{kCFStringEncodingUTF8, CFStringCreateWithBytes};
use core_foundation_sys::url::{kCFURLPOSIXPathStyle, CFURLCreateWithFileSystemPath, CFURLRef};
use std::ffi::CString;
use std::fs::File;
use std::io::IoSliceMut;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};

use libc::{c_void, proc_pidpath, PROC_PIDPATHINFO_MAXSIZE};
use nix::errno::Errno;
use nix::fcntl::{fcntl, FdFlag, F_SETFD};
use nix::sys::signal::{signal, SigHandler, Signal};
use nix::sys::socket::{
    recvmsg, socketpair, AddressFamily, ControlMessageOwned, MsgFlags, RecvMsg, SockFlag, SockType,
    UnixAddr,
};
use nix::unistd::{close, execv, fork, getpid, read, ForkResult};
use nix::{cmsg_space, NixPath};

use super::{Error::IoError, Error::SessionFailure, FuseBuf, FuseDevWriter, Reader, Result};
use crate::transport::pagesize;

// These follows definition from libfuse.
const FUSE_KERN_BUF_SIZE: usize = 256;
const FUSE_HEADER_SIZE: usize = 0x1000;

const OSXFUSE_MOUNT_PROG: &str = "/Library/Filesystems/macfuse.fs/Contents/Resources/mount_macfuse";

static K_DADISK_UNMOUNT_OPTION_FORCE: u64 = 524288;

#[repr(C)]
struct __DADisk(c_void);
type DADiskRef = *const __DADisk;
#[repr(C)]
struct __DADissenter(c_void);
type DADissenterRef = *const __DADissenter;
#[repr(C)]
struct __DASession(c_void);
type DASessionRef = *const __DASession;

type DADiskUnmountCallback = ::std::option::Option<
    unsafe extern "C" fn(disk: DADiskRef, dissenter: DADissenterRef, context: *mut c_void),
>;

extern "C" {
    fn DADiskUnmount(
        disk: DADiskRef,
        options: u64,
        callback: DADiskUnmountCallback,
        context: *mut c_void,
    );
    fn DADiskCreateFromVolumePath(
        allocator: CFAllocatorRef,
        session: DASessionRef,
        path: CFURLRef,
    ) -> DADiskRef;
    fn DASessionCreate(allocator: CFAllocatorRef) -> DASessionRef;
}

mod ioctl {
    use nix::ioctl_write_ptr;

    // #define FUSEDEVIOCSETDAEMONDEAD _IOW('F', 3,  u_int32_t)
    const FUSE_FD_DEAD_MAGIC: u8 = b'F';
    const FUSE_FD_DEAD: u8 = 3;
    ioctl_write_ptr!(set_fuse_fd_dead, FUSE_FD_DEAD_MAGIC, FUSE_FD_DEAD, u32);
}

/// A fuse session manager to manage the connection with the in kernel fuse driver.
pub struct FuseSession {
    mountpoint: PathBuf,
    fsname: String,
    subtype: String,
    file: Option<File>,
    bufsize: usize,
    disk: Arc<Mutex<Option<DADiskRef>>>,
    dasession: Arc<AtomicPtr<c_void>>,
    readonly: bool,
}

unsafe impl Send for FuseSession {}

impl FuseSession {
    /// Create a new fuse session, without mounting/connecting to the in kernel fuse driver.
    pub fn new(
        mountpoint: &Path,
        fsname: &str,
        subtype: &str,
        readonly: bool,
    ) -> Result<FuseSession> {
        let dest = mountpoint
            .canonicalize()
            .map_err(|_| SessionFailure(format!("invalid mountpoint {:?}", mountpoint)))?;
        if !dest.is_dir() {
            return Err(SessionFailure(format!("{:?} is not a directory", dest)));
        }

        Ok(FuseSession {
            mountpoint: dest,
            fsname: fsname.to_owned(),
            subtype: subtype.to_owned(),
            file: None,
            bufsize: FUSE_KERN_BUF_SIZE * pagesize() + FUSE_HEADER_SIZE,
            disk: Arc::new(Mutex::new(None)),
            dasession: Arc::new(AtomicPtr::new(unsafe {
                DASessionCreate(std::ptr::null()) as *mut c_void
            })),
            readonly,
        })
    }

    /// Mount the fuse mountpoint, building connection with the in kernel fuse driver.
    pub fn mount(&mut self) -> Result<()> {
        let mut disk = self.disk.lock().expect("lock disk failed");
        let file = fuse_kern_mount(&self.mountpoint, &self.fsname, &self.subtype, self.readonly)?;
        let session = self.dasession.load(Ordering::SeqCst);
        let mount_disk = create_disk(&self.mountpoint, session as DASessionRef);
        self.file = Some(file);
        *disk = Some(mount_disk);

        Ok(())
    }

    /// Expose the associated FUSE session file.
    pub fn get_fuse_file(&mut self) -> Option<&File> {
        self.file.as_ref()
    }

    /// Force setting the associated FUSE session file.
    pub fn set_fuse_file(&mut self, file: File) {
        self.file = Some(file);
    }

    /// Destroy a fuse session.
    pub fn umount(&mut self) -> Result<()> {
        if let Some(file) = self.file.take() {
            if self.mountpoint.to_str().is_some() {
                let mut disk = self.disk.lock().expect("lock disk failed");
                fuse_kern_umount(file, disk.take())
            } else {
                Err(SessionFailure("invalid mountpoint".to_string()))
            }
        } else {
            Ok(())
        }
    }

    /// Get the mountpoint of the session.
    pub fn mountpoint(&self) -> &Path {
        &self.mountpoint
    }

    /// Get the file system name of the session.
    pub fn fsname(&self) -> &str {
        &self.fsname
    }

    /// Get the subtype of the session.
    pub fn subtype(&self) -> &str {
        &self.subtype
    }

    /// Get the default buffer size of the session.
    pub fn bufsize(&self) -> usize {
        self.bufsize
    }

    /// Create a new fuse message channel.
    pub fn new_channel(&self) -> Result<FuseChannel> {
        if let Some(file) = &self.file {
            let file = file
                .try_clone()
                .map_err(|e| SessionFailure(format!("dup fd: {}", e)))?;
            FuseChannel::new(file, self.bufsize)
        } else {
            Err(SessionFailure("invalid fuse session".to_string()))
        }
    }

    /// Wake channel loop
    /// After macfuse unmount, read will throw ENODEV
    /// So wakers is no need for macfuse to interrupt channel
    pub fn wake(&self) -> Result<()> {
        Ok(())
    }
}

impl Drop for FuseSession {
    fn drop(&mut self) {
        let _ = self.umount();
    }
}

/// A fuse channel abstruction. Each session can hold multiple channels.
pub struct FuseChannel {
    file: File,
    buf: Vec<u8>,
}

impl FuseChannel {
    fn new(file: File, bufsize: usize) -> Result<Self> {
        Ok(FuseChannel {
            file,
            buf: vec![0x0u8; bufsize],
        })
    }

    /// Get next available FUSE request from the underlying fuse device file.
    ///
    /// Returns:
    /// - Ok(None): signal has pending on the exiting event channel
    /// - Ok(Some((reader, writer))): reader to receive request and writer to send reply
    /// - Err(e): error message
    pub fn get_request(&mut self) -> Result<Option<(Reader, FuseDevWriter)>> {
        let fd = self.file.as_raw_fd();
        loop {
            match read(fd, &mut self.buf) {
                Ok(len) => {
                    // ###############################################
                    // Note: it's a heavy hack to reuse the same underlying data
                    // buffer for both Reader and Writer, in order to reduce memory
                    // consumption. Here we assume Reader won't be used anymore once
                    // we start to write to the Writer. To get rid of this hack,
                    // just allocate a dedicated data buffer for Writer.
                    let buf = unsafe {
                        std::slice::from_raw_parts_mut(self.buf.as_mut_ptr(), self.buf.len())
                    };
                    // Reader::new() and Writer::new() should always return success.
                    let reader =
                        Reader::from_fuse_buffer(FuseBuf::new(&mut self.buf[..len])).unwrap();
                    let writer = FuseDevWriter::new(fd, buf).unwrap();
                    return Ok(Some((reader, writer)));
                }
                Err(e) => match e {
                    Errno::ENOENT => {
                        // ENOENT means the operation was interrupted, it's safe
                        // to restart
                        trace!("restart reading");
                        continue;
                    }
                    Errno::EINTR => {
                        continue;
                    }
                    // EAGIN requires the caller to handle it, and the current implementation assumes that FD is blocking.
                    Errno::EAGAIN => {
                        return Err(IoError(e.into()));
                    }
                    Errno::ENODEV => {
                        info!("fuse filesystem umounted");
                        return Ok(None);
                    }
                    e => {
                        warn! {"read fuse dev failed on fd {}: {}", fd, e};
                        return Err(SessionFailure(format!("read new request: {:?}", e)));
                    }
                },
            }
        }
    }
}

/// Mount a fuse file system
fn receive_fd(sock_fd: RawFd) -> Result<RawFd> {
    let mut buffer = vec![0u8; 4];
    let mut cmsgspace = cmsg_space!(RawFd);
    let mut iov = [IoSliceMut::new(&mut buffer)];
    let r: RecvMsg<UnixAddr> =
        recvmsg(sock_fd, &mut iov, Some(&mut cmsgspace), MsgFlags::empty()).unwrap();
    if let Some(msg) = r.cmsgs().next() {
        match msg {
            ControlMessageOwned::ScmRights(fds) => {
                let fd = fds
                    .first()
                    .ok_or_else(|| SessionFailure(String::from("control msg has no fd")))?;
                return Ok(*fd);
            }
            _ => {
                return Err(SessionFailure(String::from("unknown msg from fd")));
            }
        }
    }
    Err(SessionFailure(String::from("not get fd")))
}

fn fuse_kern_mount(mountpoint: &Path, fsname: &str, subtype: &str, rd_only: bool) -> Result<File> {
    unsafe { signal(Signal::SIGCHLD, SigHandler::SigDfl) }
        .map_err(|e| SessionFailure(format!("fail to reset SIGCHLD handler{:?}", e)))?;

    let (fd0, fd1) = socketpair(
        AddressFamily::Unix,
        SockType::Stream,
        None,
        SockFlag::empty(),
    )
    .map_err(|e| SessionFailure(format!("create socket failed {:?}", e)))?;
    let file: File = unsafe {
        match fork().map_err(|e| SessionFailure(format!("fork mount_macfuse failed {:?}", e)))? {
            ForkResult::Parent { .. } => {
                close(fd0)
                    .map_err(|e| SessionFailure(format!("parent close fd0 failed {:?}", e)))?;
                let fd = receive_fd(fd1)?;
                File::from_raw_fd(fd)
            }
            ForkResult::Child => {
                close(fd1)
                    .map_err(|e| SessionFailure(format!("child close fd1 failed {:?}", e)))?;
                fcntl(fd0, F_SETFD(FdFlag::empty()))
                    .map_err(|e| SessionFailure(format!("child fcntl fd0 failed {:?}", e)))?;
                let mut daemon_path: Vec<u8> =
                    Vec::with_capacity(PROC_PIDPATHINFO_MAXSIZE as usize);
                if proc_pidpath(
                    getpid().as_raw(),
                    daemon_path.as_mut_ptr() as *mut libc::c_void,
                    PROC_PIDPATHINFO_MAXSIZE as u32,
                ) != 0
                {
                    let daemon_path = String::from_utf8(daemon_path)
                        .map_err(|e| SessionFailure(format!("get pid path failed {:?}", e)))?;
                    std::env::set_var("_FUSE_DAEMON_PATH", daemon_path);
                }
                std::env::set_var("_FUSE_COMMFD", format!("{}", fd0));
                std::env::set_var("_FUSE_COMMVERS", "2");
                std::env::set_var("_FUSE_CALL_BY_LIB", "1");

                // TODO impl -o
                let prog_path = CString::new(OSXFUSE_MOUNT_PROG).map_err(|e| {
                    SessionFailure(format!("create mount_macfuse cstring failed: {:?}", e))
                })?;
                let mountpoint = mountpoint.to_str().ok_or_else(|| {
                    SessionFailure(format!(
                        "convert mountpoint {:?} to string failed",
                        mountpoint
                    ))
                })?;
                let fsname_opt = format!("fsname={}", fsname);
                let subtype_opt = format!("subtype={}", subtype);
                let mut args: Vec<&str> = vec![
                    OSXFUSE_MOUNT_PROG,
                    "-o",
                    "nodev",
                    "-o",
                    "nosuid",
                    "-o",
                    "noatime",
                    "-o",
                    &fsname_opt,
                    "-o",
                    &subtype_opt,
                ];
                if rd_only {
                    args.push("-o");
                    args.push("-ro");
                }
                args.push(mountpoint);
                let mut c_args: Vec<CString> = Vec::with_capacity(args.len());
                for arg in args {
                    let c_arg = CString::new(String::from(arg)).map_err(|e| {
                        SessionFailure(format!("parse option {:?} to cstring failed {:?}", arg, e))
                    })?;
                    c_args.push(c_arg);
                }
                execv(&prog_path, &c_args)
                    .map_err(|e| SessionFailure(format!("exec mount_macfuse failed {:?}", e)))?;
                panic!("never arrive here")
            }
        }
    };
    Ok(file)
}

fn create_disk(mountpoint: &Path, dasession: DASessionRef) -> DADiskRef {
    unsafe {
        let path_len = mountpoint.len();
        let mountpoint = mountpoint.as_os_str().as_bytes();
        let mountpoint = mountpoint.as_ptr();
        let url_str = CFStringCreateWithBytes(
            std::ptr::null(),
            mountpoint,
            path_len as CFIndex,
            kCFStringEncodingUTF8,
            1u8,
        );
        let url =
            CFURLCreateWithFileSystemPath(std::ptr::null(), url_str, kCFURLPOSIXPathStyle, 1u8);
        let disk = DADiskCreateFromVolumePath(std::ptr::null(), dasession, url);
        CFRelease(std::mem::transmute(url_str));
        CFRelease(std::mem::transmute(url));
        disk
    }
}

/// Umount a fuse file system
fn fuse_kern_umount(file: File, disk: Option<DADiskRef>) -> Result<()> {
    if let Err(e) = set_fuse_fd_dead(file.as_raw_fd()) {
        return Err(SessionFailure(format!(
            "ioctl set fuse deamon dead failed: {}",
            e
        )));
    }
    drop(file);

    if let Some(disk) = disk {
        unsafe {
            DADiskUnmount(
                disk,
                K_DADISK_UNMOUNT_OPTION_FORCE,
                None,
                std::ptr::null_mut(),
            );
            CFRelease(std::mem::transmute(disk));
        }
    }
    Ok(())
}

fn set_fuse_fd_dead(fd: RawFd) -> std::io::Result<()> {
    unsafe {
        match ioctl::set_fuse_fd_dead(fd, &fd as *const i32 as *const u32) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::unix::io::FromRawFd;
    use std::path::Path;
    use vmm_sys_util::tempdir::TempDir;

    #[test]
    fn test_new_session() {
        let se = FuseSession::new(Path::new("haha"), "foo", "bar", true);
        assert!(se.is_err());

        let dir = TempDir::new().unwrap();
        let se = FuseSession::new(dir.as_path(), "foo", "bar", false);
        assert!(se.is_ok());
    }

    #[test]
    fn test_new_channel() {
        let ch = FuseChannel::new(unsafe { File::from_raw_fd(0) }, 3);
        assert!(ch.is_ok());
    }
}

#[cfg(feature = "async-io")]
pub use asyncio::FuseDevTask;

#[cfg(feature = "async-io")]
/// Task context to handle fuse request in asynchronous mode.
mod asyncio {
    use std::os::unix::io::RawFd;
    use std::sync::Arc;

    use crate::api::filesystem::AsyncFileSystem;
    use crate::api::server::Server;
    use crate::async_util::{AsyncDriver, AsyncExecutorState, AsyncUtil};
    use crate::transport::{FuseBuf, Reader, Writer};

    /// Task context to handle fuse request in asynchronous mode.
    ///
    /// This structure provides a context to handle fuse request in asynchronous mode, including
    /// the fuse fd, a internal buffer and a `Server` instance to serve requests.
    ///
    /// ## Examples
    /// ```ignore
    /// let buf_size = 0x1_0000;
    /// let state = AsyncExecutorState::new();
    /// let mut task = FuseDevTask::new(buf_size, fuse_dev_fd, fs_server, state.clone());
    ///
    /// // Run the task
    /// executor.spawn(async move { task.poll_handler().await });
    ///
    /// // Stop the task
    /// state.quiesce();
    /// ```
    pub struct FuseDevTask<F: AsyncFileSystem + Sync> {
        fd: RawFd,
        buf: Vec<u8>,
        state: AsyncExecutorState,
        server: Arc<Server<F>>,
    }

    impl<F: AsyncFileSystem + Sync> FuseDevTask<F> {
        /// Create a new fuse task context for asynchronous IO.
        ///
        /// # Parameters
        /// - buf_size: size of buffer to receive requests from/send reply to the fuse fd
        /// - fd: fuse device file descriptor
        /// - server: `Server` instance to serve requests from the fuse fd
        /// - state: shared state object to control the task object
        ///
        /// # Safety
        /// The caller must ensure `fd` is valid during the lifetime of the returned task object.
        pub fn new(
            buf_size: usize,
            fd: RawFd,
            server: Arc<Server<F>>,
            state: AsyncExecutorState,
        ) -> Self {
            FuseDevTask {
                fd,
                server,
                state,
                buf: vec![0x0u8; buf_size],
            }
        }

        /// Handler to process fuse requests in asynchronous mode.
        ///
        /// An async fn to handle requests from the fuse fd. It works in asynchronous IO mode when:
        /// - receiving request from fuse fd
        /// - handling requests by calling Server::async_handle_requests()
        /// - sending reply to fuse fd
        ///
        /// The async fn repeatedly return Poll::Pending when polled until the state has been set
        /// to quiesce mode.
        pub async fn poll_handler(&mut self) {
            // TODO: register self.buf as io uring buffers.
            let drive = AsyncDriver::default();

            while !self.state.quiescing() {
                let result = AsyncUtil::read(drive.clone(), self.fd, &mut self.buf, 0).await;
                match result {
                    Ok(len) => {
                        // ###############################################
                        // Note: it's a heavy hack to reuse the same underlying data
                        // buffer for both Reader and Writer, in order to reduce memory
                        // consumption. Here we assume Reader won't be used anymore once
                        // we start to write to the Writer. To get rid of this hack,
                        // just allocate a dedicated data buffer for Writer.
                        let buf = unsafe {
                            std::slice::from_raw_parts_mut(self.buf.as_mut_ptr(), self.buf.len())
                        };
                        // Reader::new() and Writer::new() should always return success.
                        let reader = Reader::new(FuseBuf::new(&mut self.buf[0..len])).unwrap();
                        let writer = Writer::new(self.fd, buf).unwrap();
                        let result = unsafe {
                            self.server
                                .async_handle_message(drive.clone(), reader, writer, None, None)
                                .await
                        };

                        if let Err(e) = result {
                            // TODO: error handling
                            error!("failed to handle fuse request, {}", e);
                        }
                    }
                    Err(e) => {
                        // TODO: error handling
                        error!("failed to read request from fuse device fd, {}", e);
                    }
                }
            }

            // TODO: unregister self.buf as io uring buffers.

            // Report that the task has been quiesced.
            self.state.report();
        }
    }

    impl<F: AsyncFileSystem + Sync> Clone for FuseDevTask<F> {
        fn clone(&self) -> Self {
            FuseDevTask {
                fd: self.fd,
                server: self.server.clone(),
                state: self.state.clone(),
                buf: vec![0x0u8; self.buf.capacity()],
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::io::AsRawFd;
        use std::sync::Arc;

        use super::*;
        use crate::api::server::Server;
        use crate::api::{Vfs, VfsOptions};
        use crate::async_util::{AsyncDriver, AsyncExecutor, AsyncExecutorState};

        #[test]
        fn test_fuse_task() {
            let state = AsyncExecutorState::new();
            let fs = Vfs::<AsyncDriver, ()>::new(VfsOptions::default());
            let _server = Arc::new(Server::<Vfs<AsyncDriver, ()>, AsyncDriver, ()>::new(fs));
            let file = vmm_sys_util::tempfile::TempFile::new().unwrap();
            let _fd = file.as_file().as_raw_fd();

            let mut executor = AsyncExecutor::new(32);
            executor.setup().unwrap();

            /*
            // Create three tasks, which could handle three concurrent fuse requests.
            let mut task = FuseDevTask::new(0x1000, fd, server.clone(), state.clone());
            executor
                .spawn(async move { task.poll_handler().await })
                .unwrap();
            let mut task = FuseDevTask::new(0x1000, fd, server.clone(), state.clone());
            executor
                .spawn(async move { task.poll_handler().await })
                .unwrap();
            let mut task = FuseDevTask::new(0x1000, fd, server.clone(), state.clone());
            executor
                .spawn(async move { task.poll_handler().await })
                .unwrap();
             */

            for _i in 0..10 {
                executor.run_once(false).unwrap();
            }

            // Set existing flag
            state.quiesce();
            // Close the fusedev fd, so all pending async io requests will be aborted.
            drop(file);

            for _i in 0..10 {
                executor.run_once(false).unwrap();
            }
        }
    }
}
