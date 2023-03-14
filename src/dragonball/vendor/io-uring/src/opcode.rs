//! Operation codes that can be used to construct [`squeue::Entry`](crate::squeue::Entry)s.

#![allow(clippy::new_without_default)]

#[cfg(feature = "unstable")]
use std::convert::TryInto;
use std::mem;
use std::os::unix::io::RawFd;

use crate::squeue::Entry;
#[cfg(feature = "unstable")]
use crate::squeue::Entry128;
use crate::sys;
use crate::types::{self, sealed};

macro_rules! assign_fd {
    ( $sqe:ident . fd = $opfd:expr ) => {
        match $opfd {
            sealed::Target::Fd(fd) => $sqe.fd = fd,
            sealed::Target::Fixed(i) => {
                $sqe.fd = i as _;
                $sqe.flags |= crate::squeue::Flags::FIXED_FILE.bits();
            }
        }
    };
}

macro_rules! opcode {
    (@type impl sealed::UseFixed ) => {
        sealed::Target
    };
    (@type impl sealed::UseFd ) => {
        RawFd
    };
    (@type $name:ty ) => {
        $name
    };
    (
        $( #[$outer:meta] )*
        pub struct $name:ident {
            $( #[$new_meta:meta] )*

            $( $field:ident : { $( $tnt:tt )+ } ),*

            $(,)?

            ;;

            $(
                $( #[$opt_meta:meta] )*
                $opt_field:ident : $opt_tname:ty = $default:expr
            ),*

            $(,)?
        }

        pub const CODE = $opcode:expr;

        $( #[$build_meta:meta] )*
        pub fn build($self:ident) -> $entry:ty $build_block:block
    ) => {
        $( #[$outer] )*
        pub struct $name {
            $( $field : opcode!(@type $( $tnt )*), )*
            $( $opt_field : $opt_tname, )*
        }

        #[allow(deprecated)]
        impl $name {
            $( #[$new_meta] )*
            #[inline]
            pub fn new($( $field : $( $tnt )* ),*) -> Self {
                $name {
                    $( $field: $field.into(), )*
                    $( $opt_field: $default, )*
                }
            }

            /// The opcode of the operation. This can be passed to
            /// [`Probe::is_supported`](crate::Probe::is_supported) to check if this operation is
            /// supported with the current kernel.
            pub const CODE: u8 = $opcode as _;

            $(
                $( #[$opt_meta] )*
                #[inline]
                pub const fn $opt_field(mut self, $opt_field: $opt_tname) -> Self {
                    self.$opt_field = $opt_field;
                    self
                }
            )*

            $( #[$build_meta] )*
            #[inline]
            pub fn build($self) -> $entry $build_block
        }
    }
}

/// inline zeroed to improve codegen
#[inline(always)]
fn sqe_zeroed() -> sys::io_uring_sqe {
    unsafe { std::mem::zeroed() }
}

opcode!(
    /// Do not perform any I/O.
    ///
    /// This is useful for testing the performance of the io_uring implementation itself.
    #[derive(Debug)]
    pub struct Nop { ;; }

    pub const CODE = sys::IORING_OP_NOP;

    pub fn build(self) -> Entry {
        let Nop {} = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = -1;
        Entry(sqe)
    }
);

opcode!(
    /// Vectored read, equivalent to `preadv2(2)`.
    #[derive(Debug)]
    pub struct Readv {
        fd: { impl sealed::UseFixed },
        iovec: { *const libc::iovec },
        len: { u32 },
        ;;
        ioprio: u16 = 0,
        offset64: libc::off64_t = 0,
        /// specified for read operations, contains a bitwise OR of per-I/O flags,
        /// as described in the `preadv2(2)` man page.
        rw_flags: types::RwFlags = 0
    }

    pub const CODE = sys::IORING_OP_READV;

    pub fn build(self) -> Entry {
        let Readv {
            fd,
            iovec, len, offset64,
            ioprio, rw_flags
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.ioprio = ioprio;
        sqe.__bindgen_anon_2.addr = iovec as _;
        sqe.len = len;
        sqe.__bindgen_anon_1.off = offset64 as _;
        sqe.__bindgen_anon_3.rw_flags = rw_flags;
        Entry(sqe)
    }
);

impl Readv {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// Vectored write, equivalent to `pwritev2(2)`.
    #[derive(Debug)]
    pub struct Writev {
        fd: { impl sealed::UseFixed },
        iovec: { *const libc::iovec },
        len: { u32 },
        ;;
        ioprio: u16 = 0,
        offset64: libc::off64_t = 0,
        /// specified for write operations, contains a bitwise OR of per-I/O flags,
        /// as described in the `preadv2(2)` man page.
        rw_flags: types::RwFlags = 0
    }

    pub const CODE = sys::IORING_OP_WRITEV;

    pub fn build(self) -> Entry {
        let Writev {
            fd,
            iovec, len, offset64,
            ioprio, rw_flags
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.ioprio = ioprio;
        sqe.__bindgen_anon_2.addr = iovec as _;
        sqe.len = len;
        sqe.__bindgen_anon_1.off = offset64 as _;
        sqe.__bindgen_anon_3.rw_flags = rw_flags;
        Entry(sqe)
    }
);

impl Writev {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// File sync, equivalent to `fsync(2)`.
    ///
    /// Note that, while I/O is initiated in the order in which it appears in the submission queue,
    /// completions are unordered. For example, an application which places a write I/O followed by
    /// an fsync in the submission queue cannot expect the fsync to apply to the write. The two
    /// operations execute in parallel, so the fsync may complete before the write is issued to the
    /// storage. The same is also true for previously issued writes that have not completed prior to
    /// the fsync.
    #[derive(Debug)]
    pub struct Fsync {
        fd: { impl sealed::UseFixed },
        ;;
        /// The `flags` bit mask may contain either 0, for a normal file integrity sync,
        /// or [types::FsyncFlags::DATASYNC] to provide data sync only semantics.
        /// See the descriptions of `O_SYNC` and `O_DSYNC` in the `open(2)` manual page for more information.
        flags: types::FsyncFlags = types::FsyncFlags::empty()
    }

    pub const CODE = sys::IORING_OP_FSYNC;

    pub fn build(self) -> Entry {
        let Fsync { fd, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_3.fsync_flags = flags.bits();
        Entry(sqe)
    }
);

opcode!(
    /// Read from pre-mapped buffers that have been previously registered with
    /// [`Submitter::register_buffers`](crate::Submitter::register_buffers).
    ///
    /// The return values match those documented in the `preadv2(2)` man pages.
    #[derive(Debug)]
    pub struct ReadFixed {
        /// The `buf_index` is an index into an array of fixed buffers,
        /// and is only valid if fixed buffers were registered.
        fd: { impl sealed::UseFixed },
        buf: { *mut u8 },
        len: { u32 },
        buf_index: { u16 },
        ;;
        offset64: libc::off64_t = 0,
        ioprio: u16 = 0,
        /// specified for read operations, contains a bitwise OR of per-I/O flags,
        /// as described in the `preadv2(2)` man page.
        rw_flags: types::RwFlags = 0
    }

    pub const CODE = sys::IORING_OP_READ_FIXED;

    pub fn build(self) -> Entry {
        let ReadFixed {
            fd,
            buf, len, offset64,
            buf_index,
            ioprio, rw_flags
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.ioprio = ioprio;
        sqe.__bindgen_anon_2.addr = buf as _;
        sqe.len = len;
        sqe.__bindgen_anon_1.off = offset64 as _;
        sqe.__bindgen_anon_3.rw_flags = rw_flags;
        sqe.__bindgen_anon_4.buf_index = buf_index;
        Entry(sqe)
    }
);

impl ReadFixed {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// Write to pre-mapped buffers that have been previously registered with
    /// [`Submitter::register_buffers`](crate::Submitter::register_buffers).
    ///
    /// The return values match those documented in the `pwritev2(2)` man pages.
    #[derive(Debug)]
    pub struct WriteFixed {
        /// The `buf_index` is an index into an array of fixed buffers,
        /// and is only valid if fixed buffers were registered.
        fd: { impl sealed::UseFixed },
        buf: { *const u8 },
        len: { u32 },
        buf_index: { u16 },
        ;;
        ioprio: u16 = 0,
        offset64: libc::off64_t = 0,
        /// specified for write operations, contains a bitwise OR of per-I/O flags,
        /// as described in the `preadv2(2)` man page.
        rw_flags: types::RwFlags = 0
    }

    pub const CODE = sys::IORING_OP_WRITE_FIXED;

    pub fn build(self) -> Entry {
        let WriteFixed {
            fd,
            buf, len, offset64,
            buf_index,
            ioprio, rw_flags
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.ioprio = ioprio;
        sqe.__bindgen_anon_2.addr = buf as _;
        sqe.len = len;
        sqe.__bindgen_anon_1.off = offset64 as _;
        sqe.__bindgen_anon_3.rw_flags = rw_flags;
        sqe.__bindgen_anon_4.buf_index = buf_index;
        Entry(sqe)
    }
);

impl WriteFixed {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// Poll the specified fd.
    ///
    /// Unlike poll or epoll without `EPOLLONESHOT`, this interface always works in one shot mode.
    /// That is, once the poll operation is completed, it will have to be resubmitted.
    #[derive(Debug)]
    pub struct PollAdd {
        /// The bits that may be set in `flags` are defined in `<poll.h>`,
        /// and documented in `poll(2)`.
        fd: { impl sealed::UseFixed },
        flags: { u32 }
        ;;
    }

    pub const CODE = sys::IORING_OP_POLL_ADD;

    pub fn build(self) -> Entry {
        let PollAdd { fd, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);

        #[cfg(target_endian = "little")] {
            sqe.__bindgen_anon_3.poll32_events = flags;
        }

        #[cfg(target_endian = "big")] {
            let x = flags << 16;
            let y = flags >> 16;
            let flags = x | y;
            sqe.__bindgen_anon_3.poll32_events = flags;
        }

        Entry(sqe)
    }
);

opcode!(
    /// Remove an existing [poll](PollAdd) request.
    ///
    /// If found, the `result` method of the `cqueue::Entry` will return 0.
    /// If not found, `result` will return `-libc::ENOENT`.
    #[derive(Debug)]
    pub struct PollRemove {
        user_data: { u64 }
        ;;
    }

    pub const CODE = sys::IORING_OP_POLL_REMOVE;

    pub fn build(self) -> Entry {
        let PollRemove { user_data } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = -1;
        sqe.__bindgen_anon_2.addr = user_data as _;
        Entry(sqe)
    }
);

opcode!(
    /// Sync a file segment with disk, equivalent to `sync_file_range(2)`.
    #[derive(Debug)]
    pub struct SyncFileRange {
        fd: { impl sealed::UseFixed },
        len: { u32 },
        ;;
        /// the offset method holds the offset in bytes
        offset: libc::off64_t = 0,
        /// the flags method holds the flags for the command
        flags: u32 = 0
    }

    pub const CODE = sys::IORING_OP_SYNC_FILE_RANGE;

    pub fn build(self) -> Entry {
        let SyncFileRange {
            fd,
            len, offset,
            flags
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.len = len as _;
        sqe.__bindgen_anon_1.off = offset as _;
        sqe.__bindgen_anon_3.sync_range_flags = flags;
        Entry(sqe)
    }
);

opcode!(
    /// Send a message on a socket, equivalent to `send(2)`.
    ///
    /// fd must be set to the socket file descriptor, addr must contains a pointer to the msghdr
    /// structure, and flags holds the flags associated with the system call.
    #[derive(Debug)]
    pub struct SendMsg {
        fd: { impl sealed::UseFixed },
        msg: { *const libc::msghdr },
        ;;
        ioprio: u16 = 0,
        flags: u32 = 0
    }

    pub const CODE = sys::IORING_OP_SENDMSG;

    pub fn build(self) -> Entry {
        let SendMsg { fd, msg, ioprio, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.ioprio = ioprio;
        sqe.__bindgen_anon_2.addr = msg as _;
        sqe.len = 1;
        sqe.__bindgen_anon_3.msg_flags = flags;
        Entry(sqe)
    }
);

opcode!(
    /// Receive a message on a socket, equivalent to `recvmsg(2)`.
    ///
    /// See also the description of [`SendMsg`].
    #[derive(Debug)]
    pub struct RecvMsg {
        fd: { impl sealed::UseFixed },
        msg: { *mut libc::msghdr },
        ;;
        ioprio: u16 = 0,
        flags: u32 = 0
    }

    pub const CODE = sys::IORING_OP_RECVMSG;

    pub fn build(self) -> Entry {
        let RecvMsg { fd, msg, ioprio, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.ioprio = ioprio;
        sqe.__bindgen_anon_2.addr = msg as _;
        sqe.len = 1;
        sqe.__bindgen_anon_3.msg_flags = flags;
        Entry(sqe)
    }
);

opcode!(
    /// Register a timeout operation.
    ///
    /// A timeout will trigger a wakeup event on the completion ring for anyone waiting for events.
    /// A timeout condition is met when either the specified timeout expires, or the specified number of events have completed.
    /// Either condition will trigger the event.
    /// The request will complete with `-ETIME` if the timeout got completed through expiration of the timer,
    /// or 0 if the timeout got completed through requests completing on their own.
    /// If the timeout was cancelled before it expired, the request will complete with `-ECANCELED`.
    #[derive(Debug)]
    pub struct Timeout {
        timespec: { *const types::Timespec },
        ;;
        /// `count` may contain a completion event count.
        count: u32 = 0,

        /// `flags` may contain [types::TimeoutFlags::ABS] for an absolute timeout value, or 0 for a relative timeout.
        flags: types::TimeoutFlags = types::TimeoutFlags::empty()
    }

    pub const CODE = sys::IORING_OP_TIMEOUT;

    pub fn build(self) -> Entry {
        let Timeout { timespec, count, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = -1;
        sqe.__bindgen_anon_2.addr = timespec as _;
        sqe.len = 1;
        sqe.__bindgen_anon_1.off = count as _;
        sqe.__bindgen_anon_3.timeout_flags = flags.bits();
        Entry(sqe)
    }
);

// === 5.5 ===

opcode!(
    /// Attempt to remove an existing [timeout operation](Timeout).
    pub struct TimeoutRemove {
        user_data: { u64 },
        ;;
        flags: types::TimeoutFlags = types::TimeoutFlags::empty()
    }

    pub const CODE = sys::IORING_OP_TIMEOUT_REMOVE;

    pub fn build(self) -> Entry {
        let TimeoutRemove { user_data, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = -1;
        sqe.__bindgen_anon_2.addr = user_data as _;
        sqe.__bindgen_anon_3.timeout_flags = flags.bits();
        Entry(sqe)
    }
);

opcode!(
    /// Accept a new connection on a socket, equivalent to `accept4(2)`.
    pub struct Accept {
        fd: { impl sealed::UseFixed },
        addr: { *mut libc::sockaddr },
        addrlen: { *mut libc::socklen_t },
        ;;
        flags: i32 = 0
    }

    pub const CODE = sys::IORING_OP_ACCEPT;

    pub fn build(self) -> Entry {
        let Accept { fd, addr, addrlen, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_2.addr = addr as _;
        sqe.__bindgen_anon_1.addr2 = addrlen as _;
        sqe.__bindgen_anon_3.accept_flags = flags as _;
        Entry(sqe)
    }
);

opcode!(
    /// Attempt to cancel an already issued request.
    pub struct AsyncCancel {
        user_data: { u64 }
        ;;

        // TODO flags
    }

    pub const CODE = sys::IORING_OP_ASYNC_CANCEL;

    pub fn build(self) -> Entry {
        let AsyncCancel { user_data } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = -1;
        sqe.__bindgen_anon_2.addr = user_data as _;
        Entry(sqe)
    }
);

opcode!(
    /// This request must be linked with another request through
    /// [`Flags::IO_LINK`](crate::squeue::Flags::IO_LINK) which is described below.
    /// Unlike [`Timeout`], [`LinkTimeout`] acts on the linked request, not the completion queue.
    pub struct LinkTimeout {
        timespec: { *const types::Timespec },
        ;;
        flags: types::TimeoutFlags = types::TimeoutFlags::empty()
    }

    pub const CODE = sys::IORING_OP_LINK_TIMEOUT;

    pub fn build(self) -> Entry {
        let LinkTimeout { timespec, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = -1;
        sqe.__bindgen_anon_2.addr = timespec as _;
        sqe.len = 1;
        sqe.__bindgen_anon_3.timeout_flags = flags.bits();
        Entry(sqe)
    }
);

opcode!(
    /// Connect a socket, equivalent to `connect(2)`.
    pub struct Connect {
        fd: { impl sealed::UseFixed },
        addr: { *const libc::sockaddr },
        addrlen: { libc::socklen_t }
        ;;
    }

    pub const CODE = sys::IORING_OP_CONNECT;

    pub fn build(self) -> Entry {
        let Connect { fd, addr, addrlen } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_2.addr = addr as _;
        sqe.__bindgen_anon_1.off = addrlen as _;
        Entry(sqe)
    }
);

// === 5.6 ===

opcode!(
    /// Preallocate or deallocate space to a file, equivalent to `fallocate(2)`.
    #[deprecated(note = "use Fallocate64 instead, which always takes a 64-bit length")]
    pub struct Fallocate {
        fd: { impl sealed::UseFixed },
        len: { libc::off_t },
        ;;
        offset64: libc::off64_t = 0,
        mode: i32 = 0
    }

    pub const CODE = sys::IORING_OP_FALLOCATE;

    pub fn build(self) -> Entry {
        let Fallocate { fd, len, offset64, mode } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_2.addr = len as _;
        sqe.len = mode as _;
        sqe.__bindgen_anon_1.off = offset64 as _;
        Entry(sqe)
    }
);

#[allow(deprecated)]
impl Fallocate {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// Preallocate or deallocate space to a file, equivalent to `fallocate(2)`.
    pub struct Fallocate64 {
        fd: { impl sealed::UseFixed },
        len: { libc::off64_t },
        ;;
        offset64: libc::off64_t = 0,
        mode: i32 = 0
    }

    pub const CODE = sys::IORING_OP_FALLOCATE;

    pub fn build(self) -> Entry {
        let Fallocate64 { fd, len, offset64, mode } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_2.addr = len as _;
        sqe.len = mode as _;
        sqe.__bindgen_anon_1.off = offset64 as _;
        Entry(sqe)
    }
);

impl Fallocate64 {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// Open a file, equivalent to `openat(2)`.
    pub struct OpenAt {
        dirfd: { impl sealed::UseFd },
        pathname: { *const libc::c_char },
        ;;
        flags: i32 = 0,
        mode: libc::mode_t = 0
    }

    pub const CODE = sys::IORING_OP_OPENAT;

    pub fn build(self) -> Entry {
        let OpenAt { dirfd, pathname, flags, mode } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = dirfd;
        sqe.__bindgen_anon_2.addr = pathname as _;
        sqe.len = mode;
        sqe.__bindgen_anon_3.open_flags = flags as _;
        Entry(sqe)
    }
);

opcode!(
    /// Close a file descriptor, equivalent to `close(2)`.
    pub struct Close {
        fd: { impl sealed::UseFd }
        ;;
    }

    pub const CODE = sys::IORING_OP_CLOSE;

    pub fn build(self) -> Entry {
        let Close { fd } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = fd;
        Entry(sqe)
    }
);

opcode!(
    /// This command is an alternative to using
    /// [`Submitter::register_files_update`](crate::Submitter::register_files_update) which then
    /// works in an async fashion, like the rest of the io_uring commands.
    pub struct FilesUpdate {
        fds: { *const RawFd },
        len: { u32 },
        ;;
        offset: i32 = 0
    }

    pub const CODE = sys::IORING_OP_FILES_UPDATE;

    pub fn build(self) -> Entry {
        let FilesUpdate { fds, len, offset } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = -1;
        sqe.__bindgen_anon_2.addr = fds as _;
        sqe.len = len;
        sqe.__bindgen_anon_1.off = offset as _;
        Entry(sqe)
    }
);

opcode!(
    /// Get file status, equivalent to `statx(2)`.
    pub struct Statx {
        dirfd: { impl sealed::UseFd },
        pathname: { *const libc::c_char },
        statxbuf: { *mut types::statx },
        ;;
        flags: i32 = 0,
        mask: u32 = 0
    }

    pub const CODE = sys::IORING_OP_STATX;

    pub fn build(self) -> Entry {
        let Statx {
            dirfd, pathname, statxbuf,
            flags, mask
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = dirfd;
        sqe.__bindgen_anon_2.addr = pathname as _;
        sqe.len = mask;
        sqe.__bindgen_anon_1.off = statxbuf as _;
        sqe.__bindgen_anon_3.statx_flags = flags as _;
        Entry(sqe)
    }
);

opcode!(
    /// Issue the equivalent of a `pread(2)` or `pwrite(2)` system call
    ///
    /// * `fd` is the file descriptor to be operated on,
    /// * `addr` contains the buffer in question,
    /// * `len` contains the length of the IO operation,
    ///
    /// These are non-vectored versions of the `IORING_OP_READV` and `IORING_OP_WRITEV` opcodes.
    /// See also `read(2)` and `write(2)` for the general description of the related system call.
    ///
    /// Available since 5.6.
    pub struct Read {
        fd: { impl sealed::UseFixed },
        buf: { *mut u8 },
        len: { u32 },
        ;;
        /// `offset` contains the read or write offset.
        ///
        /// If `fd` does not refer to a seekable file, `offset` must be set to zero.
        /// If `offsett` is set to `-1`, the offset will use (and advance) the file position,
        /// like the `read(2)` and `write(2)` system calls.
        offset64: libc::off64_t = 0,
        ioprio: u16 = 0,
        rw_flags: types::RwFlags = 0,
        buf_group: u16 = 0
    }

    pub const CODE = sys::IORING_OP_READ;

    pub fn build(self) -> Entry {
        let Read {
            fd,
            buf, len, offset64,
            ioprio, rw_flags,
            buf_group
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.ioprio = ioprio;
        sqe.__bindgen_anon_2.addr = buf as _;
        sqe.len = len;
        sqe.__bindgen_anon_1.off = offset64 as _;
        sqe.__bindgen_anon_3.rw_flags = rw_flags;
        sqe.__bindgen_anon_4.buf_group = buf_group;
        Entry(sqe)
    }
);

impl Read {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// Issue the equivalent of a `pread(2)` or `pwrite(2)` system call
    ///
    /// * `fd` is the file descriptor to be operated on,
    /// * `addr` contains the buffer in question,
    /// * `len` contains the length of the IO operation,
    ///
    /// These are non-vectored versions of the `IORING_OP_READV` and `IORING_OP_WRITEV` opcodes.
    /// See also `read(2)` and `write(2)` for the general description of the related system call.
    ///
    /// Available since 5.6.
    pub struct Write {
        fd: { impl sealed::UseFixed },
        buf: { *const u8 },
        len: { u32 },
        ;;
        /// `offset` contains the read or write offset.
        ///
        /// If `fd` does not refer to a seekable file, `offset` must be set to zero.
        /// If `offsett` is set to `-1`, the offset will use (and advance) the file position,
        /// like the `read(2)` and `write(2)` system calls.
        offset64: libc::off64_t = 0,
        ioprio: u16 = 0,
        rw_flags: types::RwFlags = 0
    }

    pub const CODE = sys::IORING_OP_WRITE;

    pub fn build(self) -> Entry {
        let Write {
            fd,
            buf, len, offset64,
            ioprio, rw_flags
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.ioprio = ioprio;
        sqe.__bindgen_anon_2.addr = buf as _;
        sqe.len = len;
        sqe.__bindgen_anon_1.off = offset64 as _;
        sqe.__bindgen_anon_3.rw_flags = rw_flags;
        Entry(sqe)
    }
);

impl Write {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// Predeclare an access pattern for file data, equivalent to `posix_fadvise(2)`.
    pub struct Fadvise {
        fd: { impl sealed::UseFixed },
        len: { libc::off_t },
        advice: { i32 },
        ;;
        offset64: libc::off64_t = 0,
    }

    pub const CODE = sys::IORING_OP_FADVISE;

    pub fn build(self) -> Entry {
        let Fadvise { fd, len, advice, offset64 } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.len = len as _;
        sqe.__bindgen_anon_1.off = offset64 as _;
        sqe.__bindgen_anon_3.fadvise_advice = advice as _;
        Entry(sqe)
    }
);

impl Fadvise {
    #[inline]
    pub const fn offset(self, offset: libc::off_t) -> Self {
        self.offset64(offset as libc::off64_t)
    }
}

opcode!(
    /// Give advice about use of memory, equivalent to `madvise(2)`.
    pub struct Madvise {
        addr: { *const libc::c_void },
        len: { libc::off_t },
        advice: { i32 },
        ;;
    }

    pub const CODE = sys::IORING_OP_MADVISE;

    pub fn build(self) -> Entry {
        let Madvise { addr, len, advice } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = -1;
        sqe.__bindgen_anon_2.addr = addr as _;
        sqe.len = len as _;
        sqe.__bindgen_anon_3.fadvise_advice = advice as _;
        Entry(sqe)
    }
);

opcode!(
    /// Send a message on a socket, equivalent to `send(2)`.
    pub struct Send {
        fd: { impl sealed::UseFixed },
        buf: { *const u8 },
        len: { u32 },
        ;;
        flags: i32 = 0
    }

    pub const CODE = sys::IORING_OP_SEND;

    pub fn build(self) -> Entry {
        let Send { fd, buf, len, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_2.addr = buf as _;
        sqe.len = len;
        sqe.__bindgen_anon_3.msg_flags = flags as _;
        Entry(sqe)
    }
);

opcode!(
    /// Receive a message from a socket, equivalent to `recv(2)`.
    pub struct Recv {
        fd: { impl sealed::UseFixed },
        buf: { *mut u8 },
        len: { u32 },
        ;;
        flags: i32 = 0,
        buf_group: u16 = 0
    }

    pub const CODE = sys::IORING_OP_RECV;

    pub fn build(self) -> Entry {
        let Recv { fd, buf, len, flags, buf_group } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_2.addr = buf as _;
        sqe.len = len;
        sqe.__bindgen_anon_3.msg_flags = flags as _;
        sqe.__bindgen_anon_4.buf_group = buf_group;
        Entry(sqe)
    }
);

opcode!(
    /// Open a file, equivalent to `openat2(2)`.
    pub struct OpenAt2 {
        dirfd: { impl sealed::UseFd },
        pathname: { *const libc::c_char },
        how: { *const types::OpenHow }
        ;;
    }

    pub const CODE = sys::IORING_OP_OPENAT2;

    pub fn build(self) -> Entry {
        let OpenAt2 { dirfd, pathname, how } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = dirfd;
        sqe.__bindgen_anon_2.addr = pathname as _;
        sqe.len = mem::size_of::<sys::open_how>() as _;
        sqe.__bindgen_anon_1.off = how as _;
        Entry(sqe)
    }
);

opcode!(
    /// Modify an epoll file descriptor, equivalent to `epoll_ctl(2)`.
    pub struct EpollCtl {
        epfd: { impl sealed::UseFixed },
        fd: { impl sealed::UseFd },
        op: { i32 },
        ev: { *const types::epoll_event },
        ;;
    }

    pub const CODE = sys::IORING_OP_EPOLL_CTL;

    pub fn build(self) -> Entry {
        let EpollCtl { epfd, fd, op, ev } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = epfd);
        sqe.__bindgen_anon_2.addr = ev as _;
        sqe.len = op as _;
        sqe.__bindgen_anon_1.off = fd as _;
        Entry(sqe)
    }
);

// === 5.7 ===

opcode!(
    /// Splice data to/from a pipe, equivalent to `splice(2)`.
    ///
    /// if `fd_in` refers to a pipe, `off_in` must be `-1`;
    /// The description of `off_in` also applied to `off_out`.
    pub struct Splice {
        fd_in: { impl sealed::UseFixed },
        off_in: { i64 },
        fd_out: { impl sealed::UseFixed },
        off_out: { i64 },
        len: { u32 },
        ;;
        /// see man `splice(2)` for description of flags.
        flags: u32 = 0
    }

    pub const CODE = sys::IORING_OP_SPLICE;

    pub fn build(self) -> Entry {
        let Splice { fd_in, off_in, fd_out, off_out, len, mut flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd_out);
        sqe.len = len;
        sqe.__bindgen_anon_1.off = off_out as _;

        sqe.__bindgen_anon_5.splice_fd_in = match fd_in {
            sealed::Target::Fd(fd) => fd,
            sealed::Target::Fixed(i) => {
                flags |= sys::SPLICE_F_FD_IN_FIXED;
                i as _
            }
        };

        sqe.__bindgen_anon_2.splice_off_in = off_in as _;
        sqe.__bindgen_anon_3.splice_flags = flags;
        Entry(sqe)
    }
);

#[cfg(feature = "unstable")]
opcode!(
    /// Register `nbufs` buffers that each have the length `len` with ids starting from `big` in the
    /// group `bgid` that can be used for any request. See
    /// [`BUFFER_SELECT`](crate::squeue::Flags::BUFFER_SELECT) for more info.
    ///
    /// Requires the `unstable` feature.
    pub struct ProvideBuffers {
        addr: { *mut u8 },
        len: { i32 },
        nbufs: { u16 },
        bgid: { u16 },
        bid: { u16 }
        ;;
    }

    pub const CODE = sys::IORING_OP_PROVIDE_BUFFERS;

    pub fn build(self) -> Entry {
        let ProvideBuffers { addr, len, nbufs, bgid, bid } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = nbufs as _;
        sqe.__bindgen_anon_2.addr = addr as _;
        sqe.len = len as _;
        sqe.__bindgen_anon_1.off = bid as _;
        sqe.__bindgen_anon_4.buf_group = bgid;
        Entry(sqe)
    }
);

#[cfg(feature = "unstable")]
opcode!(
    /// Remove some number of buffers from a buffer group. See
    /// [`BUFFER_SELECT`](crate::squeue::Flags::BUFFER_SELECT) for more info.
    ///
    /// Requires the `unstable` feature.
    pub struct RemoveBuffers {
        nbufs: { u16 },
        bgid: { u16 }
        ;;
    }

    pub const CODE = sys::IORING_OP_REMOVE_BUFFERS;

    pub fn build(self) -> Entry {
        let RemoveBuffers { nbufs, bgid } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = nbufs as _;
        sqe.__bindgen_anon_4.buf_group = bgid;
        Entry(sqe)
    }
);

// === 5.8 ===

#[cfg(feature = "unstable")]
opcode!(
    /// Duplicate pipe content, equivalent to `tee(2)`.
    ///
    /// Requires the `unstable` feature.
    pub struct Tee {
        fd_in: { impl sealed::UseFixed },
        fd_out: { impl sealed::UseFixed },
        len: { u32 }
        ;;
        flags: u32 = 0
    }

    pub const CODE = sys::IORING_OP_TEE;

    pub fn build(self) -> Entry {
        let Tee { fd_in, fd_out, len, mut flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;

        assign_fd!(sqe.fd = fd_out);
        sqe.len = len;

        sqe.__bindgen_anon_5.splice_fd_in = match fd_in {
            sealed::Target::Fd(fd) => fd,
            sealed::Target::Fixed(i) => {
                flags |= sys::SPLICE_F_FD_IN_FIXED;
                i as _
            }
        };

        sqe.__bindgen_anon_3.splice_flags = flags;

        Entry(sqe)
    }
);

// === 5.11 ===

#[cfg(feature = "unstable")]
opcode!(
    pub struct Shutdown {
        fd: { impl sealed::UseFixed },
        how: { i32 },
        ;;
    }

    pub const CODE = sys::IORING_OP_SHUTDOWN;

    pub fn build(self) -> Entry {
        let Shutdown { fd, how } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.len = how as _;
        Entry(sqe)
    }
);

#[cfg(feature = "unstable")]
opcode!(
    pub struct RenameAt {
        olddirfd: { impl sealed::UseFd },
        oldpath: { *const libc::c_char },
        newdirfd: { impl sealed::UseFd },
        newpath: { *const libc::c_char },
        ;;
        flags: u32 = 0
    }

    pub const CODE = sys::IORING_OP_RENAMEAT;

    pub fn build(self) -> Entry {
        let RenameAt {
            olddirfd, oldpath,
            newdirfd, newpath,
            flags
        } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = olddirfd;
        sqe.__bindgen_anon_2.addr = oldpath as _;
        sqe.len = newdirfd as _;
        sqe.__bindgen_anon_1.off = newpath as _;
        sqe.__bindgen_anon_3.rename_flags = flags;
        Entry(sqe)
    }
);

#[cfg(feature = "unstable")]
opcode!(
    pub struct UnlinkAt {
        dirfd: { impl sealed::UseFd },
        pathname: { *const libc::c_char },
        ;;
        flags: i32 = 0
    }

    pub const CODE = sys::IORING_OP_UNLINKAT;

    pub fn build(self) -> Entry {
        let UnlinkAt { dirfd, pathname, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = dirfd;
        sqe.__bindgen_anon_2.addr = pathname as _;
        sqe.__bindgen_anon_3.unlink_flags = flags as _;
        Entry(sqe)
    }
);

// === 5.15 ===

#[cfg(feature = "unstable")]
opcode!(
    /// Make a directory, equivalent to `mkdirat2(2)`.
    ///
    /// Requires the `unstable` feature.
    pub struct MkDirAt {
        dirfd: { impl sealed::UseFd },
        pathname: { *const libc::c_char },
        ;;
        mode: libc::mode_t = 0
    }

    pub const CODE = sys::IORING_OP_MKDIRAT;

    pub fn build(self) -> Entry {
        let MkDirAt { dirfd, pathname, mode } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = dirfd;
        sqe.__bindgen_anon_2.addr = pathname as _;
        sqe.len = mode;
        Entry(sqe)
    }
);

#[cfg(feature = "unstable")]
opcode!(
    /// Create a symlink, equivalent to `symlinkat2(2)`.
    ///
    /// Requires the `unstable` feature.
    pub struct SymlinkAt {
        newdirfd: { impl sealed::UseFd },
        target: { *const libc::c_char },
        linkpath: { *const libc::c_char },
        ;;
    }

    pub const CODE = sys::IORING_OP_SYMLINKAT;

    pub fn build(self) -> Entry {
        let SymlinkAt { newdirfd, target, linkpath } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = newdirfd;
        sqe.__bindgen_anon_2.addr = target as _;
        sqe.__bindgen_anon_1.addr2 = linkpath as _;
        Entry(sqe)
    }
);

#[cfg(feature = "unstable")]
opcode!(
    /// Create a hard link, equivalent to `linkat2(2)`.
    ///
    /// Requires the `unstable` feature.
    pub struct LinkAt {
        olddirfd: { impl sealed::UseFd },
        oldpath: { *const libc::c_char },
        newdirfd: { impl sealed::UseFd },
        newpath: { *const libc::c_char },
        ;;
        flags: i32 = 0
    }

    pub const CODE = sys::IORING_OP_LINKAT;

    pub fn build(self) -> Entry {
        let LinkAt { olddirfd, oldpath, newdirfd, newpath, flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        sqe.fd = olddirfd as _;
        sqe.__bindgen_anon_2.addr = oldpath as _;
        sqe.len = newdirfd as _;
        sqe.__bindgen_anon_1.addr2 = newpath as _;
        sqe.__bindgen_anon_3.hardlink_flags = flags as _;
        Entry(sqe)
    }
);

// === 5.19 ===

#[cfg(feature = "unstable")]
opcode!(
    /// A file/device-specific 16-byte command, akin (but not equivalent) to `ioctl(2)`.
    pub struct UringCmd16 {
        fd: { impl sealed::UseFixed },
        cmd_op: { u32 },
        ;;
        /// Arbitrary command data.
        cmd: [u8; 16] = [0u8; 16]
    }

    pub const CODE = sys::IORING_OP_URING_CMD;

    pub fn build(self) -> Entry {
        let UringCmd16 { fd, cmd_op, cmd } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_1.__bindgen_anon_1.cmd_op = cmd_op;
        unsafe { *sqe.__bindgen_anon_6.cmd.as_mut().as_mut_ptr().cast::<[u8; 16]>() = cmd };
        Entry(sqe)
    }
);

#[cfg(feature = "unstable")]
opcode!(
    /// A file/device-specific 80-byte command, akin (but not equivalent) to `ioctl(2)`.
    pub struct UringCmd80 {
        fd: { impl sealed::UseFixed },
        cmd_op: { u32 },
        ;;
        /// Arbitrary command data.
        cmd: [u8; 80] = [0u8; 80]
    }

    pub const CODE = sys::IORING_OP_URING_CMD;

    pub fn build(self) -> Entry128 {
        let UringCmd80 { fd, cmd_op, cmd } = self;

        let cmd1 = cmd[..16].try_into().unwrap();
        let cmd2 = cmd[16..].try_into().unwrap();

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_1.__bindgen_anon_1.cmd_op = cmd_op;
        unsafe { *sqe.__bindgen_anon_6.cmd.as_mut().as_mut_ptr().cast::<[u8; 16]>() = cmd1 };
        Entry128(Entry(sqe), cmd2)
    }
);

// === 6.0 ===

#[cfg(feature = "unstable")]
opcode!(
    /// Send a zerocopy message on a socket, equivalent to `send(2)`.
    pub struct SendZc {
        fd: { impl sealed::UseFixed },
        buf: { *const u8 },
        len: { u32 },
        ;;
        flags: i32 = 0,
        zc_flags: u16 =0,
    }

    pub const CODE = sys::IORING_OP_SEND_ZC;

    pub fn build(self) -> Entry {
        let SendZc { fd, buf, len, flags, zc_flags } = self;

        let mut sqe = sqe_zeroed();
        sqe.opcode = Self::CODE;
        assign_fd!(sqe.fd = fd);
        sqe.__bindgen_anon_2.addr = buf as _;
        sqe.len = len;
        sqe.__bindgen_anon_3.msg_flags = flags as _;
        sqe.ioprio = zc_flags;
        Entry(sqe)
    }
);
