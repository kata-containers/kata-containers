// Copyright 2020-2022 Alibaba, Inc. or its affiliates. All Rights Reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Device manager for virtio-blk and vhost-user-blk devices.
use std::collections::{vec_deque, VecDeque};
use std::convert::TryInto;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dbs_virtio_devices as virtio;
use dbs_virtio_devices::block::{aio::Aio, io_uring::IoUring, Block, LocalFile, Ufile};
use serde_derive::{Deserialize, Serialize};

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::config_manager::{ConfigItem, DeviceConfigInfo, RateLimiterConfigInfo};
use crate::device_manager::blk_dev_mgr::BlockDeviceError::InvalidDeviceId;
use crate::device_manager::{DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::get_bucket_update;
use crate::vm::KernelConfigInfo;

use super::DbsMmioV2Device;

// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = true;

macro_rules! info(
    ($l:expr, $($args:tt)+) => {
        slog::info!($l, $($args)+; slog::o!("subsystem" => "block_manager"))
    };
);

macro_rules! error(
    ($l:expr, $($args:tt)+) => {
        slog::error!($l, $($args)+; slog::o!("subsystem" => "block_manager"))
    };
);

/// Default queue size for VirtIo block devices.
pub const QUEUE_SIZE: u16 = 128;

/// Errors associated with the operations allowed on a drive.
#[derive(Debug, thiserror::Error)]
pub enum BlockDeviceError {
    /// Invalid VM instance ID.
    #[error("invalid VM instance id")]
    InvalidVMID,

    /// The block device path is invalid.
    #[error("invalid block device path '{0}'")]
    InvalidBlockDevicePath(PathBuf),

    /// The block device type is invalid.
    #[error("invalid block device type")]
    InvalidBlockDeviceType,

    /// The block device path was already used for a different drive.
    #[error("block device path '{0}' already exists")]
    BlockDevicePathAlreadyExists(PathBuf),

    /// The device id doesn't exist.
    #[error("invalid block device id '{0}'")]
    InvalidDeviceId(String),

    /// Cannot perform the requested operation after booting the microVM.
    #[error("block device does not support runtime update")]
    UpdateNotAllowedPostBoot,

    /// A root block device was already added.
    #[error("could not add multiple virtual machine root devices")]
    RootBlockDeviceAlreadyAdded,

    /// Failed to send patch message to block epoll handler.
    #[error("could not send patch message to the block epoll handler")]
    BlockEpollHanderSendFail,

    /// Failure from device manager,
    #[error("device manager errors: {0}")]
    DeviceManager(#[from] DeviceMgrError),

    /// Failure from virtio subsystem.
    #[error(transparent)]
    Virtio(virtio::Error),

    /// Unable to seek the block device backing file due to invalid permissions or
    /// the file was deleted/corrupted.
    #[error("cannot create block device: {0}")]
    CreateBlockDevice(#[source] virtio::Error),

    /// Cannot open the block device backing file.
    #[error("cannot open the block device backing file: {0}")]
    OpenBlockDevice(#[source] std::io::Error),

    /// Cannot initialize a MMIO Block Device or add a device to the MMIO Bus.
    #[error("failure while registering block device: {0}")]
    RegisterBlockDevice(#[source] DeviceMgrError),
}

/// Type of low level storage device/protocol for virtio-blk devices.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlockDeviceType {
    /// Unknown low level device type.
    Unknown,
    /// Vhost-user-blk based low level device.
    /// SPOOL is a reliable NVMe virtualization system for the cloud environment.
    /// You could learn more SPOOL here: https://www.usenix.org/conference/atc20/presentation/xue
    Spool,
    /// Local disk/file based low level device.
    RawBlock,
}

impl BlockDeviceType {
    /// Get type of low level storage device/protocol by parsing `path`.
    pub fn get_type(path: &str) -> BlockDeviceType {
        // SPOOL path should be started with "spool", e.g. "spool:/device1"
        if path.starts_with("spool:/") {
            BlockDeviceType::Spool
        } else {
            BlockDeviceType::RawBlock
        }
    }
}

/// Configuration information for a block device.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BlockDeviceConfigUpdateInfo {
    /// Unique identifier of the drive.
    pub drive_id: String,
    /// Rate Limiter for I/O operations.
    pub rate_limiter: Option<RateLimiterConfigInfo>,
}

impl BlockDeviceConfigUpdateInfo {
    /// Provides a `BucketUpdate` description for the bandwidth rate limiter.
    pub fn bytes(&self) -> dbs_utils::rate_limiter::BucketUpdate {
        get_bucket_update!(self, rate_limiter, bandwidth)
    }
    /// Provides a `BucketUpdate` description for the ops rate limiter.
    pub fn ops(&self) -> dbs_utils::rate_limiter::BucketUpdate {
        get_bucket_update!(self, rate_limiter, ops)
    }
}

/// Configuration information for a block device.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BlockDeviceConfigInfo {
    /// Unique identifier of the drive.
    pub drive_id: String,
    /// Type of low level storage/protocol.
    pub device_type: BlockDeviceType,
    /// Path of the drive.
    pub path_on_host: PathBuf,
    /// If set to true, it makes the current device the root block device.
    /// Setting this flag to true will mount the block device in the
    /// guest under /dev/vda unless the part_uuid is present.
    pub is_root_device: bool,
    /// Part-UUID. Represents the unique id of the boot partition of this device.
    /// It is optional and it will be used only if the `is_root_device` field is true.
    pub part_uuid: Option<String>,
    /// If set to true, the drive is opened in read-only mode. Otherwise, the
    /// drive is opened as read-write.
    pub is_read_only: bool,
    /// If set to false, the drive is opened with buffered I/O mode. Otherwise, the
    /// drive is opened with direct I/O mode.
    pub is_direct: bool,
    /// Don't close `path_on_host` file when dropping the device.
    pub no_drop: bool,
    /// Block device multi-queue
    pub num_queues: usize,
    /// Virtio queue size. Size: byte
    pub queue_size: u16,
    /// Rate Limiter for I/O operations.
    pub rate_limiter: Option<RateLimiterConfigInfo>,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl std::default::Default for BlockDeviceConfigInfo {
    fn default() -> Self {
        Self {
            drive_id: String::default(),
            device_type: BlockDeviceType::RawBlock,
            path_on_host: PathBuf::default(),
            is_root_device: false,
            part_uuid: None,
            is_read_only: false,
            is_direct: Self::default_direct(),
            no_drop: Self::default_no_drop(),
            num_queues: Self::default_num_queues(),
            queue_size: 256,
            rate_limiter: None,
            use_shared_irq: None,
            use_generic_irq: None,
        }
    }
}

impl BlockDeviceConfigInfo {
    /// Get default queue numbers
    pub fn default_num_queues() -> usize {
        1
    }

    /// Get default value of is_direct switch
    pub fn default_direct() -> bool {
        true
    }

    /// Get default value of no_drop switch
    pub fn default_no_drop() -> bool {
        false
    }

    /// Get type of low level storage/protocol.
    pub fn device_type(&self) -> BlockDeviceType {
        self.device_type
    }

    /// Returns a reference to `path_on_host`.
    pub fn path_on_host(&self) -> &PathBuf {
        &self.path_on_host
    }

    /// Returns a reference to the part_uuid.
    pub fn get_part_uuid(&self) -> Option<&String> {
        self.part_uuid.as_ref()
    }

    /// Checks whether the drive had read only permissions.
    pub fn is_read_only(&self) -> bool {
        self.is_read_only
    }

    /// Checks whether the drive uses direct I/O
    pub fn is_direct(&self) -> bool {
        self.is_direct
    }

    /// Get number and size of queues supported.
    pub fn queue_sizes(&self) -> Vec<u16> {
        (0..self.num_queues)
            .map(|_| self.queue_size)
            .collect::<Vec<u16>>()
    }
}

impl ConfigItem for BlockDeviceConfigInfo {
    type Err = BlockDeviceError;

    fn id(&self) -> &str {
        &self.drive_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), BlockDeviceError> {
        if self.drive_id == other.drive_id {
            Ok(())
        } else if self.path_on_host == other.path_on_host {
            Err(BlockDeviceError::BlockDevicePathAlreadyExists(
                self.path_on_host.clone(),
            ))
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Debug for BlockDeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.config)
    }
}

/// Block Device Info
pub type BlockDeviceInfo = DeviceConfigInfo<BlockDeviceConfigInfo>;

/// Wrapper for the collection that holds all the Block Devices Configs
//#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[derive(Clone)]
pub struct BlockDeviceMgr {
    /// A list of `BlockDeviceInfo` objects.
    info_list: VecDeque<BlockDeviceInfo>,
    has_root_block: bool,
    has_part_uuid_root: bool,
    read_only_root: bool,
    part_uuid: Option<String>,
    use_shared_irq: bool,
}

impl BlockDeviceMgr {
    /// returns a front-to-back iterator.
    pub fn iter(&self) -> vec_deque::Iter<BlockDeviceInfo> {
        self.info_list.iter()
    }

    /// Checks whether any of the added BlockDevice is the root.
    pub fn has_root_block_device(&self) -> bool {
        self.has_root_block
    }

    /// Checks whether the root device is configured using a part UUID.
    pub fn has_part_uuid_root(&self) -> bool {
        self.has_part_uuid_root
    }

    /// Checks whether the root device has read-only permisssions.
    pub fn is_read_only_root(&self) -> bool {
        self.read_only_root
    }

    /// Gets the index of the device with the specified `drive_id` if it exists in the list.
    pub fn get_index_of_drive_id(&self, id: &str) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.id().eq(id))
    }

    /// Gets the 'BlockDeviceConfigInfo' of the device with the specified `drive_id` if it exists in the list.
    pub fn get_config_of_drive_id(&self, drive_id: &str) -> Option<BlockDeviceConfigInfo> {
        match self.get_index_of_drive_id(drive_id) {
            Some(index) => {
                let config = self.info_list.get(index).unwrap().config.clone();
                Some(config)
            }
            None => None,
        }
    }

    /// Inserts `block_device_config` in the block device configuration list.
    /// If an entry with the same id already exists, it will attempt to update
    /// the existing entry.
    /// Inserting a secondary root block device will fail.
    pub fn insert_device(
        device_mgr: &mut DeviceManager,
        mut ctx: DeviceOpContext,
        config: BlockDeviceConfigInfo,
    ) -> std::result::Result<(), BlockDeviceError> {
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            return Err(BlockDeviceError::UpdateNotAllowedPostBoot);
        }

        let mgr = &mut device_mgr.block_manager;

        // If the id of the drive already exists in the list, the operation is update.
        match mgr.get_index_of_drive_id(config.id()) {
            Some(index) => {
                // No support for runtime update yet.
                if ctx.is_hotplug {
                    Err(BlockDeviceError::BlockDevicePathAlreadyExists(
                        config.path_on_host.clone(),
                    ))
                } else {
                    for (idx, info) in mgr.info_list.iter().enumerate() {
                        if idx != index {
                            info.config.check_conflicts(&config)?;
                        }
                    }
                    mgr.update(index, config)
                }
            }
            None => {
                for info in mgr.info_list.iter() {
                    info.config.check_conflicts(&config)?;
                }
                let index = mgr.create(config.clone())?;
                if !ctx.is_hotplug {
                    return Ok(());
                }

                match config.device_type {
                    BlockDeviceType::RawBlock => {
                        let device = Self::create_blk_device(&config, &mut ctx)
                            .map_err(BlockDeviceError::Virtio)?;
                        let dev = DeviceManager::create_mmio_virtio_device(
                            device,
                            &mut ctx,
                            config.use_shared_irq.unwrap_or(mgr.use_shared_irq),
                            config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                        )
                        .map_err(BlockDeviceError::DeviceManager)?;
                        mgr.update_device_by_index(index, Arc::clone(&dev))?;
                        // live-upgrade need save/restore device from info.device.
                        mgr.info_list[index].set_device(dev.clone());
                        ctx.insert_hotplug_mmio_device(&dev, None).map_err(|e| {
                            let logger = ctx.logger().new(slog::o!());
                            BlockDeviceMgr::remove_device(device_mgr, ctx, &config.drive_id)
                                .unwrap();
                            error!(
                                logger,
                                "failed to hot-add virtio block device {}, {:?}",
                                &config.drive_id,
                                e
                            );
                            BlockDeviceError::DeviceManager(e)
                        })
                    }
                    _ => Err(BlockDeviceError::InvalidBlockDeviceType),
                }
            }
        }
    }

    /// Attaches all block devices from the BlockDevicesConfig.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), BlockDeviceError> {
        for info in self.info_list.iter_mut() {
            match info.config.device_type {
                BlockDeviceType::RawBlock => {
                    info!(
                        ctx.logger(),
                        "attach virtio-blk device, drive_id {}, path {}",
                        info.config.drive_id,
                        info.config.path_on_host.to_str().unwrap_or("<unknown>")
                    );
                    let device = Self::create_blk_device(&info.config, ctx)
                        .map_err(BlockDeviceError::Virtio)?;
                    let device = DeviceManager::create_mmio_virtio_device(
                        device,
                        ctx,
                        info.config.use_shared_irq.unwrap_or(self.use_shared_irq),
                        info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                    )
                    .map_err(BlockDeviceError::RegisterBlockDevice)?;
                    info.device = Some(device);
                }
                _ => {
                    return Err(BlockDeviceError::OpenBlockDevice(
                        std::io::Error::from_raw_os_error(libc::EINVAL),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Removes all virtio-blk devices
    pub fn remove_devices(&mut self, ctx: &mut DeviceOpContext) -> Result<(), DeviceMgrError> {
        while let Some(mut info) = self.info_list.pop_back() {
            info!(ctx.logger(), "remove drive {}", info.config.drive_id);
            if let Some(device) = info.device.take() {
                DeviceManager::destroy_mmio_virtio_device(device, ctx)?;
            }
        }

        Ok(())
    }

    fn remove(&mut self, drive_id: &str) -> Option<BlockDeviceInfo> {
        match self.get_index_of_drive_id(drive_id) {
            Some(index) => self.info_list.remove(index),
            None => None,
        }
    }

    /// remove a block device, it basically is the inverse operation of `insert_device``
    pub fn remove_device(
        dev_mgr: &mut DeviceManager,
        mut ctx: DeviceOpContext,
        drive_id: &str,
    ) -> std::result::Result<(), BlockDeviceError> {
        if !cfg!(feature = "hotplug") {
            return Err(BlockDeviceError::UpdateNotAllowedPostBoot);
        }

        let mgr = &mut dev_mgr.block_manager;
        match mgr.remove(drive_id) {
            Some(mut info) => {
                info!(ctx.logger(), "remove drive {}", info.config.drive_id);
                if let Some(device) = info.device.take() {
                    DeviceManager::destroy_mmio_virtio_device(device, &mut ctx)
                        .map_err(BlockDeviceError::DeviceManager)?;
                }
            }
            None => return Err(BlockDeviceError::InvalidDeviceId(drive_id.to_owned())),
        }

        Ok(())
    }

    fn create_blk_device(
        cfg: &BlockDeviceConfigInfo,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<Box<Block<GuestAddressSpaceImpl>>, virtio::Error> {
        let epoll_mgr = ctx.epoll_mgr.clone().ok_or(virtio::Error::InvalidInput)?;

        let mut block_files: Vec<Box<dyn Ufile>> = vec![];

        match cfg.device_type {
            BlockDeviceType::RawBlock => {
                let custom_flags = if cfg.is_direct() {
                    info!(
                        ctx.logger(),
                        "Open block device \"{}\" in direct mode.",
                        cfg.path_on_host().display()
                    );
                    libc::O_DIRECT
                } else {
                    info!(
                        ctx.logger(),
                        "Open block device \"{}\" in buffer mode.",
                        cfg.path_on_host().display(),
                    );
                    0
                };
                let io_uring_supported = IoUring::is_supported();
                for i in 0..cfg.num_queues {
                    let queue_size = cfg.queue_sizes()[i] as u32;
                    let file = OpenOptions::new()
                        .read(true)
                        .custom_flags(custom_flags)
                        .write(!cfg.is_read_only())
                        .open(cfg.path_on_host())?;
                    info!(ctx.logger(), "Queue {}: block file opened", i);

                    if io_uring_supported {
                        info!(
                            ctx.logger(),
                            "Queue {}: Using io_uring Raw disk file, queue size {}.", i, queue_size
                        );
                        let io_engine = IoUring::new(file.as_raw_fd(), queue_size)?;
                        block_files.push(Box::new(LocalFile::new(file, cfg.no_drop, io_engine)?));
                    } else {
                        info!(
                            ctx.logger(),
                            "Queue {}: Since io_uring_supported is not enabled, change to default support of Aio Raw disk file, queue size {}", i, queue_size
                        );
                        let io_engine = Aio::new(file.as_raw_fd(), queue_size)?;
                        block_files.push(Box::new(LocalFile::new(file, cfg.no_drop, io_engine)?));
                    }
                }
            }
            _ => {
                error!(
                    ctx.logger(),
                    "invalid block device type: {:?}", cfg.device_type
                );
                return Err(virtio::Error::InvalidInput);
            }
        };

        let mut limiters = vec![];
        for _i in 0..cfg.num_queues {
            if let Some(limiter) = cfg.rate_limiter.clone().map(|mut v| {
                v.resize(cfg.num_queues as u64);
                v.try_into().unwrap()
            }) {
                limiters.push(limiter);
            }
        }

        Ok(Box::new(Block::new(
            block_files,
            cfg.is_read_only,
            Arc::new(cfg.queue_sizes()),
            epoll_mgr,
            limiters,
        )?))
    }

    /// Generated guest kernel commandline related to root block device.
    pub fn generate_kernel_boot_args(
        &self,
        kernel_config: &mut KernelConfigInfo,
    ) -> std::result::Result<(), DeviceMgrError> {
        // Respect user configuration if kernel_cmdline contains "root=",
        // special attention for the case when kernel command line starting with "root=xxx"
        let old_kernel_cmdline = format!(" {}", kernel_config.kernel_cmdline().as_str());
        if !old_kernel_cmdline.contains(" root=") && self.has_root_block {
            let cmdline = kernel_config.kernel_cmdline_mut();
            if let Some(ref uuid) = self.part_uuid {
                cmdline
                    .insert("root", &format!("PART_UUID={}", uuid))
                    .map_err(DeviceMgrError::Cmdline)?;
            } else {
                cmdline
                    .insert("root", "/dev/vda")
                    .map_err(DeviceMgrError::Cmdline)?;
            }
            if self.read_only_root {
                if old_kernel_cmdline.contains(" rw") {
                    return Err(DeviceMgrError::InvalidOperation);
                }
                cmdline.insert_str("ro").map_err(DeviceMgrError::Cmdline)?;
            }
        }

        Ok(())
    }

    /// insert a block device's config. return index on success.
    fn create(
        &mut self,
        block_device_config: BlockDeviceConfigInfo,
    ) -> std::result::Result<usize, BlockDeviceError> {
        self.check_data_file_present(&block_device_config)?;
        if self
            .get_index_of_drive_path(&block_device_config.path_on_host)
            .is_some()
        {
            return Err(BlockDeviceError::BlockDevicePathAlreadyExists(
                block_device_config.path_on_host,
            ));
        }

        // check whether the Device Config belongs to a root device
        // we need to satisfy the condition by which a VMM can only have on root device
        if block_device_config.is_root_device {
            if self.has_root_block {
                return Err(BlockDeviceError::RootBlockDeviceAlreadyAdded);
            } else {
                self.has_root_block = true;
                self.read_only_root = block_device_config.is_read_only;
                self.has_part_uuid_root = block_device_config.part_uuid.is_some();
                self.part_uuid = block_device_config.part_uuid.clone();
                // Root Device should be the first in the list whether or not PART_UUID is specified
                // in order to avoid bugs in case of switching from part_uuid boot scenarios to
                // /dev/vda boot type.
                self.info_list
                    .push_front(BlockDeviceInfo::new(block_device_config));
                Ok(0)
            }
        } else {
            self.info_list
                .push_back(BlockDeviceInfo::new(block_device_config));
            Ok(self.info_list.len() - 1)
        }
    }

    /// Updates a Block Device Config. The update fails if it would result in two
    /// root block devices.
    fn update(
        &mut self,
        mut index: usize,
        new_config: BlockDeviceConfigInfo,
    ) -> std::result::Result<(), BlockDeviceError> {
        // Check if the path exists
        self.check_data_file_present(&new_config)?;
        if let Some(idx) = self.get_index_of_drive_path(&new_config.path_on_host) {
            if idx != index {
                return Err(BlockDeviceError::BlockDevicePathAlreadyExists(
                    new_config.path_on_host.clone(),
                ));
            }
        }

        if self.info_list.get(index).is_none() {
            return Err(InvalidDeviceId(index.to_string()));
        }
        // Check if the root block device is being updated.
        if self.info_list[index].config.is_root_device {
            self.has_root_block = new_config.is_root_device;
            self.read_only_root = new_config.is_root_device && new_config.is_read_only;
            self.has_part_uuid_root = new_config.part_uuid.is_some();
            self.part_uuid = new_config.part_uuid.clone();
        } else if new_config.is_root_device {
            // Check if a second root block device is being added.
            if self.has_root_block {
                return Err(BlockDeviceError::RootBlockDeviceAlreadyAdded);
            } else {
                // One of the non-root blocks is becoming root.
                self.has_root_block = true;
                self.read_only_root = new_config.is_read_only;
                self.has_part_uuid_root = new_config.part_uuid.is_some();
                self.part_uuid = new_config.part_uuid.clone();

                // Make sure the root device is on the first position.
                self.info_list.swap(0, index);
                // Block config to be updated has moved to first position.
                index = 0;
            }
        }
        // Update the config.
        self.info_list[index].config = new_config;

        Ok(())
    }

    fn check_data_file_present(
        &self,
        block_device_config: &BlockDeviceConfigInfo,
    ) -> std::result::Result<(), BlockDeviceError> {
        if block_device_config.device_type == BlockDeviceType::RawBlock
            && !block_device_config.path_on_host.exists()
        {
            Err(BlockDeviceError::InvalidBlockDevicePath(
                block_device_config.path_on_host.clone(),
            ))
        } else {
            Ok(())
        }
    }

    fn get_index_of_drive_path(&self, drive_path: &Path) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.path_on_host.eq(drive_path))
    }

    /// update devce information in `info_list`. The caller of this method is
    /// `insert_device` when hotplug is true.
    pub fn update_device_by_index(
        &mut self,
        index: usize,
        device: Arc<DbsMmioV2Device>,
    ) -> Result<(), BlockDeviceError> {
        if let Some(info) = self.info_list.get_mut(index) {
            info.device = Some(device);
            return Ok(());
        }

        Err(BlockDeviceError::InvalidDeviceId("".to_owned()))
    }

    /// Update the ratelimiter settings of a virtio blk device.
    pub fn update_device_ratelimiters(
        device_mgr: &mut DeviceManager,
        new_cfg: BlockDeviceConfigUpdateInfo,
    ) -> std::result::Result<(), BlockDeviceError> {
        let mgr = &mut device_mgr.block_manager;
        match mgr.get_index_of_drive_id(&new_cfg.drive_id) {
            Some(index) => {
                let config = &mut mgr.info_list[index].config;
                config.rate_limiter = new_cfg.rate_limiter.clone();
                let device = mgr.info_list[index]
                    .device
                    .as_mut()
                    .ok_or_else(|| BlockDeviceError::InvalidDeviceId("".to_owned()))?;
                if let Some(mmio_dev) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
                    let guard = mmio_dev.state();
                    let inner_dev = guard.get_inner_device();
                    if let Some(blk_dev) = inner_dev
                        .as_any()
                        .downcast_ref::<virtio::block::Block<GuestAddressSpaceImpl>>()
                    {
                        return blk_dev
                            .set_patch_rate_limiters(new_cfg.bytes(), new_cfg.ops())
                            .map(|_p| ())
                            .map_err(|_e| BlockDeviceError::BlockEpollHanderSendFail);
                    }
                }
                Ok(())
            }
            None => Err(BlockDeviceError::InvalidDeviceId(new_cfg.drive_id)),
        }
    }
}

impl Default for BlockDeviceMgr {
    /// Constructor for the BlockDeviceMgr. It initializes an empty LinkedList.
    fn default() -> BlockDeviceMgr {
        BlockDeviceMgr {
            info_list: VecDeque::<BlockDeviceInfo>::new(),
            has_root_block: false,
            has_part_uuid_root: false,
            read_only_root: false,
            part_uuid: None,
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}
