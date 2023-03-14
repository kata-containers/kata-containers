// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io;

use async_trait::async_trait;

use super::*;

#[async_trait]
impl AsyncFileSystem for Vfs {
    async fn async_lookup(
        &self,
        ctx: &Context,
        parent: <Self as FileSystem>::Inode,
        name: &CStr,
    ) -> Result<Entry> {
        // Don't use is_safe_path_component(), allow "." and ".." for NFS export support
        if name.to_bytes_with_nul().contains(&SLASH_ASCII) {
            return Err(io::Error::from_raw_os_error(libc::EINVAL));
        }

        match self.get_real_rootfs(parent)? {
            (Left(fs), idata) => self.lookup_pseudo(fs, idata, ctx, name),
            (Right(fs), idata) => {
                // parent is in an underlying rootfs
                let mut entry = fs.async_lookup(ctx, idata.ino(), name).await?;
                // lookup success, hash it to a real fuse inode
                entry.inode = self.convert_inode(idata.fs_idx(), entry.inode)?;
                Ok(entry)
            }
        }
    }

    async fn async_getattr(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        handle: Option<<Self as FileSystem>::Handle>,
    ) -> Result<(libc::stat64, Duration)> {
        match self.get_real_rootfs(inode)? {
            (Left(fs), idata) => fs.getattr(ctx, idata.ino(), handle),
            (Right(fs), idata) => fs.async_getattr(ctx, idata.ino(), handle).await,
        }
    }

    async fn async_setattr(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        attr: libc::stat64,
        handle: Option<<Self as FileSystem>::Handle>,
        valid: SetattrValid,
    ) -> Result<(libc::stat64, Duration)> {
        match self.get_real_rootfs(inode)? {
            (Left(fs), idata) => fs.setattr(ctx, idata.ino(), attr, handle, valid),
            (Right(fs), idata) => {
                fs.async_setattr(ctx, idata.ino(), attr, handle, valid)
                    .await
            }
        }
    }

    async fn async_open(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        flags: u32,
        fuse_flags: u32,
    ) -> Result<(Option<<Self as FileSystem>::Handle>, OpenOptions)> {
        if self.opts.load().no_open {
            Err(Error::from_raw_os_error(libc::ENOSYS))
        } else {
            match self.get_real_rootfs(inode)? {
                (Left(fs), idata) => fs.open(ctx, idata.ino(), flags, fuse_flags),
                (Right(fs), idata) => fs
                    .async_open(ctx, idata.ino(), flags, fuse_flags)
                    .await
                    .map(|(h, opt)| (h.map(Into::into), opt)),
            }
        }
    }

    async fn async_create(
        &self,
        ctx: &Context,
        parent: <Self as FileSystem>::Inode,
        name: &CStr,
        args: CreateIn,
    ) -> Result<(Entry, Option<<Self as FileSystem>::Handle>, OpenOptions)> {
        validate_path_component(name)?;

        match self.get_real_rootfs(parent)? {
            (Left(fs), idata) => fs.create(ctx, idata.ino(), name, args),
            (Right(fs), idata) => {
                fs.async_create(ctx, idata.ino(), name, args)
                    .await
                    .map(|(mut a, b, c)| {
                        a.inode = self.convert_inode(idata.fs_idx(), a.inode)?;
                        Ok((a, b, c))
                    })?
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn async_read(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        handle: <Self as FileSystem>::Handle,
        w: &mut (dyn AsyncZeroCopyWriter + Send),
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        flags: u32,
    ) -> Result<usize> {
        match self.get_real_rootfs(inode)? {
            (Left(_fs), _idata) => Err(io::Error::from_raw_os_error(libc::ENOSYS)),
            (Right(fs), idata) => {
                fs.async_read(ctx, idata.ino(), handle, w, size, offset, lock_owner, flags)
                    .await
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn async_write(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        handle: <Self as FileSystem>::Handle,
        r: &mut (dyn AsyncZeroCopyReader + Send),
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        delayed_write: bool,
        flags: u32,
        fuse_flags: u32,
    ) -> Result<usize> {
        match self.get_real_rootfs(inode)? {
            (Left(_fs), _idata) => Err(io::Error::from_raw_os_error(libc::ENOSYS)),
            (Right(fs), idata) => {
                fs.async_write(
                    ctx,
                    idata.ino(),
                    handle,
                    r,
                    size,
                    offset,
                    lock_owner,
                    delayed_write,
                    flags,
                    fuse_flags,
                )
                .await
            }
        }
    }

    async fn async_fsync(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        datasync: bool,
        handle: <Self as FileSystem>::Handle,
    ) -> Result<()> {
        match self.get_real_rootfs(inode)? {
            (Left(fs), idata) => fs.fsync(ctx, idata.ino(), datasync, handle),
            (Right(fs), idata) => fs.async_fsync(ctx, idata.ino(), datasync, handle).await,
        }
    }

    async fn async_fallocate(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        handle: <Self as FileSystem>::Handle,
        mode: u32,
        offset: u64,
        length: u64,
    ) -> Result<()> {
        match self.get_real_rootfs(inode)? {
            (Left(fs), idata) => fs.fallocate(ctx, idata.ino(), handle, mode, offset, length),
            (Right(fs), idata) => {
                fs.async_fallocate(ctx, idata.ino(), handle, mode, offset, length)
                    .await
            }
        }
    }

    async fn async_fsyncdir(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        datasync: bool,
        handle: <Self as FileSystem>::Handle,
    ) -> Result<()> {
        match self.get_real_rootfs(inode)? {
            (Left(fs), idata) => fs.fsyncdir(ctx, idata.ino(), datasync, handle),
            (Right(fs), idata) => fs.async_fsyncdir(ctx, idata.ino(), datasync, handle).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::FakeFileSystemOne;
    use super::*;
    use crate::api::Vfs;

    use std::ffi::CString;

    #[tokio::test]
    async fn test_vfs_async_lookup() {
        let vfs = Vfs::new(VfsOptions::default());
        let fs = FakeFileSystemOne {};
        let ctx = Context {
            uid: 0,
            gid: 0,
            pid: 0,
        };

        assert!(vfs.mount(Box::new(fs), "/x/y").is_ok());

        let handle = tokio::spawn(async move {
            // Lookup inode on pseudo file system.
            let name = CString::new("x").unwrap();
            let future = vfs.async_lookup(&ctx, ROOT_ID.into(), name.as_c_str());
            let entry1 = future.await.unwrap();
            assert_eq!(entry1.inode, 0x2);

            // Lookup inode on mounted file system.
            let entry2 = vfs
                .async_lookup(
                    &ctx,
                    entry1.inode.into(),
                    CString::new("y").unwrap().as_c_str(),
                )
                .await
                .unwrap();
            assert_eq!(entry2.inode, 0x100_0000_0000_0001);

            // lookup for negative result.
            let entry3 = vfs
                .async_lookup(
                    &ctx,
                    entry2.inode.into(),
                    CString::new("z").unwrap().as_c_str(),
                )
                .await
                .unwrap();
            assert_eq!(entry3.inode, 0);
        });
        handle.await.unwrap();
    }
}
