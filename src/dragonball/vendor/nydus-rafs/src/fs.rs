// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
//
// A container image Registry Acceleration File System.

//! The Rafs API layer to glue fuse, storage backend and filesystem metadata together.
//!
//! This module is core to glue fuse, filesystem format and storage backend. The main API provided
//! by this module is the [Rafs](struct.Rafs.html) structures, which implements the
//! `fuse_backend_rs::FileSystem` trait, so an instance of [Rafs] could be registered to a fuse
//! backend server. A [Rafs] instance receives fuse requests from a fuse backend server, parsing
//! the request and filesystem metadata, and eventually ask the storage backend to fetch requested
//! data. There are also [FsPrefetchControl](struct.FsPrefetchControl.html) and
//! [RafsConfig](struct.RafsConfig.html) to configure an [Rafs] instance.

use std::any::Any;
use std::cmp;
use std::convert::TryFrom;
use std::ffi::{CStr, OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::io::Result;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use fuse_backend_rs::abi::fuse_abi::Attr;
use fuse_backend_rs::abi::fuse_abi::{stat64, statvfs64};
use fuse_backend_rs::api::filesystem::*;
use fuse_backend_rs::api::BackendFileSystem;
use nix::unistd::{getegid, geteuid};
use serde::Deserialize;

use nydus_api::http::{BlobPrefetchConfig, FactoryConfig};
use nydus_storage::device::{BlobDevice, BlobPrefetchRequest};
use nydus_utils::metrics::{self, FopRecorder, StatsFop::*};
use storage::RAFS_DEFAULT_CHUNK_SIZE;

use crate::metadata::{
    Inode, PostWalkAction, RafsInode, RafsSuper, RafsSuperMeta, DOT, DOTDOT, RAFS_MAX_CHUNK_SIZE,
};
use crate::{RafsError, RafsIoReader, RafsResult};

/// Type of RAFS fuse handle.
pub type Handle = u64;

/// Rafs default attribute timeout value.
pub const RAFS_DEFAULT_ATTR_TIMEOUT: u64 = 1 << 32;
/// Rafs default entry timeout value.
pub const RAFS_DEFAULT_ENTRY_TIMEOUT: u64 = RAFS_DEFAULT_ATTR_TIMEOUT;

fn default_threads_count() -> usize {
    8
}

pub fn default_merging_size() -> usize {
    128 * 1024
}

fn default_prefetch_all() -> bool {
    true
}

fn default_amplify_io() -> u32 {
    128 * 1024
}

/// Configuration information for filesystem data prefetch.
#[derive(Clone, Default, Deserialize)]
pub struct FsPrefetchControl {
    /// Whether the filesystem layer data prefetch is enabled or not.
    #[serde(default)]
    pub enable: bool,

    /// How many working threads to prefetch data.
    #[serde(default = "default_threads_count")]
    pub threads_count: usize,

    /// Window size in unit of bytes to merge request to backend.
    #[serde(default = "default_merging_size")]
    pub merging_size: usize,

    /// Network bandwidth limitation for prefetching.
    ///
    /// In unit of Bytes. It sets a limit to prefetch bandwidth usage in order to
    /// reduce congestion with normal user IO.
    /// bandwidth_rate == 0 -- prefetch bandwidth ratelimit disabled
    /// bandwidth_rate > 0  -- prefetch bandwidth ratelimit enabled.
    ///                        Please note that if the value is less than Rafs chunk size,
    ///                        it will be raised to the chunk size.
    #[serde(default)]
    pub bandwidth_rate: u32,

    /// Whether to prefetch all filesystem data.
    #[serde(default = "default_prefetch_all")]
    pub prefetch_all: bool,
}

impl TryFrom<&RafsConfig> for BlobPrefetchConfig {
    type Error = RafsError;

    fn try_from(c: &RafsConfig) -> RafsResult<Self> {
        if c.fs_prefetch.merging_size as u64 > RAFS_MAX_CHUNK_SIZE {
            return Err(RafsError::Configure(
                "merging size can't exceed max chunk size".to_string(),
            ));
        } else if c.fs_prefetch.enable && c.fs_prefetch.threads_count == 0 {
            return Err(RafsError::Configure(
                "try to enable fs prefetching with zero working threads".to_string(),
            ));
        }

        Ok(BlobPrefetchConfig {
            enable: c.fs_prefetch.enable,
            threads_count: c.fs_prefetch.threads_count,
            merging_size: c.fs_prefetch.merging_size,
            bandwidth_rate: c.fs_prefetch.bandwidth_rate,
        })
    }
}

/// Not everything can be safely exported from configuration.
/// We trim the unneeded info from here.
#[macro_export]
macro_rules! trim_backend_config {
    ($config:expr, $($i:expr),*) => {
        let mut _n :&mut serde_json::Value = &mut $config["device"]["backend"]["config"];
        if let serde_json::Value::Object(ref mut m) = _n {
            $(if m.contains_key($i) { m[$i].take();} )*
        }
    }
}

/// Rafs storage backend configuration information.
#[derive(Clone, Default, Deserialize)]
pub struct RafsConfig {
    /// Configuration for storage subsystem.
    pub device: FactoryConfig,
    /// Filesystem working mode.
    pub mode: String,
    /// Whether to validate data digest before use.
    #[serde(default)]
    pub digest_validate: bool,
    /// Io statistics.
    #[serde(default)]
    pub iostats_files: bool,
    /// Filesystem prefetching configuration.
    #[serde(default)]
    pub fs_prefetch: FsPrefetchControl,
    /// Enable extended attributes.
    #[serde(default)]
    pub enable_xattr: bool,
    /// Record filesystem access pattern.
    #[serde(default)]
    pub access_pattern: bool,
    /// Record file name if file access trace log.
    #[serde(default)]
    pub latest_read_files: bool,
    // ZERO value means, amplifying user io is not enabled.
    #[serde(default = "default_amplify_io")]
    pub amplify_io: u32,
}

impl RafsConfig {
    /// Create a new instance of `RafsConfig`.
    pub fn new() -> RafsConfig {
        RafsConfig {
            ..Default::default()
        }
    }

    /// Load Rafs configuration information from a configuration file.
    pub fn from_file(path: &str) -> RafsResult<RafsConfig> {
        let file = File::open(path).map_err(RafsError::LoadConfig)?;
        serde_json::from_reader::<File, RafsConfig>(file).map_err(RafsError::ParseConfig)
    }
}

impl FromStr for RafsConfig {
    type Err = RafsError;

    fn from_str(s: &str) -> RafsResult<RafsConfig> {
        serde_json::from_str(s).map_err(RafsError::ParseConfig)
    }
}

impl fmt::Display for RafsConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "mode={} digest_validate={} iostats_files={} latest_read_files={}",
            self.mode, self.digest_validate, self.iostats_files, self.latest_read_files
        )
    }
}

/// Struct to glue fuse, storage backend and filesystem metadata together.
///
/// The [Rafs](struct.Rafs.html) structure implements the `fuse_backend_rs::FileSystem` trait,
/// so an instance of [Rafs] could be registered to a fuse backend server. A [Rafs] instance
/// receives fuse requests from a fuse backend server, parsing the request and filesystem metadata,
/// and eventually ask the storage backend to fetch requested data.
pub struct Rafs {
    id: String,
    device: BlobDevice,
    ios: Arc<metrics::FsIoStats>,
    sb: Arc<RafsSuper>,

    initialized: bool,
    digest_validate: bool,
    fs_prefetch: bool,
    prefetch_all: bool,
    xattr_enabled: bool,
    amplify_io: u32,

    // static inode attributes
    i_uid: u32,
    i_gid: u32,
    i_time: u64,
}

impl Rafs {
    /// Create a new instance of `Rafs`.
    pub fn new(conf: RafsConfig, id: &str, r: &mut RafsIoReader) -> RafsResult<Self> {
        let storage_conf = Self::prepare_storage_conf(&conf)?;
        let mut sb = RafsSuper::new(&conf).map_err(RafsError::FillSuperblock)?;
        sb.load(r).map_err(RafsError::FillSuperblock)?;

        let blob_infos = sb.superblock.get_blob_infos();
        let device =
            BlobDevice::new(&storage_conf, &blob_infos).map_err(RafsError::CreateDevice)?;

        let rafs = Rafs {
            id: id.to_string(),
            device,
            ios: metrics::FsIoStats::new(id),
            sb: Arc::new(sb),

            initialized: false,
            digest_validate: conf.digest_validate,
            fs_prefetch: conf.fs_prefetch.enable,
            amplify_io: conf.amplify_io,
            prefetch_all: conf.fs_prefetch.prefetch_all,
            xattr_enabled: conf.enable_xattr,

            i_uid: geteuid().into(),
            i_gid: getegid().into(),
            i_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Rafs v6 does must store chunk info into local file cache. So blob cache is required
        if rafs.metadata().is_v6() {
            if conf.device.cache.cache_type != "blobcache" {
                return Err(RafsError::Configure(
                    "Rafs v6 must have local blobcache configured".to_string(),
                ));
            }

            if conf.digest_validate {
                return Err(RafsError::Configure(
                    "Rafs v6 doesn't support integrity validation yet".to_string(),
                ));
            }
        }

        rafs.ios.toggle_files_recording(conf.iostats_files);
        rafs.ios.toggle_access_pattern(conf.access_pattern);
        rafs.ios
            .toggle_latest_read_files_recording(conf.latest_read_files);

        Ok(rafs)
    }

    /// Update storage backend for blobs.
    pub fn update(&self, r: &mut RafsIoReader, conf: RafsConfig) -> RafsResult<()> {
        info!("update");
        if !self.initialized {
            warn!("Rafs is not yet initialized");
            return Err(RafsError::Uninitialized);
        }

        // TODO: seems no need to do self.sb.update()
        // step 1: update sb.
        // No lock is needed thanks to ArcSwap.
        self.sb.update(r).map_err(|e| {
            error!("update failed due to {:?}", e);
            e
        })?;
        info!("update sb is successful");

        let storage_conf = Self::prepare_storage_conf(&conf)?;
        let blob_infos = self.sb.superblock.get_blob_infos();

        // step 2: update device (only localfs is supported)
        self.device
            .update(&storage_conf, &blob_infos, self.fs_prefetch)
            .map_err(RafsError::SwapBackend)?;
        info!("update device is successful");

        Ok(())
    }

    /// Import an rafs bootstrap to initialize the filesystem instance.
    pub fn import(
        &mut self,
        r: RafsIoReader,
        prefetch_files: Option<Vec<PathBuf>>,
    ) -> RafsResult<()> {
        if self.initialized {
            return Err(RafsError::AlreadyMounted);
        }
        if self.fs_prefetch {
            // Device should be ready before any prefetch.
            self.device.start_prefetch();
            self.prefetch(r, prefetch_files);
        }
        self.initialized = true;

        Ok(())
    }

    /// Umount a mounted Rafs Fuse filesystem.
    pub fn destroy(&mut self) -> Result<()> {
        info! {"Destroy rafs"}

        if self.initialized {
            Arc::get_mut(&mut self.sb)
                .expect("Superblock is no longer used")
                .destroy();
            if self.fs_prefetch {
                self.device.stop_prefetch();
            }
            self.device.close()?;
            self.initialized = false;
        }

        Ok(())
    }

    /// Get id of the filesystem instance.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the cached file system super block metadata.
    pub fn metadata(&self) -> &RafsSuperMeta {
        &self.sb.meta
    }

    fn prepare_storage_conf(conf: &RafsConfig) -> RafsResult<Arc<FactoryConfig>> {
        let mut storage_conf = conf.device.clone();
        storage_conf.cache.cache_validate = conf.digest_validate;
        storage_conf.cache.prefetch_config = TryFrom::try_from(conf)?;
        Ok(Arc::new(storage_conf))
    }

    fn xattr_supported(&self) -> bool {
        self.xattr_enabled || self.sb.meta.has_xattr()
    }

    fn do_readdir(
        &self,
        ino: Inode,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry) -> Result<usize>,
    ) -> Result<()> {
        if size == 0 {
            return Ok(());
        }

        let parent = self.sb.get_inode(ino, self.digest_validate)?;
        if !parent.is_dir() {
            return Err(enotdir!());
        }

        let mut handler = |_inode, name: OsString, ino, offset| {
            match add_entry(DirEntry {
                ino,
                offset,
                type_: 0,
                name: name.as_os_str().as_bytes(),
            }) {
                Ok(0) => {
                    self.ios.new_file_counter(ino);
                    Ok(PostWalkAction::Break)
                }
                Ok(_) => {
                    self.ios.new_file_counter(ino);
                    Ok(PostWalkAction::Continue)
                } // TODO: should we check `size` here?
                Err(e) => Err(e),
            }
        };

        parent.walk_children_inodes(offset, &mut handler)?;

        Ok(())
    }

    fn negative_entry(&self) -> Entry {
        Entry {
            attr: Attr {
                ..Default::default()
            }
            .into(),
            inode: 0,
            generation: 0,
            attr_flags: 0,
            attr_timeout: self.sb.meta.attr_timeout,
            entry_timeout: self.sb.meta.entry_timeout,
        }
    }

    fn get_inode_attr(&self, ino: u64) -> Result<Attr> {
        let inode = self.sb.get_inode(ino, false)?;
        let mut attr = inode.get_attr();

        // override uid/gid if there is no explicit inode uid/gid
        if !self.sb.meta.explicit_uidgid() {
            attr.uid = self.i_uid;
            attr.gid = self.i_gid;
        }

        // Older rafs image or the root inode doesn't include mtime, in such cases
        // we use runtime timestamp.
        if attr.mtime == 0 {
            attr.atime = self.i_time;
            attr.ctime = self.i_time;
            attr.mtime = self.i_time;
        }

        // Only touch permissions bits. This trick is some sort of workaround
        // since nydusify gives root directory permission of 0o750 and fuse mount
        // options `rootmode=` does not affect root directory's permission bits, ending
        // up with preventing other users from accessing the container rootfs.
        if attr.ino == self.root_ino() {
            attr.mode = attr.mode & !0o777 | 0o755;
        }

        Ok(attr)
    }

    fn get_inode_entry(&self, inode: Arc<dyn RafsInode>) -> Entry {
        let mut entry = inode.get_entry();

        // override uid/gid if there is no explicit inode uid/gid
        if !self.sb.meta.explicit_uidgid() {
            entry.attr.st_uid = self.i_uid;
            entry.attr.st_gid = self.i_gid;
        }

        // Older rafs image doesn't include mtime, in such case we use runtime timestamp.
        if entry.attr.st_mtime == 0 {
            entry.attr.st_atime = self.i_time as i64;
            entry.attr.st_ctime = self.i_time as i64;
            entry.attr.st_mtime = self.i_time as i64;
        }

        // Only touch permissions bits. This trick is some sort of workaround
        // since nydusify gives root directory permission of 0o750 and fuse mount
        // options `rootmode=` does not affect root directory's permission bits, ending
        // up with preventing other users from accessing the container rootfs.
        if entry.inode == ROOT_ID {
            entry.attr.st_mode = entry.attr.st_mode & !0o777 | 0o755;
        }

        entry
    }
}

impl Rafs {
    fn prefetch(&self, reader: RafsIoReader, prefetch_files: Option<Vec<PathBuf>>) {
        let sb = self.sb.clone();
        let device = self.device.clone();
        let prefetch_all = self.prefetch_all;
        let root_ino = self.root_ino();

        let _ = std::thread::spawn(move || {
            Self::do_prefetch(root_ino, reader, prefetch_files, prefetch_all, sb, device);
        });
    }

    /// for blobfs
    pub fn fetch_range_synchronous(&self, prefetches: &[BlobPrefetchRequest]) -> Result<()> {
        self.device.fetch_range_synchronous(prefetches)
    }

    fn root_ino(&self) -> u64 {
        self.sb.superblock.root_ino()
    }

    fn do_prefetch(
        root_ino: u64,
        mut reader: RafsIoReader,
        prefetch_files: Option<Vec<PathBuf>>,
        prefetch_all: bool,
        sb: Arc<RafsSuper>,
        device: BlobDevice,
    ) {
        // First do range based prefetch for rafs v6.
        if sb.meta.is_v6() {
            let mut prefetches = Vec::new();

            for blob in sb.superblock.get_blob_infos() {
                let sz = blob.readahead_size();
                if sz > 0 {
                    let mut offset = 0;
                    while offset < sz {
                        let len = cmp::min(sz - offset, RAFS_DEFAULT_CHUNK_SIZE);
                        prefetches.push(BlobPrefetchRequest {
                            blob_id: blob.blob_id().to_owned(),
                            offset,
                            len,
                        });
                        offset += len;
                    }
                }
            }
            if !prefetches.is_empty() {
                device.prefetch(&[], &prefetches).unwrap_or_else(|e| {
                    warn!("Prefetch error, {:?}", e);
                });
            }
        }

        let mut ignore_prefetch_all = prefetch_files
            .as_ref()
            .map(|f| f.len() == 1 && f[0].as_os_str() == "/")
            .unwrap_or(false);

        // Then do file based prefetch based on:
        // - prefetch listed passed in by user
        // - or file prefetch list in metadata
        let inodes = prefetch_files.map(|files| Self::convert_file_list(&files, &sb));
        let res = sb.prefetch_files(&mut reader, root_ino, inodes, &|desc| {
            if desc.bi_size > 0 {
                device.prefetch(&[desc], &[]).unwrap_or_else(|e| {
                    warn!("Prefetch error, {:?}", e);
                });
            }
        });
        match res {
            Ok(true) => ignore_prefetch_all = true,
            Ok(false) => {}
            Err(e) => info!("No file to be prefetched {:?}", e),
        }

        // Last optionally prefetch all data
        if prefetch_all && !ignore_prefetch_all {
            let root = vec![root_ino];
            let res = sb.prefetch_files(&mut reader, root_ino, Some(root), &|desc| {
                if desc.bi_size > 0 {
                    device.prefetch(&[desc], &[]).unwrap_or_else(|e| {
                        warn!("Prefetch error, {:?}", e);
                    });
                }
            });
            if let Err(e) = res {
                info!("No file to be prefetched {:?}", e);
            }
        }
    }

    fn convert_file_list(files: &[PathBuf], sb: &Arc<RafsSuper>) -> Vec<Inode> {
        let mut inodes = Vec::<Inode>::with_capacity(files.len());

        for f in files {
            if let Ok(inode) = sb.ino_from_path(f.as_path()) {
                inodes.push(inode);
            }
        }

        inodes
    }
}

impl BackendFileSystem for Rafs {
    fn mount(&self) -> Result<(Entry, u64)> {
        let root_inode = self.sb.get_inode(self.root_ino(), self.digest_validate)?;
        self.ios.new_file_counter(root_inode.ino());
        let e = self.get_inode_entry(root_inode);
        Ok((e, self.sb.get_max_ino()))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FileSystem for Rafs {
    type Inode = Inode;
    type Handle = Handle;

    fn init(&self, _opts: FsOptions) -> Result<FsOptions> {
        Ok(
            // These fuse features are supported by rafs by default.
            FsOptions::ASYNC_READ
                | FsOptions::PARALLEL_DIROPS
                | FsOptions::BIG_WRITES
                | FsOptions::HANDLE_KILLPRIV
                | FsOptions::ASYNC_DIO
                | FsOptions::HAS_IOCTL_DIR
                | FsOptions::WRITEBACK_CACHE
                | FsOptions::ZERO_MESSAGE_OPEN
                | FsOptions::ATOMIC_O_TRUNC
                | FsOptions::CACHE_SYMLINKS
                | FsOptions::ZERO_MESSAGE_OPENDIR,
        )
    }

    fn destroy(&self) {}

    fn lookup(&self, _ctx: &Context, ino: u64, name: &CStr) -> Result<Entry> {
        let mut rec = FopRecorder::settle(Lookup, ino, &self.ios);
        let target = OsStr::from_bytes(name.to_bytes());
        let parent = self.sb.get_inode(ino, self.digest_validate)?;
        if !parent.is_dir() {
            return Err(enotdir!());
        }

        rec.mark_success(0);
        if target == DOT || (ino == ROOT_ID && target == DOTDOT) {
            let mut entry = self.get_inode_entry(parent);
            entry.inode = ino;
            Ok(entry)
        } else if target == DOTDOT {
            Ok(self
                .sb
                .get_inode(parent.parent(), self.digest_validate)
                .map(|i| self.get_inode_entry(i))
                .unwrap_or_else(|_| self.negative_entry()))
        } else {
            Ok(parent
                .get_child_by_name(target)
                .map(|i| {
                    self.ios.new_file_counter(i.ino());
                    self.get_inode_entry(i)
                })
                .unwrap_or_else(|_| self.negative_entry()))
        }
    }

    fn forget(&self, _ctx: &Context, _inode: u64, _count: u64) {}

    fn batch_forget(&self, ctx: &Context, requests: Vec<(u64, u64)>) {
        for (inode, count) in requests {
            self.forget(ctx, inode, count)
        }
    }

    fn getattr(
        &self,
        _ctx: &Context,
        ino: u64,
        _handle: Option<u64>,
    ) -> Result<(stat64, Duration)> {
        let mut recorder = FopRecorder::settle(Getattr, ino, &self.ios);

        let attr = self.get_inode_attr(ino).map(|r| {
            recorder.mark_success(0);
            r
        })?;

        Ok((attr.into(), self.sb.meta.attr_timeout))
    }

    fn readlink(&self, _ctx: &Context, ino: u64) -> Result<Vec<u8>> {
        let mut rec = FopRecorder::settle(Readlink, ino, &self.ios);
        let inode = self.sb.get_inode(ino, self.digest_validate)?;

        Ok(inode
            .get_symlink()
            .map(|r| {
                rec.mark_success(0);
                r
            })?
            .as_bytes()
            .to_vec())
    }

    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        _ctx: &Context,
        ino: u64,
        _handle: u64,
        w: &mut dyn ZeroCopyWriter,
        size: u32,
        offset: u64,
        _lock_owner: Option<u64>,
        _flags: u32,
    ) -> Result<usize> {
        if offset.checked_add(size as u64).is_none() {
            return Err(einval!("offset + size wraps around."));
        }

        let inode = self.sb.get_inode(ino, false)?;
        let inode_size = inode.size();
        let mut recorder = FopRecorder::settle(Read, ino, &self.ios);
        // Check for zero size read.
        if size == 0 || offset >= inode_size {
            recorder.mark_success(0);
            return Ok(0);
        }

        let real_size = cmp::min(size as u64, inode_size - offset);
        let mut result = 0;
        let mut descs = inode.alloc_bio_vecs(offset, real_size as usize, true)?;
        debug_assert!(!descs.is_empty() && !descs[0].bi_vec.is_empty());

        // Try to amplify user io for Rafs v5, to improve performance.
        if self.sb.meta.is_v5() && size < self.amplify_io {
            let all_chunks_ready = self.device.all_chunks_ready(&descs);
            if !all_chunks_ready {
                let chunk_size = self.metadata().chunk_size as u64;
                let next_chunk_base = (offset + (size as u64) + chunk_size) & !chunk_size;
                let window_base = std::cmp::min(next_chunk_base, inode_size);
                let actual_size = window_base - (offset & !chunk_size);
                if actual_size < self.amplify_io as u64 {
                    let window_size = self.amplify_io as u64 - actual_size;
                    self.sb.amplify_io(
                        self.amplify_io,
                        &mut descs,
                        &inode,
                        window_base,
                        window_size,
                    )?;
                }
            }
        }

        let start = self.ios.latency_start();

        for desc in descs.iter_mut() {
            debug_assert!(desc.validate());
            debug_assert!(!desc.bi_vec.is_empty());
            debug_assert!(desc.bi_size != 0);

            // Avoid copying `desc`
            let r = self.device.read_to(w, desc)?;
            result += r;
            recorder.mark_success(r);
            if r as u32 != desc.bi_size {
                break;
            }
        }
        self.ios.latency_end(&start, Read);

        Ok(result)
    }

    fn open(
        &self,
        _ctx: &Context,
        _inode: Self::Inode,
        _flags: u32,
        _fuse_flags: u32,
    ) -> Result<(Option<Self::Handle>, OpenOptions)> {
        // Keep cache since we are readonly
        Ok((None, OpenOptions::KEEP_CACHE))
    }

    fn release(
        &self,
        _ctx: &Context,
        _inode: u64,
        _flags: u32,
        _handle: u64,
        _flush: bool,
        _flock_release: bool,
        _lock_owner: Option<u64>,
    ) -> Result<()> {
        Ok(())
    }

    fn statfs(&self, _ctx: &Context, _inode: u64) -> Result<statvfs64> {
        // Safe because we are zero-initializing a struct with only POD fields.
        let mut st: statvfs64 = unsafe { std::mem::zeroed() };

        // This matches the behavior of libfuse as it returns these values if the
        // filesystem doesn't implement this method.
        st.f_namemax = 255;
        st.f_bsize = 512;
        st.f_fsid = self.sb.meta.magic as u64;
        #[cfg(target_os = "macos")]
        {
            st.f_files = self.sb.meta.inodes_count as u32;
        }

        #[cfg(target_os = "linux")]
        {
            st.f_files = self.sb.meta.inodes_count;
        }

        Ok(st)
    }

    fn getxattr(
        &self,
        _ctx: &Context,
        inode: u64,
        name: &CStr,
        size: u32,
    ) -> Result<GetxattrReply> {
        let mut recorder = FopRecorder::settle(Getxattr, inode, &self.ios);

        if !self.xattr_supported() {
            return Err(std::io::Error::from_raw_os_error(libc::ENOSYS));
        }

        let name = OsStr::from_bytes(name.to_bytes());
        let inode = self.sb.get_inode(inode, false)?;
        let value = inode.get_xattr(name)?;
        let r = match value {
            Some(value) => match size {
                0 => Ok(GetxattrReply::Count((value.len() + 1) as u32)),
                x if x < value.len() as u32 => Err(std::io::Error::from_raw_os_error(libc::ERANGE)),
                _ => Ok(GetxattrReply::Value(value)),
            },
            None => {
                // TODO: Hopefully, we can have a 'decorator' procedure macro in
                // the future to wrap this method thus to handle different reasonable
                // errors in a clean way.
                recorder.mark_success(0);
                Err(std::io::Error::from_raw_os_error(libc::ENODATA))
            }
        };

        r.map(|v| {
            recorder.mark_success(0);
            v
        })
    }

    fn listxattr(&self, _ctx: &Context, inode: u64, size: u32) -> Result<ListxattrReply> {
        let mut rec = FopRecorder::settle(Listxattr, inode, &self.ios);
        if !self.xattr_supported() {
            return Err(std::io::Error::from_raw_os_error(libc::ENOSYS));
        }

        let inode = self.sb.get_inode(inode, false)?;
        let mut count = 0;
        let mut buf = Vec::new();
        for mut name in inode.get_xattrs()? {
            count += name.len() + 1;
            if size != 0 {
                buf.append(&mut name);
                buf.append(&mut vec![0u8; 1]);
            }
        }

        rec.mark_success(0);

        match size {
            0 => Ok(ListxattrReply::Count(count as u32)),
            x if x < count as u32 => Err(std::io::Error::from_raw_os_error(libc::ERANGE)),
            _ => Ok(ListxattrReply::Names(buf)),
        }
    }

    fn readdir(
        &self,
        _ctx: &Context,
        inode: u64,
        _handle: u64,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry) -> Result<usize>,
    ) -> Result<()> {
        let mut rec = FopRecorder::settle(Readdir, inode, &self.ios);

        self.do_readdir(inode, size, offset, add_entry).map(|r| {
            rec.mark_success(0);
            r
        })
    }

    fn readdirplus(
        &self,
        _ctx: &Context,
        ino: u64,
        _handle: u64,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry, Entry) -> Result<usize>,
    ) -> Result<()> {
        let mut rec = FopRecorder::settle(Readdirplus, ino, &self.ios);

        self.do_readdir(ino, size, offset, &mut |dir_entry| {
            let inode = self.sb.get_inode(dir_entry.ino, self.digest_validate)?;
            add_entry(dir_entry, self.get_inode_entry(inode))
        })
        .map(|r| {
            rec.mark_success(0);
            r
        })
    }

    fn opendir(
        &self,
        _ctx: &Context,
        _inode: Self::Inode,
        _flags: u32,
    ) -> Result<(Option<Self::Handle>, OpenOptions)> {
        // Cache dir since we are readonly
        Ok((None, OpenOptions::CACHE_DIR | OpenOptions::KEEP_CACHE))
    }

    fn releasedir(&self, _ctx: &Context, _inode: u64, _flags: u32, _handle: u64) -> Result<()> {
        Ok(())
    }

    fn access(&self, ctx: &Context, ino: u64, mask: u32) -> Result<()> {
        let mut rec = FopRecorder::settle(Access, ino, &self.ios);
        let st = self.get_inode_attr(ino)?;
        let mode = mask as i32 & (libc::R_OK | libc::W_OK | libc::X_OK);

        if mode == libc::F_OK {
            rec.mark_success(0);
            return Ok(());
        }

        if (mode & libc::R_OK) != 0
            && ctx.uid != 0
            && (st.uid != ctx.uid || st.mode & 0o400 == 0)
            && (st.gid != ctx.gid || st.mode & 0o040 == 0)
            && st.mode & 0o004 == 0
        {
            return Err(eacces!("permission denied"));
        }

        if (mode & libc::W_OK) != 0
            && ctx.uid != 0
            && (st.uid != ctx.uid || st.mode & 0o200 == 0)
            && (st.gid != ctx.gid || st.mode & 0o020 == 0)
            && st.mode & 0o002 == 0
        {
            return Err(eacces!("permission denied"));
        }

        // root can only execute something if it is executable by one of the owner, the group, or
        // everyone.
        if (mode & libc::X_OK) != 0
            && (ctx.uid != 0 || st.mode & 0o111 == 0)
            && (st.uid != ctx.uid || st.mode & 0o100 == 0)
            && (st.gid != ctx.gid || st.mode & 0o010 == 0)
            && st.mode & 0o001 == 0
        {
            return Err(eacces!("permission denied"));
        }

        rec.mark_success(0);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::metadata::RAFS_DEFAULT_CHUNK_SIZE;
    use crate::RafsIoRead;

    pub fn new_rafs_backend() -> Box<Rafs> {
        let config = r#"
        {
            "device": {
              "id": "test",
              "backend": {
                "type": "oss",
                "config": {
                  "endpoint": "test",
                  "access_key_id": "test",
                  "access_key_secret": "test",
                  "bucket_name": "antsys-nydus",
                  "object_prefix":"nydus_v2/",
                  "scheme": "http"
                }
              }
            },
            "mode": "direct",
            "digest_validate": false,
            "enable_xattr": true,
            "fs_prefetch": {
              "enable": true,
              "threads_count": 10,
              "merging_size": 131072,
              "bandwidth_rate": 10485760
            }
          }"#;
        let root_dir = &std::env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR");
        let mut source_path = PathBuf::from(root_dir);
        source_path.push("../tests/texture/bootstrap/image_v2.boot");
        let mountpoint = "/mnt";
        let rafs_config = RafsConfig::from_str(config).unwrap();
        let bootstrapfile = source_path.to_str().unwrap();
        let mut bootstrap = <dyn RafsIoRead>::from_file(bootstrapfile).unwrap();
        let mut rafs = Rafs::new(rafs_config, mountpoint, &mut bootstrap).unwrap();
        rafs.import(bootstrap, Some(vec![std::path::PathBuf::new()]))
            .unwrap();
        Box::new(rafs)
    }

    #[test]
    fn it_should_create_new_rafs_fs() {
        let rafs = new_rafs_backend();
        let attr = rafs.get_inode_attr(1).unwrap();
        assert_eq!(attr.ino, 1);
        assert_eq!(attr.blocks, 8);
        assert_eq!(attr.uid, 0);
        // Root inode mode must be 0755
        assert_eq!(attr.mode & 0o777, 0o755);
    }

    #[test]
    fn it_should_access() {
        let rafs = new_rafs_backend();
        let ctx = &Context {
            gid: 0,
            pid: 1,
            uid: 0,
        };
        if rafs.access(ctx, 1, 0).is_err() {
            panic!("failed to access inode 1");
        }
    }

    #[test]
    fn it_should_listxattr() {
        let rafs = new_rafs_backend();
        let ctx = &Context {
            gid: 0,
            pid: 1,
            uid: 0,
        };
        match rafs.listxattr(ctx, 1, 0) {
            Ok(reply) => match reply {
                ListxattrReply::Count(c) => assert_eq!(c, 0),
                _ => panic!(),
            },
            Err(_) => panic!("failed to access inode 1"),
        }
    }

    #[test]
    fn it_should_get_statfs() {
        let rafs = new_rafs_backend();
        let ctx = &Context {
            gid: 0,
            pid: 1,
            uid: 0,
        };
        match rafs.statfs(ctx, 1) {
            Ok(statfs) => {
                assert_eq!(statfs.f_files, 43082);
                assert_eq!(statfs.f_bsize, 512);
                assert_eq!(statfs.f_namemax, 255);
                assert_eq!(statfs.f_fsid, 1380009555);
                assert_eq!(statfs.f_ffree, 0);
            }
            Err(_) => panic!("failed to statfs"),
        }
    }

    #[test]
    fn it_should_enable_xattr() {
        let rafs = new_rafs_backend();
        assert!(rafs.xattr_supported());
    }

    #[test]
    fn it_should_lookup_entry() {
        let rafs = new_rafs_backend();
        let ctx = &Context {
            gid: 0,
            pid: 1,
            uid: 0,
        };
        match rafs.lookup(ctx, 1, &std::ffi::CString::new("/etc").unwrap()) {
            Err(e) => {
                println!("{:?}", e);
                panic!("failed to lookup /etc from ino 1");
            }
            Ok(e) => {
                assert_eq!(e.inode, 0);
            }
        }
    }

    #[test]
    fn test_fsprefetchcontrol_from_rafs_config() {
        let mut config = RafsConfig {
            fs_prefetch: FsPrefetchControl {
                enable: false,
                threads_count: 0,
                merging_size: 0,
                bandwidth_rate: 0,
                prefetch_all: false,
            },
            ..Default::default()
        };

        config.fs_prefetch.enable = true;
        assert!(BlobPrefetchConfig::try_from(&config).is_err());

        config.fs_prefetch.threads_count = 1;
        assert!(BlobPrefetchConfig::try_from(&config).is_ok());

        config.fs_prefetch.merging_size = RAFS_MAX_CHUNK_SIZE as usize + 1;
        assert!(BlobPrefetchConfig::try_from(&config).is_err());

        config.fs_prefetch.merging_size = RAFS_DEFAULT_CHUNK_SIZE as usize;
        config.fs_prefetch.bandwidth_rate = 1;
        config.fs_prefetch.prefetch_all = true;
        assert!(BlobPrefetchConfig::try_from(&config).is_ok());
    }
}
