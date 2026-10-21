// Copyright 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use kata_sys_util::netns::NetnsGuard;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::marker::PhantomData;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use caps::{CapSet, Capability};
use dbs_device::resources::{DeviceResources, ResourceConstraint};
use dbs_utils::epoll_manager::{EpollManager, SubscriberId};
use dbs_utils::rate_limiter::{BucketUpdate, RateLimiter};
use fuse_backend_rs::api::{Vfs, VfsIndex, VfsOptions};
use fuse_backend_rs::passthrough::{CachePolicy, Config as PassthroughConfig, PassthroughFs};
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use log::{debug, error, info, trace, warn};
use nix::sys::memfd;
use nydus_api::ConfigV2;
use nydus_rafs::blobfs::{BlobFs, Config as BlobfsConfig};
use nydus_rafs::{fs::Rafs, RafsIoRead};
use opendal::options::{DeleteOptions, ListOptions, WriteOptions};
use opendal::services::Oss as OssService;
use opendal::ErrorKind as OpenDalErrorKind;
use opendal::Operator as OpenDalOperator;
use rlimit::Resource;
use serde::{Deserialize, Serialize};
use tokio::runtime::Builder as TokioRuntimeBuilder;
use virtio_bindings::bindings::virtio_config::{VIRTIO_F_ACCESS_PLATFORM, VIRTIO_F_VERSION_1};
use virtio_queue::QueueT;
use vm_memory::{
    FileOffset, GuestAddress, GuestAddressSpace, GuestRegionMmap, GuestUsize, MmapRegion,
};
use vmm_sys_util::eventfd::EventFd;

use crate::{
    ActivateError, ActivateResult, ConfigResult, Error, Result, VirtioDevice, VirtioDeviceConfig,
    VirtioDeviceInfo, VirtioRegionHandler, VirtioSharedMemory, VirtioSharedMemoryList,
    TYPE_VIRTIO_FS,
};

use super::{
    CacheHandler, Error as FsError, Result as FsResult, VirtioFsEpollHandler, VIRTIO_FS_NAME,
};

const CONFIG_SPACE_TAG_SIZE: usize = 36;
const CONFIG_SPACE_NUM_QUEUES_SIZE: usize = 4;
const CONFIG_SPACE_SIZE: usize = CONFIG_SPACE_TAG_SIZE + CONFIG_SPACE_NUM_QUEUES_SIZE;
const NUM_QUEUE_OFFSET: usize = 1;

// Attr and entry timeout values
const CACHE_ALWAYS_TIMEOUT: u64 = 86_400; // 1 day
const CACHE_AUTO_TIMEOUT: u64 = 1;
const CACHE_NONE_TIMEOUT: u64 = 0;

// VirtioFs backend fs type
pub(crate) const PASSTHROUGHFS: &str = "passthroughfs";
pub(crate) const BLOBFS: &str = "blobfs";
pub(crate) const RAFS: &str = "rafs";
pub(crate) const OSSFS: &str = "ossfs";

const OSS_CACHE_DATA_DIR: &str = "data";
const OSS_CACHE_INDEX_FILE: &str = ".dragonball_oss_cache_index.json";
const OSS_CONFLICTS_DIR: &str = ".conflicts";
const OSS_WRITE_THROUGH_INTERVAL_SECS: u64 = 1;
const OSS_ASYNC_FLUSH_INTERVAL_SECS: u64 = 30;
const OSS_DEFAULT_UPLOAD_CHUNK_SIZE: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
struct OssFsConfig {
    endpoint: String,
    #[serde(alias = "bucket_name")]
    bucket: String,
    access_key_id: String,
    access_key_secret: String,
    #[serde(default)]
    security_token: Option<String>,
    #[serde(default)]
    addressing_style: Option<String>,
    #[serde(default, alias = "object_prefix")]
    object_prefix: Option<String>,
    #[serde(default)]
    local_cache_dir: Option<String>,
    #[serde(default)]
    cache_dir: Option<String>,
    #[serde(default, alias = "writeback_mode", alias = "write_mode")]
    write_policy: Option<String>,
    #[serde(default)]
    flush_interval_secs: Option<u64>,
    #[serde(default, alias = "consistency")]
    consistency_strategy: Option<String>,
    #[serde(default)]
    upload_chunk_size: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OssWritePolicy {
    ReadOnly,
    WriteThrough,
    AsyncFlush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OssConsistencyStrategy {
    LocalWins,
    EtagGuard,
    RemoteWins,
}

#[derive(Debug, Clone)]
struct OssSyncContext {
    config: OssFsConfig,
    mountpoint: String,
    prefix: String,
    cache_root: PathBuf,
    cache_data_dir: PathBuf,
    consistency_strategy: OssConsistencyStrategy,
    upload_chunk_size: usize,
    sync_lock: Arc<Mutex<()>>,
}

#[derive(Debug)]
struct OssFlushWorker {
    stop_tx: mpsc::Sender<()>,
    handle: JoinHandle<()>,
}

#[derive(Debug)]
struct OssBackendState {
    sync_ctx: OssSyncContext,
    write_policy: OssWritePolicy,
    worker: Option<OssFlushWorker>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LocalCacheState {
    size: u64,
    mtime_ns: u128,
}

#[derive(Debug, Clone)]
struct LocalCacheFile {
    path: PathBuf,
    state: LocalCacheState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct OssCacheEntry {
    etag: Option<String>,
    content_length: u64,
    #[serde(default)]
    local_size: u64,
    #[serde(default)]
    local_mtime_ns: u128,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct OssCacheIndex {
    entries: HashMap<String, OssCacheEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct OssDownloadSyncStats {
    downloaded: usize,
    reused: usize,
    removed: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct OssWritebackSyncStats {
    uploaded: usize,
    deleted: usize,
    skipped_conflicts: usize,
    unchanged: usize,
    isolated: usize,
    remote_wins: usize,
}

#[derive(Debug, Clone)]
enum RemoteObjectState {
    NotFound,
    Existing {
        etag: Option<String>,
        content_length: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OssConditionalOpOutcome {
    Applied,
    Conflict,
    Unsupported,
}

/// Info of backend filesystems of VirtioFs
#[allow(dead_code)]
pub struct BackendFsInfo {
    pub(crate) index: VfsIndex,
    pub(crate) fstype: String,
    // (source, config), only suitable for Rafs
    pub(crate) src_cfg: Option<(String, String)>,
}

/// Virtio device for virtiofs
pub struct VirtioFs<AS: GuestAddressSpace> {
    pub(crate) device_info: VirtioDeviceInfo,
    pub(crate) cache_size: u64,
    pub(crate) queue_sizes: Arc<Vec<u16>>,
    pub(crate) thread_pool_size: u16,
    pub(crate) cache_policy: CachePolicy,
    pub(crate) writeback_cache: bool,
    pub(crate) no_open: bool,
    pub(crate) killpriv_v2: bool,
    pub(crate) no_readdir: bool,
    pub(crate) xattr: bool,
    pub(crate) handler: Box<dyn VirtioRegionHandler>,
    pub(crate) fs: Arc<Vfs>,
    pub(crate) backend_fs: HashMap<String, BackendFsInfo>,
    oss_backends: HashMap<String, OssBackendState>,
    pub(crate) subscriber_id: Option<SubscriberId>,
    pub(crate) id: String,
    pub(crate) rate_limiter: Option<RateLimiter>,
    pub(crate) patch_rate_limiter_fd: EventFd,
    pub(crate) sender: Option<mpsc::Sender<(BucketUpdate, BucketUpdate)>>,
    phantom: PhantomData<AS>,
}

impl<AS> VirtioFs<AS>
where
    AS: GuestAddressSpace + 'static,
{
    pub fn set_patch_rate_limiters(&self, bytes: BucketUpdate, ops: BucketUpdate) -> Result<()> {
        match &self.sender {
            Some(sender) => {
                sender.send((bytes, ops)).map_err(|e| {
                    error!("{VIRTIO_FS_NAME}: failed to send rate-limiter patch data {e:?}");
                    Error::InternalError
                })?;
                self.patch_rate_limiter_fd.write(1).map_err(|e| {
                    error!("{VIRTIO_FS_NAME}: failed to write rate-limiter patch event {e:?}");
                    Error::InternalError
                })?;
                Ok(())
            }
            None => {
                error!(
                    "{VIRTIO_FS_NAME}: failed to establish channel to send rate-limiter patch data"
                );
                Err(Error::InternalError)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
impl<AS: GuestAddressSpace> VirtioFs<AS> {
    /// Create a new virtiofs device.
    pub fn new(
        tag: &str,
        req_num_queues: usize,
        queue_size: u16,
        cache_size: u64,
        cache_policy: &str,
        thread_pool_size: u16,
        writeback_cache: bool,
        no_open: bool,
        killpriv_v2: bool,
        xattr: bool,
        drop_sys_resource: bool,
        no_readdir: bool,
        handler: Box<dyn VirtioRegionHandler>,
        epoll_mgr: EpollManager,
        rate_limiter: Option<RateLimiter>,
        f_access_platform: bool,
    ) -> Result<Self> {
        info!(
            "{VIRTIO_FS_NAME}: tag {tag} req_num_queues {req_num_queues} queue_size {queue_size} cache_size {cache_size} cache_policy {cache_policy} thread_pool_size {thread_pool_size} writeback_cache {writeback_cache} no_open {no_open} killpriv_v2 {killpriv_v2} xattr {xattr} drop_sys_resource {drop_sys_resource} no_readdir {no_readdir}"
        );

        let num_queues = NUM_QUEUE_OFFSET + req_num_queues;

        // Create virtio device config space.
        // First by adding the tag.
        let mut config_space = tag.to_string().into_bytes();
        config_space.resize(CONFIG_SPACE_SIZE, 0);

        // And then by copying the number of queues.
        let mut num_queues_slice: [u8; 4] = (req_num_queues as u32).to_be_bytes();
        num_queues_slice.reverse();
        config_space[CONFIG_SPACE_TAG_SIZE..CONFIG_SPACE_SIZE].copy_from_slice(&num_queues_slice);

        let cache = match CachePolicy::from_str(cache_policy) {
            Ok(c) => c,
            Err(e) => {
                error!("{VIRTIO_FS_NAME}: Parse cache_policy \"{cache_policy}\" failed: {e:?}");
                return Err(Error::InvalidInput);
            }
        };

        // Set rlimit first, in case we dropped CAP_SYS_RESOURCE later and hit EPERM.
        if let Err(e) = set_default_rlimit_nofile() {
            warn!("{VIRTIO_FS_NAME}: failed to set rlimit: {e:?}");
        }

        if drop_sys_resource && writeback_cache {
            error!("{VIRTIO_FS_NAME}: writeback_cache is not compatible with drop_sys_resource");
            return Err(Error::InvalidInput);
        }

        // Drop CAP_SYS_RESOURCE when creating VirtioFs device, not in activate(), as it's vcpu
        // thread that calls activate(), but we do I/O in vmm epoll thread, so drop cap here.
        if drop_sys_resource {
            info!(
                "{}: Dropping CAP_SYS_RESOURCE, tid {:?}",
                VIRTIO_FS_NAME,
                nix::unistd::gettid()
            );
            if let Err(e) = caps::drop(None, CapSet::Effective, Capability::CAP_SYS_RESOURCE) {
                warn!("{VIRTIO_FS_NAME}: failed to drop CAP_SYS_RESOURCE: {e:?}");
            }
        }

        let vfs_opts = VfsOptions {
            no_writeback: !writeback_cache,
            no_open,
            killpriv_v2,
            no_readdir,
            ..VfsOptions::default()
        };

        let mut avail_features = 1u64 << VIRTIO_F_VERSION_1;

        if f_access_platform {
            avail_features |= 1u64 << VIRTIO_F_ACCESS_PLATFORM;
        }

        Ok(VirtioFs {
            device_info: VirtioDeviceInfo::new(
                VIRTIO_FS_NAME.to_string(),
                avail_features,
                Arc::new(vec![queue_size; num_queues]),
                config_space,
                epoll_mgr,
            ),
            cache_size,
            queue_sizes: Arc::new(vec![queue_size; num_queues]),
            thread_pool_size,
            cache_policy: cache,
            writeback_cache,
            no_open,
            no_readdir,
            killpriv_v2,
            xattr,
            handler,
            fs: Arc::new(Vfs::new(vfs_opts)),
            backend_fs: HashMap::new(),
            oss_backends: HashMap::new(),
            subscriber_id: None,
            id: tag.to_string(),
            rate_limiter,
            patch_rate_limiter_fd: EventFd::new(0).unwrap(),
            sender: None,
            phantom: PhantomData,
        })
    }

    fn is_dax_on(&self) -> bool {
        self.cache_size > 0
    }

    fn get_timeout(&self) -> Duration {
        match self.cache_policy {
            CachePolicy::Always => Duration::from_secs(CACHE_ALWAYS_TIMEOUT),
            CachePolicy::Never => Duration::from_secs(CACHE_NONE_TIMEOUT),
            CachePolicy::Auto => Duration::from_secs(CACHE_AUTO_TIMEOUT),
            CachePolicy::Metadata => Duration::from_secs(CACHE_AUTO_TIMEOUT),
        }
    }

    fn parse_blobfs_cfg(
        &self,
        source: &str,
        config: Option<String>,
        dax_threshold_size_kb: Option<u64>,
    ) -> FsResult<(String, String, Option<u64>)> {
        let (blob_cache_dir, blob_ondemand_cfg) = match config.as_ref() {
            Some(cfg) => {
                let conf = ConfigV2::from_str(cfg).map_err(|e| {
                    error!("failed to load rafs config {} error: {:?}", cfg, e);
                    FsError::InvalidData
                })?;

                // v6 doesn't support digest validation yet.
                if conf.rafs.ok_or(FsError::InvalidData)?.validate {
                    error!("config.digest_validate needs to be false");
                    return Err(FsError::InvalidData);
                }

                let work_dir = conf
                    .cache
                    .ok_or(FsError::InvalidData)?
                    .file_cache
                    .ok_or(FsError::InvalidData)?
                    .work_dir;

                let blob_ondemand_cfg = format!(
                    r#"
                    {{
                        "rafs_conf": {},
                        "bootstrap_path": "{}",
                        "blob_cache_dir": "{}"
                    }}"#,
                    cfg, source, work_dir
                );

                (work_dir, blob_ondemand_cfg)
            }
            None => return Err(FsError::BackendFs("no rafs config file".to_string())),
        };

        let dax_file_size = match dax_threshold_size_kb {
            Some(size) => Some(kb_to_bytes(size)?),
            None => None,
        };

        Ok((blob_cache_dir, blob_ondemand_cfg, dax_file_size))
    }

    pub fn manipulate_backend_fs(
        &mut self,
        source: Option<String>,
        fstype: Option<String>,
        mountpoint: &str,
        config: Option<String>,
        ops: &str,
        prefetch_list_path: Option<String>,
        dax_threshold_size_kb: Option<u64>,
    ) -> FsResult<()> {
        let config_present = config
            .as_ref()
            .map(|cfg| !cfg.trim().is_empty())
            .unwrap_or(false);
        debug!(
            "source {source:?}, fstype {fstype:?}, mountpoint {mountpoint:?}, config_present {config_present:?}, ops {ops:?}, prefetch_list_path {prefetch_list_path:?}, dax_threshold_size_kb 0x{dax_threshold_size_kb:x?}"
        );
        match ops {
            "mount" => {
                if source.is_none() {
                    error!("{VIRTIO_FS_NAME}: source is required for mount.");
                    return Err(FsError::InvalidData);
                }
                // safe because is not None
                let source = source.unwrap();
                match fstype.as_deref() {
                    Some("Blobfs") | Some(BLOBFS) => {
                        self.mount_blobfs(source, mountpoint, config, dax_threshold_size_kb)
                    }
                    Some("PassthroughFs") | Some(PASSTHROUGHFS) => {
                        self.mount_passthroughfs(source, mountpoint, dax_threshold_size_kb)
                    }
                    Some("Rafs") | Some(RAFS) => {
                        self.mount_rafs(source, mountpoint, config, prefetch_list_path)
                    }
                    Some("OssFs") | Some(OSSFS) => {
                        self.mount_ossfs(source, mountpoint, config, dax_threshold_size_kb)
                    }
                    _ => {
                        error!("http_server: type is not invalid.");
                        Err(FsError::InvalidData)
                    }
                }
            }
            "umount" => {
                self.flush_oss_backend_before_umount(mountpoint)?;
                self.fs.umount(mountpoint).map_err(|e| {
                    error!("umount {e:?}");
                    FsError::InvalidData
                })?;
                self.backend_fs.remove(mountpoint);
                Ok(())
            }
            "flush" => self.flush_oss_backend(mountpoint),
            "update" => {
                info!("switch backend");
                self.update_rafs(source, mountpoint, config)
            }
            _ => {
                error!("invalid ops, mount failed.");
                Err(FsError::InvalidData)
            }
        }
    }

    fn flush_oss_backend_before_umount(&mut self, mountpoint: &str) -> FsResult<()> {
        let Some(mut backend) = self.oss_backends.remove(mountpoint) else {
            return Ok(());
        };

        stop_oss_flush_worker(&mut backend.worker);
        if backend.write_policy == OssWritePolicy::ReadOnly {
            return Ok(());
        }

        let stats = sync_local_cache_to_oss(&backend.sync_ctx)?;
        info!(
            "{VIRTIO_FS_NAME}: ossfs final flush done at {mountpoint}, uploaded {}, deleted {}, conflicts {}, unchanged {}, isolated {}, remote_wins {}",
            stats.uploaded,
            stats.deleted,
            stats.skipped_conflicts,
            stats.unchanged,
            stats.isolated,
            stats.remote_wins
        );
        Ok(())
    }

    fn flush_oss_backend(&mut self, mountpoint: &str) -> FsResult<()> {
        let backend = self.oss_backends.get(mountpoint).ok_or_else(|| {
            FsError::BackendFs(format!("ossfs mount point {mountpoint} not found"))
        })?;

        if backend.write_policy == OssWritePolicy::ReadOnly {
            return Ok(());
        }

        let stats = sync_local_cache_to_oss(&backend.sync_ctx)?;
        info!(
            "{VIRTIO_FS_NAME}: ossfs flush done at {mountpoint}, uploaded {}, deleted {}, conflicts {}, unchanged {}, isolated {}, remote_wins {}",
            stats.uploaded,
            stats.deleted,
            stats.skipped_conflicts,
            stats.unchanged,
            stats.isolated,
            stats.remote_wins
        );
        Ok(())
    }

    fn mount_blobfs(
        &mut self,
        source: String,
        mountpoint: &str,
        config: Option<String>,
        dax_threshold_size_kb: Option<u64>,
    ) -> FsResult<()> {
        debug!("http_server blobfs");
        let timeout = self.get_timeout();
        let (blob_cache_dir, blob_ondemand_cfg, dax_file_size) =
            self.parse_blobfs_cfg(&source, config, dax_threshold_size_kb)?;

        let fs_cfg = BlobfsConfig {
            ps_config: PassthroughConfig {
                root_dir: blob_cache_dir,
                do_import: true,
                writeback: self.writeback_cache,
                no_open: self.no_open,
                xattr: self.xattr,
                cache_policy: self.cache_policy.clone(),
                entry_timeout: timeout,
                attr_timeout: timeout,
                dax_file_size,
                ..Default::default()
            },
            blob_ondemand_cfg,
        };
        let blob_fs = BlobFs::new(fs_cfg).map_err(FsError::IOError)?;
        blob_fs.import().map_err(FsError::IOError)?;
        debug!("blobfs mounted");

        let fs = Box::new(blob_fs);
        match self.fs.mount(fs, mountpoint) {
            Ok(idx) => {
                self.backend_fs.insert(
                    mountpoint.to_string(),
                    BackendFsInfo {
                        index: idx,
                        fstype: BLOBFS.to_string(),
                        src_cfg: None,
                    },
                );
                Ok(())
            }
            Err(e) => {
                error!("blobfs mount {e:?}");
                Err(FsError::InvalidData)
            }
        }
    }

    fn mount_passthroughfs(
        &mut self,
        source: String,
        mountpoint: &str,
        dax_threshold_size_kb: Option<u64>,
    ) -> FsResult<()> {
        debug!("http_server passthrough");
        let timeout = self.get_timeout();

        let dax_threshold_size = match dax_threshold_size_kb {
            Some(size) => Some(kb_to_bytes(size)?),
            None => None,
        };

        let fs_cfg = PassthroughConfig {
            root_dir: source,
            do_import: false,
            writeback: self.writeback_cache,
            no_open: self.no_open,
            no_readdir: self.no_readdir,
            killpriv_v2: self.killpriv_v2,
            xattr: self.xattr,
            cache_policy: self.cache_policy.clone(),
            entry_timeout: timeout,
            attr_timeout: timeout,
            dax_file_size: dax_threshold_size,
            ..Default::default()
        };

        let passthrough_fs = PassthroughFs::<()>::new(fs_cfg).map_err(FsError::IOError)?;
        passthrough_fs.import().map_err(FsError::IOError)?;
        debug!("passthroughfs mounted");

        let fs = Box::new(passthrough_fs);
        match self.fs.mount(fs, mountpoint) {
            Ok(idx) => {
                self.backend_fs.insert(
                    mountpoint.to_string(),
                    BackendFsInfo {
                        index: idx,
                        fstype: PASSTHROUGHFS.to_string(),
                        src_cfg: None,
                    },
                );
                Ok(())
            }
            Err(e) => {
                error!("passthroughfs mount {e:?}");
                Err(FsError::InvalidData)
            }
        }
    }

    fn mount_ossfs(
        &mut self,
        source: String,
        mountpoint: &str,
        config: Option<String>,
        dax_threshold_size_kb: Option<u64>,
    ) -> FsResult<()> {
        debug!("http_server ossfs");

        let cfg = config.ok_or_else(|| FsError::BackendFs("no oss config file".to_string()))?;
        let cfg: OssFsConfig = serde_json::from_str(&cfg)
            .map_err(|e| FsError::BackendFs(format!("invalid ossfs config: {e}")))?;
        let write_policy = parse_oss_write_policy(cfg.write_policy.as_deref())?;
        let consistency_strategy =
            parse_oss_consistency_strategy(cfg.consistency_strategy.as_deref())?;
        let flush_interval = resolve_oss_flush_interval(write_policy, cfg.flush_interval_secs)?;
        let upload_chunk_size = resolve_oss_upload_chunk_size(cfg.upload_chunk_size)?;

        let cache_base = cfg
            .local_cache_dir
            .as_deref()
            .or(cfg.cache_dir.as_deref())
            .ok_or_else(|| {
                FsError::BackendFs(
                    "ossfs requires local_cache_dir (or cache_dir) in config".to_string(),
                )
            })?;
        let prefix = normalize_oss_prefix(&source, cfg.object_prefix.as_deref());
        let namespace = oss_cache_namespace(&cfg.bucket, &prefix);
        let cache_root = Path::new(cache_base).join(namespace);
        let cache_data_dir = cache_root.join(OSS_CACHE_DATA_DIR);
        let sync_ctx = OssSyncContext {
            config: cfg,
            mountpoint: mountpoint.to_string(),
            prefix,
            cache_root,
            cache_data_dir: cache_data_dir.clone(),
            consistency_strategy,
            upload_chunk_size,
            sync_lock: Arc::new(Mutex::new(())),
        };

        fs::create_dir_all(&sync_ctx.cache_data_dir).map_err(FsError::IOError)?;
        let stats = sync_oss_objects_to_cache(&sync_ctx)?;
        info!(
            "{VIRTIO_FS_NAME}: ossfs cache pull sync done at {}, downloaded {}, reused {}, removed {}",
            mountpoint, stats.downloaded, stats.reused, stats.removed
        );

        if write_policy != OssWritePolicy::ReadOnly {
            let stats = sync_local_cache_to_oss(&sync_ctx)?;
            info!(
                "{VIRTIO_FS_NAME}: ossfs initial writeback sync done at {}, uploaded {}, deleted {}, conflicts {}, unchanged {}, isolated {}, remote_wins {}",
                mountpoint,
                stats.uploaded,
                stats.deleted,
                stats.skipped_conflicts,
                stats.unchanged,
                stats.isolated,
                stats.remote_wins
            );
        }

        self.mount_passthroughfs(
            sync_ctx.cache_data_dir.to_string_lossy().into_owned(),
            mountpoint,
            dax_threshold_size_kb,
        )?;
        if let Some(info) = self.backend_fs.get_mut(mountpoint) {
            info.fstype = OSSFS.to_string();
        }
        let worker = if write_policy == OssWritePolicy::ReadOnly {
            None
        } else {
            Some(start_oss_flush_worker(
                sync_ctx.clone(),
                flush_interval,
                mountpoint.to_string(),
            )?)
        };
        if let Some(mut old_backend) = self.oss_backends.remove(mountpoint) {
            stop_oss_flush_worker(&mut old_backend.worker);
        }
        self.oss_backends.insert(
            mountpoint.to_string(),
            OssBackendState {
                sync_ctx,
                write_policy,
                worker,
            },
        );

        Ok(())
    }

    fn mount_rafs(
        &mut self,
        source: String,
        mountpoint: &str,
        config: Option<String>,
        prefetch_list_path: Option<String>,
    ) -> FsResult<()> {
        debug!("http_server rafs");
        // We need to make sure the nydus worker thread in the runD main process's network namespace
        // instead of the vmm thread's netns, which wouldn't access the host network.
        let _netns_guard =
            NetnsGuard::new("/proc/self/ns/net").map_err(|e| FsError::BackendFs(e.to_string()))?;

        let file = Path::new(&source);
        let (mut rafs, rafs_cfg) = match config.as_ref() {
            Some(cfg) => {
                let rafs_conf: Arc<ConfigV2> = Arc::new(
                    ConfigV2::from_str(cfg).map_err(|e| FsError::BackendFs(e.to_string()))?,
                );

                (
                    Rafs::new(&rafs_conf, mountpoint, file)
                        .map_err(|e| FsError::BackendFs(format!("Rafs::new() failed: {e:?}")))?,
                    cfg.clone(),
                )
            }
            None => return Err(FsError::BackendFs("no rafs config file".to_string())),
        };
        let prefetch_files = parse_prefetch_files(prefetch_list_path.clone());
        debug!("{VIRTIO_FS_NAME}: Import rafs with prefetch_files {prefetch_files:?}");
        rafs.0
            .import(rafs.1, prefetch_files)
            .map_err(|e| FsError::BackendFs(format!("Import rafs failed: {e:?}")))?;
        info!("{VIRTIO_FS_NAME}: Rafs imported with prefetch_list_path {prefetch_list_path:?}");
        let fs = Box::new(rafs.0);
        match self.fs.mount(fs, mountpoint) {
            Ok(idx) => {
                self.backend_fs.insert(
                    mountpoint.to_string(),
                    BackendFsInfo {
                        index: idx,
                        fstype: RAFS.to_string(),
                        src_cfg: Some((source, rafs_cfg)),
                    },
                );
                Ok(())
            }
            Err(e) => {
                error!("Rafs mount failed: {e:?}");
                Err(FsError::InvalidData)
            }
        }
    }

    fn update_rafs(
        &mut self,
        source: Option<String>,
        mountpoint: &str,
        config: Option<String>,
    ) -> FsResult<()> {
        if config.is_none() {
            return Err(FsError::BackendFs("no rafs config file".to_string()));
        }
        if source.is_none() {
            return Err(FsError::BackendFs(format!(
                "rafs mounted at {mountpoint} doesn't have source configured"
            )));
        }
        // safe because config is not None.
        let config = config.unwrap();
        let source = source.unwrap();
        let rafs_conf: Arc<ConfigV2> =
            Arc::new(serde_json::from_str(&config).map_err(|e| FsError::BackendFs(e.to_string()))?);
        // Update rafs config, update BackendFsInfo as well.
        let new_info = match self.backend_fs.get(mountpoint) {
            Some(orig_info) => BackendFsInfo {
                index: orig_info.index,
                fstype: orig_info.fstype.clone(),
                src_cfg: Some((source.to_string(), config)),
            },
            None => {
                return Err(FsError::BackendFs(format!(
                    "rafs mount point {mountpoint} is not mounted"
                )));
            }
        };
        let rootfs = match self.fs.get_rootfs(mountpoint) {
            Ok(fs) => match fs {
                Some(f) => f,
                None => {
                    return Err(FsError::BackendFs(format!(
                        "rafs get_rootfs() failed: mountpoint {mountpoint} not mounted"
                    )));
                }
            },
            Err(e) => {
                return Err(FsError::BackendFs(format!(
                    "rafs get_rootfs() failed: {e:?}"
                )));
            }
        };
        let any_fs = rootfs.0.deref().as_any();
        if let Some(fs_swap) = any_fs.downcast_ref::<Rafs>() {
            let mut file = <dyn RafsIoRead>::from_file(&source)
                .map_err(|e| FsError::BackendFs(format!("RafsIoRead failed: {e:?}")))?;

            fs_swap
                .update(&mut file, &rafs_conf)
                .map_err(|e| FsError::BackendFs(format!("Update rafs failed: {e:?}")))?;
            self.backend_fs.insert(mountpoint.to_string(), new_info);
            Ok(())
        } else {
            Err(FsError::BackendFs("no rafs is found".to_string()))
        }
    }

    fn register_mmap_region(
        &mut self,
        vm_fd: Arc<VmFd>,
        guest_addr: u64,
        len: u64,
        slot_res: &[u32],
    ) -> Result<Arc<GuestRegionMmap>> {
        // Create file backend for virtiofs's mmap region to let goku and
        // vhost-user slave can remap memory by memfd. However, this is not a
        // complete solution, because when dax is actually on, they need to be
        // notified of the change in the dax memory mapping relationship.
        let file_offset = {
            let fd = memfd::memfd_create(
                // safe to unwrap, no nul byte in file name
                CString::new("virtio_fs_mem").unwrap().as_c_str(),
                memfd::MFdFlags::empty(),
            )
            .map_err(|e| Error::VirtioFs(FsError::MemFdCreate(e)))?;
            let file: File = File::from(fd);
            file.set_len(len)
                .map_err(|e| Error::VirtioFs(FsError::SetFileSize(e)))?;
            Some(FileOffset::new(file, 0))
        };

        // unmap will be handled on MmapRegion'd Drop.
        let mmap_region = MmapRegion::build(
            file_offset,
            len as usize,
            libc::PROT_NONE,
            libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE,
        )
        .map_err(Error::NewMmapRegion)?;

        let host_addr: u64 = mmap_region.as_ptr() as u64;
        let kvm_mem_region = kvm_userspace_memory_region {
            slot: slot_res[0],
            flags: 0,
            guest_phys_addr: guest_addr,
            memory_size: len,
            userspace_addr: host_addr,
        };
        debug!(
            "{}: mmio shared memory kvm_region: {:?}",
            self.id, kvm_mem_region,
        );

        // Safe because the user mem region is just created, and kvm slot is allocated
        // by resource allocator.
        unsafe {
            vm_fd
                .set_user_memory_region(kvm_mem_region)
                .map_err(Error::SetUserMemoryRegion)?
        };

        let region = Arc::new(
            GuestRegionMmap::new(mmap_region, GuestAddress(guest_addr)).ok_or(Error::InsertMmap)?,
        );
        self.handler.insert_region(region.clone())?;

        Ok(region)
    }
}

impl<AS: GuestAddressSpace> Drop for VirtioFs<AS> {
    fn drop(&mut self) {
        for backend in self.oss_backends.values_mut() {
            stop_oss_flush_worker(&mut backend.worker);
        }
    }
}

fn parse_prefetch_files(prefetch_list_path: Option<String>) -> Option<Vec<PathBuf>> {
    let prefetch_files: Option<Vec<PathBuf>> = match prefetch_list_path {
        Some(p) => {
            match File::open(p.as_str()) {
                Ok(f) => {
                    let r = BufReader::new(f);
                    // All prefetch files should be absolute path
                    let v: Vec<PathBuf> = r
                        .lines()
                        .filter(|l| {
                            let lref = l.as_ref();
                            lref.is_ok() && lref.unwrap().starts_with('/')
                        })
                        .map(|l| PathBuf::from(l.unwrap().as_str()))
                        .collect();
                    if v.is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                }
                Err(e) => {
                    // We could contineu without prefetch files, just print warning and return
                    warn!(
                        "{}: Open prefetch_file_path {} failed: {:?}",
                        VIRTIO_FS_NAME,
                        p.as_str(),
                        e
                    );
                    None
                }
            }
        }
        None => None,
    };
    prefetch_files
}

fn normalize_oss_prefix(source: &str, object_prefix: Option<&str>) -> String {
    fn normalize_prefix_part(part: &str) -> Option<String> {
        let trimmed = part.trim().trim_matches('/');
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    let mut parts = Vec::new();
    if let Some(prefix) = object_prefix.and_then(normalize_prefix_part) {
        parts.push(prefix);
    }
    if let Some(prefix) = normalize_prefix_part(source) {
        parts.push(prefix);
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("{}/", parts.join("/"))
    }
}

fn parse_oss_write_policy(policy: Option<&str>) -> FsResult<OssWritePolicy> {
    let Some(policy) = policy else {
        return Ok(OssWritePolicy::ReadOnly);
    };

    match policy.trim().to_ascii_lowercase().as_str() {
        "read-only" | "readonly" | "read_only" | "none" => Ok(OssWritePolicy::ReadOnly),
        "write-through" | "writethrough" | "write_through" | "sync" => {
            Ok(OssWritePolicy::WriteThrough)
        }
        "async-flush" | "async_flush" | "async" => Ok(OssWritePolicy::AsyncFlush),
        invalid => Err(FsError::BackendFs(format!(
            "invalid ossfs write policy {invalid}, expected read-only/write-through/async-flush"
        ))),
    }
}

fn parse_oss_consistency_strategy(strategy: Option<&str>) -> FsResult<OssConsistencyStrategy> {
    let Some(strategy) = strategy else {
        return Ok(OssConsistencyStrategy::LocalWins);
    };

    match strategy.trim().to_ascii_lowercase().as_str() {
        "local-wins" | "local_wins" | "last-write-wins" | "last_write_wins" => {
            Ok(OssConsistencyStrategy::LocalWins)
        }
        "etag-guard" | "etag_guard" | "if-match" | "if_match" => {
            Ok(OssConsistencyStrategy::EtagGuard)
        }
        "remote-wins" | "remote_wins" => Ok(OssConsistencyStrategy::RemoteWins),
        invalid => Err(FsError::BackendFs(format!(
            "invalid ossfs consistency strategy {invalid}, expected local-wins/etag-guard/remote-wins"
        ))),
    }
}

fn resolve_oss_flush_interval(
    policy: OssWritePolicy,
    interval_secs: Option<u64>,
) -> FsResult<Duration> {
    let default_interval = match policy {
        OssWritePolicy::ReadOnly => 0,
        OssWritePolicy::WriteThrough => OSS_WRITE_THROUGH_INTERVAL_SECS,
        OssWritePolicy::AsyncFlush => OSS_ASYNC_FLUSH_INTERVAL_SECS,
    };

    let secs = interval_secs.unwrap_or(default_interval);
    if matches!(policy, OssWritePolicy::ReadOnly) {
        return Ok(Duration::from_secs(0));
    }
    if secs == 0 {
        return Err(FsError::BackendFs(
            "ossfs flush_interval_secs must be greater than 0 for write policy".to_string(),
        ));
    }

    Ok(Duration::from_secs(secs))
}

fn resolve_oss_upload_chunk_size(chunk_size: Option<usize>) -> FsResult<usize> {
    let chunk_size = chunk_size.unwrap_or(OSS_DEFAULT_UPLOAD_CHUNK_SIZE);
    if chunk_size == 0 {
        return Err(FsError::BackendFs(
            "ossfs upload_chunk_size must be greater than 0".to_string(),
        ));
    }

    Ok(chunk_size)
}

fn build_oss_operator(cfg: &OssFsConfig) -> FsResult<OpenDalOperator> {
    let mut builder = OssService::default()
        .bucket(&cfg.bucket)
        .endpoint(&cfg.endpoint)
        .access_key_id(&cfg.access_key_id)
        .access_key_secret(&cfg.access_key_secret);
    if let Some(token) = cfg.security_token.as_deref() {
        builder = builder.security_token(token);
    }
    if let Some(addressing_style) = cfg.addressing_style.as_deref() {
        builder = builder.addressing_style(addressing_style);
    }

    OpenDalOperator::new(builder)
        .map_err(|e| FsError::BackendFs(format!("failed to initialize oss operator: {e}")))
}

fn start_oss_flush_worker(
    sync_ctx: OssSyncContext,
    flush_interval: Duration,
    mountpoint: String,
) -> FsResult<OssFlushWorker> {
    let (stop_tx, stop_rx) = mpsc::channel();
    let worker_name = format!("virtiofs-oss-flush-{}", mountpoint.replace('/', "_"));
    let handle = std::thread::Builder::new()
        .name(worker_name)
        .spawn(move || loop {
            match stop_rx.recv_timeout(flush_interval) {
                Ok(_) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Err(e) = sync_local_cache_to_oss(&sync_ctx) {
                        warn!(
                            "{VIRTIO_FS_NAME}: ossfs background flush failed at {}: {:?}",
                            sync_ctx.mountpoint, e
                        );
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        })
        .map_err(|e| FsError::BackendFs(format!("failed to start ossfs flush worker: {e}")))?;

    Ok(OssFlushWorker { stop_tx, handle })
}

fn stop_oss_flush_worker(worker: &mut Option<OssFlushWorker>) {
    let Some(worker) = worker.take() else {
        return;
    };

    let _ = worker.stop_tx.send(());
    if let Err(e) = worker.handle.join() {
        warn!("{VIRTIO_FS_NAME}: ossfs flush worker join failed: {:?}", e);
    }
}

fn oss_cache_namespace(bucket: &str, prefix: &str) -> String {
    let mut raw = format!("{bucket}_{prefix}");
    if raw.is_empty() {
        raw.push_str("default");
    }

    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn oss_object_relative_path(object_path: &str, prefix: &str) -> FsResult<PathBuf> {
    let normalized_object_path = object_path.trim_start_matches('/');
    let normalized_prefix = prefix.trim_start_matches('/');

    let relative = if normalized_prefix.is_empty() {
        normalized_object_path
    } else {
        normalized_object_path
            .strip_prefix(normalized_prefix)
            .ok_or_else(|| {
                FsError::BackendFs(format!(
                    "oss object path {object_path} is not under configured prefix {prefix}"
                ))
            })?
    };
    let relative = relative.trim_start_matches('/');
    if relative.is_empty() {
        return Err(FsError::BackendFs(format!(
            "oss object path {object_path} resolves to an empty local path"
        )));
    }

    let mut local_path = PathBuf::new();
    for part in Path::new(relative).components() {
        match part {
            Component::Normal(name) => local_path.push(name),
            Component::CurDir => {}
            _ => {
                return Err(FsError::BackendFs(format!(
                    "oss object path {object_path} contains invalid path components"
                )));
            }
        }
    }

    if local_path.as_os_str().is_empty() {
        return Err(FsError::BackendFs(format!(
            "oss object path {object_path} resolves to an invalid local path"
        )));
    }

    Ok(local_path)
}

fn local_relative_path_to_object_suffix(relative_path: &Path) -> FsResult<String> {
    let mut parts = Vec::new();
    for part in relative_path.components() {
        match part {
            Component::Normal(name) => parts.push(name.to_string_lossy().to_string()),
            Component::CurDir => {}
            _ => {
                return Err(FsError::BackendFs(format!(
                    "local cache path {} contains invalid path components",
                    relative_path.display()
                )));
            }
        }
    }

    if parts.is_empty() {
        return Err(FsError::BackendFs(format!(
            "local cache path {} resolves to an empty object key",
            relative_path.display()
        )));
    }

    Ok(parts.join("/"))
}

fn oss_object_path_from_local_suffix(prefix: &str, local_suffix: &str) -> String {
    if prefix.is_empty() {
        local_suffix.to_string()
    } else {
        format!("{prefix}{local_suffix}")
    }
}

fn local_cache_path_for_object(sync_ctx: &OssSyncContext, object_path: &str) -> FsResult<PathBuf> {
    let relative_path = oss_object_relative_path(object_path, &sync_ctx.prefix)?;
    Ok(sync_ctx.cache_data_dir.join(relative_path))
}

fn local_cache_state(path: &Path) -> FsResult<LocalCacheState> {
    let metadata = fs::metadata(path).map_err(FsError::IOError)?;
    let mtime_ns = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    Ok(LocalCacheState {
        size: metadata.len(),
        mtime_ns,
    })
}

fn entry_has_local_state(entry: &OssCacheEntry) -> bool {
    entry.local_size != 0 || entry.local_mtime_ns != 0
}

fn is_local_state_dirty(old: &OssCacheEntry, state: LocalCacheState) -> bool {
    if !entry_has_local_state(old) {
        return false;
    }

    old.local_size != state.size || old.local_mtime_ns != state.mtime_ns
}

fn is_oss_cache_entry_fresh(
    old: &OssCacheEntry,
    remote_etag: Option<&str>,
    remote_content_length: u64,
) -> bool {
    if let (Some(old_etag), Some(remote_etag)) = (old.etag.as_deref(), remote_etag) {
        return old_etag == remote_etag;
    }

    old.content_length == remote_content_length
}

fn load_oss_cache_index(index_path: &Path) -> OssCacheIndex {
    let content = match fs::read_to_string(index_path) {
        Ok(content) => content,
        Err(e) if e.kind() == ErrorKind::NotFound => return OssCacheIndex::default(),
        Err(e) => {
            warn!(
                "{VIRTIO_FS_NAME}: failed to read ossfs cache index {}: {:?}",
                index_path.display(),
                e
            );
            return OssCacheIndex::default();
        }
    };

    serde_json::from_str(&content).unwrap_or_else(|e| {
        warn!(
            "{VIRTIO_FS_NAME}: failed to parse ossfs cache index {}: {:?}",
            index_path.display(),
            e
        );
        OssCacheIndex::default()
    })
}

fn persist_oss_cache_index(index_path: &Path, index: &OssCacheIndex) -> FsResult<()> {
    let encoded = serde_json::to_vec(index).map_err(|e| {
        FsError::BackendFs(format!(
            "failed to serialize ossfs cache index {}: {e}",
            index_path.display()
        ))
    })?;

    let parent = index_path.parent().ok_or_else(|| {
        FsError::BackendFs(format!(
            "failed to get parent directory for index {}",
            index_path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(FsError::IOError)?;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = index_path
        .file_name()
        .ok_or_else(|| {
            FsError::BackendFs(format!(
                "failed to get file name for index {}",
                index_path.display()
            ))
        })?
        .to_string_lossy();
    let tmp_name = format!(".{file_name}.tmp-{nanos}");
    let tmp_path = parent.join(tmp_name);

    {
        let mut tmp_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(FsError::IOError)?;
        tmp_file.write_all(&encoded).map_err(FsError::IOError)?;
        tmp_file.sync_all().map_err(FsError::IOError)?;
    }

    fs::rename(&tmp_path, index_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        FsError::IOError(e)
    })?;

    let dir_file = File::open(parent).map_err(FsError::IOError)?;
    dir_file.sync_all().map_err(FsError::IOError)
}

fn prune_empty_parent_dirs(path: &Path, root: &Path) {
    let mut current = path.parent();
    while let Some(parent) = current {
        if !parent.starts_with(root) || parent == root {
            break;
        }

        match fs::remove_dir(parent) {
            Ok(_) => current = parent.parent(),
            Err(e) if e.kind() == ErrorKind::DirectoryNotEmpty => break,
            Err(_) => break,
        }
    }
}

fn collect_local_cache_files_recursive(
    base_dir: &Path,
    current_dir: &Path,
    prefix: &str,
    files: &mut HashMap<String, LocalCacheFile>,
) -> FsResult<()> {
    for entry in fs::read_dir(current_dir).map_err(FsError::IOError)? {
        let entry = entry.map_err(FsError::IOError)?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(FsError::IOError)?;
        if file_type.is_dir() {
            collect_local_cache_files_recursive(base_dir, &path, prefix, files)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative_path = path.strip_prefix(base_dir).map_err(|e| {
            FsError::BackendFs(format!(
                "failed to strip local cache prefix for {}: {e}",
                path.display()
            ))
        })?;
        let local_suffix = local_relative_path_to_object_suffix(relative_path)?;
        let object_path = oss_object_path_from_local_suffix(prefix, &local_suffix);
        let state = local_cache_state(&path)?;
        files.insert(object_path, LocalCacheFile { path, state });
    }

    Ok(())
}

fn collect_local_cache_files(
    cache_data_dir: &Path,
    prefix: &str,
) -> FsResult<HashMap<String, LocalCacheFile>> {
    let mut files = HashMap::new();
    if !cache_data_dir.exists() {
        return Ok(files);
    }
    collect_local_cache_files_recursive(cache_data_dir, cache_data_dir, prefix, &mut files)?;
    Ok(files)
}

fn query_remote_object_state(
    op: &OpenDalOperator,
    runtime: &tokio::runtime::Runtime,
    object_path: &str,
) -> FsResult<RemoteObjectState> {
    match runtime.block_on(op.stat(object_path)) {
        Ok(meta) => Ok(RemoteObjectState::Existing {
            etag: meta.etag().map(ToString::to_string),
            content_length: meta.content_length(),
        }),
        Err(e) if e.kind() == OpenDalErrorKind::NotFound => Ok(RemoteObjectState::NotFound),
        Err(e) => Err(FsError::BackendFs(format!(
            "failed to stat remote oss object {object_path}: {e}"
        ))),
    }
}

fn is_remote_state_conflicted(old: &OssCacheEntry, remote_state: &RemoteObjectState) -> bool {
    match remote_state {
        RemoteObjectState::NotFound => old.etag.is_some() || old.content_length > 0,
        RemoteObjectState::Existing {
            etag,
            content_length,
        } => {
            if let (Some(old_etag), Some(remote_etag)) = (old.etag.as_deref(), etag.as_deref()) {
                return old_etag != remote_etag;
            }

            old.content_length != *content_length
        }
    }
}

fn is_remote_entry_conflicted(
    old: &OssCacheEntry,
    remote_state: &RemoteObjectState,
    strategy: OssConsistencyStrategy,
) -> FsResult<bool> {
    if strategy == OssConsistencyStrategy::LocalWins {
        return Ok(false);
    }

    Ok(is_remote_state_conflicted(old, remote_state))
}

fn isolate_conflict_local_file(
    sync_ctx: &OssSyncContext,
    object_path: &str,
    local_path: &Path,
) -> FsResult<PathBuf> {
    let relative_path = local_path.strip_prefix(&sync_ctx.cache_data_dir).map_err(|e| {
        FsError::BackendFs(format!(
            "failed to strip local cache prefix for conflict file {}: {e}",
            local_path.display()
        ))
    })?;
    let base_conflict_path = sync_ctx.cache_root.join(OSS_CONFLICTS_DIR).join(relative_path);
    if let Some(parent) = base_conflict_path.parent() {
        fs::create_dir_all(parent).map_err(FsError::IOError)?;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = base_conflict_path
        .file_name()
        .ok_or_else(|| {
            FsError::BackendFs(format!(
                "failed to get filename for conflict file {}",
                local_path.display()
            ))
        })?
        .to_string_lossy();
    let conflict_file_name = format!("{file_name}.conflict-{nanos}");
    let conflict_path = base_conflict_path.with_file_name(conflict_file_name);

    fs::rename(local_path, &conflict_path).map_err(FsError::IOError)?;
    warn!(
        "{VIRTIO_FS_NAME}: isolated conflict file for {object_path} at {}",
        conflict_path.display()
    );
    Ok(conflict_path)
}

fn download_remote_object_to_local(
    op: &OpenDalOperator,
    runtime: &tokio::runtime::Runtime,
    sync_ctx: &OssSyncContext,
    object_path: &str,
    local_path: &Path,
    remote_state: &RemoteObjectState,
) -> FsResult<OssCacheEntry> {
    let data = runtime
        .block_on(op.read(object_path))
        .map_err(|e| FsError::BackendFs(format!("failed to read remote object {object_path}: {e}")))?;
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent).map_err(FsError::IOError)?;
    }
    let tmp_path = local_path.with_extension("oss.remote.tmp");
    fs::write(&tmp_path, data.to_vec()).map_err(FsError::IOError)?;
    fs::rename(&tmp_path, local_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        FsError::IOError(e)
    })?;
    let local_state = local_cache_state(local_path)?;
    let (etag, content_length) = match remote_state {
        RemoteObjectState::Existing {
            etag,
            content_length,
        } => (etag.clone(), *content_length),
        RemoteObjectState::NotFound => {
            return Err(FsError::BackendFs(format!(
                "remote object {object_path} disappeared while applying remote-wins at {}",
                sync_ctx.mountpoint
            )))
        }
    };

    Ok(OssCacheEntry {
        etag,
        content_length,
        local_size: local_state.size,
        local_mtime_ns: local_state.mtime_ns,
    })
}

fn classify_conditional_oss_error(e: &opendal::Error) -> Option<OssConditionalOpOutcome> {
    match e.kind() {
        OpenDalErrorKind::ConditionNotMatch
        | OpenDalErrorKind::Conflict
        | OpenDalErrorKind::NotFound => Some(OssConditionalOpOutcome::Conflict),
        OpenDalErrorKind::Unsupported => Some(OssConditionalOpOutcome::Unsupported),
        _ => None,
    }
}

fn upload_local_file_chunked(
    op: &OpenDalOperator,
    runtime: &tokio::runtime::Runtime,
    object_path: &str,
    local_path: &Path,
    chunk_size: usize,
) -> FsResult<()> {
    let mut file = File::open(local_path).map_err(FsError::IOError)?;
    let write_opts = WriteOptions {
        chunk: Some(chunk_size),
        ..Default::default()
    };
    let mut writer = runtime
        .block_on(op.writer_options(object_path, write_opts))
        .map_err(|e| FsError::BackendFs(format!("failed to open oss writer for {object_path}: {e}")))?;
    let mut buf = vec![0u8; chunk_size];
    loop {
        let bytes_read = file.read(&mut buf).map_err(FsError::IOError)?;
        if bytes_read == 0 {
            break;
        }

        runtime
            .block_on(writer.write(buf[..bytes_read].to_vec()))
            .map_err(|e| FsError::BackendFs(format!("failed to write chunk for {object_path}: {e}")))?;
    }
    runtime
        .block_on(writer.close())
        .map_err(|e| FsError::BackendFs(format!("failed to close writer for {object_path}: {e}")))?;
    Ok(())
}

fn upload_local_file_chunked_with_cas(
    op: &OpenDalOperator,
    runtime: &tokio::runtime::Runtime,
    object_path: &str,
    local_path: &Path,
    chunk_size: usize,
    expected_etag: Option<&str>,
    if_not_exists: bool,
) -> FsResult<OssConditionalOpOutcome> {
    let mut write_opts = WriteOptions {
        chunk: Some(chunk_size),
        ..Default::default()
    };
    if let Some(etag) = expected_etag {
        write_opts.if_match = Some(etag.to_string());
    }
    if if_not_exists {
        write_opts.if_not_exists = true;
    }

    let mut file = File::open(local_path).map_err(FsError::IOError)?;
    let mut writer = match runtime.block_on(op.writer_options(object_path, write_opts)) {
        Ok(writer) => writer,
        Err(e) => {
            if let Some(outcome) = classify_conditional_oss_error(&e) {
                return Ok(outcome);
            }
            return Err(FsError::BackendFs(format!(
                "failed to open conditional oss writer for {object_path}: {e}"
            )));
        }
    };

    let mut buf = vec![0u8; chunk_size];
    loop {
        let bytes_read = file.read(&mut buf).map_err(FsError::IOError)?;
        if bytes_read == 0 {
            break;
        }
        if let Err(e) = runtime.block_on(writer.write(buf[..bytes_read].to_vec())) {
            if let Some(outcome) = classify_conditional_oss_error(&e) {
                return Ok(outcome);
            }
            return Err(FsError::BackendFs(format!(
                "failed to write conditional chunk for {object_path}: {e}"
            )));
        }
    }

    if let Err(e) = runtime.block_on(writer.close()) {
        if let Some(outcome) = classify_conditional_oss_error(&e) {
            return Ok(outcome);
        }
        return Err(FsError::BackendFs(format!(
            "failed to close conditional writer for {object_path}: {e}"
        )));
    }

    Ok(OssConditionalOpOutcome::Applied)
}

fn delete_remote_object_with_cas(
    op: &OpenDalOperator,
    runtime: &tokio::runtime::Runtime,
    object_path: &str,
    expected_etag: &str,
) -> FsResult<OssConditionalOpOutcome> {
    let delete_opts = DeleteOptions {
        if_match: Some(expected_etag.to_string()),
        ..Default::default()
    };

    match runtime.block_on(op.delete_options(object_path, delete_opts)) {
        Ok(_) => Ok(OssConditionalOpOutcome::Applied),
        Err(e) => {
            if let Some(outcome) = classify_conditional_oss_error(&e) {
                return Ok(outcome);
            }
            Err(FsError::BackendFs(format!(
                "failed to conditionally delete remote object {object_path}: {e}"
            )))
        }
    }
}

fn sync_oss_objects_to_cache(sync_ctx: &OssSyncContext) -> FsResult<OssDownloadSyncStats> {
    let op = build_oss_operator(&sync_ctx.config)?;
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| FsError::BackendFs(format!("failed to initialize tokio runtime: {e}")))?;

    let list_path = if sync_ctx.prefix.is_empty() {
        "/"
    } else {
        sync_ctx.prefix.as_str()
    };
    let entries = runtime
        .block_on(op.list_options(
            list_path,
            ListOptions {
                recursive: true,
                ..Default::default()
            },
        ))
        .map_err(|e| {
            FsError::BackendFs(format!("failed to list oss objects at {list_path}: {e}"))
        })?;

    let index_path = sync_ctx.cache_root.join(OSS_CACHE_INDEX_FILE);
    let old_index = load_oss_cache_index(&index_path);
    let mut new_index = OssCacheIndex::default();
    let mut stats = OssDownloadSyncStats::default();
    let mut synced_paths = HashSet::new();

    for entry in entries {
        if !entry.metadata().mode().is_file() {
            continue;
        }

        let object_path = entry.path().to_string();
        let relative_path = oss_object_relative_path(&object_path, &sync_ctx.prefix)?;
        let local_path = sync_ctx.cache_data_dir.join(&relative_path);
        let metadata = runtime.block_on(op.stat(&object_path)).map_err(|e| {
            FsError::BackendFs(format!("failed to stat oss object {object_path}: {e}"))
        })?;
        let content_length = metadata.content_length();
        let etag = metadata.etag().map(ToString::to_string);
        let old_entry = old_index.entries.get(&object_path);

        let can_reuse = local_path.exists()
            && old_entry
                .map(|old| is_oss_cache_entry_fresh(old, etag.as_deref(), content_length))
                .unwrap_or(false);

        if can_reuse {
            stats.reused += 1;
        } else {
            let data = runtime.block_on(op.read(&object_path)).map_err(|e| {
                FsError::BackendFs(format!("failed to read oss object {object_path}: {e}"))
            })?;
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent).map_err(FsError::IOError)?;
            }

            let tmp_path = local_path.with_extension("oss.tmp");
            fs::write(&tmp_path, data.to_vec()).map_err(FsError::IOError)?;
            fs::rename(&tmp_path, &local_path).map_err(|e| {
                let _ = fs::remove_file(&tmp_path);
                FsError::IOError(e)
            })?;
            stats.downloaded += 1;
        }
        let local_state = local_cache_state(&local_path)?;
        let local_dirty = old_entry
            .map(|old| is_local_state_dirty(old, local_state))
            .unwrap_or(false);

        synced_paths.insert(object_path.clone());
        if can_reuse && local_dirty {
            if let Some(old) = old_entry {
                new_index.entries.insert(object_path, old.clone());
            }
        } else {
            new_index.entries.insert(
                object_path,
                OssCacheEntry {
                    etag,
                    content_length,
                    local_size: local_state.size,
                    local_mtime_ns: local_state.mtime_ns,
                },
            );
        }
    }

    for (old_path, old_entry) in &old_index.entries {
        if synced_paths.contains(old_path) {
            continue;
        }

        let relative_path = match oss_object_relative_path(old_path, &sync_ctx.prefix) {
            Ok(path) => path,
            Err(e) => {
                warn!("{VIRTIO_FS_NAME}: skip removing stale cache for {old_path}: {e:?}");
                continue;
            }
        };
        let local_path = sync_ctx.cache_data_dir.join(relative_path);
        if local_path.exists() {
            let local_state = local_cache_state(&local_path)?;
            if is_local_state_dirty(old_entry, local_state) {
                new_index
                    .entries
                    .insert(old_path.clone(), old_entry.clone());
                continue;
            }
        }

        match fs::remove_file(&local_path) {
            Ok(_) => {
                stats.removed += 1;
                prune_empty_parent_dirs(&local_path, &sync_ctx.cache_data_dir);
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => {
                warn!(
                    "{VIRTIO_FS_NAME}: failed to remove stale cache file {}: {:?}",
                    local_path.display(),
                    e
                );
            }
        }
    }

    persist_oss_cache_index(&index_path, &new_index)?;
    Ok(stats)
}

fn sync_local_cache_to_oss(sync_ctx: &OssSyncContext) -> FsResult<OssWritebackSyncStats> {
    let _sync_guard = sync_ctx.sync_lock.lock().map_err(|e| {
        FsError::BackendFs(format!(
            "failed to acquire ossfs sync lock at {}: {e}",
            sync_ctx.mountpoint
        ))
    })?;
    let op = build_oss_operator(&sync_ctx.config)?;
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| FsError::BackendFs(format!("failed to initialize tokio runtime: {e}")))?;
    let index_path = sync_ctx.cache_root.join(OSS_CACHE_INDEX_FILE);
    let old_index = load_oss_cache_index(&index_path);
    let local_files = collect_local_cache_files(&sync_ctx.cache_data_dir, &sync_ctx.prefix)?;
    let mut new_index = OssCacheIndex::default();
    let mut stats = OssWritebackSyncStats::default();

    for (object_path, local_file) in &local_files {
        let old_entry = old_index.entries.get(object_path);
        let local_dirty = old_entry
            .map(|old| is_local_state_dirty(old, local_file.state))
            .unwrap_or(true);
        if !local_dirty {
            stats.unchanged += 1;
            if let Some(old) = old_entry {
                if entry_has_local_state(old) {
                    new_index.entries.insert(object_path.clone(), old.clone());
                } else {
                    new_index.entries.insert(
                        object_path.clone(),
                        OssCacheEntry {
                            etag: old.etag.clone(),
                            content_length: old.content_length,
                            local_size: local_file.state.size,
                            local_mtime_ns: local_file.state.mtime_ns,
                        },
                    );
                }
            }
            continue;
        }

        if let Some(old) = old_entry {
            if sync_ctx.consistency_strategy != OssConsistencyStrategy::LocalWins && old.etag.is_none()
            {
                let remote_state = query_remote_object_state(&op, &runtime, object_path)?;
                if is_remote_entry_conflicted(old, &remote_state, sync_ctx.consistency_strategy)? {
                    if sync_ctx.consistency_strategy == OssConsistencyStrategy::RemoteWins {
                        isolate_conflict_local_file(sync_ctx, object_path, &local_file.path)?;
                        stats.isolated += 1;

                        if let RemoteObjectState::Existing { .. } = remote_state {
                            let canonical_local_path =
                                local_cache_path_for_object(sync_ctx, object_path)?;
                            let new_entry = download_remote_object_to_local(
                                &op,
                                &runtime,
                                sync_ctx,
                                object_path,
                                &canonical_local_path,
                                &remote_state,
                            )?;
                            new_index.entries.insert(object_path.clone(), new_entry);
                        }
                        stats.remote_wins += 1;
                        continue;
                    }

                    warn!(
                        "{VIRTIO_FS_NAME}: skip flushing {} at {} due to remote conflict",
                        object_path, sync_ctx.mountpoint
                    );
                    stats.skipped_conflicts += 1;
                    new_index.entries.insert(object_path.clone(), old.clone());
                    continue;
                }
            }
        }

        let mut uploaded_via_cas = false;
        let mut cas_unsupported = false;
        if sync_ctx.consistency_strategy != OssConsistencyStrategy::LocalWins {
            let cas_outcome = if let Some(old) = old_entry {
                old.etag
                    .as_deref()
                    .map(|etag| {
                        upload_local_file_chunked_with_cas(
                            &op,
                            &runtime,
                            object_path,
                            &local_file.path,
                            sync_ctx.upload_chunk_size,
                            Some(etag),
                            false,
                        )
                    })
                    .transpose()?
            } else {
                Some(upload_local_file_chunked_with_cas(
                    &op,
                    &runtime,
                    object_path,
                    &local_file.path,
                    sync_ctx.upload_chunk_size,
                    None,
                    true,
                )?)
            };

            if let Some(cas_outcome) = cas_outcome {
                match cas_outcome {
                    OssConditionalOpOutcome::Applied => {
                        uploaded_via_cas = true;
                    }
                    OssConditionalOpOutcome::Unsupported => {
                        warn!(
                            "{VIRTIO_FS_NAME}: ossfs conditional upload unsupported for {}, fallback to non-conditional write at {}",
                            object_path, sync_ctx.mountpoint
                        );
                        cas_unsupported = true;
                    }
                    OssConditionalOpOutcome::Conflict => {
                        let remote_state = query_remote_object_state(&op, &runtime, object_path)?;
                        if sync_ctx.consistency_strategy == OssConsistencyStrategy::RemoteWins {
                            isolate_conflict_local_file(sync_ctx, object_path, &local_file.path)?;
                            stats.isolated += 1;
                            if let RemoteObjectState::Existing { .. } = remote_state {
                                let canonical_local_path =
                                    local_cache_path_for_object(sync_ctx, object_path)?;
                                let new_entry = download_remote_object_to_local(
                                    &op,
                                    &runtime,
                                    sync_ctx,
                                    object_path,
                                    &canonical_local_path,
                                    &remote_state,
                                )?;
                                new_index.entries.insert(object_path.clone(), new_entry);
                            }
                            stats.remote_wins += 1;
                        } else {
                            warn!(
                                "{VIRTIO_FS_NAME}: skip flushing {} at {} due to CAS conflict",
                                object_path, sync_ctx.mountpoint
                            );
                            stats.skipped_conflicts += 1;
                            if let Some(old) = old_entry {
                                new_index.entries.insert(object_path.clone(), old.clone());
                            }
                        }
                        continue;
                    }
                }
            }
        }

        if cas_unsupported {
            if let Some(old) = old_entry {
                let remote_state = query_remote_object_state(&op, &runtime, object_path)?;
                if is_remote_entry_conflicted(old, &remote_state, sync_ctx.consistency_strategy)? {
                    if sync_ctx.consistency_strategy == OssConsistencyStrategy::RemoteWins {
                        isolate_conflict_local_file(sync_ctx, object_path, &local_file.path)?;
                        stats.isolated += 1;
                        if let RemoteObjectState::Existing { .. } = remote_state {
                            let canonical_local_path =
                                local_cache_path_for_object(sync_ctx, object_path)?;
                            let new_entry = download_remote_object_to_local(
                                &op,
                                &runtime,
                                sync_ctx,
                                object_path,
                                &canonical_local_path,
                                &remote_state,
                            )?;
                            new_index.entries.insert(object_path.clone(), new_entry);
                        }
                        stats.remote_wins += 1;
                    } else {
                        warn!(
                            "{VIRTIO_FS_NAME}: skip flushing {} at {} due to fallback conflict",
                            object_path, sync_ctx.mountpoint
                        );
                        stats.skipped_conflicts += 1;
                        new_index.entries.insert(object_path.clone(), old.clone());
                    }
                    continue;
                }
            }
        }

        if !uploaded_via_cas {
            upload_local_file_chunked(
                &op,
                &runtime,
                object_path,
                &local_file.path,
                sync_ctx.upload_chunk_size,
            )?;
        }

        let remote_state = query_remote_object_state(&op, &runtime, object_path)?;
        let (etag, content_length) = match remote_state {
            RemoteObjectState::Existing {
                etag,
                content_length,
            } => (etag, content_length),
            RemoteObjectState::NotFound => {
                return Err(FsError::BackendFs(format!(
                    "uploaded object {object_path} not found after write at {}",
                    sync_ctx.mountpoint
                )))
            }
        };
        new_index.entries.insert(
            object_path.clone(),
            OssCacheEntry {
                etag,
                content_length,
                local_size: local_file.state.size,
                local_mtime_ns: local_file.state.mtime_ns,
            },
        );
        stats.uploaded += 1;
    }

    for (old_path, old_entry) in &old_index.entries {
        if local_files.contains_key(old_path) {
            continue;
        }

        if sync_ctx.consistency_strategy != OssConsistencyStrategy::LocalWins {
            let mut cas_unsupported = false;
            if let Some(etag) = old_entry.etag.as_deref() {
                match delete_remote_object_with_cas(&op, &runtime, old_path, etag)? {
                    OssConditionalOpOutcome::Applied => {
                        stats.deleted += 1;
                        continue;
                    }
                    OssConditionalOpOutcome::Unsupported => {
                        warn!(
                            "{VIRTIO_FS_NAME}: ossfs conditional delete unsupported for {}, fallback to stat+delete at {}",
                            old_path, sync_ctx.mountpoint
                        );
                        cas_unsupported = true;
                    }
                    OssConditionalOpOutcome::Conflict => {
                        let remote_state = query_remote_object_state(&op, &runtime, old_path)?;
                        if sync_ctx.consistency_strategy == OssConsistencyStrategy::RemoteWins {
                            if let RemoteObjectState::Existing { .. } = remote_state {
                                let local_path = local_cache_path_for_object(sync_ctx, old_path)?;
                                let new_entry = download_remote_object_to_local(
                                    &op,
                                    &runtime,
                                    sync_ctx,
                                    old_path,
                                    &local_path,
                                    &remote_state,
                                )?;
                                new_index.entries.insert(old_path.clone(), new_entry);
                            }
                            stats.remote_wins += 1;
                        } else {
                            warn!(
                                "{VIRTIO_FS_NAME}: skip deleting remote {} at {} due to CAS conflict",
                                old_path, sync_ctx.mountpoint
                            );
                            stats.skipped_conflicts += 1;
                            new_index
                                .entries
                                .insert(old_path.clone(), old_entry.clone());
                        }
                        continue;
                    }
                }
            }

            if cas_unsupported || old_entry.etag.is_none() {
                let remote_state = query_remote_object_state(&op, &runtime, old_path)?;
                if is_remote_entry_conflicted(old_entry, &remote_state, sync_ctx.consistency_strategy)?
                {
                    if sync_ctx.consistency_strategy == OssConsistencyStrategy::RemoteWins {
                        if let RemoteObjectState::Existing { .. } = remote_state {
                            let local_path = local_cache_path_for_object(sync_ctx, old_path)?;
                            let new_entry = download_remote_object_to_local(
                                &op,
                                &runtime,
                                sync_ctx,
                                old_path,
                                &local_path,
                                &remote_state,
                            )?;
                            new_index.entries.insert(old_path.clone(), new_entry);
                        }
                        stats.remote_wins += 1;
                        continue;
                    }

                    warn!(
                        "{VIRTIO_FS_NAME}: skip deleting remote {} at {} due to conflict",
                        old_path, sync_ctx.mountpoint
                    );
                    stats.skipped_conflicts += 1;
                    new_index
                        .entries
                        .insert(old_path.clone(), old_entry.clone());
                    continue;
                }
            }
        }

        match runtime.block_on(op.delete(old_path)) {
            Ok(_) => {
                stats.deleted += 1;
            }
            Err(e) if e.kind() == OpenDalErrorKind::NotFound => {}
            Err(e) => {
                return Err(FsError::BackendFs(format!(
                    "failed to delete remote object {old_path}: {e}"
                )))
            }
        }
    }

    persist_oss_cache_index(&index_path, &new_index)?;
    Ok(stats)
}

fn kb_to_bytes(kb: u64) -> FsResult<u64> {
    if (kb & 0xffc0_0000_0000_0000) != 0 {
        error!("dax_threshold_size_kb * 1024 overflow. dax_threshold_size_kb is 0x{kb:x}.");
        return Err(FsError::InvalidData);
    }

    let bytes = kb << 10;
    Ok(bytes)
}

fn set_default_rlimit_nofile() -> Result<()> {
    // Our default RLIMIT_NOFILE target.
    let mut max_fds: u64 = 300_000;
    // leave at least this many fds free
    let reserved_fds: u64 = 16_384;

    // Reduce max_fds below the system-wide maximum, if necessary.
    // This ensures there are fds available for other processes so we
    // don't cause resource exhaustion.
    let mut file_max = String::new();
    let mut f = File::open("/proc/sys/fs/file-max").map_err(|e| {
        error!("{VIRTIO_FS_NAME}: failed to read /proc/sys/fs/file-max {e:?}");
        Error::IOError(e)
    })?;
    f.read_to_string(&mut file_max)?;
    let file_max = file_max.trim().parse::<u64>().map_err(|e| {
        error!("{VIRTIO_FS_NAME}: read fs.file-max sysctl wrong {e:?}");
        Error::InvalidInput
    })?;
    if file_max < 2 * reserved_fds {
        error!(
            "{}: The fs.file-max sysctl ({}) is too low to allow a reasonable number of open files ({}).",
            VIRTIO_FS_NAME, file_max, 2 * reserved_fds
        );
        return Err(Error::InvalidInput);
    }

    max_fds = std::cmp::min(file_max - reserved_fds, max_fds);
    let rlimit_nofile = Resource::NOFILE
        .get()
        .map(|(curr, _)| if curr >= max_fds { 0 } else { max_fds })
        .map_err(|e| {
            error!("{VIRTIO_FS_NAME}: failed to get rlimit {e:?}");
            Error::IOError(e)
        })?;

    if rlimit_nofile == 0 {
        info!(
            "{VIRTIO_FS_NAME}: original rlimit nofile is greater than max_fds({max_fds}), keep rlimit nofile setting"
        );
        Ok(())
    } else {
        info!("{VIRTIO_FS_NAME}: set rlimit {rlimit_nofile} (max_fds {max_fds})");

        Resource::NOFILE
            .set(rlimit_nofile, rlimit_nofile)
            .map_err(|e| {
                error!("{VIRTIO_FS_NAME}: failed to set rlimit {e:?}");
                Error::IOError(e)
            })
    }
}

impl<'a, AS: GuestAddressSpace> crate::persist::VirtioDevicePersist<'a> for VirtioFs<AS> {
    type State = crate::persist::VirtioDeviceInfoState;
    type SaveArgs = ();
    type RestoreArgs = ();
    type Error = crate::Error;

    /// Capture the guest-negotiated state of this device.
    ///
    /// Backend filesystem state (open handles, inodes, DAX mappings) is not
    /// captured: a snapshot must be taken at a clean quiesce point.
    fn save_state(&mut self, _args: ()) -> crate::Result<Self::State> {
        Ok(self.device_info.save_state())
    }

    /// Restore the guest-negotiated state of this device.
    ///
    /// The device must have been re-created with the same configuration and
    /// must not have been activated yet.
    fn restore_state(&mut self, state: &Self::State, _args: ()) -> crate::Result<()> {
        self.device_info.restore_state(state)
    }
}

impl<AS, Q> VirtioDevice<AS, Q, GuestRegionMmap> for VirtioFs<AS>
where
    AS: 'static + GuestAddressSpace + Clone + Send + Sync,
    AS::T: Send,
    AS::M: Sync + Send,
    Q: QueueT + Send + 'static,
{
    fn device_type(&self) -> u32 {
        TYPE_VIRTIO_FS
    }

    fn queue_max_sizes(&self) -> &[u16] {
        &self.queue_sizes
    }

    fn get_avail_features(&self, page: u32) -> u32 {
        self.device_info.get_avail_features(page)
    }

    fn set_acked_features(&mut self, page: u32, value: u32) {
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: VirtioDevice::set_acked_features({}, 0x{:x})",
            self.id,
            page,
            value
        );
        self.device_info.set_acked_features(page, value)
    }

    fn read_config(&mut self, offset: u64, data: &mut [u8]) -> ConfigResult {
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: VirtioDevice::read_config(0x{:x}, {:?})",
            self.id,
            offset,
            data
        );
        self.device_info.read_config(offset, data)
    }

    fn write_config(&mut self, offset: u64, data: &[u8]) -> ConfigResult {
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: VirtioDevice::write_config(0x{:x}, {:?})",
            self.id,
            offset,
            data
        );
        self.device_info.write_config(offset, data)
    }

    fn activate(&mut self, config: VirtioDeviceConfig<AS, Q>) -> ActivateResult {
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: VirtioDevice::activate()",
            self.id
        );

        self.device_info.check_queue_sizes(&config.queues)?;

        let (sender, receiver) = mpsc::channel();
        self.sender = Some(sender);
        let rate_limiter = self.rate_limiter.take().unwrap_or_default();
        let patch_rate_limiter_fd = self.patch_rate_limiter_fd.try_clone().map_err(|e| {
            error!("{VIRTIO_FS_NAME}: failed to clone patch rate limiter eventfd {e:?}");
            ActivateError::InternalError
        })?;

        let cache_handler = if let Some((addr, _guest_addr)) = config.get_shm_region_addr() {
            let handler = CacheHandler {
                cache_size: self.cache_size,
                mmap_cache_addr: addr,
                id: self.id.clone(),
            };

            Some(handler)
        } else {
            None
        };

        let handler = VirtioFsEpollHandler::new(
            config,
            self.fs.clone(),
            cache_handler,
            self.thread_pool_size,
            self.id.clone(),
            rate_limiter,
            patch_rate_limiter_fd,
            Some(receiver),
        );

        self.subscriber_id = Some(self.device_info.register_event_handler(Box::new(handler)));

        Ok(())
    }

    // Please keep in synchronization with vhost/fs.rs
    fn get_resource_requirements(
        &self,
        requests: &mut Vec<ResourceConstraint>,
        use_generic_irq: bool,
    ) {
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: VirtioDevice::get_resource_requirements()",
            self.id
        );
        requests.push(ResourceConstraint::LegacyIrq { irq: None });
        if use_generic_irq {
            // Allocate one irq for device configuration change events, and one irq for each queue.
            requests.push(ResourceConstraint::GenericIrq {
                size: (self.queue_sizes.len() + 1) as u32,
            });
        }

        // Check if we have dax enabled or not, just return if no dax window requested.
        if !self.is_dax_on() {
            info!("{}: DAX window is disabled.", self.id);
            return;
        }

        // Request for DAX window. The memory needs to be 2MiB aligned in order to support
        // hugepages, and needs to be above 4G to avoid confliction with lapic/ioapic devices.
        requests.push(ResourceConstraint::MmioAddress {
            range: Some((0x1_0000_0000, u64::MAX)),
            align: 0x0020_0000,
            size: self.cache_size,
        });

        // Request for new kvm memory slot for DAX window.
        requests.push(ResourceConstraint::KvmMemSlot {
            slot: None,
            size: 1,
        });
    }

    // Please keep in synchronization with vhost/fs.rs
    fn set_resource(
        &mut self,
        vm_fd: Arc<VmFd>,
        resource: DeviceResources,
    ) -> Result<Option<VirtioSharedMemoryList<GuestRegionMmap>>> {
        trace!(
            target: VIRTIO_FS_NAME,
            "{}: VirtioDevice::set_resource()",
            self.id
        );

        let mmio_res = resource.get_mmio_address_ranges();
        let slot_res = resource.get_kvm_mem_slots();

        // Do nothing if there's no dax window requested.
        if mmio_res.is_empty() {
            return Ok(None);
        }

        // Make sure we have the correct resource as requested, and currently we only support one
        // shm region for DAX window (version table and journal are not supported yet).
        if mmio_res.len() != slot_res.len() || mmio_res.len() != 1 {
            error!(
                "{}: wrong number of mmio or kvm slot resource ({}, {})",
                self.id,
                mmio_res.len(),
                slot_res.len()
            );
            return Err(Error::InvalidResource);
        }

        let guest_addr = mmio_res[0].0;
        let cache_len = mmio_res[0].1;

        let mmap_region = self.register_mmap_region(vm_fd, guest_addr, cache_len, &slot_res)?;

        Ok(Some(VirtioSharedMemoryList {
            host_addr: mmap_region.deref().deref().as_ptr() as u64,
            guest_addr: GuestAddress(guest_addr),
            len: cache_len as GuestUsize,
            kvm_userspace_memory_region_flags: 0,
            kvm_userspace_memory_region_slot: slot_res[0],
            region_list: vec![VirtioSharedMemory {
                offset: 0,
                len: cache_len,
            }],
            mmap_region,
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
pub mod tests {
    #[cfg(feature = "test-resources")]
    use std::env::temp_dir;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;
    use test_utils::skip_if_kvm_unaccessable;

    use dbs_device::resources::DeviceResources;
    use dbs_interrupt::NoopNotifier;
    use kvm_ioctls::Kvm;
    use virtio_queue::QueueSync;
    use vm_memory::GuestMemoryRegion;
    use vm_memory::{GuestAddress, GuestMemoryMmap, GuestRegionMmap};
    use vmm_sys_util::tempfile::TempFile;
    use Error as VirtioError;

    use super::*;
    use crate::device::VirtioRegionHandler;
    use crate::tests::create_address_space;
    use crate::{ActivateError, VirtioQueueConfig, TYPE_VIRTIO_FS};

    pub(crate) const TAG: &str = "test";
    pub(crate) const NUM_QUEUES: usize = 1;
    pub(crate) const QUEUE_SIZE: u16 = 1024;
    pub(crate) const CACHE_SIZE: u64 = 0;
    pub(crate) const THREAD_NUM: u16 = 10;
    pub(crate) const CACHE_POLICY: &str = "auto";
    pub(crate) const WB_CACHE: bool = true;
    pub(crate) const NO_OPEN: bool = true;
    pub(crate) const NO_READDIR: bool = false;
    pub(crate) const KILLPRIV_V2: bool = false;
    pub(crate) const XATTR: bool = false;
    pub(crate) const DROP_SYS_RSC: bool = false;
    pub(crate) const FS_EVENTS_COUNT: u32 = 4;

    pub struct DummyVirtioRegionHandler {}

    impl VirtioRegionHandler for DummyVirtioRegionHandler {
        fn insert_region(
            &mut self,
            _region: Arc<GuestRegionMmap>,
        ) -> std::result::Result<(), VirtioError> {
            Ok(())
        }
    }

    pub fn new_dummy_handler_helper() -> Box<dyn VirtioRegionHandler> {
        Box::new(DummyVirtioRegionHandler {})
    }

    #[cfg(feature = "test-resources")]
    fn create_fs_device_default() -> VirtioFs<Arc<GuestMemoryMmap>> {
        let epoll_manager = EpollManager::default();
        let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
        let fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
            TAG,
            NUM_QUEUES,
            QUEUE_SIZE,
            CACHE_SIZE,
            CACHE_POLICY,
            THREAD_NUM,
            WB_CACHE,
            NO_OPEN,
            KILLPRIV_V2,
            XATTR,
            DROP_SYS_RSC,
            NO_READDIR,
            new_dummy_handler_helper(),
            epoll_manager,
            Some(rate_limiter),
            false,
        )
        .unwrap();

        fs
    }

    pub(crate) fn create_fs_epoll_handler(
        id: String,
    ) -> VirtioFsEpollHandler<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap> {
        let vfs = Arc::new(Vfs::new(VfsOptions::default()));
        let mem = Arc::new(GuestMemoryMmap::from_ranges(&[(GuestAddress(0x0), 0x10000)]).unwrap());
        let queues = vec![
            VirtioQueueConfig::create(256, 0).unwrap(),
            VirtioQueueConfig::create(256, 0).unwrap(),
        ];
        let rate_limiter = RateLimiter::default();

        // Call for kvm too frequently would cause error in some host kernel.
        std::thread::sleep(std::time::Duration::from_millis(5));

        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let resources = DeviceResources::new();
        let address_space = create_address_space();
        let config = VirtioDeviceConfig::new(
            mem,
            address_space,
            vm_fd,
            resources,
            queues,
            None,
            Arc::new(NoopNotifier::new()),
        );
        VirtioFsEpollHandler::new(
            config,
            vfs,
            None,
            2,
            id,
            rate_limiter,
            EventFd::new(0).unwrap(),
            None,
        )
    }

    #[test]
    fn test_virtio_fs_device_create_error() {
        let epoll_manager = EpollManager::default();
        let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();

        // invalid cache policy
        let res: Result<VirtioFs<Arc<GuestMemoryMmap>>> = VirtioFs::new(
            TAG,
            NUM_QUEUES,
            QUEUE_SIZE,
            CACHE_SIZE,
            "dummy_policy",
            THREAD_NUM,
            WB_CACHE,
            NO_OPEN,
            KILLPRIV_V2,
            XATTR,
            DROP_SYS_RSC,
            NO_READDIR,
            new_dummy_handler_helper(),
            epoll_manager.clone(),
            Some(rate_limiter),
            false,
        );
        assert!(res.is_err());

        // drop_sys_resource with write_back_cache
        let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
        let res: Result<VirtioFs<Arc<GuestMemoryMmap>>> = VirtioFs::new(
            TAG,
            NUM_QUEUES,
            QUEUE_SIZE,
            CACHE_SIZE,
            CACHE_POLICY,
            THREAD_NUM,
            true,
            NO_OPEN,
            KILLPRIV_V2,
            XATTR,
            true,
            NO_READDIR,
            new_dummy_handler_helper(),
            epoll_manager,
            Some(rate_limiter),
            false,
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_virtio_fs_device_normal() {
        let epoll_manager = EpollManager::default();
        let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
        let mut fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
            TAG,
            NUM_QUEUES,
            QUEUE_SIZE,
            CACHE_SIZE,
            CACHE_POLICY,
            THREAD_NUM,
            WB_CACHE,
            NO_OPEN,
            KILLPRIV_V2,
            XATTR,
            DROP_SYS_RSC,
            NO_READDIR,
            new_dummy_handler_helper(),
            epoll_manager,
            Some(rate_limiter),
            false,
        )
        .unwrap();

        assert!(!fs.is_dax_on());
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&fs),
            TYPE_VIRTIO_FS
        );
        let queue_size = [QUEUE_SIZE; NUM_QUEUE_OFFSET + NUM_QUEUES];
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::queue_max_sizes(
                &fs
            ),
            &queue_size[..]
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&fs, 0),
            fs.device_info.get_avail_features(0)
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&fs, 1),
            fs.device_info.get_avail_features(1)
        );
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&fs, 2),
            fs.device_info.get_avail_features(2)
        );
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::set_acked_features(
            &mut fs, 2, 0,
        );
        assert_eq!(
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::get_avail_features(&fs, 2),
            0);
        let mut config: [u8; 1] = [0];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::read_config(
            &mut fs,
            0,
            &mut config,
        )
        .unwrap();
        let config: [u8; 16] = [0; 16];
        VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::write_config(
            &mut fs, 0, &config,
        )
        .unwrap();
    }

    #[test]
    fn test_virtio_fs_device_active() {
        skip_if_kvm_unaccessable!();
        let epoll_manager = EpollManager::default();
        {
            // config queue size is not 2
            let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
            let mut fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
                TAG,
                NUM_QUEUES,
                QUEUE_SIZE,
                CACHE_SIZE,
                CACHE_POLICY,
                THREAD_NUM,
                WB_CACHE,
                NO_OPEN,
                KILLPRIV_V2,
                XATTR,
                DROP_SYS_RSC,
                NO_READDIR,
                new_dummy_handler_helper(),
                epoll_manager.clone(),
                Some(rate_limiter),
                false,
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues: Vec<VirtioQueueConfig<QueueSync>> = Vec::new();

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::new()),
            );
            assert!(matches!(
                fs.activate(config),
                Err(ActivateError::InvalidParam)
            ));
        }

        {
            // Ok
            let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
            let mut fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
                TAG,
                NUM_QUEUES,
                QUEUE_SIZE,
                CACHE_SIZE,
                CACHE_POLICY,
                THREAD_NUM,
                WB_CACHE,
                NO_OPEN,
                KILLPRIV_V2,
                XATTR,
                DROP_SYS_RSC,
                NO_READDIR,
                new_dummy_handler_helper(),
                epoll_manager,
                Some(rate_limiter),
                false,
            )
            .unwrap();

            let mem = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10000)]).unwrap();
            let queues = vec![
                VirtioQueueConfig::<QueueSync>::create(1024, 0).unwrap(),
                VirtioQueueConfig::<QueueSync>::create(2, 0).unwrap(),
            ];

            let kvm = Kvm::new().unwrap();
            let vm_fd = Arc::new(kvm.create_vm().unwrap());
            let resources = DeviceResources::new();
            let address_space = create_address_space();
            let config = VirtioDeviceConfig::new(
                Arc::new(mem),
                address_space,
                vm_fd,
                resources,
                queues,
                None,
                Arc::new(NoopNotifier::new()),
            );

            let result = fs.activate(config);
            assert!(result.is_ok());
        }
    }

    // this test case need specific resources and is recommended to run
    // via dbuvm docker image
    #[test]
    #[cfg(feature = "test-resources")]
    fn test_fs_manipulate_backend_fs() {
        let source = "/test_resources/nydus-rs/bootstrap/image_v2.boot";
        let source_path = PathBuf::from(&source);
        if !source_path.exists() {
            eprintln!("Test resource file not found: {}", source);
            return;
        }
        let bootstrapfile = source.to_string();
        // mount
        {
            // invalid fs type
            {
                let mut fs = create_fs_device_default();
                let res = fs.manipulate_backend_fs(
                    None,
                    Some(String::from("dummyFs")),
                    "/mountpoint",
                    None,
                    "mount",
                    None,
                    None,
                );
                assert!(matches!(res, Err(FsError::BackendFs(_))));
            }
            // passthroughFs
            {
                let mut fs = create_fs_device_default();

                // no mount source
                let res = fs.manipulate_backend_fs(
                    None,
                    Some(String::from("PassthroughFs")),
                    "/mountpoint",
                    None,
                    "mount",
                    None,
                    None,
                );
                assert!(matches!(res, Err(FsError::BackendFs(_))));

                // invalid mount source
                let res = fs.manipulate_backend_fs(
                    Some(String::from("dummy_source_path")),
                    Some(String::from("PassthroughFs")),
                    "/mountpoint",
                    None,
                    "mount",
                    None,
                    None,
                );
                assert!(matches!(res, Err(FsError::BackendFs(_))));

                // success
                let mount_dir = temp_dir();
                let mount_path = mount_dir.into_os_string().into_string().unwrap();
                fs.manipulate_backend_fs(
                    Some(mount_path),
                    Some(String::from("PassthroughFs")),
                    "/mountpoint",
                    None,
                    "mount",
                    None,
                    None,
                )
                .unwrap();
            }
            // Rafs
            {
                let mut fs = create_fs_device_default();

                // no mount source
                let res = fs.manipulate_backend_fs(
                    None,
                    Some(String::from("Rafs")),
                    "/mountpoint",
                    None,
                    "mount",
                    None,
                    None,
                );
                assert!(matches!(res, Err(FsError::BackendFs(_))));

                // invalid mount source
                let res = fs.manipulate_backend_fs(
                    Some(String::from("dummy_source_path")),
                    Some(String::from("Rafs")),
                    "/mountpoint",
                    None,
                    "mount",
                    None,
                    None,
                );
                assert!(matches!(res, Err(FsError::BackendFs(_))));

                // invalid rafs cfg format
                let dummy_rafs_cfg = r#"
                {
                    "device": {
                        "backend": {
                            "type": "oss",
                            "config": {
                                "endpoint": "test"
                            }
                        }
                    }
                }"#;
                let res = fs.manipulate_backend_fs(
                    Some(bootstrapfile.clone()),
                    Some(String::from("Rafs")),
                    "/mountpoint",
                    Some(String::from(dummy_rafs_cfg)),
                    "mount",
                    None,
                    None,
                );
                assert!(matches!(res, Err(FsError::BackendFs(_))));

                // success
                let rafs_cfg = r#"
                {
                    "device": {
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
                fs.manipulate_backend_fs(
                    Some(bootstrapfile.clone()),
                    Some(String::from("Rafs")),
                    "/mountpoint",
                    Some(String::from(rafs_cfg)),
                    "mount",
                    None,
                    None,
                )
                .unwrap();
            }
        }
        // umount
        {
            let mut fs = create_fs_device_default();

            // invalid mountpoint
            let res = fs.manipulate_backend_fs(
                None,
                None,
                "/dummy_mountpoint",
                None,
                "umount",
                None,
                None,
            );
            assert!(matches!(res, Err(FsError::BackendFs(_))));

            // success
            let mut fs = create_fs_device_default();
            let dummy_dir = temp_dir();
            let dummy_path = dummy_dir.into_os_string().into_string().unwrap();
            fs.manipulate_backend_fs(
                Some(dummy_path),
                Some(String::from("PassthroughFs")),
                "/mountpoint",
                None,
                "mount",
                None,
                None,
            )
            .unwrap();
            fs.manipulate_backend_fs(None, None, "/mountpoint", None, "umount", None, None)
                .unwrap();
        }

        // update
        {
            let mut fs = create_fs_device_default();
            let rafs_cfg = r#"
                {
                    "device": {
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
            // no config
            let res = fs.manipulate_backend_fs(
                Some(bootstrapfile.clone()),
                Some(String::from("Rafs")),
                "/mountpoint",
                None,
                "update",
                None,
                None,
            );
            assert!(matches!(res, Err(FsError::BackendFs(_))));

            // no source configured
            let res = fs.manipulate_backend_fs(
                Some(bootstrapfile.clone()),
                Some(String::from("Rafs")),
                "/mountpoint",
                Some(String::from(rafs_cfg)),
                "update",
                None,
                None,
            );
            assert!(matches!(res, Err(FsError::BackendFs(_))));

            // invalid mountpoint
            fs.manipulate_backend_fs(
                Some(bootstrapfile.clone()),
                Some(String::from("Rafs")),
                "/mountpoint",
                Some(String::from(rafs_cfg)),
                "mount",
                None,
                None,
            )
            .unwrap();

            let res = fs.manipulate_backend_fs(
                Some(bootstrapfile.clone()),
                Some(String::from("Rafs")),
                "/dummy_mountpoint",
                Some(String::from(rafs_cfg)),
                "update",
                None,
                None,
            );
            assert!(matches!(res, Err(FsError::BackendFs(_))));

            // success
            fs.manipulate_backend_fs(
                Some(bootstrapfile.clone()),
                Some(String::from("Rafs")),
                "/mountpoint",
                Some(String::from(rafs_cfg)),
                "mount",
                None,
                None,
            )
            .unwrap();

            let res = fs.manipulate_backend_fs(
                Some(bootstrapfile),
                Some(String::from("Rafs")),
                "/mountpoint",
                Some(String::from(rafs_cfg)),
                "update",
                None,
                None,
            );
            assert!(res.is_ok());
        }

        // invalid operation
        {
            let mut fs = create_fs_device_default();
            let res = fs.manipulate_backend_fs(
                None,
                None,
                "/mountpoint",
                None,
                "dummy_ops",
                None,
                Some(1024 * 1024 * 1024),
            );
            assert!(matches!(res, Err(FsError::BackendFs(_))));
        }
    }

    #[test]
    fn test_parse_prefetch_files() {
        // Non-empty prefetch list
        let tmp_file = TempFile::new().unwrap();
        writeln!(tmp_file.as_file(), "/hello.txt").unwrap();
        writeln!(tmp_file.as_file()).unwrap();
        writeln!(tmp_file.as_file(), "  ").unwrap();
        writeln!(tmp_file.as_file(), "\t").unwrap();
        writeln!(tmp_file.as_file(), "/").unwrap();
        writeln!(tmp_file.as_file(), "\n").unwrap();
        writeln!(tmp_file.as_file(), "test").unwrap();

        let files = parse_prefetch_files(Some(tmp_file.as_path().to_str().unwrap().to_string()));
        assert_eq!(
            files,
            Some(vec![PathBuf::from("/hello.txt"), PathBuf::from("/")])
        );

        // Empty prefetch list
        let tmp_file = TempFile::new().unwrap();
        let files = parse_prefetch_files(Some(tmp_file.as_path().to_str().unwrap().to_string()));
        assert_eq!(files, None);

        // None prefetch list
        let files = parse_prefetch_files(None);
        assert_eq!(files, None);

        // Not exist prefetch list
        let files = parse_prefetch_files(Some("no_such_file".to_string()));
        assert_eq!(files, None);
    }

    #[test]
    fn test_normalize_oss_prefix() {
        assert_eq!(normalize_oss_prefix("/", None), "");
        assert_eq!(normalize_oss_prefix("/dataset", None), "dataset/");
        assert_eq!(
            normalize_oss_prefix("/dataset", Some("base-prefix")),
            "base-prefix/dataset/"
        );
        assert_eq!(
            normalize_oss_prefix("/", Some("base-prefix/")),
            "base-prefix/"
        );
    }

    #[test]
    fn test_oss_object_relative_path() {
        assert_eq!(
            oss_object_relative_path("base/prefix/hello/world.txt", "base/prefix/").unwrap(),
            PathBuf::from("hello/world.txt")
        );
        assert!(oss_object_relative_path("../escape.txt", "").is_err());
        assert!(oss_object_relative_path("base/prefix/../../escape.txt", "base/prefix/").is_err());
    }

    #[test]
    fn test_oss_cache_namespace() {
        let namespace = oss_cache_namespace("my-bucket", "base/prefix/");
        assert!(!namespace.contains('/'));
        assert!(!namespace.contains(':'));
    }

    #[test]
    fn test_parse_oss_write_policy() {
        assert_eq!(
            parse_oss_write_policy(None).unwrap(),
            OssWritePolicy::ReadOnly
        );
        assert_eq!(
            parse_oss_write_policy(Some("write-through")).unwrap(),
            OssWritePolicy::WriteThrough
        );
        assert_eq!(
            parse_oss_write_policy(Some("async_flush")).unwrap(),
            OssWritePolicy::AsyncFlush
        );
        assert!(parse_oss_write_policy(Some("invalid-mode")).is_err());
    }

    #[test]
    fn test_parse_oss_consistency_strategy() {
        assert_eq!(
            parse_oss_consistency_strategy(None).unwrap(),
            OssConsistencyStrategy::LocalWins
        );
        assert_eq!(
            parse_oss_consistency_strategy(Some("etag-guard")).unwrap(),
            OssConsistencyStrategy::EtagGuard
        );
        assert_eq!(
            parse_oss_consistency_strategy(Some("remote-wins")).unwrap(),
            OssConsistencyStrategy::RemoteWins
        );
        assert!(parse_oss_consistency_strategy(Some("unknown")).is_err());
    }

    #[test]
    fn test_resolve_oss_flush_interval() {
        assert_eq!(
            resolve_oss_flush_interval(OssWritePolicy::WriteThrough, None).unwrap(),
            Duration::from_secs(OSS_WRITE_THROUGH_INTERVAL_SECS)
        );
        assert_eq!(
            resolve_oss_flush_interval(OssWritePolicy::AsyncFlush, Some(15)).unwrap(),
            Duration::from_secs(15)
        );
        assert_eq!(
            resolve_oss_flush_interval(OssWritePolicy::ReadOnly, Some(0)).unwrap(),
            Duration::from_secs(0)
        );
        assert!(resolve_oss_flush_interval(OssWritePolicy::AsyncFlush, Some(0)).is_err());
    }

    #[test]
    fn test_resolve_oss_upload_chunk_size() {
        assert_eq!(
            resolve_oss_upload_chunk_size(None).unwrap(),
            OSS_DEFAULT_UPLOAD_CHUNK_SIZE
        );
        assert_eq!(resolve_oss_upload_chunk_size(Some(4096)).unwrap(), 4096);
        assert!(resolve_oss_upload_chunk_size(Some(0)).is_err());
    }

    #[test]
    fn test_is_remote_state_conflicted() {
        let old = OssCacheEntry {
            etag: Some("etag-1".to_string()),
            content_length: 4,
            local_size: 4,
            local_mtime_ns: 100,
        };
        assert!(!is_remote_state_conflicted(
            &old,
            &RemoteObjectState::Existing {
                etag: Some("etag-1".to_string()),
                content_length: 4,
            }
        ));
        assert!(is_remote_state_conflicted(
            &old,
            &RemoteObjectState::Existing {
                etag: Some("etag-2".to_string()),
                content_length: 4,
            }
        ));
        assert!(is_remote_state_conflicted(&old, &RemoteObjectState::NotFound));
    }

    #[test]
    fn test_classify_conditional_oss_error() {
        let conflict = opendal::Error::new(OpenDalErrorKind::ConditionNotMatch, "conflict");
        assert_eq!(
            classify_conditional_oss_error(&conflict),
            Some(OssConditionalOpOutcome::Conflict)
        );

        let unsupported = opendal::Error::new(OpenDalErrorKind::Unsupported, "unsupported");
        assert_eq!(
            classify_conditional_oss_error(&unsupported),
            Some(OssConditionalOpOutcome::Unsupported)
        );

        let other = opendal::Error::new(OpenDalErrorKind::PermissionDenied, "permission denied");
        assert_eq!(classify_conditional_oss_error(&other), None);
    }

    #[test]
    fn test_persist_oss_cache_index_roundtrip() {
        let tmp_file = TempFile::new().unwrap();
        let index_path = tmp_file.as_path().with_extension("oss_index.json");
        let mut index = OssCacheIndex::default();
        index.entries.insert(
            "prefix/path/file.txt".to_string(),
            OssCacheEntry {
                etag: Some("etag-123".to_string()),
                content_length: 1234,
                local_size: 1234,
                local_mtime_ns: 42,
            },
        );

        persist_oss_cache_index(&index_path, &index).unwrap();
        let loaded = load_oss_cache_index(&index_path);
        assert_eq!(loaded, index);
    }

    #[test]
    fn test_is_local_state_dirty() {
        let old = OssCacheEntry {
            etag: Some("etag".to_string()),
            content_length: 4,
            local_size: 4,
            local_mtime_ns: 100,
        };
        let state = LocalCacheState {
            size: 4,
            mtime_ns: 100,
        };
        assert!(!is_local_state_dirty(&old, state));

        let changed = LocalCacheState {
            size: 8,
            mtime_ns: 101,
        };
        assert!(is_local_state_dirty(&old, changed));

        let unknown_old = OssCacheEntry {
            etag: Some("etag".to_string()),
            content_length: 4,
            local_size: 0,
            local_mtime_ns: 0,
        };
        assert!(!is_local_state_dirty(&unknown_old, changed));
    }

    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn test_kb_to_bytes() {
        let kb = 0x1000;
        assert_eq!(kb_to_bytes(kb).unwrap(), 0x400_000);

        let kb = 0x100_0000;
        assert_eq!(kb_to_bytes(kb).unwrap(), 0x400_00_0000);

        let kb = 0x20_0000_0000_0000;
        assert_eq!(kb_to_bytes(kb).unwrap(), 0x8000_0000_0000_0000);

        let kb = 0x100_0000_0000_0000;
        assert!(kb_to_bytes(kb).is_err());

        let kb = 0x1000_0000_0000_0000;
        assert!(kb_to_bytes(kb).is_err());

        let kb = 0x1100_0000_0000_0000;
        assert!(kb_to_bytes(kb).is_err());
    }

    #[test]
    fn test_get_timeout() {
        fn create_fs_device_with_cache_policy(policy: &str) -> VirtioFs<Arc<GuestMemoryMmap>> {
            let epoll_manager = EpollManager::default();
            let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
            let fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
                TAG,
                NUM_QUEUES,
                QUEUE_SIZE,
                CACHE_SIZE,
                policy,
                THREAD_NUM,
                WB_CACHE,
                NO_OPEN,
                KILLPRIV_V2,
                XATTR,
                DROP_SYS_RSC,
                NO_READDIR,
                new_dummy_handler_helper(),
                epoll_manager,
                Some(rate_limiter),
                false,
            )
            .unwrap();
            fs
        }
        let fs = create_fs_device_with_cache_policy("auto");
        assert_eq!(fs.get_timeout(), Duration::from_secs(CACHE_AUTO_TIMEOUT));
        let fs = create_fs_device_with_cache_policy("always");
        assert_eq!(fs.get_timeout(), Duration::from_secs(CACHE_ALWAYS_TIMEOUT));
        let fs = create_fs_device_with_cache_policy("never");
        assert_eq!(fs.get_timeout(), Duration::from_secs(CACHE_NONE_TIMEOUT));
    }

    #[test]
    fn test_register_mmap_region() {
        skip_if_kvm_unaccessable!();
        let epoll_manager = EpollManager::default();
        let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
        let mut fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
            TAG,
            NUM_QUEUES,
            QUEUE_SIZE,
            CACHE_SIZE,
            CACHE_POLICY,
            THREAD_NUM,
            WB_CACHE,
            NO_OPEN,
            KILLPRIV_V2,
            XATTR,
            DROP_SYS_RSC,
            NO_READDIR,
            new_dummy_handler_helper(),
            epoll_manager,
            Some(rate_limiter),
            false,
        )
        .unwrap();
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let mut resources = DeviceResources::new();
        let entry = dbs_device::resources::Resource::MmioAddressRange {
            base: 0x1000,
            size: 0x1000,
        };
        resources.append(entry);
        let entry = dbs_device::resources::Resource::KvmMemSlot(0);
        resources.append(entry);

        let mmio_res = resources.get_mmio_address_ranges();
        let slot_res = resources.get_kvm_mem_slots();
        let start = mmio_res[0].0;
        let len = mmio_res[0].1;
        let res = fs.register_mmap_region(vm_fd, start, len, &slot_res);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().start_addr(), GuestAddress(0x1000));
    }

    #[test]
    fn test_get_resource_requirements() {
        skip_if_kvm_unaccessable!();
        let epoll_manager = EpollManager::default();
        let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
        let dax_on = 0x4000;
        let fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
            TAG,
            NUM_QUEUES,
            QUEUE_SIZE,
            dax_on,
            CACHE_POLICY,
            THREAD_NUM,
            WB_CACHE,
            NO_OPEN,
            KILLPRIV_V2,
            XATTR,
            DROP_SYS_RSC,
            NO_READDIR,
            new_dummy_handler_helper(),
            epoll_manager,
            Some(rate_limiter),
            false,
        )
        .unwrap();
        let mut requirements = vec![
            ResourceConstraint::new_mmio(0x1),
            ResourceConstraint::new_mmio(0x2),
        ];
        VirtioDevice::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::get_resource_requirements(
            &fs,
            &mut requirements,
            true,
        );

        assert_eq!(requirements[2], ResourceConstraint::LegacyIrq { irq: None });
        assert_eq!(requirements[3], ResourceConstraint::GenericIrq { size: 3 });
        assert_eq!(
            requirements[5],
            ResourceConstraint::KvmMemSlot {
                slot: None,
                size: 1
            }
        );
    }

    #[test]
    fn test_set_resource() {
        skip_if_kvm_unaccessable!();
        let epoll_manager = EpollManager::default();
        let rate_limiter = RateLimiter::new(100, 0, 300, 10, 0, 300).unwrap();
        let mut fs: VirtioFs<Arc<GuestMemoryMmap>> = VirtioFs::new(
            TAG,
            NUM_QUEUES,
            QUEUE_SIZE,
            CACHE_SIZE,
            CACHE_POLICY,
            THREAD_NUM,
            WB_CACHE,
            NO_OPEN,
            KILLPRIV_V2,
            XATTR,
            DROP_SYS_RSC,
            NO_READDIR,
            new_dummy_handler_helper(),
            epoll_manager,
            Some(rate_limiter),
            false,
        )
        .unwrap();
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let mut resources = DeviceResources::new();
        let entry = dbs_device::resources::Resource::MmioAddressRange {
            base: 0x1000,
            size: 0x1000,
        };
        resources.append(entry);
        let entry = dbs_device::resources::Resource::KvmMemSlot(0);
        resources.append(entry);

        let res = VirtioDevice::<Arc<GuestMemoryMmap>, QueueSync, GuestRegionMmap>::set_resource(
            &mut fs, vm_fd, resources,
        );
        assert!(res.is_ok());
        let content = res.unwrap().unwrap();
        assert_eq!(content.kvm_userspace_memory_region_slot, 0);
        assert_eq!(content.region_list[0].offset, 0);
        assert_eq!(content.region_list[0].len, 0x1000);
    }
}
