// Copyright 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

use std::any::Any;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::unix::io::FromRawFd;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{mpsc, Arc};
use std::time::Duration;

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
use rlimit::Resource;
use virtio_bindings::bindings::virtio_blk::VIRTIO_F_VERSION_1;
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
                    error!(
                        "{}: failed to send rate-limiter patch data {:?}",
                        VIRTIO_FS_NAME, e
                    );
                    Error::InternalError
                })?;
                self.patch_rate_limiter_fd.write(1).map_err(|e| {
                    error!(
                        "{}: failed to write rate-limiter patch event {:?}",
                        VIRTIO_FS_NAME, e
                    );
                    Error::InternalError
                })?;
                Ok(())
            }
            None => {
                error!(
                    "{}: failed to establish channel to send rate-limiter patch data",
                    VIRTIO_FS_NAME
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
    ) -> Result<Self> {
        info!(
            "{}: tag {} req_num_queues {} queue_size {} cache_size {} cache_policy {} thread_pool_size {} writeback_cache {} no_open {} killpriv_v2 {} xattr {} drop_sys_resource {} no_readdir {}",
            VIRTIO_FS_NAME, tag, req_num_queues, queue_size, cache_size, cache_policy, thread_pool_size, writeback_cache, no_open, killpriv_v2, xattr, drop_sys_resource, no_readdir
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
                error!(
                    "{}: Parse cache_policy \"{}\" failed: {:?}",
                    VIRTIO_FS_NAME, cache_policy, e
                );
                return Err(Error::InvalidInput);
            }
        };

        // Set rlimit first, in case we dropped CAP_SYS_RESOURCE later and hit EPERM.
        if let Err(e) = set_default_rlimit_nofile() {
            warn!("{}: failed to set rlimit: {:?}", VIRTIO_FS_NAME, e);
        }

        if drop_sys_resource && writeback_cache {
            error!(
                "{}: writeback_cache is not compatible with drop_sys_resource",
                VIRTIO_FS_NAME
            );
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
                warn!(
                    "{}: failed to drop CAP_SYS_RESOURCE: {:?}",
                    VIRTIO_FS_NAME, e
                );
            }
        }

        let vfs_opts = VfsOptions {
            no_writeback: !writeback_cache,
            no_open,
            killpriv_v2,
            no_readdir,
            ..VfsOptions::default()
        };

        Ok(VirtioFs {
            device_info: VirtioDeviceInfo::new(
                VIRTIO_FS_NAME.to_string(),
                1u64 << VIRTIO_F_VERSION_1,
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
                    error!("failed to load rafs config {} error: {:?}", &cfg, e);
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
                    cfg, source, &work_dir
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
        debug!(
            "source {:?}, fstype {:?}, mountpoint {:?}, config {:?}, ops {:?}, prefetch_list_path {:?}, dax_threshold_size_kb 0x{:x?}",
            source, fstype, mountpoint, config, ops, prefetch_list_path, dax_threshold_size_kb
        );
        match ops {
            "mount" => {
                if source.is_none() {
                    error!("{}: source is required for mount.", VIRTIO_FS_NAME);
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
                    _ => {
                        error!("http_server: type is not invalid.");
                        Err(FsError::InvalidData)
                    }
                }
            }
            "umount" => {
                self.fs.umount(mountpoint).map_err(|e| {
                    error!("umount {:?}", e);
                    FsError::InvalidData
                })?;
                self.backend_fs.remove(mountpoint);
                Ok(())
            }
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
                error!("blobfs mount {:?}", e);
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
                error!("passthroughfs mount {:?}", e);
                Err(FsError::InvalidData)
            }
        }
    }

    fn mount_rafs(
        &mut self,
        source: String,
        mountpoint: &str,
        config: Option<String>,
        prefetch_list_path: Option<String>,
    ) -> FsResult<()> {
        debug!("http_server rafs");
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
        debug!(
            "{}: Import rafs with prefetch_files {:?}",
            VIRTIO_FS_NAME, prefetch_files
        );
        rafs.0
            .import(rafs.1, prefetch_files)
            .map_err(|e| FsError::BackendFs(format!("Import rafs failed: {e:?}")))?;
        info!(
            "{}: Rafs imported with prefetch_list_path {:?}",
            VIRTIO_FS_NAME, prefetch_list_path
        );
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
                error!("Rafs mount failed: {:?}", e);
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
        let any_fs = rootfs.deref().as_any();
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
                &CString::new("virtio_fs_mem").unwrap(),
                memfd::MemFdCreateFlag::empty(),
            )
            .map_err(|e| Error::VirtioFs(FsError::MemFdCreate(e)))?;
            let file: File = unsafe { File::from_raw_fd(fd) };
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
            GuestRegionMmap::new(mmap_region, GuestAddress(guest_addr))
                .map_err(Error::InsertMmap)?,
        );
        self.handler.insert_region(region.clone())?;

        Ok(region)
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

fn kb_to_bytes(kb: u64) -> FsResult<u64> {
    if (kb & 0xffc0_0000_0000_0000) != 0 {
        error!(
            "dax_threshold_size_kb * 1024 overflow. dax_threshold_size_kb is 0x{:x}.",
            kb
        );
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
        error!(
            "{}: failed to read /proc/sys/fs/file-max {:?}",
            VIRTIO_FS_NAME, e
        );
        Error::IOError(e)
    })?;
    f.read_to_string(&mut file_max)?;
    let file_max = file_max.trim().parse::<u64>().map_err(|e| {
        error!("{}: read fs.file-max sysctl wrong {:?}", VIRTIO_FS_NAME, e);
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
            error!("{}: failed to get rlimit {:?}", VIRTIO_FS_NAME, e);
            Error::IOError(e)
        })?;

    if rlimit_nofile == 0 {
        info!(
            "{}: original rlimit nofile is greater than max_fds({}), keep rlimit nofile setting",
            VIRTIO_FS_NAME, max_fds
        );
        Ok(())
    } else {
        info!(
            "{}: set rlimit {} (max_fds {})",
            VIRTIO_FS_NAME, rlimit_nofile, max_fds
        );

        Resource::NOFILE
            .set(rlimit_nofile, rlimit_nofile)
            .map_err(|e| {
                error!("{}: failed to set rlimit {:?}", VIRTIO_FS_NAME, e);
                Error::IOError(e)
            })
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
            error!(
                "{}: failed to clone patch rate limiter eventfd {:?}",
                VIRTIO_FS_NAME, e
            );
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
            range: Some((0x1_0000_0000, std::u64::MAX)),
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
        )
        .unwrap();

        assert!(!fs.is_dax_on());
        assert_eq!(
            VirtioDevice::<Arc<GuestMemoryMmap<()>>, QueueSync, GuestRegionMmap>::device_type(&fs),
            TYPE_VIRTIO_FS
        );
        let queue_size = vec![QUEUE_SIZE; NUM_QUEUE_OFFSET + NUM_QUEUES];
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
        let source_path = PathBuf::from(source);
        let bootstrapfile = source_path.to_str().unwrap().to_string();
        if !source_path.exists() {
            panic!("Test resource file not found: {}", bootstrapfile);
        }
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
