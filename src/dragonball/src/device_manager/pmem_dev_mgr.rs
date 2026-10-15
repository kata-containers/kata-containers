// Copyright 2026 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fs::OpenOptions;
use std::io;
use std::ops::Range;

use dbs_virtio_devices::{self as virtio, pmem::Pmem};
use serde_derive::{Deserialize, Serialize};
use slog::{error, info};

use crate::address_space_manager::GuestAddressSpaceImpl;
#[cfg(target_arch = "x86_64")]
use crate::api::v1::ConfidentialVmType;
use crate::config_manager::{ConfigItem, DeviceConfigInfos};
use crate::device_manager::{
    DeviceManager, DeviceMgrError, DeviceOpContext, DeviceVirtioRegionHandler,
};

const USE_SHARED_IRQ: bool = true;
const USE_GENERIC_IRQ: bool = false;

/// Errors associated with virtio-pmem configuration.
#[derive(Debug, thiserror::Error)]
pub enum PmemDeviceError {
    /// The device ID is already configured.
    #[error("the virtio-pmem ID was already added to a different device")]
    DeviceAlreadyExists,
    /// The device cannot be added after boot.
    #[error("the update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,
    /// The backing file cannot be opened.
    #[error("failed to open virtio-pmem backing file: {0}")]
    OpenBackingFile(#[source] io::Error),
    /// The virtio-pmem device cannot be created.
    #[error("failed to create virtio-pmem device: {0}")]
    CreateDevice(#[source] virtio::Error),
    /// The device cannot be registered.
    #[error("failed to register virtio-pmem device: {0}")]
    RegisterDevice(#[source] DeviceMgrError),
    /// Required VM state is unavailable.
    #[error("device manager error: {0}")]
    DeviceManager(#[source] DeviceMgrError),
}

/// Configuration for a virtio-pmem device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PmemDeviceConfigInfo {
    /// Unique device identifier.
    pub id: String,
    /// Host backing file path.
    pub path: String,
    /// Optional byte range within the backing file.
    pub file_range: Option<Range<u64>>,
    /// Expose the device read-only.
    pub read_only: bool,
    /// Use a shared interrupt.
    pub use_shared_irq: Option<bool>,
    /// Use generic interrupts.
    pub use_generic_irq: Option<bool>,
}

impl ConfigItem for PmemDeviceConfigInfo {
    type Err = PmemDeviceError;

    fn id(&self) -> &str {
        &self.id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), Self::Err> {
        if self.id == other.id {
            Err(PmemDeviceError::DeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Stores virtio-pmem device configurations.
#[derive(Clone, Default)]
pub struct PmemDeviceMgr {
    info_list: DeviceConfigInfos<PmemDeviceConfigInfo>,
}

impl PmemDeviceMgr {
    /// Adds a cold-plug virtio-pmem configuration.
    pub fn insert_or_update_device(
        &mut self,
        ctx: DeviceOpContext,
        config: PmemDeviceConfigInfo,
    ) -> Result<(), PmemDeviceError> {
        if ctx.is_hotplug {
            error!(ctx.logger(), "virtio-pmem does not support hotplug";
                "subsystem" => "pmem_dev_mgr",
                "id" => &config.id,
            );
            return Err(PmemDeviceError::UpdateNotAllowedPostBoot);
        }

        info!(ctx.logger(), "add virtio-pmem device configuration";
            "subsystem" => "pmem_dev_mgr",
            "id" => &config.id,
        );
        self.info_list.insert_or_update(&config)?;
        Ok(())
    }

    /// Attaches all configured virtio-pmem devices.
    pub fn attach_devices(&mut self, ctx: &mut DeviceOpContext) -> Result<(), PmemDeviceError> {
        let epoll_mgr = ctx
            .get_epoll_mgr()
            .map_err(PmemDeviceError::DeviceManager)?;
        let vm_as = ctx.get_vm_as().map_err(PmemDeviceError::DeviceManager)?;
        let address_space = ctx
            .get_address_space()
            .map_err(PmemDeviceError::DeviceManager)?;
        #[cfg(not(target_arch = "x86_64"))]
        let f_access_platform = false;
        #[cfg(target_arch = "x86_64")]
        let f_access_platform = ctx.get_confidential_vm_type() == Some(ConfidentialVmType::TDX);

        for info in self.info_list.iter_mut() {
            let mut options = OpenOptions::new();
            options.read(true).write(!info.config.read_only);
            let file = options
                .open(&info.config.path)
                .map_err(PmemDeviceError::OpenBackingFile)?;
            let handler = DeviceVirtioRegionHandler {
                vm_as: vm_as.clone(),
                address_space: address_space.clone(),
            };
            let device = Pmem::<GuestAddressSpaceImpl>::new(
                info.config.id.clone(),
                file,
                info.config
                    .file_range
                    .as_ref()
                    .map(|range| std::range::Range {
                        start: range.start,
                        end: range.end,
                    }),
                info.config.read_only,
                Box::new(handler),
                epoll_mgr.clone(),
                f_access_platform,
            )
            .map_err(PmemDeviceError::CreateDevice)?;
            let mmio_dev = DeviceManager::create_mmio_virtio_device(
                Box::new(device),
                ctx,
                info.config.use_shared_irq.unwrap_or(USE_SHARED_IRQ),
                info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
            )
            .map_err(PmemDeviceError::RegisterDevice)?;
            info.set_device(mmio_dev);
        }
        Ok(())
    }
}
