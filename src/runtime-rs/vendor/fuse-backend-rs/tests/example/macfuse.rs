// Copyright 2021 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use libc::time_t;
use log::{error, info, warn};
use std::any::Any;
use std::ffi::CStr;
use std::io::Result;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use fuse_backend_rs::abi::fuse_abi::Attr;

use fuse_backend_rs::api::filesystem::{Context, DirEntry, Entry, FileSystem, ZeroCopyWriter};
use fuse_backend_rs::api::{server::Server, BackendFileSystem, Vfs, VfsOptions};
use fuse_backend_rs::transport::{FuseChannel, FuseSession};

pub(crate) struct HelloFileSystem {}

impl FileSystem for HelloFileSystem {
    type Inode = u64;
    type Handle = u64;
    #[allow(unused_variables)]
    fn lookup(&self, _: &Context, parent: Self::Inode, name: &CStr) -> Result<Entry> {
        let content = "hello, fuse".as_bytes();
        let now = SystemTime::now();
        let time = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Ok(Entry {
            inode: 2,
            generation: 0,
            attr: Attr {
                ino: 2,
                size: content.len() as u64,
                blocks: 1,
                atime: time,
                mtime: time,
                ctime: time,
                crtime: time,
                atimensec: 0,
                mtimensec: 0,
                ctimensec: 0,
                crtimensec: 0,
                mode: (libc::S_IFREG
                    | libc::S_IREAD
                    | libc::S_IEXEC
                    | libc::S_IRGRP
                    | libc::S_IXGRP
                    | libc::S_IROTH
                    | libc::S_IXOTH) as u32,
                nlink: 1,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
                blksize: 4096,
                padding: 0,
            }
            .into(),
            attr_flags: 0,
            attr_timeout: Duration::new(0, 0),
            entry_timeout: Duration::new(0, 0),
        })
    }

    #[allow(unused_variables)]
    fn readdir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry) -> Result<usize>,
    ) -> Result<()> {
        if offset != 0 {
            return Ok(());
        }
        let mut offset: usize = offset as usize;
        let entry = DirEntry {
            ino: 1,
            offset: offset as u64,
            type_: libc::DT_DIR as u32,
            name: ".".as_bytes(),
        };
        offset += add_entry(entry).unwrap();

        let entry = DirEntry {
            ino: 1,
            offset: offset as u64,
            type_: libc::DT_DIR as u32,
            name: "..".as_bytes(),
        };
        offset += add_entry(entry).unwrap();

        let entry = DirEntry {
            ino: 2,
            offset: offset as u64,
            type_: libc::DT_REG as u32,
            name: "hello".as_bytes(),
        };
        add_entry(entry).unwrap();
        Ok(())
    }

    #[allow(unused_variables)]
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
    ) -> Result<usize> {
        let offset = offset as usize;
        let content = "hello, fuse".as_bytes();
        let mut buf = Vec::<u8>::with_capacity(size as usize);
        let can_read_size = content.len() - offset;
        let read_size = if can_read_size < size as usize {
            can_read_size
        } else {
            size as usize
        };
        let read_end = (offset as usize) + read_size;
        buf.extend_from_slice(&content[(offset as usize)..(read_end as usize)]);
        w.write(buf.as_slice())?;
        Ok(read_size)
    }

    #[allow(unused_variables)]
    fn getattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Option<Self::Handle>,
    ) -> Result<(libc::stat, Duration)> {
        if inode == 1 {
            let now = SystemTime::now();
            let time = now
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as time_t;
            return Ok((
                libc::stat {
                    st_dev: 0,
                    st_mode: (libc::S_IFDIR
                        | libc::S_IREAD
                        | libc::S_IEXEC
                        | libc::S_IRGRP
                        | libc::S_IXGRP
                        | libc::S_IROTH
                        | libc::S_IXOTH),
                    st_nlink: 1,
                    st_ino: 1,
                    st_uid: 0,
                    st_gid: 0,
                    st_rdev: 0,
                    st_atime: time,
                    st_atime_nsec: 0,
                    st_mtime: time,
                    st_mtime_nsec: 0,
                    st_ctime: time,
                    st_ctime_nsec: 0,
                    st_birthtime: 0,
                    st_birthtime_nsec: 0,
                    st_size: 0,
                    st_blocks: 0,
                    st_blksize: 4096,
                    st_flags: 0,
                    st_gen: 0,
                    st_lspare: 0,
                    st_qspare: [0, 0],
                },
                Duration::from_secs(1),
            ));
        } else {
            let content = "hello, fuse".as_bytes();
            let now = SystemTime::now();
            let time = now
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as time_t;
            return Ok((
                libc::stat {
                    st_dev: 0,
                    st_mode: (libc::S_IFREG
                        | libc::S_IREAD
                        | libc::S_IEXEC
                        | libc::S_IRGRP
                        | libc::S_IXGRP
                        | libc::S_IROTH
                        | libc::S_IXOTH),
                    st_nlink: 1,
                    st_ino: 1,
                    st_uid: 0,
                    st_gid: 0,
                    st_rdev: 0,
                    st_atime: time,
                    st_atime_nsec: 0,
                    st_mtime: time,
                    st_mtime_nsec: 0,
                    st_ctime: time,
                    st_ctime_nsec: 0,
                    st_birthtime: 0,
                    st_birthtime_nsec: 0,
                    st_size: content.len() as libc::off_t,
                    st_blocks: 1,
                    st_blksize: 4096,
                    st_flags: 0,
                    st_gen: 0,
                    st_lspare: 0,
                    st_qspare: [0, 0],
                },
                Duration::from_secs(1),
            ));
        }
    }

    #[allow(unused_variables)]
    fn access(&self, ctx: &Context, inode: Self::Inode, mask: u32) -> Result<()> {
        return Ok(());
    }
}

impl BackendFileSystem for HelloFileSystem {
    fn mount(&self) -> Result<(Entry, u64)> {
        Ok((
            Entry {
                inode: 1,
                generation: 0,
                attr: Attr::default().into(),
                attr_flags: 0,
                attr_timeout: Duration::new(0, 0),
                entry_timeout: Duration::new(0, 0),
            },
            0,
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A fusedev daemon example
#[allow(dead_code)]
pub struct Daemon {
    mountpoint: String,
    server: Arc<Server<Arc<Vfs>>>,
    thread_cnt: u32,
    session: Option<FuseSession>,
}

#[allow(dead_code)]
impl Daemon {
    /// Creates a fusedev daemon instance
    pub fn new(mountpoint: &str, thread_cnt: u32) -> Result<Self> {
        // create vfs
        let vfs = Vfs::new(VfsOptions {
            no_open: false,
            no_opendir: false,
            ..Default::default()
        });

        let fs = HelloFileSystem {};
        vfs.mount(Box::new(fs), "/").unwrap();

        Ok(Daemon {
            mountpoint: mountpoint.to_string(),
            server: Arc::new(Server::new(Arc::new(vfs))),
            thread_cnt,
            session: None,
        })
    }

    /// Mounts a fusedev daemon to the mountpoint, then start service threads to handle
    /// FUSE requests.
    pub fn mount(&mut self) -> Result<()> {
        let mut se =
            FuseSession::new(Path::new(&self.mountpoint), "passthru_example", "", true).unwrap();
        se.mount().unwrap();
        for _ in 0..self.thread_cnt {
            let mut server = FuseServer {
                server: self.server.clone(),
                ch: se.new_channel().unwrap(),
            };
            let _thread = thread::Builder::new()
                .name("fuse_server".to_string())
                .spawn(move || {
                    info!("new fuse thread");
                    let _ = server.svc_loop();
                    warn!("fuse service thread exits");
                })
                .unwrap();
        }
        self.session = Some(se);
        Ok(())
    }

    /// Umounts and destroies a fusedev daemon
    pub fn umount(&mut self) -> Result<()> {
        if let Some(mut se) = self.session.take() {
            se.umount().unwrap();
        }
        Ok(())
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.umount();
    }
}

struct FuseServer {
    server: Arc<Server<Arc<Vfs>>>,
    ch: FuseChannel,
}

impl FuseServer {
    fn svc_loop(&mut self) -> Result<()> {
        // Given error EBADF, it means kernel has shut down this session.
        let _ebadf = std::io::Error::from_raw_os_error(libc::EBADF);
        loop {
            if let Some((reader, writer)) = self
                .ch
                .get_request()
                .map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))?
            {
                if let Err(e) = self
                    .server
                    .handle_message(reader, writer.into(), None, None)
                {
                    match e {
                        fuse_backend_rs::Error::EncodeMessage(_ebadf) => {
                            break;
                        }
                        _ => {
                            error!("Handling fuse message failed");
                            continue;
                        }
                    }
                }
            } else {
                info!("fuse server exits");
                break;
            }
        }
        Ok(())
    }
}
