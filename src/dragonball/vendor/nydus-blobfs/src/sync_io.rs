// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.

//! Fuse passthrough file system, mirroring an existing FS hierarchy.

use super::*;
use fuse_backend_rs::abi::fuse_abi::CreateIn;
#[cfg(feature = "virtiofs")]
use fuse_backend_rs::abi::virtio_fs;
#[cfg(feature = "virtiofs")]
use fuse_backend_rs::transport::FsCacheReqHandler;
use nydus_error::eacces;
#[cfg(feature = "virtiofs")]
use nydus_storage::device::BlobPrefetchRequest;
#[cfg(feature = "virtiofs")]
use std::cmp::min;
use std::ffi::CStr;
use std::io;
#[cfg(feature = "virtiofs")]
use std::path::Path;
use std::time::Duration;

impl BlobFs {
    #[cfg(feature = "virtiofs")]
    fn check_st_size(blob_id: &Path, size: i64) -> io::Result<()> {
        if size < 0 {
            return Err(einval!(format!(
                "load_chunks_on_demand: blob_id {:?}, size: {:?} is less than 0",
                blob_id, size
            )));
        }
        Ok(())
    }

    #[cfg(feature = "virtiofs")]
    fn get_blob_id_and_size(&self, inode: Inode) -> io::Result<(String, u64)> {
        // locate blob file that the inode refers to
        let blob_id_full_path = self.pfs.readlinkat_proc_file(inode)?;
        let parent = blob_id_full_path
            .parent()
            .ok_or_else(|| einval!("blobfs: failed to find parent"))?;

        trace!(
            "parent: {:?}, blob id path: {:?}",
            parent,
            blob_id_full_path
        );

        let blob_file = Self::open_file(
            libc::AT_FDCWD,
            blob_id_full_path.as_path(),
            libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0,
        )
        .map_err(|e| einval!(e))?;
        let st = Self::stat(&blob_file).map_err(|e| {
            error!("get_blob_id_and_size: stat failed {:?}", e);
            e
        })?;
        let blob_id = blob_id_full_path
            .file_name()
            .ok_or_else(|| einval!("blobfs: failed to find blob file"))?;

        trace!("load_chunks_on_demand: blob_id {:?}", blob_id);

        Self::check_st_size(blob_id_full_path.as_path(), st.st_size)?;

        Ok((
            blob_id.to_os_string().into_string().unwrap(),
            st.st_size as u64,
        ))
    }

    #[cfg(feature = "virtiofs")]
    fn load_chunks_on_demand(&self, inode: Inode, offset: u64) -> io::Result<()> {
        // prepare BlobPrefetchRequest and call device.prefetch().
        // Make sure prefetch doesn't use delay_persist as we need the
        // data immediately.
        let (blob_id, size) = self.get_blob_id_and_size(inode)?;
        if size <= offset {
            return Err(einval!(format!(
                "load_chunks_on_demand: blob_id {:?}, offset {:?} is larger than size {:?}",
                blob_id, offset, size
            )));
        }

        let len = size - offset;
        let req = BlobPrefetchRequest {
            blob_id,
            offset,
            len: min(len, 0x0020_0000_u64), // 2M range
        };

        self.bootstrap_args.fetch_range_sync(&[req]).map_err(|e| {
            warn!("load chunks: error, {:?}", e);
            e
        })
    }
}

impl FileSystem for BlobFs {
    type Inode = Inode;
    type Handle = Handle;

    fn init(&self, capable: FsOptions) -> io::Result<FsOptions> {
        #[cfg(feature = "virtiofs")]
        let _ = self.bootstrap_args.get_rafs_handle()?;
        self.pfs.init(capable)
    }

    fn destroy(&self) {
        self.pfs.destroy()
    }

    fn statfs(&self, _ctx: &Context, inode: Inode) -> io::Result<libc::statvfs64> {
        self.pfs.statfs(_ctx, inode)
    }

    fn lookup(&self, _ctx: &Context, parent: Inode, name: &CStr) -> io::Result<Entry> {
        self.pfs.lookup(_ctx, parent, name)
    }

    fn forget(&self, _ctx: &Context, inode: Inode, count: u64) {
        self.pfs.forget(_ctx, inode, count)
    }

    fn batch_forget(&self, _ctx: &Context, requests: Vec<(Inode, u64)>) {
        self.pfs.batch_forget(_ctx, requests)
    }

    fn opendir(
        &self,
        _ctx: &Context,
        inode: Inode,
        flags: u32,
    ) -> io::Result<(Option<Handle>, OpenOptions)> {
        self.pfs.opendir(_ctx, inode, flags)
    }

    fn releasedir(
        &self,
        _ctx: &Context,
        inode: Inode,
        _flags: u32,
        handle: Handle,
    ) -> io::Result<()> {
        self.pfs.releasedir(_ctx, inode, _flags, handle)
    }

    #[allow(unused)]
    fn mkdir(
        &self,
        _ctx: &Context,
        _parent: Inode,
        _name: &CStr,
        _mode: u32,
        _umask: u32,
    ) -> io::Result<Entry> {
        error!("do mkdir req error: blob file can not be written.");
        Err(eacces!("Mkdir request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn rmdir(&self, _ctx: &Context, _parent: Inode, _name: &CStr) -> io::Result<()> {
        error!("do rmdir req error: blob file can not be written.");
        Err(eacces!("Rmdir request is not allowed in blobfs"))
    }

    fn readdir(
        &self,
        _ctx: &Context,
        inode: Inode,
        handle: Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry) -> io::Result<usize>,
    ) -> io::Result<()> {
        self.pfs
            .readdir(_ctx, inode, handle, size, offset, add_entry)
    }

    fn readdirplus(
        &self,
        _ctx: &Context,
        inode: Inode,
        handle: Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry, Entry) -> io::Result<usize>,
    ) -> io::Result<()> {
        self.pfs
            .readdirplus(_ctx, inode, handle, size, offset, add_entry)
    }

    fn open(
        &self,
        _ctx: &Context,
        inode: Inode,
        flags: u32,
        _fuse_flags: u32,
    ) -> io::Result<(Option<Handle>, OpenOptions)> {
        self.pfs.open(_ctx, inode, flags, _fuse_flags)
    }

    fn release(
        &self,
        _ctx: &Context,
        inode: Inode,
        _flags: u32,
        handle: Handle,
        _flush: bool,
        _flock_release: bool,
        _lock_owner: Option<u64>,
    ) -> io::Result<()> {
        self.pfs.release(
            _ctx,
            inode,
            _flags,
            handle,
            _flush,
            _flock_release,
            _lock_owner,
        )
    }

    #[allow(unused)]
    fn create(
        &self,
        _ctx: &Context,
        _parent: Inode,
        _name: &CStr,
        _args: CreateIn,
    ) -> io::Result<(Entry, Option<Handle>, OpenOptions)> {
        error!("do create req error: blob file cannot write.");
        Err(eacces!("Create request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn unlink(&self, _ctx: &Context, _parent: Inode, _name: &CStr) -> io::Result<()> {
        error!("do unlink req error: blob file cannot write.");
        Err(eacces!("Unlink request is not allowed in blobfs"))
    }

    #[cfg(feature = "virtiofs")]
    fn setupmapping(
        &self,
        _ctx: &Context,
        inode: Inode,
        _handle: Handle,
        foffset: u64,
        len: u64,
        flags: u64,
        moffset: u64,
        vu_req: &mut dyn FsCacheReqHandler,
    ) -> io::Result<()> {
        debug!(
            "blobfs: setupmapping ino {:?} foffset {} len {} flags {} moffset {}",
            inode, foffset, len, flags, moffset
        );

        if (flags & virtio_fs::SetupmappingFlags::WRITE.bits()) != 0 {
            return Err(eacces!("blob file cannot write in dax"));
        }
        self.load_chunks_on_demand(inode, foffset)?;
        self.pfs
            .setupmapping(_ctx, inode, _handle, foffset, len, flags, moffset, vu_req)
    }

    #[cfg(feature = "virtiofs")]
    fn removemapping(
        &self,
        _ctx: &Context,
        _inode: Inode,
        requests: Vec<virtio_fs::RemovemappingOne>,
        vu_req: &mut dyn FsCacheReqHandler,
    ) -> io::Result<()> {
        self.pfs.removemapping(_ctx, _inode, requests, vu_req)
    }

    fn read(
        &self,
        _ctx: &Context,
        _inode: Inode,
        _handle: Handle,
        _w: &mut dyn ZeroCopyWriter,
        _size: u32,
        _offset: u64,
        _lock_owner: Option<u64>,
        _flags: u32,
    ) -> io::Result<usize> {
        error!(
            "do Read req error: blob file cannot do nondax read, please check if dax is enabled"
        );
        Err(eacces!("Read request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn write(
        &self,
        _ctx: &Context,
        _inode: Inode,
        _handle: Handle,
        _r: &mut dyn ZeroCopyReader,
        _size: u32,
        _offset: u64,
        _lock_owner: Option<u64>,
        _delayed_write: bool,
        _flags: u32,
        _fuse_flags: u32,
    ) -> io::Result<usize> {
        error!("do Write req error: blob file cannot write.");
        Err(eacces!("Write request is not allowed in blobfs"))
    }

    fn getattr(
        &self,
        _ctx: &Context,
        inode: Inode,
        _handle: Option<Handle>,
    ) -> io::Result<(libc::stat64, Duration)> {
        self.pfs.getattr(_ctx, inode, _handle)
    }

    #[allow(unused)]
    fn setattr(
        &self,
        _ctx: &Context,
        _inode: Inode,
        _attr: libc::stat64,
        _handle: Option<Handle>,
        _valid: SetattrValid,
    ) -> io::Result<(libc::stat64, Duration)> {
        error!("do setattr req error: blob file cannot write.");
        Err(eacces!("Setattr request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn rename(
        &self,
        _ctx: &Context,
        _olddir: Inode,
        _oldname: &CStr,
        _newdir: Inode,
        _newname: &CStr,
        _flags: u32,
    ) -> io::Result<()> {
        error!("do rename req error: blob file cannot write.");
        Err(eacces!("Rename request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn mknod(
        &self,
        _ctx: &Context,
        _parent: Inode,
        _name: &CStr,
        _mode: u32,
        _rdev: u32,
        _umask: u32,
    ) -> io::Result<Entry> {
        error!("do mknode req error: blob file cannot write.");
        Err(eacces!("Mknod request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn link(
        &self,
        _ctx: &Context,
        _inode: Inode,
        _newparent: Inode,
        _newname: &CStr,
    ) -> io::Result<Entry> {
        error!("do link req error: blob file cannot write.");
        Err(eacces!("Link request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn symlink(
        &self,
        _ctx: &Context,
        _linkname: &CStr,
        _parent: Inode,
        _name: &CStr,
    ) -> io::Result<Entry> {
        error!("do symlink req error: blob file cannot write.");
        Err(eacces!("Symlink request is not allowed in blobfs"))
    }

    fn readlink(&self, _ctx: &Context, inode: Inode) -> io::Result<Vec<u8>> {
        self.pfs.readlink(_ctx, inode)
    }

    fn flush(
        &self,
        _ctx: &Context,
        inode: Inode,
        handle: Handle,
        _lock_owner: u64,
    ) -> io::Result<()> {
        self.pfs.flush(_ctx, inode, handle, _lock_owner)
    }

    fn fsync(
        &self,
        _ctx: &Context,
        inode: Inode,
        datasync: bool,
        handle: Handle,
    ) -> io::Result<()> {
        self.pfs.fsync(_ctx, inode, datasync, handle)
    }

    fn fsyncdir(
        &self,
        ctx: &Context,
        inode: Inode,
        datasync: bool,
        handle: Handle,
    ) -> io::Result<()> {
        self.pfs.fsyncdir(ctx, inode, datasync, handle)
    }

    fn access(&self, ctx: &Context, inode: Inode, mask: u32) -> io::Result<()> {
        self.pfs.access(ctx, inode, mask)
    }

    #[allow(unused)]
    fn setxattr(
        &self,
        _ctx: &Context,
        _inode: Inode,
        _name: &CStr,
        _value: &[u8],
        _flags: u32,
    ) -> io::Result<()> {
        error!("do setxattr req error: blob file cannot write.");
        Err(eacces!("Setxattr request is not allowed in blobfs"))
    }

    fn getxattr(
        &self,
        _ctx: &Context,
        inode: Inode,
        name: &CStr,
        size: u32,
    ) -> io::Result<GetxattrReply> {
        self.pfs.getxattr(_ctx, inode, name, size)
    }

    fn listxattr(&self, _ctx: &Context, inode: Inode, size: u32) -> io::Result<ListxattrReply> {
        self.pfs.listxattr(_ctx, inode, size)
    }

    #[allow(unused)]
    fn removexattr(&self, _ctx: &Context, _inode: Inode, _name: &CStr) -> io::Result<()> {
        error!("do removexattr req error: blob file cannot write.");
        Err(eacces!("Removexattr request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn fallocate(
        &self,
        _ctx: &Context,
        _inode: Inode,
        _handle: Handle,
        _mode: u32,
        _offset: u64,
        _length: u64,
    ) -> io::Result<()> {
        error!("do fallocate req error: blob file cannot write.");
        Err(eacces!("Fallocate request is not allowed in blobfs"))
    }

    #[allow(unused)]
    fn lseek(
        &self,
        _ctx: &Context,
        inode: Inode,
        handle: Handle,
        offset: u64,
        whence: u32,
    ) -> io::Result<u64> {
        self.pfs.lseek(_ctx, inode, handle, offset, whence)
    }
}
