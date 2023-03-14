// Copyright (C) 2021-2022 Alibaba Cloud. All rights reserved.
// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.

use std::ffi::CStr;
use std::io;
use std::mem;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use super::{
    Context, DirEntry, Entry, FileLock, GetxattrReply, IoctlData, ListxattrReply, ZeroCopyReader,
    ZeroCopyWriter,
};
use crate::abi::fuse_abi::{stat64, statvfs64, CreateIn, FsOptions, OpenOptions, SetattrValid};
#[cfg(feature = "virtiofs")]
pub use crate::abi::virtio_fs::RemovemappingOne;
#[cfg(feature = "virtiofs")]
use crate::transport::FsCacheReqHandler;

/// The main trait that connects a file system with a transport.
#[allow(unused_variables)]
pub trait FileSystem {
    /// Represents a location in the filesystem tree and can be used to perform operations that act
    /// on the metadata of a file/directory (e.g., `getattr` and `setattr`). Can also be used as the
    /// starting point for looking up paths in the filesystem tree. An `Inode` may support operating
    /// directly on the content of the path that to which it points. `FileSystem` implementations
    /// that support this should set the `FsOptions::ZERO_MESSAGE_OPEN` option in the return value
    /// of the `init` function. On linux based systems, an `Inode` is equivalent to opening a file
    /// or directory with the `libc::O_PATH` flag.
    ///
    /// # Lookup Count
    ///
    /// The `FileSystem` implementation is required to keep a "lookup count" for every `Inode`.
    /// Every time an `Entry` is returned by a `FileSystem` trait method, this lookup count should
    /// increase by 1. The lookup count for an `Inode` decreases when the kernel sends a `forget`
    /// request. `Inode`s with a non-zero lookup count may receive requests from the kernel even
    /// after calls to `unlink`, `rmdir` or (when overwriting an existing file) `rename`.
    /// `FileSystem` implementations must handle such requests properly and it is recommended to
    /// defer removal of the `Inode` until the lookup count reaches zero. Calls to `unlink`, `rmdir`
    /// or `rename` will be followed closely by `forget` unless the file or directory is open, in
    /// which case the kernel issues `forget` only after the `release` or `releasedir` calls.
    ///
    /// Note that if a file system will be exported over NFS the `Inode`'s lifetime must extend even
    /// beyond `forget`. See the `generation` field in `Entry`.
    type Inode: From<u64> + Into<u64>;

    /// Represents a file or directory that is open for reading/writing.
    type Handle: From<u64> + Into<u64>;

    /// Initialize the file system.
    ///
    /// This method is called when a connection to the FUSE kernel module is first established. The
    /// `capable` parameter indicates the features that are supported by the kernel module. The
    /// implementation should return the options that it supports. Any options set in the returned
    /// `FsOptions` that are not also set in `capable` are silently dropped.
    fn init(&self, capable: FsOptions) -> io::Result<FsOptions> {
        Ok(FsOptions::empty())
    }

    /// Clean up the file system.
    ///
    /// Called when the filesystem exits. All open `Handle`s should be closed and the lookup count
    /// for all open `Inode`s implicitly goes to zero. At this point the connection to the FUSE
    /// kernel module may already be gone so implementations should not rely on being able to
    /// communicate with the kernel.
    fn destroy(&self) {}

    /// Look up a directory entry by name and get its attributes.
    ///
    /// If this call is successful then the lookup count of the `Inode` associated with the returned
    /// `Entry` must be increased by 1.
    fn lookup(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<Entry> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Forget about an inode.
    ///
    /// Called when the kernel removes an inode from its internal caches. `count` indicates the
    /// amount by which the lookup count for the inode should be decreased. If reducing the lookup
    /// count by `count` causes it to go to zero, then the implementation may delete the `Inode`.
    fn forget(&self, ctx: &Context, inode: Self::Inode, count: u64) {}

    /// Forget about multiple inodes.
    ///
    /// `requests` is a vector of `(inode, count)` pairs. See the documentation for `forget` for
    /// more information.
    fn batch_forget(&self, ctx: &Context, requests: Vec<(Self::Inode, u64)>) {
        for (inode, count) in requests {
            self.forget(ctx, inode, count)
        }
    }

    /// Get attributes for a file / directory.
    ///
    /// If `handle` is not `None`, then it contains the handle previously returned by the
    /// implementation after a call to `open` or `opendir`. However, implementations should still
    /// take care to verify the handle if they do not trust the client (e.g., virtio-fs).
    ///
    /// If writeback caching is enabled (`FsOptions::WRITEBACK_CACHE`), then the kernel module
    /// likely has a better idea of the length of the file than the file system (for
    /// example, if there was a write that extended the size of the file but has not yet been
    /// flushed). In this case, the `st_size` field of the returned struct is ignored.
    ///
    /// The returned `Duration` indicates how long the returned attributes should be considered
    /// valid by the client. If the attributes are only changed via the FUSE kernel module (i.e.,
    /// the kernel module has exclusive access), then this should be a very large value.
    fn getattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Option<Self::Handle>,
    ) -> io::Result<(stat64, Duration)> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Set attributes for a file / directory.
    ///
    /// If `handle` is not `None`, then it contains the handle previously returned by the
    /// implementation after a call to `open` or `opendir`. However, implementations should still
    /// take care to verify the handle if they do not trust the client (e.g., virtio-fs).
    ///
    /// The `valid` parameter indicates the fields of `attr` that may be considered valid and should
    /// be set by the file system. The content of all other fields in `attr` is undefined.
    ///
    /// If the `FsOptions::HANDLE_KILLPRIV` was set during `init`, then the implementation is
    /// expected to reset the setuid and setgid bits if the file size or owner is being changed.
    ///
    /// This method returns the new attributes after making the modifications requested by the
    /// client. The returned `Duration` indicates how long the returned attributes should be
    /// considered valid by the client. If the attributes are only changed via the FUSE kernel
    /// module (i.e., the kernel module has exclusive access), then this should be a very large
    /// value.
    fn setattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        attr: stat64,
        handle: Option<Self::Handle>,
        valid: SetattrValid,
    ) -> io::Result<(stat64, Duration)> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Read a symbolic link.
    fn readlink(&self, ctx: &Context, inode: Self::Inode) -> io::Result<Vec<u8>> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Create a symbolic link.
    ///
    /// The file system must create a symbolic link named `name` in the directory represented by
    /// `parent`, which contains the string `linkname`. Returns an `Entry` for the newly created
    /// symlink.
    ///
    /// If this call is successful then the lookup count of the `Inode` associated with the returned
    /// `Entry` must be increased by 1.
    fn symlink(
        &self,
        ctx: &Context,
        linkname: &CStr,
        parent: Self::Inode,
        name: &CStr,
    ) -> io::Result<Entry> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Create a file node.
    ///
    /// Create a regular file, character device, block device, fifo, or socket node named `name` in
    /// the directory represented by `inode`. Valid values for `mode` and `rdev` are the same as
    /// those accepted by the `mknod(2)` system call. Returns an `Entry` for the newly created node.
    ///
    /// When the `FsOptions::DONT_MASK` feature is set, the file system is responsible for setting
    /// the permissions of the created node to `mode & !umask`.
    ///
    /// If this call is successful then the lookup count of the `Inode` associated with the returned
    /// `Entry` must be increased by 1.
    fn mknod(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        mode: u32,
        rdev: u32,
        umask: u32,
    ) -> io::Result<Entry> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Create a directory.
    ///
    /// When the `FsOptions::DONT_MASK` feature is set, the file system is responsible for setting
    /// the permissions of the created directory to `mode & !umask`. Returns an `Entry` for the
    /// newly created directory.
    ///
    /// If this call is successful then the lookup count of the `Inode` associated with the returned
    /// `Entry` must be increased by 1.
    fn mkdir(
        &self,
        ctx: &Context,
        parent: Self::Inode,
        name: &CStr,
        mode: u32,
        umask: u32,
    ) -> io::Result<Entry> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Remove a file.
    ///
    /// If the file's inode lookup count is non-zero, then the file system is expected to delay
    /// removal of the inode until the lookup count goes to zero. See the documentation of the
    /// `forget` function for more information.
    fn unlink(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Remove a directory.
    ///
    /// If the directory's inode lookup count is non-zero, then the file system is expected to delay
    /// removal of the inode until the lookup count goes to zero. See the documentation of the
    /// `forget` function for more information.
    fn rmdir(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Rename a file / directory.
    ///
    /// If the destination exists, it should be atomically replaced. If the destination's inode
    /// lookup count is non-zero, then the file system is expected to delay removal of the inode
    /// until the lookup count goes to zero. See the documentation of the `forget` function for more
    /// information.
    ///
    /// `flags` may be `libc::RENAME_EXCHANGE` or `libc::RENAME_NOREPLACE`. If
    /// `libc::RENAME_NOREPLACE` is specified, the implementation must not overwrite `newname` if it
    /// exists and must return an error instead. If `libc::RENAME_EXCHANGE` is specified, the
    /// implementation must atomically exchange the two files, i.e., both must exist and neither may
    /// be deleted.
    fn rename(
        &self,
        ctx: &Context,
        olddir: Self::Inode,
        oldname: &CStr,
        newdir: Self::Inode,
        newname: &CStr,
        flags: u32,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Create a hard link.
    ///
    /// Create a hard link from `inode` to `newname` in the directory represented by `newparent`.
    ///
    /// If this call is successful then the lookup count of the `Inode` associated with the returned
    /// `Entry` must be increased by 1.
    fn link(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        newparent: Self::Inode,
        newname: &CStr,
    ) -> io::Result<Entry> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Open a file.
    ///
    /// Open the file associated with `inode` for reading / writing. All values accepted by the
    /// `open(2)` system call are valid values for `flags` and must be handled by the file system.
    /// However, there are some additional rules:
    ///
    /// * Creation flags (`libc::O_CREAT`, `libc::O_EXCL`, `libc::O_NOCTTY`) will be filtered out
    ///   and handled by the kernel.
    ///
    /// * The file system should check the access modes (`libc::O_RDONLY`, `libc::O_WRONLY`,
    ///   `libc::O_RDWR`) to determine if the operation is permitted. If the file system was mounted
    ///   with the `-o default_permissions` mount option, then this check will also be carried out
    ///   by the kernel before sending the open request.
    ///
    /// * When writeback caching is enabled (`FsOptions::WRITEBACK_CACHE`) the kernel may send read
    ///   requests even for files opened with `libc::O_WRONLY`. The file system should be prepared
    ///   to handle this.
    ///
    /// * When writeback caching is enabled, the kernel will handle the `libc::O_APPEND` flag.
    ///   However, this will not work reliably unless the kernel has exclusive access to the file.
    ///   In this case the file system may either ignore the `libc::O_APPEND` flag or return an
    ///   error to indicate that reliable `libc::O_APPEND` handling is not available.
    ///
    /// * When writeback caching is disabled, the file system is expected to properly handle
    ///   `libc::O_APPEND` and ensure that each write is appended to the end of the file.
    ///
    /// The file system may choose to return a `Handle` to refer to the newly opened file. The
    /// kernel will then use this `Handle` for all operations on the content of the file (`read`,
    /// `write`, `flush`, `release`, `fsync`). If the file system does not return a
    /// `Handle` then the kernel will use the `Inode` for the file to operate on its contents. In
    /// this case the file system may wish to enable the `FsOptions::ZERO_MESSAGE_OPEN` feature if
    /// it is supported by the kernel (see below).
    ///
    /// The returned `OpenOptions` allow the file system to change the way the opened file is
    /// handled by the kernel. See the documentation of `OpenOptions` for more information.
    ///
    /// If the `FsOptions::ZERO_MESSAGE_OPEN` feature is enabled by both the file system
    /// implementation and the kernel, then the file system may return an error of `ENOSYS`. This
    /// will be interpreted by the kernel as success and future calls to `open` and `release` will
    /// be handled by the kernel without being passed on to the file system.
    fn open(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        fuse_flags: u32,
    ) -> io::Result<(Option<Self::Handle>, OpenOptions)> {
        // Matches the behavior of libfuse.
        Ok((None, OpenOptions::empty()))
    }

    /// Create and open a file.
    ///
    /// If the file does not already exist, the file system should create it with the specified
    /// `mode`. When the `FsOptions::DONT_MASK` feature is set, the file system is responsible for
    /// setting the permissions of the created file to `mode & !umask`.
    ///
    /// If the file system returns an `ENOSYS` error, then the kernel will treat this method as
    /// unimplemented and all future calls to `create` will be handled by calling the `mknod` and
    /// `open` methods instead.
    ///
    /// See the documentation for the `open` method for more information about opening the file. In
    /// addition to the optional `Handle` and the `OpenOptions`, the file system must also return an
    /// `Entry` for the file. This increases the lookup count for the `Inode` associated with the
    /// file by 1.
    fn create(
        &self,
        ctx: &Context,
        parent: Self::Inode,
        name: &CStr,
        args: CreateIn,
    ) -> io::Result<(Entry, Option<Self::Handle>, OpenOptions)> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Read data from a file.
    ///
    /// Returns `size` bytes of data starting from offset `off` from the file associated with
    /// `inode` or `handle`.
    ///
    /// `flags` contains the flags used to open the file. Similarly, `handle` is the `Handle`
    /// returned by the file system from the `open` method, if any. If the file system
    /// implementation did not return a `Handle` from `open` then the contents of `handle` are
    /// undefined.
    ///
    /// This method should return exactly the number of bytes requested by the kernel, except in the
    /// case of error or EOF. Otherwise, the kernel will substitute the rest of the data with
    /// zeroes. An exception to this rule is if the file was opened with the "direct I/O" option
    /// (`libc::O_DIRECT`), in which case the kernel will forward the return code from this method
    /// to the userspace application that made the system call.
    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        w: &mut dyn ZeroCopyWriter,
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        flags: u32,
    ) -> io::Result<usize> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Write data to a file.
    ///
    /// Writes `size` bytes of data starting from offset `off` to the file associated with `inode`
    /// or `handle`.
    ///
    /// `flags` contains the flags used to open the file. Similarly, `handle` is the `Handle`
    /// returned by the file system from the `open` method, if any. If the file system
    /// implementation did not return a `Handle` from `open` then the contents of `handle` are
    /// undefined.
    ///
    /// If the `FsOptions::HANDLE_KILLPRIV` feature is not enabled then then the file system is
    /// expected to clear the setuid and setgid bits.
    ///
    /// If `delayed_write` is true then it indicates that this is a write for buffered data.
    ///
    /// This method should return exactly the number of bytes requested by the kernel, except in the
    /// case of error. An exception to this rule is if the file was opened with the "direct I/O"
    /// option (`libc::O_DIRECT`), in which case the kernel will forward the return code from this
    /// method to the userspace application that made the system call.
    #[allow(clippy::too_many_arguments)]
    fn write(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        r: &mut dyn ZeroCopyReader,
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        delayed_write: bool,
        flags: u32,
        fuse_flags: u32,
    ) -> io::Result<usize> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Flush the contents of a file.
    ///
    /// This method is called on every `close()` of a file descriptor. Since it is possible to
    /// duplicate file descriptors there may be many `flush` calls for one call to `open`.
    ///
    /// File systems should not make any assumptions about when `flush` will be
    /// called or even if it will be called at all.
    ///
    /// `handle` is the `Handle` returned by the file system from the `open` method, if any. If the
    /// file system did not return a `Handle` from `open` then the contents of `handle` are
    /// undefined.
    ///
    /// Unlike `fsync`, the file system is not required to flush pending writes. One reason to flush
    /// data is if the file system wants to return write errors during close. However, this is not
    /// portable because POSIX does not require `close` to wait for delayed I/O to complete.
    ///
    /// If the `FsOptions::POSIX_LOCKS` feature is enabled, then the file system must remove all
    /// locks belonging to `lock_owner`.
    ///
    /// If this method returns an `ENOSYS` error then the kernel will treat it as success and all
    /// subsequent calls to `flush` will be handled by the kernel without being forwarded to the
    /// file system.
    fn flush(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        lock_owner: u64,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Synchronize file contents.
    ///
    /// File systems must ensure that the file contents have been flushed to disk before returning
    /// from this method. If `datasync` is true then only the file data (but not the metadata) needs
    /// to be flushed.
    ///
    /// `handle` is the `Handle` returned by the file system from the `open` method, if any. If the
    /// file system did not return a `Handle` from `open` then the contents of
    /// `handle` are undefined.
    ///
    /// If this method returns an `ENOSYS` error then the kernel will treat it as success and all
    /// subsequent calls to `fsync` will be handled by the kernel without being forwarded to the
    /// file system.
    fn fsync(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        datasync: bool,
        handle: Self::Handle,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Allocate requested space for file data.
    ///
    /// If this function returns success, then the file sytem must guarantee that it is possible to
    /// write up to `length` bytes of data starting at `offset` without failing due to a lack of
    /// free space on the disk.
    ///
    /// `handle` is the `Handle` returned by the file system from the `open` method, if any. If the
    /// file system did not return a `Handle` from `open` then the contents of `handle` are
    /// undefined.
    ///
    /// If this method returns an `ENOSYS` error then the kernel will treat that as a permanent
    /// failure: all future calls to `fallocate` will fail with `EOPNOTSUPP` without being forwarded
    /// to the file system.
    fn fallocate(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        mode: u32,
        offset: u64,
        length: u64,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Release an open file.
    ///
    /// This method is called when there are no more references to an open file: all file
    /// descriptors are closed and all memory mappings are unmapped.
    ///
    /// For every `open` call there will be exactly one `release` call (unless the file system is
    /// force-unmounted).
    ///
    /// The file system may reply with an error, but error values are not returned to the `close()`
    /// or `munmap()` which triggered the release.
    ///
    /// `handle` is the `Handle` returned by the file system from the `open` method, if any. If the
    /// file system did not return a `Handle` from `open` then the contents of
    /// `handle` are undefined.
    ///
    /// If `flush` is `true` then the contents of the file should also be flushed to disk.
    #[allow(clippy::too_many_arguments)]
    fn release(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        handle: Self::Handle,
        flush: bool,
        flock_release: bool,
        lock_owner: Option<u64>,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Get information about the file system.
    fn statfs(&self, ctx: &Context, inode: Self::Inode) -> io::Result<statvfs64> {
        // Safe because we are zero-initializing a struct with only POD fields.
        let mut st: statvfs64 = unsafe { mem::zeroed() };

        // This matches the behavior of libfuse as it returns these values if the
        // filesystem doesn't implement this method.
        st.f_namemax = 255;
        st.f_bsize = 512;

        Ok(st)
    }

    /// Set an extended attribute.
    ///
    /// If this method fails with an `ENOSYS` error, then the kernel will treat that as a permanent
    /// failure. The kernel will return `EOPNOTSUPP` for all future calls to `setxattr` without
    /// forwarding them to the file system.
    ///
    /// Valid values for flags are the same as those accepted by the `setxattr(2)` system call and
    /// have the same behavior.
    fn setxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        value: &[u8],
        flags: u32,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Get an extended attribute.
    ///
    /// If `size` is 0, then the file system should respond with `GetxattrReply::Count` and the
    /// number of bytes needed to hold the value. If `size` is large enough to hold the value, then
    /// the file system should reply with `GetxattrReply::Value` and the value of the extended
    /// attribute. If `size` is not 0 but is also not large enough to hold the value, then the file
    /// system should reply with an `ERANGE` error.
    ///
    /// If this method fails with an `ENOSYS` error, then the kernel will treat that as a permanent
    /// failure. The kernel will return `EOPNOTSUPP` for all future calls to `getxattr` without
    /// forwarding them to the file system.
    fn getxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        size: u32,
    ) -> io::Result<GetxattrReply> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// List extended attribute names.
    ///
    /// If `size` is 0, then the file system should respond with `ListxattrReply::Count` and the
    /// number of bytes needed to hold a `\0` byte separated list of the names of all the extended
    /// attributes. If `size` is large enough to hold the `\0` byte separated list of the attribute
    /// names, then the file system should reply with `ListxattrReply::Names` and the list. If
    /// `size` is not 0 but is also not large enough to hold the list, then the file system should
    /// reply with an `ERANGE` error.
    ///
    /// If this method fails with an `ENOSYS` error, then the kernel will treat that as a permanent
    /// failure. The kernel will return `EOPNOTSUPP` for all future calls to `listxattr` without
    /// forwarding them to the file system.
    fn listxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        size: u32,
    ) -> io::Result<ListxattrReply> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Remove an extended attribute.
    ///
    /// If this method fails with an `ENOSYS` error, then the kernel will treat that as a permanent
    /// failure. The kernel will return `EOPNOTSUPP` for all future calls to `removexattr` without
    /// forwarding them to the file system.
    fn removexattr(&self, ctx: &Context, inode: Self::Inode, name: &CStr) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Open a directory for reading.
    ///
    /// The file system may choose to return a `Handle` to refer to the newly opened directory. The
    /// kernel will then use this `Handle` for all operations on the content of the directory
    /// (`readdir`, `readdirplus`, `fsyncdir`, `releasedir`). If the file system does not return a
    /// `Handle` then the kernel will use the `Inode` for the directory to operate on its contents.
    /// In this case the file system may wish to enable the `FsOptions::ZERO_MESSAGE_OPENDIR`
    /// feature if it is supported by the kernel (see below).
    ///
    /// The returned `OpenOptions` allow the file system to change the way the opened directory is
    /// handled by the kernel. See the documentation of `OpenOptions` for more information.
    ///
    /// If the `FsOptions::ZERO_MESSAGE_OPENDIR` feature is enabled by both the file system
    /// implementation and the kernel, then the file system may return an error of `ENOSYS`. This
    /// will be interpreted by the kernel as success and future calls to `opendir` and `releasedir`
    /// will be handled by the kernel without being passed on to the file system.
    fn opendir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
    ) -> io::Result<(Option<Self::Handle>, OpenOptions)> {
        // Matches the behavior of libfuse.
        Ok((None, OpenOptions::empty()))
    }

    /// Read a directory.
    ///
    /// `handle` is the `Handle` returned by the file system from the `opendir` method, if any. If
    /// the file system did not return a `Handle` from `opendir` then the contents of `handle` are
    /// undefined.
    ///
    /// `size` indicates the maximum number of bytes that should be returned by this method.
    ///
    /// If `offset` is non-zero then it corresponds to one of the `offset` values from a `DirEntry`
    /// that was previously returned by a call to `readdir` for the same handle. In this case the
    /// file system should skip over the entries before the position defined by the `offset` value.
    /// If entries were added or removed while the `Handle` is open then the file system may still
    /// include removed entries or skip newly created entries. However, adding or removing entries
    /// should never cause the file system to skip over unrelated entries or include an entry more
    /// than once. This means that `offset` cannot be a simple index and must include sufficient
    /// information to uniquely determine the next entry in the list even when the set of entries is
    /// being changed.
    ///
    /// The file system may return entries for the current directory (".") and parent directory
    /// ("..") but is not required to do so. If the file system does not return these entries, then
    /// they are implicitly added by the kernel.
    ///
    /// The lookup count for `Inode`s associated with the returned directory entries is **NOT**
    /// affected by this method.
    ///
    // TODO(chirantan): Change method signature to return `Iterator<DirEntry>` rather than using an
    // `FnMut` for adding entries.
    fn readdir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry) -> io::Result<usize>,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Read a directory with entry attributes.
    ///
    /// Like `readdir` but also includes the attributes for each directory entry.
    ///
    /// `handle` is the `Handle` returned by the file system from the `opendir` method, if any. If
    /// the file system did not return a `Handle` from `opendir` then the contents of `handle` are
    /// undefined.
    ///
    /// `size` indicates the maximum number of bytes that should be returned by this method.
    ///
    /// Unlike `readdir`, the lookup count for `Inode`s associated with the returned directory
    /// entries **IS** affected by this method (since it returns an `Entry` for each `DirEntry`).
    /// The count for each `Inode` should be increased by 1.
    ///
    /// File systems that implement this method should enable the `FsOptions::DO_READDIRPLUS`
    /// feature when supported by the kernel. The kernel will not call this method unless that
    /// feature is enabled.
    ///
    /// Additionally, file systems that implement both `readdir` and `readdirplus` should enable the
    /// `FsOptions::READDIRPLUS_AUTO` feature to allow the kernel to issue both `readdir` and
    /// `readdirplus` requests, depending on how much information is expected to be required.
    ///
    /// TODO(chirantan): Change method signature to return `Iterator<(DirEntry, Entry)>` rather than
    /// using an `FnMut` for adding entries.
    fn readdirplus(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry, Entry) -> io::Result<usize>,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Synchronize the contents of a directory.
    ///
    /// File systems must ensure that the directory contents have been flushed to disk before
    /// returning from this method. If `datasync` is true then only the directory data (but not the
    /// metadata) needs to be flushed.
    ///
    /// `handle` is the `Handle` returned by the file system from the `opendir` method, if any. If
    /// the file system did not return a `Handle` from `opendir` then the contents of
    /// `handle` are undefined.
    ///
    /// If this method returns an `ENOSYS` error then the kernel will treat it as success and all
    /// subsequent calls to `fsyncdir` will be handled by the kernel without being forwarded to the
    /// file system.
    fn fsyncdir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        datasync: bool,
        handle: Self::Handle,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Release an open directory.
    ///
    /// For every `opendir` call there will be exactly one `releasedir` call (unless the file system
    /// is force-unmounted).
    ///
    /// `handle` is the `Handle` returned by the file system from the `opendir` method, if any. If
    /// the file system did not return a `Handle` from `opendir` then the contents of `handle` are
    /// undefined.
    ///
    /// `flags` contains used the flags used to open the directory in `opendir`.
    fn releasedir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        handle: Self::Handle,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    #[cfg(feature = "virtiofs")]
    /// Setup a mapping so that guest can access files in DAX style.
    ///
    /// The virtio-fs DAX Window allows bypassing guest page cache and allows mapping host
    /// page cache directly in guest address space.
    ///
    /// When a page of file is needed, guest sends a request to map that page (in host page cache)
    /// in VMM address space. Inside guest this is a physical memory range controlled by virtiofs
    /// device. And guest directly maps this physical address range using DAX and hence gets
    /// access to file data on host.
    ///
    /// This can speed up things considerably in many situations. Also this can result in
    /// substantial memory savings as file data does not have to be copied in guest and it is
    /// directly accessed from host page cache.
    #[allow(clippy::too_many_arguments)]
    fn setupmapping(
        &self,
        _ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        foffset: u64,
        len: u64,
        flags: u64,
        moffset: u64,
        vu_req: &mut dyn FsCacheReqHandler,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    #[cfg(feature = "virtiofs")]
    /// Teardown a mapping which was setup for guest DAX style access.
    fn removemapping(
        &self,
        _ctx: &Context,
        _inode: Self::Inode,
        requests: Vec<RemovemappingOne>,
        vu_req: &mut dyn FsCacheReqHandler,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Check file access permissions.
    ///
    /// This method is called when a userspace process in the client makes an `access()` or
    /// `chdir()` system call. If the file system was mounted with the `-o default_permissions`
    /// mount option, then the kernel will perform these checks itself and this method will not be
    /// called.
    ///
    /// If this method returns an `ENOSYS` error, then the kernel will treat it as a permanent
    /// success: all future calls to `access` will return success without being forwarded to the
    /// file system.
    fn access(&self, ctx: &Context, inode: Self::Inode, mask: u32) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Reposition read/write file offset.
    fn lseek(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        offset: u64,
        whence: u32,
    ) -> io::Result<u64> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Query file lock status
    fn getlk(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<FileLock> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Grab a file read lock
    fn setlk(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Grab a file write lock
    fn setlkw(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// send ioctl to the file
    #[allow(clippy::too_many_arguments)]
    fn ioctl(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        flags: u32,
        cmd: u32,
        data: IoctlData,
        out_size: u32,
    ) -> io::Result<IoctlData> {
        // Rather than ENOSYS, let's return ENOTTY so simulate that the ioctl call is implemented
        // but no ioctl number is supported.
        Err(io::Error::from_raw_os_error(libc::ENOTTY))
    }

    /// Query a file's block mapping info
    fn bmap(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        block: u64,
        blocksize: u32,
    ) -> io::Result<u64> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// Poll a file's events
    fn poll(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        khandle: Self::Handle,
        flags: u32,
        events: u32,
    ) -> io::Result<u32> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }

    /// TODO: support this
    fn notify_reply(&self) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(libc::ENOSYS))
    }
}

impl<FS: FileSystem> FileSystem for Arc<FS> {
    type Inode = FS::Inode;
    type Handle = FS::Handle;

    fn init(&self, capable: FsOptions) -> io::Result<FsOptions> {
        self.deref().init(capable)
    }

    fn destroy(&self) {
        self.deref().destroy()
    }

    fn lookup(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<Entry> {
        self.deref().lookup(ctx, parent, name)
    }

    fn forget(&self, ctx: &Context, inode: Self::Inode, count: u64) {
        self.deref().forget(ctx, inode, count)
    }

    fn batch_forget(&self, ctx: &Context, requests: Vec<(Self::Inode, u64)>) {
        self.deref().batch_forget(ctx, requests)
    }

    fn getattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Option<Self::Handle>,
    ) -> io::Result<(stat64, Duration)> {
        self.deref().getattr(ctx, inode, handle)
    }

    fn setattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        attr: stat64,
        handle: Option<Self::Handle>,
        valid: SetattrValid,
    ) -> io::Result<(stat64, Duration)> {
        self.deref().setattr(ctx, inode, attr, handle, valid)
    }

    fn readlink(&self, ctx: &Context, inode: Self::Inode) -> io::Result<Vec<u8>> {
        self.deref().readlink(ctx, inode)
    }

    fn symlink(
        &self,
        ctx: &Context,
        linkname: &CStr,
        parent: Self::Inode,
        name: &CStr,
    ) -> io::Result<Entry> {
        self.deref().symlink(ctx, linkname, parent, name)
    }

    fn mknod(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        mode: u32,
        rdev: u32,
        umask: u32,
    ) -> io::Result<Entry> {
        self.deref().mknod(ctx, inode, name, mode, rdev, umask)
    }

    fn mkdir(
        &self,
        ctx: &Context,
        parent: Self::Inode,
        name: &CStr,
        mode: u32,
        umask: u32,
    ) -> io::Result<Entry> {
        self.deref().mkdir(ctx, parent, name, mode, umask)
    }

    fn unlink(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<()> {
        self.deref().unlink(ctx, parent, name)
    }

    fn rmdir(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<()> {
        self.deref().rmdir(ctx, parent, name)
    }

    fn rename(
        &self,
        ctx: &Context,
        olddir: Self::Inode,
        oldname: &CStr,
        newdir: Self::Inode,
        newname: &CStr,
        flags: u32,
    ) -> io::Result<()> {
        self.deref()
            .rename(ctx, olddir, oldname, newdir, newname, flags)
    }

    fn link(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        newparent: Self::Inode,
        newname: &CStr,
    ) -> io::Result<Entry> {
        self.deref().link(ctx, inode, newparent, newname)
    }

    fn open(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        fuse_flags: u32,
    ) -> io::Result<(Option<Self::Handle>, OpenOptions)> {
        self.deref().open(ctx, inode, flags, fuse_flags)
    }

    fn create(
        &self,
        ctx: &Context,
        parent: Self::Inode,
        name: &CStr,
        args: CreateIn,
    ) -> io::Result<(Entry, Option<Self::Handle>, OpenOptions)> {
        self.deref().create(ctx, parent, name, args)
    }

    fn read(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        w: &mut dyn ZeroCopyWriter,
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        flags: u32,
    ) -> io::Result<usize> {
        self.deref()
            .read(ctx, inode, handle, w, size, offset, lock_owner, flags)
    }

    #[allow(clippy::too_many_arguments)]
    fn write(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        r: &mut dyn ZeroCopyReader,
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        delayed_write: bool,
        flags: u32,
        fuse_flags: u32,
    ) -> io::Result<usize> {
        self.deref().write(
            ctx,
            inode,
            handle,
            r,
            size,
            offset,
            lock_owner,
            delayed_write,
            flags,
            fuse_flags,
        )
    }

    fn flush(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        lock_owner: u64,
    ) -> io::Result<()> {
        self.deref().flush(ctx, inode, handle, lock_owner)
    }

    fn fsync(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        datasync: bool,
        handle: Self::Handle,
    ) -> io::Result<()> {
        self.deref().fsync(ctx, inode, datasync, handle)
    }

    fn fallocate(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        mode: u32,
        offset: u64,
        length: u64,
    ) -> io::Result<()> {
        self.deref()
            .fallocate(ctx, inode, handle, mode, offset, length)
    }

    #[allow(clippy::too_many_arguments)]
    fn release(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        handle: Self::Handle,
        flush: bool,
        flock_release: bool,
        lock_owner: Option<u64>,
    ) -> io::Result<()> {
        self.deref()
            .release(ctx, inode, flags, handle, flush, flock_release, lock_owner)
    }

    fn statfs(&self, ctx: &Context, inode: Self::Inode) -> io::Result<statvfs64> {
        self.deref().statfs(ctx, inode)
    }

    fn setxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        value: &[u8],
        flags: u32,
    ) -> io::Result<()> {
        self.deref().setxattr(ctx, inode, name, value, flags)
    }

    fn getxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        size: u32,
    ) -> io::Result<GetxattrReply> {
        self.deref().getxattr(ctx, inode, name, size)
    }

    fn listxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        size: u32,
    ) -> io::Result<ListxattrReply> {
        self.deref().listxattr(ctx, inode, size)
    }

    fn removexattr(&self, ctx: &Context, inode: Self::Inode, name: &CStr) -> io::Result<()> {
        self.deref().removexattr(ctx, inode, name)
    }

    fn opendir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
    ) -> io::Result<(Option<Self::Handle>, OpenOptions)> {
        self.deref().opendir(ctx, inode, flags)
    }

    fn readdir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry) -> io::Result<usize>,
    ) -> io::Result<()> {
        self.deref()
            .readdir(ctx, inode, handle, size, offset, add_entry)
    }

    fn readdirplus(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry, Entry) -> io::Result<usize>,
    ) -> io::Result<()> {
        self.deref()
            .readdirplus(ctx, inode, handle, size, offset, add_entry)
    }

    fn fsyncdir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        datasync: bool,
        handle: Self::Handle,
    ) -> io::Result<()> {
        self.deref().fsyncdir(ctx, inode, datasync, handle)
    }

    fn releasedir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        handle: Self::Handle,
    ) -> io::Result<()> {
        self.deref().releasedir(ctx, inode, flags, handle)
    }

    #[cfg(feature = "virtiofs")]
    #[allow(clippy::too_many_arguments)]
    fn setupmapping(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        foffset: u64,
        len: u64,
        flags: u64,
        moffset: u64,
        vu_req: &mut dyn FsCacheReqHandler,
    ) -> io::Result<()> {
        self.deref()
            .setupmapping(ctx, inode, handle, foffset, len, flags, moffset, vu_req)
    }

    #[cfg(feature = "virtiofs")]
    fn removemapping(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        requests: Vec<RemovemappingOne>,
        vu_req: &mut dyn FsCacheReqHandler,
    ) -> io::Result<()> {
        self.deref().removemapping(ctx, inode, requests, vu_req)
    }

    fn access(&self, ctx: &Context, inode: Self::Inode, mask: u32) -> io::Result<()> {
        self.deref().access(ctx, inode, mask)
    }

    fn lseek(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        offset: u64,
        whence: u32,
    ) -> io::Result<u64> {
        self.deref().lseek(ctx, inode, handle, offset, whence)
    }

    /// Query file lock status
    fn getlk(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<FileLock> {
        self.deref().getlk(ctx, inode, handle, owner, lock, flags)
    }

    /// Grab a file read lock
    fn setlk(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<()> {
        self.deref().setlk(ctx, inode, handle, owner, lock, flags)
    }

    /// Grab a file write lock
    fn setlkw(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<()> {
        self.deref().setlkw(ctx, inode, handle, owner, lock, flags)
    }

    /// send ioctl to the file
    #[allow(clippy::too_many_arguments)]
    fn ioctl(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        flags: u32,
        cmd: u32,
        data: IoctlData,
        out_size: u32,
    ) -> io::Result<IoctlData> {
        self.deref()
            .ioctl(ctx, inode, handle, flags, cmd, data, out_size)
    }

    /// Query a file's block mapping info
    fn bmap(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        block: u64,
        blocksize: u32,
    ) -> io::Result<u64> {
        self.deref().bmap(ctx, inode, block, blocksize)
    }

    /// Poll a file's events
    fn poll(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        khandle: Self::Handle,
        flags: u32,
        events: u32,
    ) -> io::Result<u32> {
        self.deref()
            .poll(ctx, inode, handle, khandle, flags, events)
    }

    /// Send notify reply.
    fn notify_reply(&self) -> io::Result<()> {
        self.deref().notify_reply()
    }
}
