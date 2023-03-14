// Copyright (C) 2021-2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]

use std::io;
use std::mem::ManuallyDrop;

use async_trait::async_trait;

use super::*;
use crate::abi::fuse_abi::{
    CreateIn, OpenOptions, SetattrValid, FOPEN_IN_KILL_SUIDGID, WRITE_KILL_PRIV,
};
use crate::api::filesystem::{
    AsyncFileSystem, AsyncZeroCopyReader, AsyncZeroCopyWriter, Context, FileSystem,
};

impl<S: BitmapSlice + Send + Sync + 'static> BackendFileSystem for PassthroughFs<S> {
    fn mount(&self) -> io::Result<(Entry, u64)> {
        let entry = self.do_lookup(fuse::ROOT_ID, &CString::new(".").unwrap())?;
        Ok((entry, VFS_MAX_INO))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<'a> InodeData {
    async fn async_get_file(&self, mount_fds: &MountFds) -> io::Result<InodeFile<'_>> {
        // The io_uring doesn't support open_by_handle_at yet, so use sync io.
        self.get_file(mount_fds)
    }
}

impl<S: BitmapSlice + Send + Sync> PassthroughFs<S> {
    /*
    async fn async_open_file(
        &self,
        ctx: &Context,
        dir_fd: i32,
        pathname: &'_ CStr,
        flags: i32,
        mode: u32,
    ) -> io::Result<File> {
        AsyncUtil::open_at(drive, dir_fd, pathname, flags, mode)
            .await
            .map(|fd| unsafe { File::from_raw_fd(fd as i32) })
        }

        async fn async_open_proc_file(
            &self,
            ctx: &Context,
            fd: RawFd,
            flags: i32,
            mode: u32,
        ) -> io::Result<File> {
            if !is_safe_inode(mode) {
                return Err(ebadf());
            }

            let pathname = CString::new(format!("{}", fd))
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            // We don't really check `flags` because if the kernel can't handle poorly specified flags
            // then we have much bigger problems. Also, clear the `O_NOFOLLOW` flag if it is set since
            // we need to follow the `/proc/self/fd` symlink to get the file.
            self.async_open_file(
                ctx,
                self.proc_self_fd.as_raw_fd(),
                pathname.as_c_str(),
                (flags | libc::O_CLOEXEC) & (!libc::O_NOFOLLOW),
                0,
            )
            .await
        }

        /// Create a File or File Handle for `name` under directory `dir_fd` to support `lookup()`.
        async fn async_open_file_or_handle<F>(
            &self,
            ctx: &Context,
            dir_fd: RawFd,
            name: &CStr,
            reopen_dir: F,
        ) -> io::Result<(FileOrHandle, InodeStat, InodeAltKey, Option<InodeAltKey>)>
        where
            F: FnOnce(RawFd, libc::c_int, u32) -> io::Result<File>,
        {
            let handle = if self.cfg.inode_file_handles {
                FileHandle::from_name_at_with_mount_fds(dir_fd, name, &self.mount_fds, reopen_dir)
            } else {
                Err(io::Error::from_raw_os_error(libc::ENOTSUP))
            };

            // Ignore errors, because having a handle is optional
            let file_or_handle = if let Ok(h) = handle {
                FileOrHandle::Handle(h)
            } else {
                let f = self
                    .async_open_file(
                        ctx,
                        dir_fd,
                        name,
                        libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                        0,
                    )
                    .await?;

                FileOrHandle::File(f)
            };

            let st = match &file_or_handle {
                FileOrHandle::File(f) => {
                    // TODO: use statx(2) to query mntid when 5.8 kernel or later are widely used.
                    //
                    // Some filesystems don't support file handle, for example overlayfs mounted
                    // without index feature, if so just use mntid 0 in that case.
                    let mnt_id = match FileHandle::from_name_at(dir_fd, name) {
                        Ok(h) => h.mnt_id,
                        Err(_) => 0,
                    };
                    InodeStat {
                        stat: self.async_stat(ctx, f, None).await?,
                        mnt_id,
                    }
                }
                FileOrHandle::Handle(h) => InodeStat {
                    stat: self.async_stat_fd(ctx, dir_fd, Some(name)).await?,
                    mnt_id: h.mnt_id,
                },
            };
            let ids_altkey = InodeAltKey::ids_from_stat(&st);

            // Note that this will always be `None` if `cfg.inode_file_handles` is false, but we only
            // really need this alt key when we do not have an `O_PATH` fd open for every inode.  So if
            // `cfg.inode_file_handles` is false, we do not need this key anyway.
            let handle_altkey = file_or_handle.handle().map(|h| InodeAltKey::Handle(*h));

            Ok((file_or_handle, st, ids_altkey, handle_altkey))
        }

        async fn async_open_inode(
            &self,
            ctx: &Context,
            inode: Inode,
            mut flags: i32,
        ) -> io::Result<File> {
            // When writeback caching is enabled, the kernel may send read requests even if the
            // userspace program opened the file write-only. So we need to ensure that we have opened
            // the file for reading as well as writing.
            let writeback = self.writeback.load(Ordering::Relaxed);
            if writeback && flags & libc::O_ACCMODE == libc::O_WRONLY {
                flags &= !libc::O_ACCMODE;
                flags |= libc::O_RDWR;
            }

            // When writeback caching is enabled the kernel is responsible for handling `O_APPEND`.
            // However, this breaks atomicity as the file may have changed on disk, invalidating the
            // cached copy of the data in the kernel and the offset that the kernel thinks is the end of
            // the file. Just allow this for now as it is the user's responsibility to enable writeback
            // caching only for directories that are not shared. It also means that we need to clear the
            // `O_APPEND` flag.
            if writeback && flags & libc::O_APPEND != 0 {
                flags &= !libc::O_APPEND;
            }

            let data = self.inode_map.get(inode)?;
            let file = data.async_get_file(&self.mount_fds).await?;

            self.async_open_proc_file(ctx, file.as_raw_fd(), flags, data.mode)
                .await
        }

        async fn async_do_open(
            &self,
            ctx: &Context,
            inode: Inode,
            flags: u32,
            fuse_flags: u32,
        ) -> io::Result<(Option<Handle>, OpenOptions)> {
            let killpriv = if self.killpriv_v2.load(Ordering::Relaxed)
                && (fuse_flags & FOPEN_IN_KILL_SUIDGID != 0)
            {
                self::drop_cap_fsetid()?
            } else {
                None
            };
            let file = self.async_open_inode(ctx, inode, flags as i32).await?;
            drop(killpriv);

            let data = HandleData::new(inode, file);
            let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
            let mut opts = OpenOptions::empty();

            self.handle_map.insert(handle, data);
            match self.cfg.cache_policy {
                // We only set the direct I/O option on files.
                CachePolicy::Never => opts.set(
                    OpenOptions::DIRECT_IO,
                    flags & (libc::O_DIRECTORY as u32) == 0,
                ),
                CachePolicy::Always => opts |= OpenOptions::KEEP_CACHE,
                _ => {}
            };

            Ok((Some(handle), opts))
        }
        */

    async fn async_do_getattr(
        &self,
        ctx: &Context,
        inode: Inode,
        handle: Option<<Self as FileSystem>::Handle>,
    ) -> io::Result<(libc::stat64, Duration)> {
        unimplemented!()
        /*
        let st;
        let fd;
        let data = self.inode_map.get(inode).map_err(|e| {
            error!("fuse: do_getattr ino {} Not find err {:?}", inode, e);
            e
        })?;

        // kernel sends 0 as handle in case of no_open, and it depends on fuse server to handle
        // this case correctly.
        if !self.no_open.load(Ordering::Relaxed) && handle.is_some() {
            // Safe as we just checked handle
            let hd = self.handle_map.get(handle.unwrap(), inode)?;
            fd = hd.get_handle_raw_fd();
            st = self.async_stat_fd(ctx, fd, None).await;
        } else {
            match &data.file_or_handle {
                FileOrHandle::File(f) => {
                    fd = f.as_raw_fd();
                    st = self.async_stat_fd(ctx, fd, None).await;
                }
                FileOrHandle::Handle(_h) => {
                    let file = data.async_get_file(&self.mount_fds).await?;
                    fd = file.as_raw_fd();
                    st = self.async_stat_fd(ctx, fd, None).await;
                }
            }
        }

        let st = st.map_err(|e| {
            error!(
                "fuse: do_getattr stat failed ino {} fd: {:?} err {:?}",
                inode, fd, e
            );
            e
        })?;

        Ok((st, self.cfg.attr_timeout))
        */
    }

    /*
    async fn async_stat(
        &self,
        ctx: &Context,
        dir: &impl AsRawFd,
        path: Option<&CStr>,
    ) -> io::Result<libc::stat64> {
        self.async_stat_fd(ctx, dir.as_raw_fd(), path).await
    }

    async fn async_stat_fd(
        &self,
        _ctx: &Context,
        dir_fd: RawFd,
        path: Option<&CStr>,
    ) -> io::Result<libc::stat64> {
        // Safe because this is a constant value and a valid C string.
        let pathname =
            path.unwrap_or_else(|| unsafe { CStr::from_bytes_with_nul_unchecked(EMPTY_CSTR) });
        let mut st = MaybeUninit::<libc::stat64>::zeroed();

        // Safe because the kernel will only write data in `st` and we check the return value.
        let res = unsafe {
            libc::fstatat64(
                dir_fd,
                pathname.as_ptr(),
                st.as_mut_ptr(),
                libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if res >= 0 {
            // Safe because the kernel guarantees that the struct is now fully initialized.
            Ok(unsafe { st.assume_init() })
        } else {
            Err(io::Error::last_os_error())
        }
    }

    async fn async_get_data(
        &self,
        ctx: &Context,
        handle: Handle,
        inode: Inode,
        flags: libc::c_int,
    ) -> io::Result<Arc<HandleData>> {
        let no_open = self.no_open.load(Ordering::Relaxed);
        if !no_open {
            self.handle_map.get(handle, inode)
        } else {
            let file = self.async_open_inode(ctx, inode, flags as i32).await?;
            Ok(Arc::new(HandleData::new(inode, file)))
        }
    }
     */
}

#[async_trait]
impl<S: BitmapSlice + Send + Sync> AsyncFileSystem for PassthroughFs<S> {
    async fn async_lookup(
        &self,
        ctx: &Context,
        parent: <Self as FileSystem>::Inode,
        name: &CStr,
    ) -> io::Result<Entry> {
        unimplemented!()
        /*
        // Don't use is_safe_path_component(), allow "." and ".." for NFS export support
        if name.to_bytes_with_nul().contains(&SLASH_ASCII) {
            return Err(io::Error::from_raw_os_error(libc::EINVAL));
        }

        let dir = self.inode_map.get(parent)?;
        let dir_file = dir.async_get_file(&self.mount_fds).await?;
        let (file_or_handle, st, ids_altkey, handle_altkey) = self
            .async_open_file_or_handle(ctx, dir_file.as_raw_fd(), name, |fd, flags, mode| {
                Self::open_proc_file(&self.proc_self_fd, fd, flags, mode)
            })
            .await?;

        let mut attr_flags: u32 = 0;
        if let Some(dax_file_size) = self.cfg.dax_file_size {
            // st.stat.st_size is i64
            if self.perfile_dax.load(Ordering::Relaxed)
                && st.stat.st_size >= 0x0
                && st.stat.st_size as u64 >= dax_file_size
            {
                attr_flags |= fuse::FUSE_ATTR_DAX;
            }
        }

        let mut found = None;
        'search: loop {
            match self.inode_map.get_alt(&ids_altkey, handle_altkey.as_ref()) {
                // No existing entry found
                None => break 'search,
                Some(data) => {
                    let curr = data.refcount.load(Ordering::Acquire);
                    // forgot_one() has just destroyed the entry, retry...
                    if curr == 0 {
                        continue 'search;
                    }

                    // Saturating add to avoid integer overflow, it's not realistic to saturate u64.
                    let new = curr.saturating_add(1);

                    // Synchronizes with the forgot_one()
                    if data
                        .refcount
                        .compare_exchange(curr, new, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        found = Some(data.inode);
                        break;
                    }
                }
            }
        }

        let inode = if let Some(v) = found {
            v
        } else {
            // Write guard get_alt_locked() and insert_lock() to avoid race conditions.
            let mut inodes = self.inode_map.get_map_mut();

            // Lookup inode_map again after acquiring the inode_map lock, as there might be another
            // racing thread already added an inode with the same altkey while we're not holding
            // the lock. If so just use the newly added inode, otherwise the inode will be replaced
            // and results in EBADF.
            match InodeMap::get_alt_locked(inodes.deref(), &ids_altkey, handle_altkey.as_ref()) {
                Some(data) => {
                    trace!(
                        "fuse: do_lookup sees existing inode {} ids_altkey {:?}",
                        data.inode,
                        ids_altkey
                    );
                    data.refcount.fetch_add(1, Ordering::Relaxed);
                    data.inode
                }
                None => {
                    let inode = self.next_inode.fetch_add(1, Ordering::Relaxed);
                    if inode > VFS_MAX_INO {
                        error!("fuse: max inode number reached: {}", VFS_MAX_INO);
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!("max inode number reached: {}", VFS_MAX_INO),
                        ));
                    }
                    trace!(
                        "fuse: do_lookup adds new inode {} ids_altkey {:?} handle_altkey {:?}",
                        inode,
                        ids_altkey,
                        handle_altkey
                    );

                    InodeMap::insert_locked(
                        inodes.deref_mut(),
                        inode,
                        InodeData::new(inode, file_or_handle, 1, ids_altkey, st.get_stat().st_mode),
                        ids_altkey,
                        handle_altkey,
                    );
                    inode
                }
            }
        };

        Ok(Entry {
            inode,
            generation: 0,
            attr: st.get_stat(),
            attr_flags,
            attr_timeout: self.cfg.attr_timeout,
            entry_timeout: self.cfg.entry_timeout,
        })
        */
    }

    async fn async_getattr(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        handle: Option<<Self as FileSystem>::Handle>,
    ) -> io::Result<(libc::stat64, Duration)> {
        self.async_do_getattr(ctx, inode, handle).await
    }

    async fn async_setattr(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        attr: libc::stat64,
        handle: Option<<Self as FileSystem>::Handle>,
        valid: SetattrValid,
    ) -> io::Result<(libc::stat64, Duration)> {
        unimplemented!()
        /*
        enum Data {
            Handle(Arc<HandleData>, RawFd),
            ProcPath(CString),
        }

        let inode_data = self.inode_map.get(inode)?;
        let file = inode_data.async_get_file(&self.mount_fds).await?;
        let data = if self.no_open.load(Ordering::Relaxed) {
            let pathname = CString::new(format!("self/fd/{}", file.as_raw_fd()))
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Data::ProcPath(pathname)
        } else {
            // If we have a handle then use it otherwise get a new fd from the inode.
            if let Some(handle) = handle {
                let hd = self.handle_map.get(handle, inode)?;
                let fd = hd.get_handle_raw_fd();
                Data::Handle(hd, fd)
            } else {
                let pathname = CString::new(format!("self/fd/{}", file.as_raw_fd()))
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Data::ProcPath(pathname)
            }
        };

        if valid.contains(SetattrValid::MODE) {
            // Safe because this doesn't modify any memory and we check the return value.
            let res = unsafe {
                match data {
                    Data::Handle(_, fd) => libc::fchmod(fd, attr.st_mode),
                    Data::ProcPath(ref p) => {
                        libc::fchmodat(self.proc_self_fd.as_raw_fd(), p.as_ptr(), attr.st_mode, 0)
                    }
                }
            };
            if res < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        if valid.intersects(SetattrValid::UID | SetattrValid::GID) {
            let uid = if valid.contains(SetattrValid::UID) {
                attr.st_uid
            } else {
                // Cannot use -1 here because these are unsigned values.
                ::std::u32::MAX
            };
            let gid = if valid.contains(SetattrValid::GID) {
                attr.st_gid
            } else {
                // Cannot use -1 here because these are unsigned values.
                ::std::u32::MAX
            };

            // Safe because this is a constant value and a valid C string.
            let empty = unsafe { CStr::from_bytes_with_nul_unchecked(EMPTY_CSTR) };

            // Safe because this doesn't modify any memory and we check the return value.
            let res = unsafe {
                libc::fchownat(
                    file.as_raw_fd(),
                    empty.as_ptr(),
                    uid,
                    gid,
                    libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if res < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        if valid.contains(SetattrValid::SIZE) {
            // Cap restored when _killpriv is dropped
            let _killpriv = if self.killpriv_v2.load(Ordering::Relaxed)
                && valid.contains(SetattrValid::KILL_SUIDGID)
            {
                self::drop_cap_fsetid()?
            } else {
                None
            };

            // Safe because this doesn't modify any memory and we check the return value.
            let res = match data {
                Data::Handle(_, fd) => unsafe { libc::ftruncate(fd, attr.st_size) },
                Data::ProcPath(_) => {
                    // There is no `ftruncateat` so we need to get a new fd and truncate it.
                    let f = self
                        .async_open_inode(ctx, inode, libc::O_NONBLOCK | libc::O_RDWR)
                        .await?;
                    unsafe { libc::ftruncate(f.as_raw_fd(), attr.st_size) }
                }
            };
            if res < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        if valid.intersects(SetattrValid::ATIME | SetattrValid::MTIME) {
            let mut tvs = [
                libc::timespec {
                    tv_sec: 0,
                    tv_nsec: libc::UTIME_OMIT,
                },
                libc::timespec {
                    tv_sec: 0,
                    tv_nsec: libc::UTIME_OMIT,
                },
            ];

            if valid.contains(SetattrValid::ATIME_NOW) {
                tvs[0].tv_nsec = libc::UTIME_NOW;
            } else if valid.contains(SetattrValid::ATIME) {
                tvs[0].tv_sec = attr.st_atime;
                tvs[0].tv_nsec = attr.st_atime_nsec;
            }

            if valid.contains(SetattrValid::MTIME_NOW) {
                tvs[1].tv_nsec = libc::UTIME_NOW;
            } else if valid.contains(SetattrValid::MTIME) {
                tvs[1].tv_sec = attr.st_mtime;
                tvs[1].tv_nsec = attr.st_mtime_nsec;
            }

            // Safe because this doesn't modify any memory and we check the return value.
            let res = match data {
                Data::Handle(_, fd) => unsafe { libc::futimens(fd, tvs.as_ptr()) },
                Data::ProcPath(ref p) => unsafe {
                    libc::utimensat(self.proc_self_fd.as_raw_fd(), p.as_ptr(), tvs.as_ptr(), 0)
                },
            };
            if res < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        self.async_do_getattr(ctx, inode, handle).await
         */
    }

    async fn async_open(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        flags: u32,
        fuse_flags: u32,
    ) -> io::Result<(Option<<Self as FileSystem>::Handle>, OpenOptions)> {
        unimplemented!()
        /*
        if self.no_open.load(Ordering::Relaxed) {
            info!("fuse: open is not supported.");
            Err(io::Error::from_raw_os_error(libc::ENOSYS))
        } else {
            self.async_do_open(ctx, inode, flags, fuse_flags).await
        }
         */
    }

    async fn async_create(
        &self,
        ctx: &Context,
        parent: <Self as FileSystem>::Inode,
        name: &CStr,
        args: CreateIn,
    ) -> io::Result<(Entry, Option<<Self as FileSystem>::Handle>, OpenOptions)> {
        unimplemented!()
        /*
        self.validate_path_component(name)?;

        let dir = self.inode_map.get(parent)?;
        let dir_file = dir.async_get_file(&self.mount_fds).await?;

        let new_file = {
            let (_uid, _gid) = set_creds(ctx.uid, ctx.gid)?;

            Self::create_file_excl(
                dir_file.as_raw_fd(),
                name,
                args.flags as i32,
                args.mode & !(args.umask & 0o777),
            )?
        };

        let entry = self.async_lookup(ctx, parent, name).await?;
        let file = match new_file {
            // File didn't exist, now created by create_file_excl()
            Some(f) => f,
            // File exists, and args.flags doesn't contain O_EXCL. Now let's open it with
            // open_inode().
            None => {
                // Cap restored when _killpriv is dropped
                let _killpriv = if self.killpriv_v2.load(Ordering::Relaxed)
                    && (args.fuse_flags & FOPEN_IN_KILL_SUIDGID != 0)
                {
                    self::drop_cap_fsetid()?
                } else {
                    None
                };

                let (_uid, _gid) = set_creds(ctx.uid, ctx.gid)?;
                self.async_open_inode(ctx, entry.inode, args.flags as i32)
                    .await?
            }
        };

        let ret_handle = if !self.no_open.load(Ordering::Relaxed) {
            let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
            let data = HandleData::new(entry.inode, file);

            self.handle_map.insert(handle, data);
            Some(handle)
        } else {
            None
        };

        let mut opts = OpenOptions::empty();
        match self.cfg.cache_policy {
            CachePolicy::Never => opts |= OpenOptions::DIRECT_IO,
            CachePolicy::Always => opts |= OpenOptions::KEEP_CACHE,
            _ => {}
        };

        Ok((entry, ret_handle, opts))
         */
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
        _lock_owner: Option<u64>,
        _flags: u32,
    ) -> io::Result<usize> {
        unimplemented!()
        /*
        let data = self
            .async_get_data(ctx, handle, inode, libc::O_RDONLY)
            .await?;
        let drive = ctx
            .get_drive::<D>()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;

        w.async_write_from(drive, data.get_handle_raw_fd(), size as usize, offset)
            .await
         */
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
        _lock_owner: Option<u64>,
        _delayed_write: bool,
        _flags: u32,
        fuse_flags: u32,
    ) -> io::Result<usize> {
        unimplemented!()
        /*
        let data = self
            .async_get_data(ctx, handle, inode, libc::O_RDWR)
            .await?;

        // Fallback to sync io if KILLPRIV_V2 is enabled to work around a limitation of io_uring.
        if self.killpriv_v2.load(Ordering::Relaxed) && (fuse_flags & WRITE_KILL_PRIV != 0) {
            // Manually implement File::try_clone() by borrowing fd of data.file instead of dup().
            // It's safe because the `data` variable's lifetime spans the whole function,
            // so data.file won't be closed.
            let f = unsafe { File::from_raw_fd(data.get_handle_raw_fd()) };
            let mut f = ManuallyDrop::new(f);
            // Cap restored when _killpriv is dropped
            let _killpriv = self::drop_cap_fsetid()?;

            r.read_to(&mut *f, size as usize, offset)
        } else {
            let drive = ctx
                .get_drive::<D>()
                .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;

            r.async_read_to(drive, data.get_handle_raw_fd(), size as usize, offset)
                .await
        }
         */
    }

    async fn async_fsync(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        datasync: bool,
        handle: <Self as FileSystem>::Handle,
    ) -> io::Result<()> {
        unimplemented!()
        /*
        let data = self
            .async_get_data(ctx, handle, inode, libc::O_RDONLY)
            .await?;
        let drive = ctx
            .get_drive::<D>()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;

        AsyncUtil::fsync(drive, data.get_handle_raw_fd(), datasync).await
         */
    }

    async fn async_fallocate(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        handle: <Self as FileSystem>::Handle,
        mode: u32,
        offset: u64,
        length: u64,
    ) -> io::Result<()> {
        unimplemented!()
        /*
        // Let the Arc<HandleData> in scope, otherwise fd may get invalid.
        let data = self
            .async_get_data(ctx, handle, inode, libc::O_RDWR)
            .await?;
        let drive = ctx
            .get_drive::<D>()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;

        AsyncUtil::fallocate(drive, data.get_handle_raw_fd(), offset, length, mode).await
         */
    }

    async fn async_fsyncdir(
        &self,
        ctx: &Context,
        inode: <Self as FileSystem>::Inode,
        datasync: bool,
        handle: <Self as FileSystem>::Handle,
    ) -> io::Result<()> {
        self.async_fsync(ctx, inode, datasync, handle).await
    }
}
