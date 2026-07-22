// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2026 Ant Group. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Device manager for all network device backends.
//!
//! virtio-net, vhost-net and vhost-user-net are all network interfaces of the
//! guest and are configured through the same API request, so they share one
//! manager and one `info_list`, and the backend is selected per device by
//! [`Backend`]. This mirrors how the block manager keeps its backends in one
//! list keyed by `BlockDeviceType`.

use std::convert::TryInto;
use std::sync::Arc;

use dbs_utils::net::MacAddr;
use dbs_utils::rate_limiter::BucketUpdate;
use dbs_virtio_devices as virtio;
use dbs_virtio_devices::Error as VirtioError;
use serde::{Deserialize, Serialize};

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::config_manager::{ConfigItem, DeviceConfigInfos, RateLimiterConfigInfo};
use crate::device_manager::{DbsVirtioDevice, DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::get_bucket_update;

use super::DbsMmioV2Device;

/// Default number of virtio queues, one rx/tx pair.
pub const DEFAULT_NUM_QUEUES: usize = 2;
/// Default size of virtio queues.
pub const DEFAULT_QUEUE_SIZE: u16 = 256;
// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq, for the in-VMM backends.
const USE_GENERIC_IRQ: bool = true;
// vhost-user-net does not use the generic irq.
#[cfg(feature = "vhost-user-net")]
const VHOST_USER_USE_GENERIC_IRQ: bool = false;

/// Errors associated with network device operations.
#[derive(Debug, thiserror::Error)]
pub enum NetworkDeviceError {
    /// The virtual machine instance ID is invalid.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVMID,

    /// The iface ID is invalid.
    #[error("invalid network iface id '{0}'")]
    InvalidIfaceId(String),

    /// No iface ID was supplied for the device.
    #[error("no iface id supplied for the network device")]
    MissingIfaceId,

    /// Invalid queue number configuration for the device.
    #[error("invalid queue number {0} for network device")]
    InvalidQueueNum(usize),

    /// Failure from device manager.
    #[error("failure in device manager operations, {0}")]
    DeviceManager(#[source] DeviceMgrError),

    /// The device ID is already in use.
    #[error("the device ID {0} already exists")]
    DeviceIDAlreadyExist(String),

    /// The MAC address is already in use.
    #[error("the guest MAC address {0} is already in use")]
    GuestMacAddressInUse(String),

    /// The host device name is already in use.
    #[error("the host device name {0} is already in use")]
    HostDeviceNameInUse(String),

    /// Duplicated Unix domain socket path for a vhost-user-net device.
    #[error("duplicated Unix domain socket path {0} for vhost-user-net device")]
    DuplicatedUdsPath(String),

    /// Failure from virtio subsystem.
    #[error(transparent)]
    Virtio(VirtioError),

    /// Failed to send patch message to the net epoll handler.
    #[error("could not send patch message to the net epoll handler")]
    NetEpollHandlerSendFail,

    /// The update is not allowed after booting the microvm.
    #[error("update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// The requested operation is not supported by this backend.
    #[error("network device '{iface_id}': operation not supported by the {backend} backend")]
    UnsupportedBackend {
        /// Identifier of the device.
        iface_id: String,
        /// Backend that does not support the operation.
        backend: &'static str,
    },

    /// Split this at some point.
    /// Internal errors are due to resource exhaustion.
    /// Users errors are due to invalid permissions.
    #[error("cannot create network device: {0}")]
    CreateNetDevice(#[source] VirtioError),

    /// Cannot initialize a MMIO Network Device or add a device to the MMIO Bus.
    #[error("failure while registering network device: {0}")]
    RegisterNetDevice(#[source] DeviceMgrError),
}

/// Virtio network config, working for virtio-net and vhost-net.
#[cfg(any(feature = "virtio-net", feature = "vhost-net"))]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct VirtioConfig {
    /// ID of the guest network interface.
    pub iface_id: String,
    /// Host level path for the guest network interface.
    pub host_dev_name: String,
    /// Rate Limiter for received packages.
    pub rx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// Rate Limiter for transmitted packages.
    pub tx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// Allow duplicate mac
    pub allow_duplicate_mac: bool,
}

/// Config for a vhost-user-net device.
#[cfg(feature = "vhost-user-net")]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct VhostUserConfig {
    /// ID of the guest network interface.
    #[serde(default)]
    pub iface_id: String,
    /// Vhost-user socket path.
    pub sock_path: String,
}

/// An enum to specify a backend of a network device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Backend {
    #[serde(rename = "virtio")]
    #[cfg(feature = "virtio-net")]
    /// Virtio-net
    Virtio(VirtioConfig),
    #[serde(rename = "vhost")]
    #[cfg(feature = "vhost-net")]
    /// Vhost-net
    Vhost(VirtioConfig),
    #[serde(rename = "vhost-user")]
    #[cfg(feature = "vhost-user-net")]
    /// Vhost-user-net
    VhostUser(VhostUserConfig),
}

impl Backend {
    /// Name of the backend, for diagnostics.
    pub fn name(&self) -> &'static str {
        #[allow(unreachable_patterns)]
        match self {
            #[cfg(feature = "virtio-net")]
            Backend::Virtio(_) => "virtio-net",
            #[cfg(feature = "vhost-net")]
            Backend::Vhost(_) => "vhost-net",
            #[cfg(feature = "vhost-user-net")]
            Backend::VhostUser(_) => "vhost-user-net",
            _ => "unknown",
        }
    }
}

impl Default for Backend {
    #[allow(unreachable_code)]
    fn default() -> Self {
        #[cfg(feature = "virtio-net")]
        return Self::Virtio(VirtioConfig::default());
        #[cfg(feature = "vhost-net")]
        return Self::Vhost(VirtioConfig::default());

        panic!("no available default network backend")
    }
}

/// This struct represents the strongly typed equivalent of the json body from
/// net iface related requests, and is the configuration the manager stores for
/// every network device regardless of its backend.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NetworkInterfaceConfig {
    /// Number of virtqueue pairs to use. (https://www.linux-kvm.org/page/Multiqueue)
    pub num_queues: Option<usize>,
    /// Size of each virtqueue.
    pub queue_size: Option<u16>,
    /// Net backend driver.
    #[serde(default = "Backend::default")]
    pub backend: Backend,
    /// mac of the interface.
    pub guest_mac: Option<MacAddr>,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl NetworkInterfaceConfig {
    /// Returns a reference to the mac address. If the mac address is not
    /// configured, it returns None.
    pub fn guest_mac(&self) -> Option<&MacAddr> {
        self.guest_mac.as_ref()
    }

    /// Number of virtqueues to use.
    ///
    /// The virtio-net backend has always ignored the requested queue count and
    /// used the default; that behaviour is preserved here rather than changed
    /// as a side effect of merging the managers.
    pub fn num_queues(&self) -> usize {
        #[cfg(feature = "virtio-net")]
        if matches!(self.backend, Backend::Virtio(_)) {
            return DEFAULT_NUM_QUEUES;
        }

        match self.num_queues {
            Some(0) | None => DEFAULT_NUM_QUEUES,
            Some(num_queues) => num_queues,
        }
    }

    /// Size of each virtqueue.
    pub fn queue_size(&self) -> u16 {
        match self.queue_size {
            Some(0) | None => DEFAULT_QUEUE_SIZE,
            Some(queue_size) => queue_size,
        }
    }

    /// Rx and Tx queue and max queue sizes.
    pub fn queue_sizes(&self) -> Vec<u16> {
        let queue_size = self.queue_size();
        (0..self.num_queues()).map(|_| queue_size).collect()
    }

    /// Whether the device uses the generic irq, defaulting per backend.
    fn use_generic_irq(&self) -> bool {
        #[allow(unused_mut, unused_assignments)]
        let mut default = USE_GENERIC_IRQ;
        #[cfg(feature = "vhost-user-net")]
        if matches!(self.backend, Backend::VhostUser(_)) {
            default = VHOST_USER_USE_GENERIC_IRQ;
        }
        self.use_generic_irq.unwrap_or(default)
    }

    /// Host level path for the guest network interface, for the backends that
    /// are backed by a tap device. `None` for backends that are not.
    fn host_dev_name(&self) -> Option<&str> {
        #[allow(unreachable_patterns)]
        match &self.backend {
            #[cfg(feature = "virtio-net")]
            Backend::Virtio(config) => Some(&config.host_dev_name),
            #[cfg(feature = "vhost-net")]
            Backend::Vhost(config) => Some(&config.host_dev_name),
            _ => None,
        }
    }

    /// Vhost-user socket path, for the backends that have one. `None` for
    /// backends that are not vhost-user.
    fn sock_path(&self) -> Option<&str> {
        #[allow(unreachable_patterns)]
        match &self.backend {
            #[cfg(feature = "vhost-user-net")]
            Backend::VhostUser(config) => Some(&config.sock_path),
            _ => None,
        }
    }

    /// Whether a duplicate guest MAC address is allowed for this device.
    fn allow_duplicate_mac(&self) -> bool {
        #[allow(unreachable_patterns)]
        match &self.backend {
            #[cfg(feature = "virtio-net")]
            Backend::Virtio(config) => config.allow_duplicate_mac,
            #[cfg(feature = "vhost-net")]
            Backend::Vhost(config) => config.allow_duplicate_mac,
            _ => false,
        }
    }
}

impl ConfigItem for NetworkInterfaceConfig {
    type Err = NetworkDeviceError;

    /// Identity of the device: the interface id, as for every other device
    /// class, and never a host resource such as a tap name or a socket path.
    /// Supplied by the caller; an unnamed device is refused on insertion.
    fn id(&self) -> &str {
        #[allow(unreachable_patterns)]
        match &self.backend {
            #[cfg(feature = "virtio-net")]
            Backend::Virtio(config) => &config.iface_id,
            #[cfg(feature = "vhost-net")]
            Backend::Vhost(config) => &config.iface_id,
            #[cfg(feature = "vhost-user-net")]
            Backend::VhostUser(config) => &config.iface_id,
            _ => "",
        }
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), NetworkDeviceError> {
        if let Some(mac) = self.guest_mac.as_ref() {
            if !other.allow_duplicate_mac() && Some(mac) == other.guest_mac.as_ref() {
                return Err(NetworkDeviceError::GuestMacAddressInUse(mac.to_string()));
            }
        }

        if let (Some(name), Some(other_name)) = (self.host_dev_name(), other.host_dev_name()) {
            if name == other_name {
                return Err(NetworkDeviceError::HostDeviceNameInUse(name.to_owned()));
            }
        }

        // A vhost-user socket is served by one backend process for one
        // device, so two devices may not share one.
        if let (Some(path), Some(other_path)) = (self.sock_path(), other.sock_path()) {
            if path == other_path {
                return Err(NetworkDeviceError::DuplicatedUdsPath(path.to_owned()));
            }
        }

        Ok(())
    }
}

/// The data fed into a network iface update request. Currently, only the RX and
/// TX rate limiters can be updated, and only for the virtio-net backend.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NetworkInterfaceUpdateConfig {
    /// ID of the guest network interface.
    pub iface_id: String,
    /// New RX rate limiter config. Only provided data will be updated. I.e. if any optional data
    /// is missing, it will not be nullified, but left unchanged.
    pub rx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// New TX rate limiter config. Only provided data will be updated. I.e. if any optional data
    /// is missing, it will not be nullified, but left unchanged.
    pub tx_rate_limiter: Option<RateLimiterConfigInfo>,
}

impl NetworkInterfaceUpdateConfig {
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

/// Device manager to manage all network devices, of every backend.
pub struct NetworkDeviceMgr {
    pub(crate) info_list: DeviceConfigInfos<NetworkInterfaceConfig>,
    pub(crate) use_shared_irq: bool,
}

impl NetworkDeviceMgr {
    /// Gets the index of the device with the specified `iface_id` if it exists
    /// in the list.
    pub fn get_index_of_iface_id(&self, if_id: &str) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.id() == if_id)
    }

    /// Insert or update a network device into the manager.
    pub fn insert_device(
        &mut self,
        mut ctx: DeviceOpContext,
        config: NetworkInterfaceConfig,
    ) -> std::result::Result<(), NetworkDeviceError> {
        // The id is the name the interface is given inside the guest, so an
        // unnamed device is not a device the guest can have. Refusing it here
        // fails VM start, rather than letting it quietly take the place of
        // another unnamed device and booting with one interface missing.
        if config.id().is_empty() {
            return Err(NetworkDeviceError::MissingIfaceId);
        }

        if !config.num_queues().is_multiple_of(2) {
            return Err(NetworkDeviceError::InvalidQueueNum(config.num_queues()));
        }
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            return Err(NetworkDeviceError::UpdateNotAllowedPostBoot);
        }
        // Before boot a repeated id reconfigures the device, as it does for
        // every other device class. Once the device is attached that is no
        // longer possible: the config would be replaced while the live device
        // stayed on the bus with no handle left to remove it, and the guest
        // would end up with two interfaces sharing one name.
        if ctx.is_hotplug
            && self
                .info_list
                .iter()
                .any(|info| info.config.id() == config.id())
        {
            return Err(NetworkDeviceError::DeviceIDAlreadyExist(
                config.id().to_owned(),
            ));
        }

        slog::info!(
            ctx.logger(),
            "add network device configuration";
            "subsystem" => "net_dev_mgr",
            "backend" => config.backend.name(),
            "id" => config.id(),
        );

        let device_index = self.info_list.insert_or_update(&config)?;

        if ctx.is_hotplug {
            slog::info!(
                ctx.logger(),
                "attach network device";
                "subsystem" => "net_dev_mgr",
                "backend" => config.backend.name(),
                "id" => config.id(),
            );

            match Self::create_device(&config, &mut ctx) {
                Ok(device) => {
                    let dev = DeviceManager::create_mmio_virtio_device(
                        device,
                        &mut ctx,
                        config.use_shared_irq.unwrap_or(self.use_shared_irq),
                        config.use_generic_irq(),
                    )
                    .map_err(NetworkDeviceError::DeviceManager)?;
                    ctx.insert_hotplug_mmio_device(&dev, None)
                        .map_err(NetworkDeviceError::DeviceManager)?;
                    // live-upgrade need save/restore device from info.device.
                    self.info_list[device_index].set_device(dev);
                }
                Err(e) => {
                    self.info_list.remove(device_index);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Update the ratelimiter settings of a virtio-net device.
    ///
    /// Only the virtio-net backend implements rate limiting; the request is
    /// refused for the others rather than silently ignored.
    pub fn update_device_ratelimiters(
        &mut self,
        new_cfg: NetworkInterfaceUpdateConfig,
    ) -> std::result::Result<(), NetworkDeviceError> {
        match self.get_index_of_iface_id(&new_cfg.iface_id) {
            Some(index) => {
                let config = &mut self.info_list[index].config;
                #[allow(unreachable_patterns)]
                match &mut config.backend {
                    #[cfg(feature = "virtio-net")]
                    Backend::Virtio(ref mut virtio_config) => {
                        // Patch semantics: an omitted (None) limiter is left
                        // unchanged rather than nulled, matching this struct's
                        // doc and the live-device patch below, where a None
                        // field maps to BucketUpdate::None ("no update"). A
                        // limiter is cleared by sending a zero-size bucket.
                        if new_cfg.rx_rate_limiter.is_some() {
                            virtio_config.rx_rate_limiter = new_cfg.rx_rate_limiter.clone();
                        }
                        if new_cfg.tx_rate_limiter.is_some() {
                            virtio_config.tx_rate_limiter = new_cfg.tx_rate_limiter.clone();
                        }
                    }
                    backend => {
                        let backend = backend.name();
                        return Err(NetworkDeviceError::UnsupportedBackend {
                            iface_id: new_cfg.iface_id.clone(),
                            backend,
                        });
                    }
                }
                let device = self.info_list[index]
                    .device
                    .as_mut()
                    .ok_or_else(|| NetworkDeviceError::InvalidIfaceId(new_cfg.iface_id.clone()))?;

                // Only the in-VMM virtio-net device implements rate limiting,
                // so the patch path exists only with that feature.
                #[cfg(feature = "virtio-net")]
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
                            .map_err(|_e| NetworkDeviceError::NetEpollHandlerSendFail);
                    }
                }
                Ok(())
            }
            None => Err(NetworkDeviceError::InvalidIfaceId(new_cfg.iface_id.clone())),
        }
    }

    /// Attach all configured network devices to the virtual machine instance.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), NetworkDeviceError> {
        for info in self.info_list.iter_mut() {
            slog::info!(
                ctx.logger(),
                "attach network device";
                "subsystem" => "net_dev_mgr",
                "backend" => info.config.backend.name(),
                "id" => info.config.id(),
            );

            let device = Self::create_device(&info.config, ctx)?;
            let device = DeviceManager::create_mmio_virtio_device(
                device,
                ctx,
                info.config.use_shared_irq.unwrap_or(self.use_shared_irq),
                info.config.use_generic_irq(),
            )
            .map_err(NetworkDeviceError::RegisterNetDevice)?;
            info.set_device(device);
        }

        Ok(())
    }

    /// Create the backing device for `cfg`, dispatching on its backend.
    fn create_device(
        cfg: &NetworkInterfaceConfig,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<DbsVirtioDevice, NetworkDeviceError> {
        #[allow(unreachable_patterns)]
        match &cfg.backend {
            #[cfg(feature = "virtio-net")]
            Backend::Virtio(config) => Self::create_virtio_net_device(cfg, config, ctx),
            #[cfg(feature = "vhost-net")]
            Backend::Vhost(config) => Self::create_vhost_net_device(cfg, config, ctx),
            #[cfg(feature = "vhost-user-net")]
            Backend::VhostUser(config) => Self::create_vhost_user_net_device(cfg, config, ctx),
            _ => Err(NetworkDeviceError::UnsupportedBackend {
                iface_id: cfg.id().to_owned(),
                backend: cfg.backend.name(),
            }),
        }
    }

    #[cfg(feature = "virtio-net")]
    fn create_virtio_net_device(
        cfg: &NetworkInterfaceConfig,
        config: &VirtioConfig,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<DbsVirtioDevice, NetworkDeviceError> {
        let epoll_mgr = ctx
            .epoll_mgr
            .clone()
            .ok_or(NetworkDeviceError::Virtio(VirtioError::InvalidInput))?;
        let rx_rate_limiter = match config.rx_rate_limiter.as_ref() {
            Some(rl) => Some(
                rl.try_into()
                    .map_err(|e| NetworkDeviceError::CreateNetDevice(VirtioError::IOError(e)))?,
            ),
            None => None,
        };
        let tx_rate_limiter = match config.tx_rate_limiter.as_ref() {
            Some(rl) => Some(
                rl.try_into()
                    .map_err(|e| NetworkDeviceError::CreateNetDevice(VirtioError::IOError(e)))?,
            ),
            None => None,
        };

        let net_device = virtio::net::Net::new(
            config.host_dev_name.clone(),
            cfg.guest_mac(),
            Arc::new(cfg.queue_sizes()),
            epoll_mgr,
            rx_rate_limiter,
            tx_rate_limiter,
        )
        .map_err(NetworkDeviceError::CreateNetDevice)?;

        Ok(Box::new(net_device))
    }

    #[cfg(feature = "vhost-net")]
    fn create_vhost_net_device(
        cfg: &NetworkInterfaceConfig,
        config: &VirtioConfig,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<DbsVirtioDevice, NetworkDeviceError> {
        slog::info!(
            ctx.logger(),
            "create a vhost-net device";
            "subsystem" => "net_dev_mgr",
            "id" => &config.iface_id,
            "host_dev_name" => &config.host_dev_name,
        );
        let epoll_mgr = ctx
            .epoll_mgr
            .clone()
            .ok_or(NetworkDeviceError::Virtio(VirtioError::InvalidInput))?;

        Ok(Box::new(
            virtio::vhost::vhost_kern::net::Net::new(
                config.host_dev_name.clone(),
                cfg.guest_mac(),
                Arc::new(cfg.queue_sizes()),
                epoll_mgr,
            )
            .map_err(NetworkDeviceError::CreateNetDevice)?,
        ))
    }

    #[cfg(feature = "vhost-user-net")]
    fn create_vhost_user_net_device(
        cfg: &NetworkInterfaceConfig,
        config: &VhostUserConfig,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<DbsVirtioDevice, NetworkDeviceError> {
        let epoll_mgr = ctx
            .epoll_mgr
            .clone()
            .ok_or(NetworkDeviceError::Virtio(VirtioError::InvalidInput))?;

        Ok(Box::new(
            virtio::vhost::vhost_user::net::VhostUserNet::new_server(
                &config.sock_path,
                cfg.guest_mac(),
                Arc::new(cfg.queue_sizes()),
                epoll_mgr,
            )
            .map_err(NetworkDeviceError::CreateNetDevice)?,
        ))
    }

    /// Remove all network devices.
    ///
    /// A device that fails to be destroyed is logged and skipped rather than
    /// aborting the teardown: this runs while the VM is being torn down, and
    /// one broken device (e.g. a vhost-user device whose backend process is
    /// already gone) must not leave every remaining device attached.
    pub fn remove_devices(&mut self, ctx: &mut DeviceOpContext) -> Result<(), DeviceMgrError> {
        while let Some(mut info) = self.info_list.pop() {
            slog::info!(ctx.logger(), "remove network device: {}", info.config.id());
            if let Some(device) = info.device.take() {
                if let Err(e) = DeviceManager::destroy_mmio_device(device, ctx) {
                    slog::error!(
                        ctx.logger(),
                        "failed to destroy network device, continuing teardown";
                        "subsystem" => "net_dev_mgr",
                        "id" => info.config.id(),
                        "error" => format!("{e:?}"),
                    );
                }
            }
        }
        Ok(())
    }
}

impl Default for NetworkDeviceMgr {
    /// Create a new network device manager.
    fn default() -> Self {
        NetworkDeviceMgr {
            info_list: DeviceConfigInfos::new(),
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}

#[cfg(test)]
mod tests {
    use dbs_utils::net::MacAddr;
    use test_utils::{skip_if_kvm_unaccessable, skip_if_not_root};

    use super::*;
    use crate::device_manager::DeviceManager;
    use crate::test_utils::tests::create_vm_for_test;
    use crate::vm::VmConfigInfo;

    fn ctx_without_epoll_mgr(mgr: &DeviceManager, vm: &crate::vm::Vm) -> DeviceOpContext {
        DeviceOpContext::new(
            None,
            mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        )
    }

    #[cfg(any(feature = "virtio-net", feature = "vhost-net"))]
    fn virtio_backend_config(iface_id: &str, host_dev_name: &str) -> VirtioConfig {
        VirtioConfig {
            iface_id: String::from(iface_id),
            host_dev_name: String::from(host_dev_name),
            rx_rate_limiter: None,
            tx_rate_limiter: None,
            allow_duplicate_mac: false,
        }
    }

    fn net_config(backend: Backend, guest_mac: &str, num_queues: usize) -> NetworkInterfaceConfig {
        NetworkInterfaceConfig {
            num_queues: Some(num_queues),
            queue_size: Some(128),
            backend,
            guest_mac: Some(MacAddr::parse_str(guest_mac).unwrap()),
            use_shared_irq: None,
            use_generic_irq: None,
        }
    }

    #[cfg(feature = "virtio-net")]
    #[test]
    fn test_network_interface_config_from_json() {
        let json_str = r#"{
            "num_queues": 4,
            "queue_size": 512,
            "backend": {
                "type": "virtio",
                "iface_id": "eth0",
                "host_dev_name": "tap0",
                "allow_duplicate_mac": true
            },
            "guest_mac": "81:87:1D:00:08:A9"
        }"#;
        let net_config: NetworkInterfaceConfig = serde_json::from_str(json_str).unwrap();
        assert_eq!(net_config.num_queues, Some(4));
        assert_eq!(net_config.queue_size, Some(512));
        assert_eq!(
            net_config.guest_mac,
            Some(MacAddr::from_bytes(&[129, 135, 29, 0, 8, 169]).unwrap())
        );
        if let Backend::Virtio(config) = &net_config.backend {
            assert_eq!(config.iface_id, "eth0");
            assert_eq!(config.host_dev_name, "tap0");
            assert!(config.allow_duplicate_mac);
        } else {
            panic!("Unexpected backend type");
        }
        // The virtio backend has always used the default queue count.
        assert_eq!(net_config.num_queues(), DEFAULT_NUM_QUEUES);
        assert_eq!(net_config.queue_size(), 512);
        assert_eq!(net_config.id(), "eth0");
    }

    #[test]
    fn test_backend_identity_and_name() {
        #[cfg(feature = "virtio-net")]
        {
            let cfg = net_config(
                Backend::Virtio(virtio_backend_config("id_1", "dev1")),
                "01:23:45:67:89:0a",
                2,
            );
            assert_eq!(cfg.id(), "id_1");
            assert_eq!(cfg.backend.name(), "virtio-net");
        }
        #[cfg(feature = "vhost-net")]
        {
            let cfg = net_config(
                Backend::Vhost(virtio_backend_config("id_2", "dev2")),
                "01:23:45:67:89:0b",
                2,
            );
            assert_eq!(cfg.id(), "id_2");
            assert_eq!(cfg.backend.name(), "vhost-net");
        }
        #[cfg(feature = "vhost-user-net")]
        {
            // Supplied id is the identity, exactly as for the other backends;
            // the socket path is a resource, not an identity.
            let cfg = net_config(
                Backend::VhostUser(VhostUserConfig {
                    iface_id: String::from("id_3"),
                    sock_path: String::from("/tmp/sock_1"),
                }),
                "01:23:45:67:89:0c",
                2,
            );
            assert_eq!(cfg.id(), "id_3");
            assert_eq!(cfg.backend.name(), "vhost-user-net");

            // With no id supplied, id() is empty until a caller sets one.
            let cfg = net_config(
                Backend::VhostUser(VhostUserConfig {
                    iface_id: String::new(),
                    sock_path: String::from("/tmp/sock_2"),
                }),
                "01:23:45:67:89:0d",
                2,
            );
            assert_eq!(cfg.id(), "");
        }
    }

    #[cfg(feature = "vhost-net")]
    #[test]
    fn test_create_vhost_net_device() {
        skip_if_kvm_unaccessable!();
        let vm = create_vm_for_test();
        let mgr = DeviceManager::new_test_mgr();
        let netif_1 = net_config(
            Backend::Vhost(virtio_backend_config("id_1", "dev1")),
            "01:23:45:67:89:0a",
            2,
        );

        // no epoll manager
        let mut ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(NetworkDeviceMgr::create_device(&netif_1, &mut ctx).is_err());
    }

    #[cfg(feature = "vhost-user-net")]
    #[test]
    fn test_create_vhost_user_net_device() {
        skip_if_kvm_unaccessable!();
        let vm = create_vm_for_test();
        let mgr = DeviceManager::new_test_mgr();
        let netif_1 = net_config(
            Backend::VhostUser(VhostUserConfig {
                iface_id: String::new(),
                sock_path: String::from("/tmp/vhost_user_net_test"),
            }),
            "01:23:45:67:89:0a",
            2,
        );

        // no epoll manager
        let mut ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(NetworkDeviceMgr::create_device(&netif_1, &mut ctx).is_err());
    }

    #[cfg(feature = "vhost-net")]
    #[test]
    fn test_attach_vhost_net_device() {
        skip_if_kvm_unaccessable!();
        // Attaching the device opens the tap named by the config, and
        // TUNSETIFF creates it when absent, which needs CAP_NET_ADMIN.
        skip_if_not_root!();
        let mut vm = create_vm_for_test();
        let device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );

        let netif_1 = net_config(
            Backend::Vhost(virtio_backend_config("id_1", "dev1")),
            "01:23:45:67:89:0a",
            2,
        );

        assert!(vm
            .device_manager_mut()
            .net_manager
            .insert_device(device_op_ctx, netif_1)
            .is_ok());
        assert_eq!(vm.device_manager().net_manager.info_list.len(), 1);

        let mut device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );

        assert!(vm
            .device_manager_mut()
            .net_manager
            .attach_devices(&mut device_op_ctx)
            .is_ok());
    }

    #[cfg(feature = "vhost-net")]
    #[test]
    fn test_insert_vhost_net_device() {
        skip_if_kvm_unaccessable!();
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();

        let mut netif_1 = net_config(
            Backend::Vhost(virtio_backend_config("id_1", "dev1")),
            "01:23:45:67:89:0a",
            2,
        );

        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(mgr.net_manager.insert_device(ctx, netif_1.clone()).is_ok());
        assert_eq!(mgr.net_manager.info_list.len(), 1);

        // Before boot, repeating an id reconfigures that device in place, as
        // it does for every other device class.
        netif_1.guest_mac = Some(MacAddr::parse_str("01:23:45:67:89:0b").unwrap());
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(mgr.net_manager.insert_device(ctx, netif_1.clone()).is_ok());
        assert_eq!(mgr.net_manager.info_list.len(), 1);

        netif_1.backend = Backend::Vhost(virtio_backend_config("id_1", "dev2"));
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(mgr.net_manager.insert_device(ctx, netif_1).is_ok());
        assert_eq!(mgr.net_manager.info_list.len(), 1);
    }

    #[cfg(feature = "vhost-user-net")]
    #[test]
    fn test_insert_vhost_user_net_device() {
        skip_if_kvm_unaccessable!();
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();

        let netif_1 = net_config(
            Backend::VhostUser(VhostUserConfig {
                iface_id: String::from("id_1"),
                sock_path: String::from("/tmp/vhost_user_net_insert"),
            }),
            "01:23:45:67:89:0a",
            2,
        );

        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(mgr.net_manager.insert_device(ctx, netif_1).is_ok());
        assert_eq!(mgr.net_manager.info_list.len(), 1);
    }

    #[cfg(feature = "vhost-net")]
    #[test]
    fn test_net_insert_error_cases() {
        skip_if_kvm_unaccessable!();
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();

        // invalid queue num
        let mut netif_1 = net_config(
            Backend::Vhost(virtio_backend_config("id_1", "dev_1")),
            "01:23:45:67:89:0a",
            1,
        );
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        let res = mgr.net_manager.insert_device(ctx, netif_1.clone());
        assert!(matches!(res, Err(NetworkDeviceError::InvalidQueueNum(1))));
        assert_eq!(mgr.net_manager.info_list.len(), 0);

        // Adding the first valid network config.
        netif_1.num_queues = Some(2);
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(mgr.net_manager.insert_device(ctx, netif_1.clone()).is_ok());
        assert_eq!(mgr.net_manager.info_list.len(), 1);

        // Error Case: add a new device sharing the same host_dev_name.
        let mut netif_2 = netif_1.clone();
        netif_2.backend = Backend::Vhost(virtio_backend_config("id_2", "dev_1"));
        netif_2.guest_mac = Some(MacAddr::parse_str("11:45:45:67:89:0b").unwrap());
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        let res = mgr.net_manager.insert_device(ctx, netif_2.clone());
        assert!(matches!(
            res,
            Err(NetworkDeviceError::HostDeviceNameInUse(_))
        ));

        // Error Case: add a new device sharing the same guest MAC address.
        netif_2.backend = Backend::Vhost(virtio_backend_config("id_2", "dev_2"));
        netif_2.guest_mac = netif_1.guest_mac;
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        let res = mgr.net_manager.insert_device(ctx, netif_2);
        assert!(matches!(
            res,
            Err(NetworkDeviceError::GuestMacAddressInUse(_))
        ));
    }

    #[cfg(feature = "vhost-user-net")]
    #[test]
    fn test_vhost_user_net_insert_error_cases() {
        skip_if_kvm_unaccessable!();
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();

        // invalid queue num
        let netif_1 = net_config(
            Backend::VhostUser(VhostUserConfig {
                iface_id: String::from("id_1"),
                sock_path: String::from("/tmp/vhost_user_net_err"),
            }),
            "01:23:45:67:89:0a",
            1,
        );
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        let res = mgr.net_manager.insert_device(ctx, netif_1);
        assert!(matches!(res, Err(NetworkDeviceError::InvalidQueueNum(1))));
        assert_eq!(mgr.net_manager.info_list.len(), 0);
    }

    #[cfg(feature = "vhost-user-net")]
    #[test]
    fn test_vhost_user_net_duplicate_sock_path() {
        skip_if_kvm_unaccessable!();
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();

        // Distinct MACs, so it is the socket clash that is reported rather
        // than the MAC one.
        let mk = |iface_id: &str, mac: &str| {
            net_config(
                Backend::VhostUser(VhostUserConfig {
                    iface_id: String::from(iface_id),
                    sock_path: String::from("/tmp/vhost_user_net_dup"),
                }),
                mac,
                2,
            )
        };

        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(mgr
            .net_manager
            .insert_device(ctx, mk("id_1", "01:23:45:67:89:0a"))
            .is_ok());
        assert_eq!(mgr.net_manager.info_list.len(), 1);

        // A second device with its own id but the same socket is refused:
        // one vhost-user socket is served by one backend for one device.
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        let res = mgr
            .net_manager
            .insert_device(ctx, mk("id_2", "01:23:45:67:89:0b"));
        assert!(matches!(res, Err(NetworkDeviceError::DuplicatedUdsPath(_))));
        assert_eq!(mgr.net_manager.info_list.len(), 1);

        // Re-inserting the same id before boot reconfigures that device: its
        // own socket is not a clash with itself.
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        assert!(mgr
            .net_manager
            .insert_device(ctx, mk("id_1", "01:23:45:67:89:0c"))
            .is_ok());
        assert_eq!(mgr.net_manager.info_list.len(), 1);
    }

    #[cfg(feature = "vhost-user-net")]
    #[test]
    fn test_unnamed_device_is_refused() {
        skip_if_kvm_unaccessable!();
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();

        // The id becomes the interface name in the guest, so an unnamed
        // device is refused instead of silently replacing another one.
        let cfg = net_config(
            Backend::VhostUser(VhostUserConfig {
                iface_id: String::new(),
                sock_path: String::from("/tmp/sock_a"),
            }),
            "01:23:45:67:89:0a",
            2,
        );
        let ctx = ctx_without_epoll_mgr(&mgr, &vm);
        let res = mgr.net_manager.insert_device(ctx, cfg);
        assert!(matches!(res, Err(NetworkDeviceError::MissingIfaceId)));
        assert_eq!(mgr.net_manager.info_list.len(), 0);
    }

    #[test]
    fn test_net_device_error_display() {
        let err = NetworkDeviceError::DuplicatedUdsPath(String::from("1"));
        let _ = format!("{err}{err:?}");

        let err = NetworkDeviceError::InvalidQueueNum(1);
        let _ = format!("{err}{err:?}");

        let err = NetworkDeviceError::UnsupportedBackend {
            iface_id: String::from("id_1"),
            backend: "vhost-user-net",
        };
        let _ = format!("{err}{err:?}");
    }
}
