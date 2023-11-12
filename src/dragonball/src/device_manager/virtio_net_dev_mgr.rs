// Copyright 2020-2022 Alibaba, Inc. or its affiliates. All Rights Reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::convert::TryInto;
use std::sync::Arc;

use dbs_utils::net::{MacAddr, Tap, TapError};
use dbs_utils::rate_limiter::BucketUpdate;
use dbs_virtio_devices as virtio;
use dbs_virtio_devices::net::Net;
use dbs_virtio_devices::Error as VirtioError;
use serde_derive::{Deserialize, Serialize};

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::config_manager::{
    ConfigItem, DeviceConfigInfo, DeviceConfigInfos, RateLimiterConfigInfo,
};
use crate::device_manager::{DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::get_bucket_update;

use super::DbsMmioV2Device;

/// Default number of virtio queues, one rx/tx pair.
pub const DEFAULT_NUM_QUEUES: usize = 2;
/// Default size of virtio queues.
pub const DEFAULT_QUEUE_SIZE: u16 = 256;
// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = true;

/// Errors associated with virtio net device operations.
#[derive(Debug, thiserror::Error)]
pub enum VirtioNetDeviceError {
    /// The virtual machine instance ID is invalid.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVMID,

    /// The iface ID is invalid.
    #[error("invalid virtio-net iface id '{0}'")]
    InvalidIfaceId(String),

    /// Invalid queue number configuration for virtio_net device.
    #[error("invalid queue number {0} for virtio-net device")]
    InvalidQueueNum(usize),

    /// Failure from device manager,
    #[error("failure in device manager operations, {0}")]
    DeviceManager(#[source] DeviceMgrError),

    /// The Context Identifier is already in use.
    #[error("the device ID {0} already exists")]
    DeviceIDAlreadyExist(String),

    /// The MAC address is already in use.
    #[error("the guest MAC address {0} is already in use")]
    GuestMacAddressInUse(String),

    /// The host device name is already in use.
    #[error("the host device name {0} is already in use")]
    HostDeviceNameInUse(String),

    /// Cannot open/create tap device.
    #[error("cannot open TAP device")]
    OpenTap(#[source] TapError),

    /// Failure from virtio subsystem.
    #[error(transparent)]
    Virtio(VirtioError),

    /// Failed to send patch message to net epoll handler.
    #[error("could not send patch message to the net epoll handler")]
    NetEpollHanderSendFail,

    /// The update is not allowed after booting the microvm.
    #[error("update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// Split this at some point.
    /// Internal errors are due to resource exhaustion.
    /// Users errors are due to invalid permissions.
    #[error("cannot create network device: {0}")]
    CreateNetDevice(#[source] VirtioError),

    /// Cannot initialize a MMIO Network Device or add a device to the MMIO Bus.
    #[error("failure while registering network device: {0}")]
    RegisterNetDevice(#[source] DeviceMgrError),
}

/// Configuration information for virtio net devices.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VirtioNetDeviceConfigUpdateInfo {
    /// ID of the guest network interface.
    pub iface_id: String,
    /// Rate Limiter for received packages.
    pub rx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// Rate Limiter for transmitted packages.
    pub tx_rate_limiter: Option<RateLimiterConfigInfo>,
}

impl VirtioNetDeviceConfigUpdateInfo {
    /// Provides a `BucketUpdate` description for the RX bandwidth rate limiter.
    pub fn rx_bytes(&self) -> BucketUpdate {
        get_bucket_update!(self, rx_rate_limiter, bandwidth)
    }
    /// Provides a `BucketUpdate` description for the RX ops rate limiter.
    pub fn rx_ops(&self) -> BucketUpdate {
        get_bucket_update!(self, rx_rate_limiter, ops)
    }
    /// Provides a `BucketUpdate` description for the TX bandwidth rate limiter.
    pub fn tx_bytes(&self) -> BucketUpdate {
        get_bucket_update!(self, tx_rate_limiter, bandwidth)
    }
    /// Provides a `BucketUpdate` description for the TX ops rate limiter.
    pub fn tx_ops(&self) -> BucketUpdate {
        get_bucket_update!(self, tx_rate_limiter, ops)
    }
}

/// Configuration information for virtio net devices.
/// TODO: https://github.com/kata-containers/kata-containers/issues/8382.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, Default)]
pub struct VirtioNetDeviceConfigInfo {
    /// ID of the guest network interface.
    pub iface_id: String,
    /// Host level path for the guest network interface.
    pub host_dev_name: String,
    /// Number of virtqueues to use.
    pub num_queues: usize,
    /// Size of each virtqueue. Unit: byte.
    pub queue_size: u16,
    /// Guest MAC address.
    pub guest_mac: Option<MacAddr>,
    /// Rate Limiter for received packages.
    pub rx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// Rate Limiter for transmitted packages.
    pub tx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// allow duplicate mac
    pub allow_duplicate_mac: bool,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl VirtioNetDeviceConfigInfo {
    /// Returns the tap device that `host_dev_name` refers to.
    pub fn open_tap(&self) -> std::result::Result<Tap, VirtioNetDeviceError> {
        Tap::open_named(self.host_dev_name.as_str(), false).map_err(VirtioNetDeviceError::OpenTap)
    }

    /// Returns a reference to the mac address. It the mac address is not configured, it
    /// return None.
    pub fn guest_mac(&self) -> Option<&MacAddr> {
        self.guest_mac.as_ref()
    }

    ///Rx and Tx queue and max queue sizes
    pub fn queue_sizes(&self) -> Vec<u16> {
        let mut queue_size = self.queue_size;
        if queue_size == 0 {
            queue_size = DEFAULT_QUEUE_SIZE;
        }
        let num_queues = if self.num_queues > 0 {
            self.num_queues
        } else {
            DEFAULT_NUM_QUEUES
        };

        (0..num_queues).map(|_| queue_size).collect::<Vec<u16>>()
    }
}

impl ConfigItem for VirtioNetDeviceConfigInfo {
    type Err = VirtioNetDeviceError;

    fn id(&self) -> &str {
        &self.iface_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), VirtioNetDeviceError> {
        if self.iface_id == other.iface_id {
            Err(VirtioNetDeviceError::DeviceIDAlreadyExist(
                self.iface_id.clone(),
            ))
        } else if !other.allow_duplicate_mac
            && self.guest_mac.is_some()
            && self.guest_mac == other.guest_mac
        {
            Err(VirtioNetDeviceError::GuestMacAddressInUse(
                self.guest_mac.as_ref().unwrap().to_string(),
            ))
        } else if self.host_dev_name == other.host_dev_name {
            Err(VirtioNetDeviceError::HostDeviceNameInUse(
                self.host_dev_name.clone(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Virtio Net Device Info
pub type VirtioNetDeviceInfo = DeviceConfigInfo<VirtioNetDeviceConfigInfo>;

/// Device manager to manage all virtio net devices.
pub struct VirtioNetDeviceMgr {
    pub(crate) info_list: DeviceConfigInfos<VirtioNetDeviceConfigInfo>,
    pub(crate) use_shared_irq: bool,
}

impl VirtioNetDeviceMgr {
    /// Gets the index of the device with the specified `drive_id` if it exists in the list.
    pub fn get_index_of_iface_id(&self, if_id: &str) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.iface_id.eq(if_id))
    }

    /// Insert or update a virtio net device into the manager.
    pub fn insert_device(
        &mut self,
        mut ctx: DeviceOpContext,
        config: VirtioNetDeviceConfigInfo,
    ) -> std::result::Result<(), VirtioNetDeviceError> {
        if config.num_queues % 2 != 0 {
            return Err(VirtioNetDeviceError::InvalidQueueNum(config.num_queues));
        }
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            return Err(VirtioNetDeviceError::UpdateNotAllowedPostBoot);
        }

        slog::info!(
            ctx.logger(),
            "add virtio-net device configuration";
            "subsystem" => "net_dev_mgr",
            "id" => &config.iface_id,
            "host_dev_name" => &config.host_dev_name,
        );

        let device_index = self.info_list.insert_or_update(&config)?;

        if ctx.is_hotplug {
            slog::info!(
                ctx.logger(),
                "attach virtio-net device";
                "subsystem" => "net_dev_mgr",
                "id" => &config.iface_id,
                "host_dev_name" => &config.host_dev_name,
            );

            match Self::create_device(&config, &mut ctx) {
                Ok(device) => {
                    let dev = DeviceManager::create_mmio_virtio_device(
                        device,
                        &mut ctx,
                        config.use_shared_irq.unwrap_or(self.use_shared_irq),
                        config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                    )
                    .map_err(VirtioNetDeviceError::DeviceManager)?;
                    ctx.insert_hotplug_mmio_device(&dev, None)
                        .map_err(VirtioNetDeviceError::DeviceManager)?;
                    // live-upgrade need save/restore device from info.device.
                    self.info_list[device_index].set_device(dev);
                }
                Err(e) => {
                    self.info_list.remove(device_index);
                    return Err(VirtioNetDeviceError::Virtio(e));
                }
            }
        }

        Ok(())
    }

    /// Update the ratelimiter settings of a virtio net device.
    pub fn update_device_ratelimiters(
        &mut self,
        new_cfg: VirtioNetDeviceConfigUpdateInfo,
    ) -> std::result::Result<(), VirtioNetDeviceError> {
        match self.get_index_of_iface_id(&new_cfg.iface_id) {
            Some(index) => {
                let config = &mut self.info_list[index].config;
                config.rx_rate_limiter = new_cfg.rx_rate_limiter.clone();
                config.tx_rate_limiter = new_cfg.tx_rate_limiter.clone();
                let device = self.info_list[index].device.as_mut().ok_or_else(|| {
                    VirtioNetDeviceError::InvalidIfaceId(new_cfg.iface_id.clone())
                })?;

                if let Some(mmio_dev) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
                    let guard = mmio_dev.state();
                    let inner_dev = guard.get_inner_device();
                    if let Some(net_dev) = inner_dev
                        .as_any()
                        .downcast_ref::<virtio::net::Net<GuestAddressSpaceImpl>>()
                    {
                        return net_dev
                            .set_patch_rate_limiters(
                                new_cfg.rx_bytes(),
                                new_cfg.rx_ops(),
                                new_cfg.tx_bytes(),
                                new_cfg.tx_ops(),
                            )
                            .map(|_p| ())
                            .map_err(|_e| VirtioNetDeviceError::NetEpollHanderSendFail);
                    }
                }
                Ok(())
            }
            None => Err(VirtioNetDeviceError::InvalidIfaceId(
                new_cfg.iface_id.clone(),
            )),
        }
    }

    /// Attach all configured net device to the virtual machine instance.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), VirtioNetDeviceError> {
        for info in self.info_list.iter_mut() {
            slog::info!(
                ctx.logger(),
                "attach virtio-net device";
                "subsystem" => "net_dev_mgr",
                "id" => &info.config.iface_id,
                "host_dev_name" => &info.config.host_dev_name,
            );

            let device = Self::create_device(&info.config, ctx)
                .map_err(VirtioNetDeviceError::CreateNetDevice)?;
            let device = DeviceManager::create_mmio_virtio_device(
                device,
                ctx,
                info.config.use_shared_irq.unwrap_or(self.use_shared_irq),
                info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
            )
            .map_err(VirtioNetDeviceError::RegisterNetDevice)?;
            info.set_device(device);
        }

        Ok(())
    }

    fn create_device(
        cfg: &VirtioNetDeviceConfigInfo,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<Box<Net<GuestAddressSpaceImpl>>, virtio::Error> {
        let epoll_mgr = ctx.epoll_mgr.clone().ok_or(virtio::Error::InvalidInput)?;
        let rx_rate_limiter = match cfg.rx_rate_limiter.as_ref() {
            Some(rl) => Some(rl.try_into().map_err(virtio::Error::IOError)?),
            None => None,
        };
        let tx_rate_limiter = match cfg.tx_rate_limiter.as_ref() {
            Some(rl) => Some(rl.try_into().map_err(virtio::Error::IOError)?),
            None => None,
        };

        let net_device = Net::new(
            cfg.host_dev_name.clone(),
            cfg.guest_mac(),
            Arc::new(cfg.queue_sizes()),
            epoll_mgr,
            rx_rate_limiter,
            tx_rate_limiter,
        )?;

        Ok(Box::new(net_device))
    }

    /// Remove all virtio-net devices.
    pub fn remove_devices(&mut self, ctx: &mut DeviceOpContext) -> Result<(), DeviceMgrError> {
        while let Some(mut info) = self.info_list.pop() {
            slog::info!(
                ctx.logger(),
                "remove virtio-net device: {}",
                info.config.iface_id
            );
            if let Some(device) = info.device.take() {
                DeviceManager::destroy_mmio_virtio_device(device, ctx)?;
            }
        }
        Ok(())
    }
}

impl Default for VirtioNetDeviceMgr {
    /// Create a new virtio net device manager.
    fn default() -> Self {
        VirtioNetDeviceMgr {
            info_list: DeviceConfigInfos::new(),
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}
