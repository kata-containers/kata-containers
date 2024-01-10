// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::result::Result;
use std::sync::Arc;

use dbs_utils::net::MacAddr;
use dbs_virtio_devices::vhost::vhost_user::net::VhostUserNet;
use dbs_virtio_devices::Error as VirtioError;
use serde::{Deserialize, Serialize};

use super::{DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::config_manager::{ConfigItem, DeviceConfigInfos};

/// Default number of virtio queues, one rx/tx pair.
pub const DEFAULT_NUM_QUEUES: usize = 2;
/// Default size of virtio queues.
pub const DEFAULT_QUEUE_SIZE: u16 = 256;
// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = false;

/// Errors associated with vhost user net devices.
#[derive(Debug, thiserror::Error)]
pub enum VhostUserNetDeviceError {
    /// The virtual machine instance ID is invalid.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVmId,
    /// Internal error. Invalid queue number configuration for vhost_user_net device.
    #[error("invalid queue number {0} for vhost_user_net device")]
    InvalidQueueNum(usize),
    /// Failure from device manager,
    #[error("failure in device manager operations: {0:?}")]
    DeviceManager(DeviceMgrError),
    /// Duplicated Unix domain socket path for vhost_user_net device.
    #[error("duplicated Unix domain socket path {0} for vhost_user_net device")]
    DuplicatedUdsPath(String),
    /// Failure from Virtio subsystem.
    #[error(transparent)]
    Virtio(VirtioError),
    /// The update is not allowed after booting the microvm.
    #[error("update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,
    /// Split this at some point.
    /// Internal errors are due to resource exhaustion.
    /// Users errors are due to invalid permissions.
    #[error("cannot create a vhost-user-net device: {0:?}")]
    CreateNetDevice(VirtioError),
    /// Cannot initialize a MMIO Network Device or add a device to the MMIO Bus.
    #[error("failure while registering network device: {0:?}")]
    RegisterNetDevice(DeviceMgrError),
}
/// Configuration information for vhost user net devices.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct VhostUserNetDeviceConfigInfo {
    /// vhost-user socket path.
    pub sock_path: String,
    /// Number of virtqueues to use.
    pub num_queues: usize,
    /// Size of each virtqueue.
    pub queue_size: u16,
    /// mac of the interface.
    pub guest_mac: Option<MacAddr>,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl VhostUserNetDeviceConfigInfo {
    /// Returns a reference to the mac address. If the mac address is not configured, it
    /// returns None.
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

impl ConfigItem for VhostUserNetDeviceConfigInfo {
    type Err = VhostUserNetDeviceError;

    fn check_conflicts(&self, _other: &Self) -> std::result::Result<(), Self::Err> {
        Ok(())
    }

    fn id(&self) -> &str {
        &self.sock_path
    }
}

/// Device manager to manage all vhost user net devices.
pub struct VhostUserNetDeviceMgr {
    pub(crate) configs: DeviceConfigInfos<VhostUserNetDeviceConfigInfo>,
}

impl VhostUserNetDeviceMgr {
    fn create_device(
        cfg: &VhostUserNetDeviceConfigInfo,
        ctx: &mut DeviceOpContext,
    ) -> Result<Box<VhostUserNet<GuestAddressSpaceImpl>>, VirtioError> {
        match ctx.epoll_mgr.as_ref() {
            Some(epoll_mgr) => Ok(Box::new(VhostUserNet::new_server(
                &cfg.sock_path,
                cfg.guest_mac(),
                Arc::new(cfg.queue_sizes()),
                epoll_mgr.clone(),
            )?)),
            None => Err(VirtioError::InvalidInput),
        }
    }

    /// Insert or update a vhost user net device into the manager.
    pub fn insert_device(
        device_mgr: &mut DeviceManager,
        mut ctx: DeviceOpContext,
        config: VhostUserNetDeviceConfigInfo,
    ) -> Result<(), VhostUserNetDeviceError> {
        // Validate device configuration first.
        if config.num_queues % 2 != 0 {
            return Err(VhostUserNetDeviceError::InvalidQueueNum(config.num_queues));
        }
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            return Err(VhostUserNetDeviceError::UpdateNotAllowedPostBoot);
        }
        slog::info!(
            ctx.logger(),
            "add vhost-user-net device configuration";
            "subsystem" => "vhost_net_dev_mgr",
            "sock_path" => &config.sock_path,
        );
        let device_index = device_mgr
            .vhost_user_net_manager
            .configs
            .insert_or_update(&config)?;
        if ctx.is_hotplug {
            slog::info!(
                ctx.logger(),
                "attach virtio-net device";
                "subsystem" => "vhost_net_dev_mgr",
                "sock_path" => &config.sock_path,
            );
            match Self::create_device(&config, &mut ctx) {
                Ok(device) => {
                    let dev = DeviceManager::create_mmio_virtio_device(
                        device,
                        &mut ctx,
                        config.use_shared_irq.unwrap_or(USE_SHARED_IRQ),
                        config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                    )
                    .map_err(VhostUserNetDeviceError::DeviceManager)?;
                    ctx.insert_hotplug_mmio_device(&dev, None)
                        .map_err(VhostUserNetDeviceError::DeviceManager)?;
                }
                Err(err) => {
                    device_mgr
                        .vhost_user_net_manager
                        .configs
                        .remove(device_index);
                    return Err(VhostUserNetDeviceError::Virtio(err));
                }
            }
        }
        Ok(())
    }

    /// Attach all configured vhost user net device to the virtual machine
    /// instance.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> Result<(), VhostUserNetDeviceError> {
        for info in self.configs.iter() {
            slog::info!(
                ctx.logger(),
                "attach vhost-user-net device";
                "subsystem" => "vhost_net_dev_mgr",
                "sock_path" => &info.config.sock_path,
            );
            let device = Self::create_device(&info.config, ctx)
                .map_err(VhostUserNetDeviceError::CreateNetDevice)?;
            DeviceManager::create_mmio_virtio_device(
                device,
                ctx,
                info.config.use_shared_irq.unwrap_or(USE_SHARED_IRQ),
                info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
            )
            .map_err(VhostUserNetDeviceError::RegisterNetDevice)?;
        }
        Ok(())
    }
}

impl Default for VhostUserNetDeviceMgr {
    /// Create a new vhost user net device manager.
    fn default() -> Self {
        VhostUserNetDeviceMgr {
            configs: DeviceConfigInfos::<VhostUserNetDeviceConfigInfo>::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::create_vm_for_test;

    #[test]
    fn test_create_vhost_user_net_device() {
        let vm = create_vm_for_test();
        let mgr = DeviceManager::new_test_mgr();
        let sock_1 = String::from("id_1");
        let guest_mac_1 = "01:23:45:67:89:0a";
        let netif_1 = VhostUserNetDeviceConfigInfo {
            sock_path: sock_1,
            num_queues: 2,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_1).unwrap()),
            use_shared_irq: None,
            use_generic_irq: None,
        };
        // no epoll manager
        let mut ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );
        assert!(VhostUserNetDeviceMgr::create_device(&netif_1, &mut ctx).is_err());
    }

    #[test]
    fn test_insert_vhost_user_net_device() {
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();
        let sock_1 = String::from("id_1");
        let guest_mac_1 = "01:23:45:67:89:0a";
        // Test create.
        let netif_1 = VhostUserNetDeviceConfigInfo {
            sock_path: sock_1,
            num_queues: 2,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_1).unwrap()),
            use_shared_irq: None,
            use_generic_irq: None,
        };
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );
        assert!(VhostUserNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1).is_ok());
        assert_eq!(mgr.vhost_user_net_manager.configs.len(), 1);
    }

    #[test]
    fn test_vhost_user_net_insert_error_cases() {
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();
        let sock_1 = String::from("id_1");
        let guest_mac_1 = "01:23:45:67:89:0a";
        // invalid queue num.
        let netif_1 = VhostUserNetDeviceConfigInfo {
            sock_path: sock_1,
            num_queues: 1,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_1).unwrap()),
            use_shared_irq: None,
            use_generic_irq: None,
        };
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );
        let res = VhostUserNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1);
        if let Err(VhostUserNetDeviceError::InvalidQueueNum(1)) = res {
            assert_eq!(mgr.vhost_user_net_manager.configs.len(), 0);
        } else {
            panic!()
        }
    }

    #[test]
    fn test_vhost_user_net_error_display() {
        let err = VhostUserNetDeviceError::InvalidVmId;
        let _ = format!("{}{:?}", err, err);
        let err = VhostUserNetDeviceError::InvalidQueueNum(0);
        let _ = format!("{}{:?}", err, err);
        let err = VhostUserNetDeviceError::DeviceManager(DeviceMgrError::GetDeviceResource);
        let _ = format!("{}{:?}", err, err);
        let err = VhostUserNetDeviceError::DuplicatedUdsPath(String::from("1"));
        let _ = format!("{}{:?}", err, err);
        let err = VhostUserNetDeviceError::Virtio(VirtioError::DescriptorChainTooShort);
        let _ = format!("{}{:?}", err, err);
        let err = VhostUserNetDeviceError::UpdateNotAllowedPostBoot;
        let _ = format!("{}{:?}", err, err);
    }
}
