// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Fuse blob passthrough file system, mirroring an existing FS hierarchy.
//!
//! This file system mirrors the existing file system hierarchy of the system, starting at the
//! root file system. This is implemented by just "passing through" all requests to the
//! corresponding underlying file system.
//!
//! The code is derived from the
//! [CrosVM](https://chromium.googlesource.com/chromiumos/platform/crosvm/) project,
//! with heavy modification/enhancements from Alibaba Cloud OS team.

#[macro_use]
extern crate log;

use fuse_backend_rs::{
    api::{filesystem::*, BackendFileSystem, VFS_MAX_INO},
    passthrough::Config as PassthroughConfig,
    passthrough::PassthroughFs,
};
use nydus_error::{einval, eother};
use nydus_rafs::{
    fs::{Rafs, RafsConfig},
    RafsIoRead,
};
use serde::Deserialize;
use std::any::Any;
#[cfg(feature = "virtiofs")]
use std::ffi::CStr;
use std::ffi::CString;
use std::fs::create_dir_all;
#[cfg(feature = "virtiofs")]
use std::fs::File;
use std::io;
#[cfg(feature = "virtiofs")]
use std::mem::MaybeUninit;
#[cfg(feature = "virtiofs")]
use std::os::unix::ffi::OsStrExt;
#[cfg(feature = "virtiofs")]
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(feature = "virtiofs")]
use nydus_storage::device::BlobPrefetchRequest;
use vm_memory::ByteValued;

mod sync_io;

#[cfg(feature = "virtiofs")]
const EMPTY_CSTR: &[u8] = b"\0";

type Inode = u64;
type Handle = u64;

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
struct LinuxDirent64 {
    d_ino: libc::ino64_t,
    d_off: libc::off64_t,
    d_reclen: libc::c_ushort,
    d_ty: libc::c_uchar,
}
unsafe impl ByteValued for LinuxDirent64 {}

/// Options that configure xxx
#[derive(Clone, Default, Deserialize)]
pub struct BlobOndemandConfig {
    /// The rafs config used to set up rafs device for the purpose of
    /// `on demand read`.
    pub rafs_conf: RafsConfig,

    /// THe path of bootstrap of an container image (for rafs in
    /// kernel).
    ///
    /// The default is ``.
    #[serde(default)]
    pub bootstrap_path: String,

    /// The path of blob cache directory.
    #[serde(default)]
    pub blob_cache_dir: String,
}

impl FromStr for BlobOndemandConfig {
    type Err = io::Error;

    fn from_str(s: &str) -> io::Result<BlobOndemandConfig> {
        serde_json::from_str(s).map_err(|e| einval!(e))
    }
}

/// Options that configure the behavior of the blobfs fuse file system.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Config {
    /// Blobfs config is embedded with passthrough config
    pub ps_config: PassthroughConfig,
    /// This provides on demand config of blob management.
    pub blob_ondemand_cfg: String,
}

#[allow(dead_code)]
struct RafsHandle {
    rafs: Arc<Mutex<Option<Rafs>>>,
    handle: Arc<Mutex<Option<thread::JoinHandle<Option<Rafs>>>>>,
}

#[allow(dead_code)]
struct BootstrapArgs {
    rafs_handle: RafsHandle,
    blob_cache_dir: String,
}

// Safe to Send/Sync because the underlying data structures are readonly
unsafe impl Sync for BootstrapArgs {}
unsafe impl Send for BootstrapArgs {}

#[cfg(feature = "virtiofs")]
impl BootstrapArgs {
    fn get_rafs_handle(&self) -> io::Result<()> {
        let mut c = self.rafs_handle.rafs.lock().unwrap();
        match (*self.rafs_handle.handle.lock().unwrap()).take() {
            Some(handle) => {
                let rafs = handle.join().unwrap().ok_or_else(|| {
                    error!("blobfs: get rafs failed.");
                    einval!("create rafs failed in thread.")
                })?;
                debug!("blobfs: async create Rafs finish!");

                *c = Some(rafs);
                Ok(())
            }
            None => Err(einval!("create rafs failed in thread.")),
        }
    }

    fn fetch_range_sync(&self, prefetches: &[BlobPrefetchRequest]) -> io::Result<()> {
        let c = self.rafs_handle.rafs.lock().unwrap();
        match &*c {
            Some(rafs) => rafs.fetch_range_synchronous(prefetches),
            None => Err(einval!("create rafs failed in thread.")),
        }
    }
}

/// A file system that simply "passes through" all requests it receives to the underlying file
/// system.
///
/// To keep the implementation simple it servers the contents of its root directory. Users
/// that wish to serve only a specific directory should set up the environment so that that
/// directory ends up as the root of the file system process. One way to accomplish this is via a
/// combination of mount namespaces and the pivot_root system call.
pub struct BlobFs {
    pfs: PassthroughFs,
    #[allow(dead_code)]
    bootstrap_args: BootstrapArgs,
}

impl BlobFs {
    fn ensure_path_exist(path: &Path) -> io::Result<()> {
        if path.as_os_str().is_empty() {
            return Err(einval!("path is empty"));
        }
        if !path.exists() {
            create_dir_all(path).map_err(|e| {
                error!(
                    "create dir error. directory is {:?}. {}:{}",
                    path,
                    file!(),
                    line!()
                );
                e
            })?;
        }

        Ok(())
    }

    /// Create a Blob file system instance.
    pub fn new(cfg: Config) -> io::Result<BlobFs> {
        trace!("BlobFs config is: {:?}", cfg);

        let bootstrap_args = Self::load_bootstrap(&cfg)?;
        let pfs = PassthroughFs::new(cfg.ps_config)?;
        Ok(BlobFs {
            pfs,
            bootstrap_args,
        })
    }

    fn load_bootstrap(cfg: &Config) -> io::Result<BootstrapArgs> {
        let blob_ondemand_conf = BlobOndemandConfig::from_str(&cfg.blob_ondemand_cfg)?;
        // check if blob cache dir exists.
        let path = Path::new(blob_ondemand_conf.blob_cache_dir.as_str());
        Self::ensure_path_exist(path).map_err(|e| {
            error!("blob_cache_dir not exist");
            e
        })?;

        let path = Path::new(blob_ondemand_conf.bootstrap_path.as_str());
        if !path.exists() || blob_ondemand_conf.bootstrap_path == String::default() {
            return Err(einval!("no valid bootstrap"));
        }

        let mut rafs_conf = blob_ondemand_conf.rafs_conf.clone();
        // we must use direct mode to get mmap'd bootstrap.
        rafs_conf.mode = "direct".to_string();
        let mut bootstrap =
            <dyn RafsIoRead>::from_file(path.to_str().unwrap()).map_err(|e| eother!(e))?;

        trace!("blobfs: async create Rafs start!");
        let rafs_join_handle = std::thread::spawn(move || {
            let mut rafs = match Rafs::new(rafs_conf, "blobfs", &mut bootstrap) {
                Ok(rafs) => rafs,
                Err(e) => {
                    error!("blobfs: new rafs failed {:?}.", e);
                    return None;
                }
            };
            match rafs.import(bootstrap, None) {
                Ok(_) => {}
                Err(e) => {
                    error!("blobfs: new rafs failed {:?}.", e);
                    return None;
                }
            }
            Some(rafs)
        });
        let rafs_handle = RafsHandle {
            rafs: Arc::new(Mutex::new(None)),
            handle: Arc::new(Mutex::new(Some(rafs_join_handle))),
        };

        Ok(BootstrapArgs {
            rafs_handle,
            blob_cache_dir: blob_ondemand_conf.blob_cache_dir,
        })
    }

    #[cfg(feature = "virtiofs")]
    fn stat(f: &File) -> io::Result<libc::stat64> {
        // Safe because this is a constant value and a valid C string.
        let pathname = unsafe { CStr::from_bytes_with_nul_unchecked(EMPTY_CSTR) };
        let mut st = MaybeUninit::<libc::stat64>::zeroed();

        // Safe because the kernel will only write data in `st` and we check the return value.
        let res = unsafe {
            libc::fstatat64(
                f.as_raw_fd(),
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

    /// Initialize the PassthroughFs
    pub fn import(&self) -> io::Result<()> {
        self.pfs.import()
    }

    #[cfg(feature = "virtiofs")]
    fn open_file(dfd: i32, pathname: &Path, flags: i32, mode: u32) -> io::Result<File> {
        let pathname = CString::new(pathname.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let fd = if flags & libc::O_CREAT == libc::O_CREAT {
            unsafe { libc::openat(dfd, pathname.as_ptr(), flags, mode) }
        } else {
            unsafe { libc::openat(dfd, pathname.as_ptr(), flags) }
        };

        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Safe because we just opened this fd.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

impl BackendFileSystem for BlobFs {
    fn mount(&self) -> io::Result<(Entry, u64)> {
        let ctx = &Context::default();
        let entry = self.lookup(ctx, ROOT_ID, &CString::new(".").unwrap())?;
        Ok((entry, VFS_MAX_INO))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test2)]
mod tests {
    use super::*;
    use fuse_backend_rs::abi::virtio_fs;
    use fuse_backend_rs::transport::FsCacheReqHandler;
    use nydus_app::setup_logging;
    use std::os::unix::prelude::RawFd;

    struct DummyCacheReq {}

    impl FsCacheReqHandler for DummyCacheReq {
        fn map(
            &mut self,
            _foffset: u64,
            _moffset: u64,
            _len: u64,
            _flags: u64,
            _fd: RawFd,
        ) -> io::Result<()> {
            Ok(())
        }

        fn unmap(&mut self, _requests: Vec<virtio_fs::RemovemappingOne>) -> io::Result<()> {
            Ok(())
        }
    }

    // #[test]
    // #[cfg(feature = "virtiofs")]
    // fn test_blobfs_new() {
    //     setup_logging(None, log::LevelFilter::Trace, 0).unwrap();
    //     let config = r#"
    //     {
    //         "device": {
    //           "backend": {
    //             "type": "localfs",
    //             "config": {
    //               "dir": "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1/test4k"
    //             }
    //           },
    //           "cache": {
    //             "type": "blobcache",
    //             "compressed": false,
    //             "config": {
    //               "work_dir": "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1/blobcache"
    //             }
    //           }
    //         },
    //         "mode": "direct",
    //         "digest_validate": true,
    //         "enable_xattr": false,
    //         "fs_prefetch": {
    //           "enable": false,
    //           "threads_count": 10,
    //           "merging_size": 131072,
    //           "bandwidth_rate": 10485760
    //         }
    //       }"#;
    //     //        let rafs_conf = RafsConfig::from_str(config).unwrap();

    //     let fs_cfg = Config {
    //         root_dir: "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1"
    //             .to_string(),
    //         bootstrap_path: "test4k/bootstrap-link".to_string(),
    //         //            blob_cache_dir: "blobcache".to_string(),
    //         do_import: false,
    //         no_open: true,
    //         rafs_conf: config.to_string(),
    //         ..Default::default()
    //     };

    //     assert!(BlobFs::new(fs_cfg).is_err());

    //     let fs_cfg = Config {
    //         root_dir: "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1"
    //             .to_string(),
    //         bootstrap_path: "test4k/bootstrap-link".to_string(),
    //         blob_cache_dir: "blobcache1".to_string(),
    //         do_import: false,
    //         no_open: true,
    //         rafs_conf: config.to_string(),
    //         ..Default::default()
    //     };

    //     assert!(BlobFs::new(fs_cfg).is_err());

    //     let fs_cfg = Config {
    //         root_dir: "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1"
    //             .to_string(),
    //         //            bootstrap_path: "test4k/bootstrap-link".to_string(),
    //         blob_cache_dir: "blobcache".to_string(),
    //         do_import: false,
    //         no_open: true,
    //         rafs_conf: config.to_string(),
    //         ..Default::default()
    //     };

    //     assert!(BlobFs::new(fs_cfg).is_err());

    //     let fs_cfg = Config {
    //         root_dir: "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1"
    //             .to_string(),
    //         bootstrap_path: "test4k/bootstrap-foo".to_string(),
    //         blob_cache_dir: "blobcache".to_string(),
    //         do_import: false,
    //         no_open: true,
    //         rafs_conf: config.to_string(),
    //         ..Default::default()
    //     };

    //     assert!(BlobFs::new(fs_cfg).is_err());

    //     let fs_cfg = Config {
    //         root_dir: "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1"
    //             .to_string(),
    //         bootstrap_path: "test4k/bootstrap-link".to_string(),
    //         blob_cache_dir: "blobcache".to_string(),
    //         do_import: false,
    //         no_open: true,
    //         rafs_conf: config.to_string(),
    //         ..Default::default()
    //     };

    //     assert!(BlobFs::new(fs_cfg).is_ok());
    // }

    #[test]
    fn test_blobfs_setupmapping() {
        setup_logging(None, log::LevelFilter::Trace, 0).unwrap();
        let config = r#"
    {
            "rafs_conf": {
                "device": {
                  "backend": {
                    "type": "localfs",
                    "config": {
                      "blob_file": "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1/nydus-rs/myblob1/v6/blob-btrfs"
                    }
                  },
                  "cache": {
                    "type": "blobcache",
                    "compressed": false,
                    "config": {
                      "work_dir": "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1/blobcache"
                    }
                  }
                },
                "mode": "direct",
                "digest_validate": false,
                "enable_xattr": false,
                "fs_prefetch": {
                  "enable": false,
                  "threads_count": 10,
                  "merging_size": 131072,
                  "bandwidth_rate": 10485760
                }
              },
         "bootstrap_path": "nydus-rs/myblob1/v6/bootstrap-btrfs",
         "blob_cache_dir": "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1/blobcache"
    }"#;
        //        let rafs_conf = RafsConfig::from_str(config).unwrap();

        let ps_config = PassthroughConfig {
            root_dir: "/home/b.liu/1_source/3_ali/virtiofs/qemu-my/build-kangaroo/share_dir1"
                .to_string(),
            do_import: false,
            no_open: true,
            ..Default::default()
        };
        let fs_cfg = Config {
            ps_config,
            blob_ondemand_cfg: config.to_string(),
        };

        let fs = BlobFs::new(fs_cfg).unwrap();
        fs.import().unwrap();

        fs.mount().unwrap();

        let ctx = &Context::default();

        // read bootstrap first, should return err as it's not in blobcache dir.
        // let bootstrap = CString::new("foo").unwrap();
        // let entry = fs.lookup(ctx, ROOT_ID, &bootstrap).unwrap();
        // let mut req = DummyCacheReq {};
        // fs.setupmapping(ctx, entry.inode, 0, 0, 4096, 0, 0, &mut req)
        //     .unwrap();

        // FIXME: use a real blob id under test4k.
        let blob_cache_dir = CString::new("blobcache").unwrap();
        let parent_entry = fs.lookup(ctx, ROOT_ID, &blob_cache_dir).unwrap();

        let blob_id = CString::new("80da976ee69d68af6bb9170395f71b4ef1e235e815e2").unwrap();
        let entry = fs.lookup(ctx, parent_entry.inode, &blob_id).unwrap();

        let foffset = 0;
        let len = 1 << 21;
        let mut req = DummyCacheReq {};
        fs.setupmapping(ctx, entry.inode, 0, foffset, len, 0, 0, &mut req)
            .unwrap();

        // FIXME: release fs
        fs.destroy();
    }
}
