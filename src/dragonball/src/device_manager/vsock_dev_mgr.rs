// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::sync::Arc;

use dbs_virtio_devices as virtio;
use dbs_virtio_devices::mmio::DRAGONBALL_FEATURE_INTR_USED;
use dbs_virtio_devices::vsock::backend::{
    VsockInnerBackend, VsockInnerConnector, VsockTcpBackend, VsockUnixStreamBackend,
};
use dbs_virtio_devices::vsock::Vsock;
use dbs_virtio_devices::Error as VirtioError;
use serde_derive::{Deserialize, Serialize};

use super::{DeviceMgrError, StartMicroVmError};
use crate::config_manager::{ConfigItem, DeviceConfigInfo, DeviceConfigInfos};
use crate::device_manager::{DeviceManager, DeviceOpContext};

pub use dbs_virtio_devices::vsock::QUEUE_SIZES;

const SUBSYSTEM: &str = "vsock_dev_mgr";
// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = true;

/// Errors associated with `VsockDeviceConfigInfo`.
#[derive(Debug, thiserror::Error)]
pub enum VsockDeviceError {
    /// The virtual machine instance ID is invalid.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVMID,

    /// The Context Identifier is already in use.
    #[error("the device ID {0} already exists")]
    DeviceIDAlreadyExist(String),

    /// The Context Identifier is invalid.
    #[error("the guest CID {0} is invalid")]
    GuestCIDInvalid(u32),

    /// The Context Identifier is already in use.
    #[error("the guest CID {0} is already in use")]
    GuestCIDAlreadyInUse(u32),

    /// The Unix Domain Socket path is already in use.
    #[error("the Unix Domain Socket path {0} is already in use")]
    UDSPathAlreadyInUse(String),

    /// The net address is already in use.
    #[error("the net address {0} is already in use")]
    NetAddrAlreadyInUse(String),

    /// The update is not allowed after booting the microvm.
    #[error("update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// The VsockId Already Exists
    #[error("vsock id {0} already exists")]
    VsockIdAlreadyExists(String),

    /// Inner backend create error
    #[error("vsock inner backend create error: {0}")]
    CreateInnerBackend(#[source] std::io::Error),
}

/// Configuration information for a vsock device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VsockDeviceConfigInfo {
    /// ID of the vsock device.
    pub id: String,
    /// A 32-bit Context Identifier (CID) used to identify the guest.
    pub guest_cid: u32,
    /// unix domain socket path.
    pub uds_path: Option<String>,
    /// tcp socket address.
    pub tcp_addr: Option<String>,
    /// Virtio queue size.
    pub queue_size: Vec<u16>,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl Default for VsockDeviceConfigInfo {
    fn default() -> Self {
        Self {
            id: String::default(),
            guest_cid: 0,
            uds_path: None,
            tcp_addr: None,
            queue_size: Vec::from(QUEUE_SIZES),
            use_shared_irq: None,
            use_generic_irq: None,
        }
    }
}

impl VsockDeviceConfigInfo {
    /// Get number and size of queues supported.
    pub fn queue_sizes(&self) -> Vec<u16> {
        self.queue_size.clone()
    }
}

impl ConfigItem for VsockDeviceConfigInfo {
    type Err = VsockDeviceError;

    fn id(&self) -> &str {
        &self.id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), VsockDeviceError> {
        if self.id == other.id {
            return Err(VsockDeviceError::DeviceIDAlreadyExist(self.id.clone()));
        }
        if self.guest_cid == other.guest_cid {
            return Err(VsockDeviceError::GuestCIDAlreadyInUse(self.guest_cid));
        }
        if let (Some(self_uds_path), Some(other_uds_path)) =
            (self.uds_path.as_ref(), other.uds_path.as_ref())
        {
            if self_uds_path == other_uds_path {
                return Err(VsockDeviceError::UDSPathAlreadyInUse(self_uds_path.clone()));
            }
        }
        if let (Some(self_net_addr), Some(other_net_addr)) =
            (self.tcp_addr.as_ref(), other.tcp_addr.as_ref())
        {
            if self_net_addr == other_net_addr {
                return Err(VsockDeviceError::NetAddrAlreadyInUse(self_net_addr.clone()));
            }
        }

        Ok(())
    }
}

/// Vsock Device Info
pub type VsockDeviceInfo = DeviceConfigInfo<VsockDeviceConfigInfo>;

/// Device manager to manage all vsock devices.
pub struct VsockDeviceMgr {
    pub(crate) info_list: DeviceConfigInfos<VsockDeviceConfigInfo>,
    pub(crate) default_inner_backend: Option<VsockInnerBackend>,
    pub(crate) default_inner_connector: Option<VsockInnerConnector>,
    pub(crate) use_shared_irq: bool,
}

impl VsockDeviceMgr {
    /// Insert or update a vsock device into the manager.
    pub fn insert_device(
        &mut self,
        ctx: DeviceOpContext,
        config: VsockDeviceConfigInfo,
    ) -> std::result::Result<(), VsockDeviceError> {
        if ctx.is_hotplug {
            slog::error!(
                ctx.logger(),
                "no support of virtio-vsock device hotplug";
                "subsystem" => SUBSYSTEM,
                "id" => &config.id,
                "uds_path" => &config.uds_path,
            );

            return Err(VsockDeviceError::UpdateNotAllowedPostBoot);
        }

        // VMADDR_CID_ANY (-1U) means any address for binding;
        // VMADDR_CID_HYPERVISOR (0) is reserved for services built into the hypervisor;
        // VMADDR_CID_RESERVED (1) must not be used;
        // VMADDR_CID_HOST (2) is the well-known address of the host.
        if config.guest_cid <= 2 {
            return Err(VsockDeviceError::GuestCIDInvalid(config.guest_cid));
        }

        slog::info!(
            ctx.logger(),
            "add virtio-vsock device configuration";
            "subsystem" => SUBSYSTEM,
            "id" => &config.id,
            "uds_path" => &config.uds_path,
        );

        self.lazy_make_default_connector()?;

        self.info_list.insert_or_update(&config)?;

        Ok(())
    }

    /// Attach all configured vsock device to the virtual machine instance.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), StartMicroVmError> {
        let epoll_mgr = ctx
            .epoll_mgr
            .clone()
            .ok_or(StartMicroVmError::CreateVsockDevice(
                virtio::Error::InvalidInput,
            ))?;

        for info in self.info_list.iter_mut() {
            slog::info!(
                ctx.logger(),
                "attach virtio-vsock device";
                "subsystem" => SUBSYSTEM,
                "id" => &info.config.id,
                "uds_path" => &info.config.uds_path,
            );

            let mut device = Box::new(
                Vsock::new(
                    info.config.guest_cid as u64,
                    Arc::new(info.config.queue_sizes()),
                    epoll_mgr.clone(),
                )
                .map_err(VirtioError::VirtioVsockError)
                .map_err(StartMicroVmError::CreateVsockDevice)?,
            );
            if let Some(uds_path) = info.config.uds_path.as_ref() {
                let unix_backend = VsockUnixStreamBackend::new(uds_path.clone())
                    .map_err(VirtioError::VirtioVsockError)
                    .map_err(StartMicroVmError::CreateVsockDevice)?;
                device
                    .add_backend(Box::new(unix_backend), true)
                    .map_err(VirtioError::VirtioVsockError)
                    .map_err(StartMicroVmError::CreateVsockDevice)?;
            }
            if let Some(tcp_addr) = info.config.tcp_addr.as_ref() {
                let tcp_backend = VsockTcpBackend::new(tcp_addr.clone())
                    .map_err(VirtioError::VirtioVsockError)
                    .map_err(StartMicroVmError::CreateVsockDevice)?;
                device
                    .add_backend(Box::new(tcp_backend), false)
                    .map_err(VirtioError::VirtioVsockError)
                    .map_err(StartMicroVmError::CreateVsockDevice)?;
            }
            // add inner backend to the the first added vsock device
            if let Some(inner_backend) = self.default_inner_backend.take() {
                device
                    .add_backend(Box::new(inner_backend), false)
                    .map_err(VirtioError::VirtioVsockError)
                    .map_err(StartMicroVmError::CreateVsockDevice)?;
            }
            let device = DeviceManager::create_mmio_virtio_device_with_features(
                device,
                ctx,
                Some(DRAGONBALL_FEATURE_INTR_USED),
                info.config.use_shared_irq.unwrap_or(self.use_shared_irq),
                info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
            )
            .map_err(StartMicroVmError::RegisterVsockDevice)?;
            info.device = Some(device);
        }

        Ok(())
    }

    // check the default connector is present, or build it.
    fn lazy_make_default_connector(&mut self) -> std::result::Result<(), VsockDeviceError> {
        if self.default_inner_connector.is_none() {
            let inner_backend =
                VsockInnerBackend::new().map_err(VsockDeviceError::CreateInnerBackend)?;
            self.default_inner_connector = Some(inner_backend.get_connector());
            self.default_inner_backend = Some(inner_backend);
        }
        Ok(())
    }

    /// Get the default vsock inner connector.
    pub fn get_default_connector(
        &mut self,
    ) -> std::result::Result<VsockInnerConnector, VsockDeviceError> {
        self.lazy_make_default_connector()?;

        // safe to unwrap, because we created the inner connector before
        Ok(self.default_inner_connector.clone().unwrap())
    }

    /// Remove all virtio-vsock devices
    pub fn remove_devices(&mut self, ctx: &mut DeviceOpContext) -> Result<(), DeviceMgrError> {
        while let Some(mut info) = self.info_list.pop() {
            slog::info!(
                ctx.logger(),
                "remove virtio-vsock device: {}",
                info.config.id
            );
            if let Some(device) = info.device.take() {
                DeviceManager::destroy_mmio_virtio_device(device, ctx)?;
            }
        }
        Ok(())
    }
}

impl Default for VsockDeviceMgr {
    /// Create a new Vsock device manager.
    fn default() -> Self {
        VsockDeviceMgr {
            info_list: DeviceConfigInfos::new(),
            default_inner_backend: None,
            default_inner_connector: None,
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}
