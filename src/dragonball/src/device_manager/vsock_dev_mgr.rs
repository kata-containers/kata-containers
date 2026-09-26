// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::collections::HashSet;
use std::sync::Arc;

use dbs_virtio_devices as virtio;
use dbs_virtio_devices::mmio::DRAGONBALL_FEATURE_INTR_USED;
use dbs_virtio_devices::vsock::backend::{
    VsockInnerBackend, VsockInnerConnector, VsockTcpBackend, VsockUnixStreamBackend,
};
use dbs_virtio_devices::vsock::{Vsock, VsockState};
use dbs_virtio_devices::Error as VirtioError;
use serde_derive::{Deserialize, Serialize};

use super::{persist, DeviceMgrError, StartMicroVmError};
use crate::address_space_manager::GuestAddressSpaceImpl;
#[cfg(target_arch = "x86_64")]
use crate::api::v1::ConfidentialVmType;
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

    /// Virtio device operation error.
    #[error("virtio device operation error: {0}")]
    Virtio(#[source] VirtioError),

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

        #[cfg(not(target_arch = "x86_64"))]
        let f_access_platform = false;
        #[cfg(target_arch = "x86_64")]
        let f_access_platform = ctx.get_confidential_vm_type() == Some(ConfidentialVmType::TDX);

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
                    f_access_platform,
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
                DeviceManager::destroy_mmio_device(device, ctx)?;
            }
        }
        Ok(())
    }
}

impl<'a> dbs_snapshot::Persist<'a> for VsockDeviceMgr {
    type State = VsockDeviceMgrState;
    type SaveArgs = ();
    type RestoreArgs = ();
    type Error = VsockDeviceError;

    /// Capture the state of all vsock devices.
    ///
    /// The virtual machine must be paused when this is called. The host half
    /// of a live vsock connection cannot be captured, so each device records
    /// the identity of its live connections instead; restore resets them.
    fn save_state(&mut self, _args: ()) -> std::result::Result<Self::State, Self::Error> {
        let mut devices = Vec::new();
        for info in self.info_list.iter() {
            let device = info
                .device
                .as_ref()
                .ok_or(VsockDeviceError::Virtio(VirtioError::InvalidInput))?;
            let (device_info, transport) =
                persist::save_device_state::<Vsock<GuestAddressSpaceImpl>>(device, ())
                    .map_err(VsockDeviceError::Virtio)?;
            devices.push(persist::VirtioDevState {
                config: info.config.clone(),
                device_info,
                transport,
            });
        }
        Ok(VsockDeviceMgrState { devices })
    }

    /// Restore the runtime state of all vsock devices.
    ///
    /// The devices must have been re-created from the same configuration
    /// (matched by id) and must not have been activated yet. Must be called
    /// before the guest vCPUs resume.
    ///
    /// The state is validated in full before any of it is applied, so that a
    /// malformed snapshot is refused rather than half-restored.
    fn restore_state(
        &mut self,
        state: &Self::State,
        _args: (),
    ) -> std::result::Result<(), Self::Error> {
        // A repeated device id would restore two sets of connection resets
        // onto the same device and leave another device with none. This is a
        // property of the state alone, so check it before looking at the VM.
        let mut seen_ids = HashSet::with_capacity(state.devices.len());
        for dev_state in &state.devices {
            if !seen_ids.insert(dev_state.config.id()) {
                return Err(VsockDeviceError::DeviceIDAlreadyExist(
                    dev_state.config.id().to_string(),
                ));
            }
        }

        // Saving records every configured device, so a state naming fewer is
        // not one this VM produced. Restoring it anyway would silently leave
        // the missing device un-restored, and with no resets for the stale
        // guest sockets that restored RAM still holds.
        if state.devices.len() != self.info_list.len() {
            return Err(VsockDeviceError::Virtio(VirtioError::InvalidInput));
        }

        // Every device in the state must match a configured device: the
        // resets it carries belong to that device's guest sockets, and have
        // nowhere else to go. With the count equal and no id repeated, this
        // makes the two sets identical.
        for dev_state in &state.devices {
            if !self
                .info_list
                .iter()
                .any(|info| info.config.id() == dev_state.config.id())
            {
                return Err(VsockDeviceError::Virtio(VirtioError::InvalidInput));
            }
        }

        for dev_state in &state.devices {
            let info = self
                .info_list
                .iter()
                .find(|info| info.config.id() == dev_state.config.id())
                .ok_or(VsockDeviceError::Virtio(VirtioError::InvalidInput))?;
            let device = info
                .device
                .as_ref()
                .ok_or(VsockDeviceError::Virtio(VirtioError::InvalidInput))?;
            persist::restore_device_state::<Vsock<GuestAddressSpaceImpl>>(
                device,
                &dev_state.device_info,
                &dev_state.transport,
                (),
            )
            .map_err(VsockDeviceError::Virtio)?;
        }
        Ok(())
    }
}

/// Snapshot state of one vsock device (config + device state + transport
/// state); see [`persist::VirtioDevState`].
///
/// The device state is [`VsockState`] rather than the default
/// `VirtioDeviceInfoState`: besides the guest-negotiated device state it
/// carries the connections restore has to reset.
pub type VsockDevState = persist::VirtioDevState<VsockDeviceConfigInfo, VsockState>;

/// Snapshot state of the vsock device manager.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct VsockDeviceMgrState {
    /// Per-device state, in insertion order.
    pub devices: Vec<VsockDevState>,
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

#[cfg(test)]
mod tests {
    use dbs_snapshot::Persist;
    use dbs_virtio_devices::persist::MmioV2TransportState;

    use super::*;
    use crate::device_manager::persist::VirtioTransportState;

    fn dev_state(id: &str) -> VsockDevState {
        VsockDevState {
            config: VsockDeviceConfigInfo {
                id: id.to_string(),
                guest_cid: 3,
                ..Default::default()
            },
            device_info: VsockState::default(),
            transport: VirtioTransportState::Mmio(MmioV2TransportState::default()),
        }
    }

    #[test]
    fn test_restore_state_rejects_duplicate_device_id() {
        // A repeated id would restore two sets of connection resets onto the
        // same device, leaving another with none.
        let mut mgr = VsockDeviceMgr::default();
        let state = VsockDeviceMgrState {
            devices: vec![dev_state("vsock0"), dev_state("vsock0")],
        };

        assert!(matches!(
            mgr.restore_state(&state, ()),
            Err(VsockDeviceError::DeviceIDAlreadyExist(id)) if id == "vsock0"
        ));
    }

    #[test]
    fn test_restore_state_rejects_missing_device() {
        // Saving records every configured device, so a state naming fewer is
        // not one this VM produced. Accepting it would leave the missing
        // device un-restored and its stale guest sockets un-reset.
        let mut mgr = VsockDeviceMgr::default();
        mgr.info_list
            .insert_or_update(&VsockDeviceConfigInfo {
                id: "vsock0".to_string(),
                guest_cid: 3,
                ..Default::default()
            })
            .unwrap();
        mgr.info_list
            .insert_or_update(&VsockDeviceConfigInfo {
                id: "vsock1".to_string(),
                guest_cid: 4,
                ..Default::default()
            })
            .unwrap();

        let state = VsockDeviceMgrState {
            devices: vec![dev_state("vsock0")],
        };
        assert!(matches!(
            mgr.restore_state(&state, ()),
            Err(VsockDeviceError::Virtio(VirtioError::InvalidInput))
        ));
    }

    #[test]
    fn test_restore_state_rejects_unknown_device() {
        // The VM must have been re-created from the configuration the
        // snapshot was taken with; a device the state names but the VM does
        // not have has nowhere to put its resets.
        let mut mgr = VsockDeviceMgr::default();
        let state = VsockDeviceMgrState {
            devices: vec![dev_state("vsock0")],
        };

        assert!(matches!(
            mgr.restore_state(&state, ()),
            Err(VsockDeviceError::Virtio(VirtioError::InvalidInput))
        ));
    }

    #[test]
    fn test_restore_state_without_devices_is_a_no_op() {
        let mut mgr = VsockDeviceMgr::default();
        assert!(mgr
            .restore_state(&VsockDeviceMgrState::default(), ())
            .is_ok());
    }

    #[test]
    fn test_device_mgr_state_json_roundtrip() {
        let mut state = dev_state("vsock0");
        state.device_info.reset_connections = vec![dbs_virtio_devices::vsock::VsockConnectionId {
            local_port: 1024,
            peer_port: 7,
        }];
        let state = VsockDeviceMgrState {
            devices: vec![state],
        };

        let json = serde_json::to_string(&state).unwrap();
        let loaded: VsockDeviceMgrState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(
            loaded.devices[0].device_info.reset_connections,
            state.devices[0].device_info.reset_connections
        );
    }
}
