// Copyright 2020-2022 Alibaba Cloud. All Rights Reserved.
// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::convert::TryInto;

use dbs_utils::epoll_manager::EpollManager;
use dbs_virtio_devices::{self as virtio, Error as VirtioError};
use serde_derive::{Deserialize, Serialize};
use slog::{error, info};

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::config_manager::{
    ConfigItem, DeviceConfigInfo, DeviceConfigInfos, RateLimiterConfigInfo,
};
use crate::device_manager::{
    DbsMmioV2Device, DeviceManager, DeviceMgrError, DeviceOpContext, DeviceVirtioRegionHandler,
};
use crate::get_bucket_update;

use super::DbsVirtioDevice;

// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = true;
// Default cache size is 2 Gi since this is a typical VM memory size.
const DEFAULT_CACHE_SIZE: u64 = 2 * 1024 * 1024 * 1024;
// We have 2 supported fs device mode, vhostuser and virtio
const VHOSTUSER_FS_MODE: &str = "vhostuser";
// We have 2 supported fs device mode, vhostuser and virtio
const VIRTIO_FS_MODE: &str = "virtio";

/// Errors associated with `FsDeviceConfig`.
#[derive(Debug, thiserror::Error)]
pub enum FsDeviceError {
    /// Invalid fs, "virtio" or "vhostuser" is allowed.
    #[error("the fs type is invalid, virtio or vhostuser is allowed")]
    InvalidFs,

    /// Cannot access address space.
    #[error("Cannot access address space.")]
    AddressSpaceNotInitialized,

    /// Cannot convert RateLimterConfigInfo into RateLimiter.
    #[error("failure while converting RateLimterConfigInfo into RateLimiter: {0}")]
    RateLimterConfigInfoTryInto(#[source] std::io::Error),

    /// The fs device tag was already used for a different fs.
    #[error("VirtioFs device tag {0} already exists")]
    FsDeviceTagAlreadyExists(String),

    /// The fs device path was already used for a different fs.
    #[error("VirtioFs device tag {0} already exists")]
    FsDevicePathAlreadyExists(String),

    /// The update is not allowed after booting the microvm.
    #[error("update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// The attachbackendfs operation fails.
    #[error("Fs device attach a backend fs failed")]
    AttachBackendFailed(String),

    /// attach backend fs must be done when vm is running.
    #[error("vm is not running when attaching a backend fs")]
    MicroVMNotRunning,

    /// The mount tag doesn't exist.
    #[error("fs tag'{0}' doesn't exist")]
    TagNotExists(String),

    /// Failed to send patch message to VirtioFs epoll handler.
    #[error("could not send patch message to the VirtioFs epoll handler")]
    VirtioFsEpollHanderSendFail,

    /// Creating a shared-fs device fails (if the vhost-user socket cannot be open.)
    #[error("cannot create shared-fs device: {0}")]
    CreateFsDevice(#[source] VirtioError),

    /// Cannot initialize a shared-fs device or add a device to the MMIO Bus.
    #[error("failure while registering shared-fs device: {0}")]
    RegisterFsDevice(#[source] DeviceMgrError),

    /// The device manager errors.
    #[error("DeviceManager error: {0}")]
    DeviceManager(#[source] DeviceMgrError),
}

/// Configuration information for a vhost-user-fs device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct FsDeviceConfigInfo {
    /// vhost-user socket path.
    pub sock_path: String,
    /// virtiofs mount tag name used inside the guest.
    /// used as the device name during mount.
    pub tag: String,
    /// Number of virtqueues to use.
    pub num_queues: usize,
    /// Size of each virtqueue. Unit: byte.
    pub queue_size: u16,
    /// DAX cache window size
    pub cache_size: u64,
    /// Number of thread pool workers.
    pub thread_pool_size: u16,
    /// The caching policy the file system should use (auto, always or never).
    /// This cache policy is set for virtio-fs, visit https://gitlab.com/virtio-fs/virtiofsd to get further information.
    pub cache_policy: String,
    /// Writeback cache
    pub writeback_cache: bool,
    /// Enable no_open or not
    pub no_open: bool,
    /// Enable xattr or not
    pub xattr: bool,
    /// Drop CAP_SYS_RESOURCE or not
    pub drop_sys_resource: bool,
    /// virtio fs or vhostuser fs.
    pub mode: String,
    /// Enable kill_priv_v2 or not
    pub fuse_killpriv_v2: bool,
    /// Enable no_readdir or not
    pub no_readdir: bool,
    /// Rate Limiter for I/O operations.
    pub rate_limiter: Option<RateLimiterConfigInfo>,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl std::default::Default for FsDeviceConfigInfo {
    fn default() -> Self {
        Self {
            sock_path: String::default(),
            tag: String::default(),
            num_queues: 1,
            queue_size: 1024,
            cache_size: DEFAULT_CACHE_SIZE,
            thread_pool_size: 0,
            cache_policy: Self::default_cache_policy(),
            writeback_cache: Self::default_writeback_cache(),
            no_open: Self::default_no_open(),
            fuse_killpriv_v2: Self::default_fuse_killpriv_v2(),
            no_readdir: Self::default_no_readdir(),
            xattr: Self::default_xattr(),
            drop_sys_resource: Self::default_drop_sys_resource(),
            mode: Self::default_fs_mode(),
            rate_limiter: Some(RateLimiterConfigInfo::default()),
            use_shared_irq: None,
            use_generic_irq: None,
        }
    }
}

impl FsDeviceConfigInfo {
    /// The default mode is set to 'virtio' for 'virtio-fs' device.
    pub fn default_fs_mode() -> String {
        String::from(VIRTIO_FS_MODE)
    }

    /// The default cache policy
    pub fn default_cache_policy() -> String {
        "always".to_string()
    }

    /// The default setting of writeback cache
    pub fn default_writeback_cache() -> bool {
        true
    }

    /// The default setting of no_open
    pub fn default_no_open() -> bool {
        true
    }

    /// The default setting of killpriv_v2
    pub fn default_fuse_killpriv_v2() -> bool {
        false
    }

    /// The default setting of xattr
    pub fn default_xattr() -> bool {
        false
    }

    /// The default setting of drop_sys_resource
    pub fn default_drop_sys_resource() -> bool {
        false
    }

    /// The default setting of no_readdir
    pub fn default_no_readdir() -> bool {
        false
    }

    /// The default setting of rate limiter
    pub fn default_fs_rate_limiter() -> Option<RateLimiterConfigInfo> {
        None
    }
}

/// Configuration information for virtio-fs.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct FsDeviceConfigUpdateInfo {
    /// virtiofs mount tag name used inside the guest.
    /// used as the device name during mount.
    pub tag: String,
    /// Rate Limiter for I/O operations.
    pub rate_limiter: Option<RateLimiterConfigInfo>,
}

impl FsDeviceConfigUpdateInfo {
    /// Provides a `BucketUpdate` description for the bandwidth rate limiter.
    pub fn bytes(&self) -> dbs_utils::rate_limiter::BucketUpdate {
        get_bucket_update!(self, rate_limiter, bandwidth)
    }
    /// Provides a `BucketUpdate` description for the ops rate limiter.
    pub fn ops(&self) -> dbs_utils::rate_limiter::BucketUpdate {
        get_bucket_update!(self, rate_limiter, ops)
    }
}

impl ConfigItem for FsDeviceConfigInfo {
    type Err = FsDeviceError;

    fn id(&self) -> &str {
        &self.tag
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), FsDeviceError> {
        if self.tag == other.tag {
            Err(FsDeviceError::FsDeviceTagAlreadyExists(self.tag.clone()))
        } else if self.mode.as_str() == VHOSTUSER_FS_MODE && self.sock_path == other.sock_path {
            Err(FsDeviceError::FsDevicePathAlreadyExists(
                self.sock_path.clone(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Configuration information of manipulating backend fs for a virtiofs device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, Default)]
pub struct FsMountConfigInfo {
    /// Mount operations, mount, update, umount
    pub ops: String,
    /// The backend fs type to mount.
    pub fstype: Option<String>,
    /// the source file/directory the backend fs points to
    pub source: Option<String>,
    /// where the backend fs gets mounted
    pub mountpoint: String,
    /// backend fs config content in json format
    pub config: Option<String>,
    /// virtiofs mount tag name used inside the guest.
    /// used as the device name during mount.
    pub tag: String,
    /// Path to file that contains file lists that should be prefetched by rafs
    pub prefetch_list_path: Option<String>,
    /// What size file supports dax
    pub dax_threshold_size_kb: Option<u64>,
}

pub(crate) type FsDeviceInfo = DeviceConfigInfo<FsDeviceConfigInfo>;

impl ConfigItem for FsDeviceInfo {
    type Err = FsDeviceError;
    fn id(&self) -> &str {
        &self.config.tag
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), FsDeviceError> {
        if self.config.tag == other.config.tag {
            Err(FsDeviceError::FsDeviceTagAlreadyExists(
                self.config.tag.clone(),
            ))
        } else if self.config.sock_path == other.config.sock_path {
            Err(FsDeviceError::FsDevicePathAlreadyExists(
                self.config.sock_path.clone(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Wrapper for the collection that holds all the Fs Devices Configs
pub struct FsDeviceMgr {
    /// A list of `FsDeviceConfig` objects.
    pub(crate) info_list: DeviceConfigInfos<FsDeviceConfigInfo>,
    pub(crate) use_shared_irq: bool,
}

impl FsDeviceMgr {
    /// Inserts `fs_cfg` in the shared-fs device configuration list.
    pub fn insert_device(
        device_mgr: &mut DeviceManager,
        ctx: DeviceOpContext,
        fs_cfg: FsDeviceConfigInfo,
    ) -> std::result::Result<(), FsDeviceError> {
        // It's too complicated to manage life cycle of shared-fs service process for hotplug.
        if ctx.is_hotplug {
            error!(
                ctx.logger(),
                "no support of shared-fs device hotplug";
                "subsystem" => "shared-fs",
                "tag" => &fs_cfg.tag,
            );
            return Err(FsDeviceError::UpdateNotAllowedPostBoot);
        }

        info!(
            ctx.logger(),
            "add shared-fs device configuration";
            "subsystem" => "shared-fs",
            "tag" => &fs_cfg.tag,
        );
        device_mgr
            .fs_manager
            .lock()
            .unwrap()
            .info_list
            .insert_or_update(&fs_cfg)?;

        Ok(())
    }

    /// Attaches all vhost-user-fs devices from the FsDevicesConfig.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), FsDeviceError> {
        let epoll_mgr = ctx
            .epoll_mgr
            .clone()
            .ok_or(FsDeviceError::CreateFsDevice(virtio::Error::InvalidInput))?;

        for info in self.info_list.iter_mut() {
            let device = Self::create_fs_device(&info.config, ctx, epoll_mgr.clone())?;
            let mmio_device = DeviceManager::create_mmio_virtio_device(
                device,
                ctx,
                info.config.use_shared_irq.unwrap_or(self.use_shared_irq),
                info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
            )
            .map_err(FsDeviceError::RegisterFsDevice)?;

            info.set_device(mmio_device);
        }

        Ok(())
    }

    fn create_fs_device(
        config: &FsDeviceConfigInfo,
        ctx: &mut DeviceOpContext,
        epoll_mgr: EpollManager,
    ) -> std::result::Result<DbsVirtioDevice, FsDeviceError> {
        match &config.mode as &str {
            VIRTIO_FS_MODE => Self::attach_virtio_fs_devices(config, ctx, epoll_mgr),
            #[cfg(feature = "vhost-user-fs")]
            VHOSTUSER_FS_MODE => Self::attach_vhostuser_fs_devices(config, ctx, epoll_mgr),
            _ => Err(FsDeviceError::CreateFsDevice(virtio::Error::InvalidInput)),
        }
    }

    fn attach_virtio_fs_devices(
        config: &FsDeviceConfigInfo,
        ctx: &mut DeviceOpContext,
        epoll_mgr: EpollManager,
    ) -> std::result::Result<DbsVirtioDevice, FsDeviceError> {
        info!(
            ctx.logger(),
            "add virtio-fs device configuration";
            "subsystem" => "virito-fs",
            "tag" => &config.tag,
            "dax_window_size" => &config.cache_size,
        );

        let limiter = if let Some(rlc) = config.rate_limiter.clone() {
            Some(
                rlc.try_into()
                    .map_err(FsDeviceError::RateLimterConfigInfoTryInto)?,
            )
        } else {
            None
        };

        let vm_as = ctx.get_vm_as().map_err(|e| {
            error!(ctx.logger(), "virtio-fs get vm_as error: {:?}", e; 
                "subsystem" => "virito-fs");
            FsDeviceError::DeviceManager(e)
        })?;
        let address_space = match ctx.address_space.as_ref() {
            Some(address_space) => address_space.clone(),
            None => {
                error!(ctx.logger(), "virtio-fs get address_space error"; "subsystem" => "virito-fs");
                return Err(FsDeviceError::AddressSpaceNotInitialized);
            }
        };
        let handler = DeviceVirtioRegionHandler {
            vm_as,
            address_space,
        };

        let device = Box::new(
            virtio::fs::VirtioFs::new(
                &config.tag,
                config.num_queues,
                config.queue_size,
                config.cache_size,
                &config.cache_policy,
                config.thread_pool_size,
                config.writeback_cache,
                config.no_open,
                config.fuse_killpriv_v2,
                config.xattr,
                config.drop_sys_resource,
                config.no_readdir,
                Box::new(handler),
                epoll_mgr,
                limiter,
            )
            .map_err(FsDeviceError::CreateFsDevice)?,
        );

        Ok(device)
    }

    #[cfg(feature = "vhost-user-fs")]
    fn attach_vhostuser_fs_devices(
        config: &FsDeviceConfigInfo,
        ctx: &mut DeviceOpContext,
        epoll_mgr: EpollManager,
    ) -> std::result::Result<DbsVirtioDevice, FsDeviceError> {
        slog::info!(
            ctx.logger(),
            "attach vhost-fs device";
            "subsystem" => "vhost-fs",
            "tag" => &config.tag,
            "dax_window_size" => &config.cache_size,
            "sock_path" => &config.sock_path,
        );

        let device = Box::new(
            virtio::vhost::vhost_user::fs::VhostUserFs::new(
                config.sock_path.clone(),
                config.tag.clone(),
                config.num_queues,
                config.queue_size,
                config.cache_size,
                epoll_mgr,
            )
            .map_err(FsDeviceError::CreateFsDevice)?,
        );

        Ok(device)
    }

    /// Attach a backend fs to a VirtioFs device or detach a backend
    /// fs from a Virtiofs device
    pub fn manipulate_backend_fs(
        device_mgr: &mut DeviceManager,
        config: FsMountConfigInfo,
    ) -> std::result::Result<(), FsDeviceError> {
        let mut found = false;

        let mgr = &mut device_mgr.fs_manager.lock().unwrap();
        for info in mgr
            .info_list
            .iter()
            .filter(|info| info.config.tag.as_str() == config.tag.as_str())
        {
            found = true;
            if let Some(device) = info.device.as_ref() {
                if let Some(mmio_dev) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
                    let mut guard = mmio_dev.state();
                    let inner_dev = guard.get_inner_device_mut();
                    if let Some(virtio_fs_dev) = inner_dev
                        .as_any_mut()
                        .downcast_mut::<virtio::fs::VirtioFs<GuestAddressSpaceImpl>>()
                    {
                        return virtio_fs_dev
                            .manipulate_backend_fs(
                                config.source,
                                config.fstype,
                                &config.mountpoint,
                                config.config,
                                &config.ops,
                                config.prefetch_list_path,
                                config.dax_threshold_size_kb,
                            )
                            .map(|_p| ())
                            .map_err(|e| FsDeviceError::AttachBackendFailed(e.to_string()));
                    }
                }
            }
        }
        if !found {
            Err(FsDeviceError::AttachBackendFailed(
                "fs tag not found".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// Gets the index of the device with the specified `tag` if it exists in the list.
    pub fn get_index_of_tag(&self, tag: &str) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.id().eq(tag))
    }

    /// Update the ratelimiter settings of a virtio fs device.
    pub fn update_device_ratelimiters(
        device_mgr: &mut DeviceManager,
        new_cfg: FsDeviceConfigUpdateInfo,
    ) -> std::result::Result<(), FsDeviceError> {
        let mgr = &mut device_mgr.fs_manager.lock().unwrap();
        match mgr.get_index_of_tag(&new_cfg.tag) {
            Some(index) => {
                let config = &mut mgr.info_list[index].config;
                config.rate_limiter = new_cfg.rate_limiter.clone();
                let device = mgr.info_list[index]
                    .device
                    .as_mut()
                    .ok_or_else(|| FsDeviceError::TagNotExists("".to_owned()))?;

                if let Some(mmio_dev) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
                    let guard = mmio_dev.state();
                    let inner_dev = guard.get_inner_device();
                    if let Some(fs_dev) = inner_dev
                        .as_any()
                        .downcast_ref::<virtio::fs::VirtioFs<GuestAddressSpaceImpl>>()
                    {
                        return fs_dev
                            .set_patch_rate_limiters(new_cfg.bytes(), new_cfg.ops())
                            .map(|_p| ())
                            .map_err(|_e| FsDeviceError::VirtioFsEpollHanderSendFail);
                    }
                }
                Ok(())
            }
            None => Err(FsDeviceError::TagNotExists(new_cfg.tag)),
        }
    }
}

impl Default for FsDeviceMgr {
    /// Create a new `FsDeviceMgr` object..
    fn default() -> Self {
        FsDeviceMgr {
            info_list: DeviceConfigInfos::new(),
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}
