// Copyright (C) 2021-2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io;
use std::mem::size_of;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use vm_memory::ByteValued;

use crate::abi::fuse_abi::{
    stat64, AttrOut, CreateIn, EntryOut, FallocateIn, FsyncIn, GetattrIn, Opcode, OpenIn, OpenOut,
    OutHeader, ReadIn, SetattrIn, SetattrValid, WriteIn, WriteOut, FATTR_FH, GETATTR_FH,
    KERNEL_MINOR_VERSION_LOOKUP_NEGATIVE_ENTRY_ZERO, READ_LOCKOWNER, WRITE_CACHE, WRITE_LOCKOWNER,
};
use crate::api::filesystem::{
    AsyncFileSystem, AsyncZeroCopyReader, AsyncZeroCopyWriter, ZeroCopyReader, ZeroCopyWriter,
};
use crate::api::server::{
    MetricsHook, Server, ServerUtil, SrvContext, BUFFER_HEADER_SIZE, MAX_BUFFER_SIZE,
};
use crate::file_traits::{AsyncFileReadWriteVolatile, FileReadWriteVolatile};
use crate::transport::{FsCacheReqHandler, Reader, Writer};
use crate::{bytes_to_cstr, encode_io_error_kind, BitmapSlice, Error, Result};

struct AsyncZcReader<'a, S: BitmapSlice = ()>(Reader<'a, S>);

// The underlying VolatileSlice contains "*mut u8", which is just a pointer to a u8 array.
// Actually we rely on the AsyncExecutor is a single-threaded worker, and we do not really send
// 'Reader' to other threads.
unsafe impl<'a, S: BitmapSlice> Send for AsyncZcReader<'a, S> {}

#[async_trait(?Send)]
impl<'a, S: BitmapSlice> AsyncZeroCopyReader for AsyncZcReader<'a, S> {
    async fn async_read_to(
        &mut self,
        f: Arc<dyn AsyncFileReadWriteVolatile>,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.0.async_read_to_at(&f, count, off).await
    }
}

impl<'a, S: BitmapSlice> ZeroCopyReader for AsyncZcReader<'a, S> {
    fn read_to(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.0.read_to_at(f, count, off)
    }
}

impl<'a, S: BitmapSlice> io::Read for AsyncZcReader<'a, S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

struct AsyncZcWriter<'a, S: BitmapSlice = ()>(Writer<'a, S>);

// The underlying VolatileSlice contains "*mut u8", which is just a pointer to a u8 array.
// Actually we rely on the AsyncExecutor is a single-threaded worker, and we do not really send
// 'Reader' to other threads.
unsafe impl<'a, S: BitmapSlice> Send for AsyncZcWriter<'a, S> {}

#[async_trait(?Send)]
impl<'a, S: BitmapSlice> AsyncZeroCopyWriter for AsyncZcWriter<'a, S> {
    async fn async_write_from(
        &mut self,
        f: Arc<dyn AsyncFileReadWriteVolatile>,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.0.async_write_from_at(&f, count, off).await
    }
}

impl<'a, S: BitmapSlice> ZeroCopyWriter for AsyncZcWriter<'a, S> {
    fn write_from(
        &mut self,
        f: &mut dyn FileReadWriteVolatile,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.0.write_from_at(f, count, off)
    }
}

impl<'a, S: BitmapSlice> io::Write for AsyncZcWriter<'a, S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<F: AsyncFileSystem + Sync> Server<F> {
    /// Main entrance to handle requests from the transport layer.
    ///
    /// It receives Fuse requests from transport layers, parses the request according to Fuse ABI,
    /// invokes filesystem drivers to server the requests, and eventually send back the result to
    /// the transport layer.
    ///
    /// ## Safety
    /// The async io framework borrows underlying buffers from `Reader` and `Writer`, so the caller
    /// must ensure all data buffers managed by the `Reader` and `Writer` are valid until the
    /// `Future` object returned has completed. Other subsystems, such as the transport layer, rely
    /// on the invariant.
    #[allow(unused_variables)]
    pub async unsafe fn async_handle_message<S: BitmapSlice>(
        &self,
        mut r: Reader<'_, S>,
        w: Writer<'_, S>,
        vu_req: Option<&mut dyn FsCacheReqHandler>,
        hook: Option<&dyn MetricsHook>,
    ) -> Result<usize> {
        let in_header = r.read_obj().map_err(Error::DecodeMessage)?;
        let mut ctx = SrvContext::<F, S>::new(in_header, r, w);
        if ctx.in_header.len > (MAX_BUFFER_SIZE + BUFFER_HEADER_SIZE)
            || ctx.w.available_bytes() < size_of::<OutHeader>()
        {
            return ctx
                .async_do_reply_error(io::Error::from_raw_os_error(libc::ENOMEM), true)
                .await;
        }
        let in_header = &ctx.in_header;

        trace!(
            "fuse: new req {:?}: {:?}",
            Opcode::from(in_header.opcode),
            in_header
        );
        hook.map_or((), |h| h.collect(in_header));

        let res = match in_header.opcode {
            x if x == Opcode::Lookup as u32 => self.async_lookup(ctx).await,
            x if x == Opcode::Forget as u32 => self.forget(ctx), // No reply.
            x if x == Opcode::Getattr as u32 => self.async_getattr(ctx).await,
            x if x == Opcode::Setattr as u32 => self.async_setattr(ctx).await,
            x if x == Opcode::Readlink as u32 => self.readlink(ctx),
            x if x == Opcode::Symlink as u32 => self.symlink(ctx),
            x if x == Opcode::Mknod as u32 => self.mknod(ctx),
            x if x == Opcode::Mkdir as u32 => self.mkdir(ctx),
            x if x == Opcode::Unlink as u32 => self.unlink(ctx),
            x if x == Opcode::Rmdir as u32 => self.rmdir(ctx),
            x if x == Opcode::Rename as u32 => self.rename(ctx),
            x if x == Opcode::Link as u32 => self.link(ctx),
            x if x == Opcode::Open as u32 => self.async_open(ctx).await,
            x if x == Opcode::Read as u32 => self.async_read(ctx).await,
            x if x == Opcode::Write as u32 => self.async_write(ctx).await,
            x if x == Opcode::Statfs as u32 => self.statfs(ctx),
            x if x == Opcode::Release as u32 => self.release(ctx),
            x if x == Opcode::Fsync as u32 => self.async_fsync(ctx).await,
            x if x == Opcode::Setxattr as u32 => self.setxattr(ctx),
            x if x == Opcode::Getxattr as u32 => self.getxattr(ctx),
            x if x == Opcode::Listxattr as u32 => self.listxattr(ctx),
            x if x == Opcode::Removexattr as u32 => self.removexattr(ctx),
            x if x == Opcode::Flush as u32 => self.flush(ctx),
            x if x == Opcode::Init as u32 => self.init(ctx),
            x if x == Opcode::Opendir as u32 => self.opendir(ctx),
            x if x == Opcode::Readdir as u32 => self.readdir(ctx),
            x if x == Opcode::Releasedir as u32 => self.releasedir(ctx),
            x if x == Opcode::Fsyncdir as u32 => self.async_fsyncdir(ctx).await,
            x if x == Opcode::Getlk as u32 => self.getlk(ctx),
            x if x == Opcode::Setlk as u32 => self.setlk(ctx),
            x if x == Opcode::Setlkw as u32 => self.setlkw(ctx),
            x if x == Opcode::Access as u32 => self.access(ctx),
            x if x == Opcode::Create as u32 => self.async_create(ctx).await,
            x if x == Opcode::Bmap as u32 => self.bmap(ctx),
            x if x == Opcode::Ioctl as u32 => self.ioctl(ctx),
            x if x == Opcode::Poll as u32 => self.poll(ctx),
            x if x == Opcode::NotifyReply as u32 => self.notify_reply(ctx),
            x if x == Opcode::BatchForget as u32 => self.batch_forget(ctx),
            x if x == Opcode::Fallocate as u32 => self.async_fallocate(ctx).await,
            x if x == Opcode::Readdirplus as u32 => self.readdirplus(ctx),
            x if x == Opcode::Rename2 as u32 => self.rename2(ctx),
            x if x == Opcode::Lseek as u32 => self.lseek(ctx),
            #[cfg(feature = "virtiofs")]
            x if x == Opcode::SetupMapping as u32 => self.setupmapping(ctx, vu_req),
            #[cfg(feature = "virtiofs")]
            x if x == Opcode::RemoveMapping as u32 => self.removemapping(ctx, vu_req),
            // Group reqeusts don't need reply together
            x => match x {
                x if x == Opcode::Interrupt as u32 => {
                    self.interrupt(ctx);
                    Ok(0)
                }
                x if x == Opcode::Destroy as u32 => {
                    self.destroy(ctx);
                    Ok(0)
                }
                _ => {
                    ctx.async_reply_error(io::Error::from_raw_os_error(libc::ENOSYS))
                        .await
                }
            },
        };

        // Pass `None` because current API handler's design does not allow us to catch
        // the `out_header`. Hopefully, we can reach to `out_header` after some
        // refactoring work someday.
        hook.map_or((), |h| h.release(None));

        res
    }

    async fn async_lookup<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let buf = ServerUtil::get_message_body(&mut ctx.r, &ctx.in_header, 0)?;
        let name = bytes_to_cstr(buf.as_ref())?;
        let version = self.vers.load();
        let result = self
            .fs
            .async_lookup(ctx.context(), ctx.nodeid(), name)
            .await;

        match result {
            // before ABI 7.4 inode == 0 was invalid, only ENOENT means negative dentry
            Ok(entry)
                if version.minor < KERNEL_MINOR_VERSION_LOOKUP_NEGATIVE_ENTRY_ZERO
                    && entry.inode == 0 =>
            {
                ctx.async_reply_error(io::Error::from_raw_os_error(libc::ENOENT))
                    .await
            }
            Ok(entry) => {
                let out = EntryOut::from(entry);
                ctx.async_reply_ok(Some(out), None).await
            }
            Err(e) => ctx.async_reply_error(e).await,
        }
    }

    async fn async_getattr<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let GetattrIn { flags, fh, .. } = ctx.r.read_obj().map_err(Error::DecodeMessage)?;
        let handle = if (flags & GETATTR_FH) != 0 {
            Some(fh.into())
        } else {
            None
        };
        let result = self
            .fs
            .async_getattr(ctx.context(), ctx.nodeid(), handle)
            .await;

        ctx.async_handle_attr_result(result).await
    }

    async fn async_setattr<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let setattr_in: SetattrIn = ctx.r.read_obj().map_err(Error::DecodeMessage)?;
        let handle = if setattr_in.valid & FATTR_FH != 0 {
            Some(setattr_in.fh.into())
        } else {
            None
        };
        let valid = SetattrValid::from_bits_truncate(setattr_in.valid);
        let st: stat64 = setattr_in.into();
        let result = self
            .fs
            .async_setattr(ctx.context(), ctx.nodeid(), st, handle, valid)
            .await;

        ctx.async_handle_attr_result(result).await
    }

    async fn async_open<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let OpenIn { flags, fuse_flags } = ctx.r.read_obj().map_err(Error::DecodeMessage)?;
        let result = self
            .fs
            .async_open(ctx.context(), ctx.nodeid(), flags, fuse_flags)
            .await;

        match result {
            Ok((handle, opts)) => {
                let out = OpenOut {
                    fh: handle.map(Into::into).unwrap_or(0),
                    open_flags: opts.bits(),
                    ..Default::default()
                };

                ctx.async_reply_ok(Some(out), None).await
            }
            Err(e) => ctx.async_reply_error(e).await,
        }
    }

    async fn async_read<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let ReadIn {
            fh,
            offset,
            size,
            read_flags,
            lock_owner,
            flags,
            ..
        } = ctx.r.read_obj().map_err(Error::DecodeMessage)?;

        if size > MAX_BUFFER_SIZE {
            return ctx
                .async_reply_error_explicit(io::Error::from_raw_os_error(libc::ENOMEM))
                .await;
        }

        let owner = if read_flags & READ_LOCKOWNER != 0 {
            Some(lock_owner)
        } else {
            None
        };

        // Split the writer into 2 pieces: one for the `OutHeader` and the rest for the data.
        let w2 = match ctx.w.split_at(size_of::<OutHeader>()) {
            Ok(v) => v,
            Err(_e) => return Err(Error::InvalidHeaderLength),
        };
        let mut data_writer = AsyncZcWriter(w2);
        let result = self
            .fs
            .async_read(
                ctx.context(),
                ctx.nodeid(),
                fh.into(),
                &mut data_writer,
                size,
                offset,
                owner,
                flags,
            )
            .await;

        match result {
            Ok(count) => {
                // Don't use `reply_ok` because we need to set a custom size length for the
                // header.
                let out = OutHeader {
                    len: (size_of::<OutHeader>() + count) as u32,
                    error: 0,
                    unique: ctx.unique(),
                };

                ctx.w
                    .async_write_all(out.as_slice())
                    .await
                    .map_err(Error::EncodeMessage)?;
                ctx.w
                    .async_commit(Some(&data_writer.0))
                    .await
                    .map_err(Error::EncodeMessage)?;
                Ok(out.len as usize)
            }
            Err(e) => ctx.async_reply_error_explicit(e).await,
        }
    }

    async fn async_write<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let WriteIn {
            fh,
            offset,
            size,
            fuse_flags,
            lock_owner,
            flags,
            ..
        } = ctx.r.read_obj().map_err(Error::DecodeMessage)?;

        if size > MAX_BUFFER_SIZE {
            return ctx
                .async_reply_error_explicit(io::Error::from_raw_os_error(libc::ENOMEM))
                .await;
        }

        let owner = if fuse_flags & WRITE_LOCKOWNER != 0 {
            Some(lock_owner)
        } else {
            None
        };
        let delayed_write = fuse_flags & WRITE_CACHE != 0;
        let mut data_reader = AsyncZcReader(ctx.take_reader());
        let result = self
            .fs
            .async_write(
                ctx.context(),
                ctx.nodeid(),
                fh.into(),
                &mut data_reader,
                size,
                offset,
                owner,
                delayed_write,
                flags,
                fuse_flags,
            )
            .await;

        match result {
            Ok(count) => {
                let out = WriteOut {
                    size: count as u32,
                    ..Default::default()
                };
                ctx.async_reply_ok(Some(out), None).await
            }
            Err(e) => ctx.async_reply_error_explicit(e).await,
        }
    }

    async fn async_fsync<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let FsyncIn {
            fh, fsync_flags, ..
        } = ctx.r.read_obj().map_err(Error::DecodeMessage)?;
        let datasync = fsync_flags & 0x1 != 0;

        match self
            .fs
            .async_fsync(ctx.context(), ctx.nodeid(), datasync, fh.into())
            .await
        {
            Ok(()) => ctx.async_reply_ok(None::<u8>, None).await,
            Err(e) => ctx.async_reply_error(e).await,
        }
    }

    async fn async_fsyncdir<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let FsyncIn {
            fh, fsync_flags, ..
        } = ctx.r.read_obj().map_err(Error::DecodeMessage)?;
        let datasync = fsync_flags & 0x1 != 0;
        let result = self
            .fs
            .async_fsyncdir(ctx.context(), ctx.nodeid(), datasync, fh.into())
            .await;

        match result {
            Ok(()) => ctx.async_reply_ok(None::<u8>, None).await,
            Err(e) => ctx.async_reply_error(e).await,
        }
    }

    async fn async_create<S: BitmapSlice>(&self, mut ctx: SrvContext<'_, F, S>) -> Result<usize> {
        let args: CreateIn = ctx.r.read_obj().map_err(Error::DecodeMessage)?;
        let buf = ServerUtil::get_message_body(&mut ctx.r, &ctx.in_header, size_of::<CreateIn>())?;
        let name = bytes_to_cstr(&buf)?;
        let result = self
            .fs
            .async_create(ctx.context(), ctx.nodeid(), name, args)
            .await;

        match result {
            Ok((entry, handle, opts)) => {
                let entry_out = EntryOut {
                    nodeid: entry.inode,
                    generation: entry.generation,
                    entry_valid: entry.entry_timeout.as_secs(),
                    attr_valid: entry.attr_timeout.as_secs(),
                    entry_valid_nsec: entry.entry_timeout.subsec_nanos(),
                    attr_valid_nsec: entry.attr_timeout.subsec_nanos(),
                    attr: entry.attr.into(),
                };
                let open_out = OpenOut {
                    fh: handle.map(Into::into).unwrap_or(0),
                    open_flags: opts.bits(),
                    ..Default::default()
                };

                // Kind of a hack to write both structs.
                ctx.async_reply_ok(Some(entry_out), Some(open_out.as_slice()))
                    .await
            }
            Err(e) => ctx.async_reply_error(e).await,
        }
    }

    async fn async_fallocate<S: BitmapSlice>(
        &self,
        mut ctx: SrvContext<'_, F, S>,
    ) -> Result<usize> {
        let FallocateIn {
            fh,
            offset,
            length,
            mode,
            ..
        } = ctx.r.read_obj().map_err(Error::DecodeMessage)?;
        let result = self
            .fs
            .async_fallocate(ctx.context(), ctx.nodeid(), fh.into(), mode, offset, length)
            .await;

        match result {
            Ok(()) => ctx.async_reply_ok(None::<u8>, None).await,
            Err(e) => ctx.async_reply_error(e).await,
        }
    }
}

impl<'a, F: AsyncFileSystem, S: BitmapSlice> SrvContext<'a, F, S> {
    async fn async_reply_ok<T: ByteValued>(
        &mut self,
        out: Option<T>,
        data: Option<&[u8]>,
    ) -> Result<usize> {
        let data2 = out.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
        let data3 = data.unwrap_or(&[]);
        let len = size_of::<OutHeader>() + data2.len() + data3.len();
        let header = OutHeader {
            len: len as u32,
            error: 0,
            unique: self.in_header.unique,
        };
        trace!("fuse: new reply {:?}", header);

        let result = match (data2.len(), data3.len()) {
            (0, 0) => self.w.async_write(header.as_slice()).await,
            (0, _) => self.w.async_write2(header.as_slice(), data3).await,
            (_, 0) => self.w.async_write2(header.as_slice(), data2).await,
            (_, _) => self.w.async_write3(header.as_slice(), data2, data3).await,
        };
        result.map_err(Error::EncodeMessage)?;

        debug_assert_eq!(len, self.w.bytes_written());
        Ok(self.w.bytes_written())
    }

    async fn async_do_reply_error(&mut self, err: io::Error, internal_err: bool) -> Result<usize> {
        let header = OutHeader {
            len: size_of::<OutHeader>() as u32,
            error: -err
                .raw_os_error()
                .unwrap_or_else(|| encode_io_error_kind(err.kind())),
            unique: self.in_header.unique,
        };

        trace!("fuse: reply error header {:?}, error {:?}", header, err);
        if internal_err {
            error!("fuse: reply error header {:?}, error {:?}", header, err);
        }
        self.w
            .async_write_all(header.as_slice())
            .await
            .map_err(Error::EncodeMessage)?;

        // Commit header if it is buffered otherwise kernel gets nothing back.
        self.w
            .async_commit(None)
            .await
            .map(|_| {
                debug_assert_eq!(header.len as usize, self.w.bytes_written());
                self.w.bytes_written()
            })
            .map_err(Error::EncodeMessage)
    }

    // reply operation error back to fuse client, don't print error message, as they are not
    // server's internal error, and client could deal with them.
    async fn async_reply_error(&mut self, err: io::Error) -> Result<usize> {
        self.async_do_reply_error(err, false).await
    }

    async fn async_reply_error_explicit(&mut self, err: io::Error) -> Result<usize> {
        self.async_do_reply_error(err, true).await
    }

    async fn async_handle_attr_result(
        &mut self,
        result: io::Result<(stat64, Duration)>,
    ) -> Result<usize> {
        match result {
            Ok((st, timeout)) => {
                let out = AttrOut {
                    attr_valid: timeout.as_secs(),
                    attr_valid_nsec: timeout.subsec_nanos(),
                    dummy: 0,
                    attr: st.into(),
                };
                self.async_reply_ok(Some(out), None).await
            }
            Err(e) => self.async_reply_error(e).await,
        }
    }
}

#[cfg(feature = "fusedev")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Vfs;
    use crate::transport::{FuseBuf, FuseDevWriter};

    use std::os::unix::io::AsRawFd;

    #[test]
    fn test_vfs_async_invalid_header() {
        let vfs = Vfs::default();
        let server = Server::new(vfs);
        let mut r_buf = [0u8];
        let r = Reader::<()>::from_fuse_buffer(FuseBuf::new(&mut r_buf)).unwrap();
        let file = vmm_sys_util::tempfile::TempFile::new().unwrap();
        let mut buf = vec![0x0u8; 1000];
        let w = FuseDevWriter::<()>::new(file.as_file().as_raw_fd(), &mut buf)
            .unwrap()
            .into();

        let result = crate::async_runtime::block_on(async {
            unsafe { server.async_handle_message(r, w, None, None).await }
        });
        assert!(result.is_err());
    }
}
