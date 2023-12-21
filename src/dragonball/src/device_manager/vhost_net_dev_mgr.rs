// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::result::Result;
use std::sync::Arc;

use dbs_utils::net::MacAddr;
use dbs_virtio_devices::vhost::vhost_kern::net::Net;
use dbs_virtio_devices::Error as VirtioError;
use serde::{Deserialize, Serialize};
use virtio_queue::QueueSync;

use super::{DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::address_space_manager::{GuestAddressSpaceImpl, GuestRegionImpl};
use crate::config_manager::{ConfigItem, DeviceConfigInfos};

/// Default number of virtio queues, one rx/tx pair.
pub const DEFAULT_NUM_QUEUES: usize = 2;
/// Default size of virtio queues.
pub const DEFAULT_QUEUE_SIZE: u16 = 256;
// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = true;

#[derive(Debug, thiserror::Error)]
/// Errors associated with vhost-net device operations
pub enum VhostNetDeviceError {
    /// The Context Identifier is already in use.
    #[error("the device id {0} already exists")]
    DeviceIdAlreadyExist(String),
    /// The MAC address is already in use.
    #[error("the guest Mac address {0} is already in use")]
    GuestMacAddressInUse(String),
    /// The host device name is already in use.
    #[error("the host device name {0} is already in use")]
    HostDeviceNameInUse(String),
    /// The update isn't allowed after booting the mircovm.
    #[error("update operation is not allowed after booting")]
    UpdateNotAllowedPostBoot,
    /// Invalid queue number for vhost-net device.
    #[error("invalid queue number {0} for vhost-net device")]
    InvalidQueueNum(usize),
    /// Failure from device manager.
    #[error("failure in device manager operations: {0:?}")]
    DeviceManager(#[source] DeviceMgrError),
    /// Failure from virtio subsystem.
    #[error("virtio error: {0:?}")]
    Virtio(VirtioError),
    /// Split this at some point.
    /// Internal errors are due to resource exhaustion.
    /// Users errors are due to invalid permissions.
    #[error("cannot create a vhost-net device: {0}")]
    CreateNetDevice(#[source] VirtioError),
    /// Cannot initialize a MMIO Network Device or add a device to the MMIO Bus.
    #[error("failure while registering vhost-net device: {0}")]
    RegisterNetDevice(#[source] DeviceMgrError),
}

/// Configuration information for vhost net devices.
/// TODO: https://github.com/kata-containers/kata-containers/issues/8382.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VhostNetDeviceConfigInfo {
    /// Id of the guest network interface.
    pub iface_id: String,
    /// Host level path for the guest network interface.
    pub host_dev_name: String,
    /// Number of virtqueues to use.
    pub num_queues: usize,
    /// Size of each virtqueue.
    pub queue_size: u16,
    /// Guest MAC address.
    pub guest_mac: Option<MacAddr>,
    /// allow duplicate mac
    pub allow_duplicate_mac: bool,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use shared irq
    pub use_generic_irq: Option<bool>,
}

impl VhostNetDeviceConfigInfo {
    /// Returns a reference to the mac address. Its mac address is not
    /// configured, it returns None.
    pub fn guest_mac(&self) -> Option<&MacAddr> {
        self.guest_mac.as_ref()
    }

    /// Returns rx and tx queue sizes, the length is num_queues, each value is
    /// queue_size.
    pub fn queue_sizes(&self) -> Vec<u16> {
        let queue_size = if self.queue_size > 0 {
            self.queue_size
        } else {
            DEFAULT_QUEUE_SIZE
        };
        let num_queues = if self.num_queues > 0 {
            self.num_queues
        } else {
            DEFAULT_NUM_QUEUES
        };

        (0..num_queues).map(|_| queue_size).collect()
    }
}

impl ConfigItem for VhostNetDeviceConfigInfo {
    type Err = VhostNetDeviceError;

    fn id(&self) -> &str {
        &self.iface_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), Self::Err> {
        if self.iface_id == other.iface_id {
            Err(VhostNetDeviceError::DeviceIdAlreadyExist(
                self.iface_id.clone(),
            ))
        } else if !other.allow_duplicate_mac
            && self.guest_mac.is_some()
            && self.guest_mac == other.guest_mac
        {
            Err(VhostNetDeviceError::GuestMacAddressInUse(
                self.guest_mac.as_ref().unwrap().to_string(),
            ))
        } else if self.host_dev_name == other.host_dev_name {
            Err(VhostNetDeviceError::HostDeviceNameInUse(
                self.host_dev_name.clone(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Device manager to manage all vhost net devices.
pub struct VhostNetDeviceMgr {
    info_list: DeviceConfigInfos<VhostNetDeviceConfigInfo>,
    use_shared_irq: bool,
}

impl VhostNetDeviceMgr {
    /// Create a `vhost_kern::net::Net` struct representing a vhost-net device.
    fn create_device(
        cfg: &VhostNetDeviceConfigInfo,
        ctx: &mut DeviceOpContext,
    ) -> Result<Box<Net<GuestAddressSpaceImpl, QueueSync, GuestRegionImpl>>, VirtioError> {
        slog::info!(
            ctx.logger(),
            "create a vhost-net device";
            "subsystem" => "vhost_net_dev_mgr",
            "id" => &cfg.iface_id,
            "host_dev_name" => &cfg.host_dev_name,
        );
        let epoll_mgr = ctx.epoll_mgr.clone().ok_or(VirtioError::InvalidInput)?;
        Ok(Box::new(Net::new(
            cfg.host_dev_name.clone(),
            cfg.guest_mac(),
            Arc::new(cfg.queue_sizes()),
            epoll_mgr,
        )?))
    }

    /// Insert or update a vhost-net device into the device manager. If it is a
    /// hotplug device, then it will be attached to the hypervisor.
    pub fn insert_device(
        device_mgr: &mut DeviceManager,
        mut ctx: DeviceOpContext,
        config: VhostNetDeviceConfigInfo,
    ) -> Result<(), VhostNetDeviceError> {
        if config.num_queues % 2 != 0 {
            return Err(VhostNetDeviceError::InvalidQueueNum(config.num_queues));
        }
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            return Err(VhostNetDeviceError::UpdateNotAllowedPostBoot);
        }

        slog::info!(
            ctx.logger(),
            "add vhost-net device configuration";
            "subsystem" => "vhost_net_dev_mgr",
            "id" => &config.iface_id,
            "host_dev_name" => &config.host_dev_name,
        );

        let mgr = &mut device_mgr.vhost_net_manager;
        let device_index = mgr.info_list.insert_or_update(&config)?;

        // If it is a hotplug device, then it will be attached immediately.
        if ctx.is_hotplug {
            slog::info!(
                ctx.logger(),
                "attach vhost-net device";
                "subsystem" => "vhost_net_dev_mgr",
                "id" => &config.iface_id,
                "host_dev_name" => &config.host_dev_name,
            );

            match Self::create_device(&config, &mut ctx) {
                Ok(device) => {
                    let mmio_dev = DeviceManager::create_mmio_virtio_device(
                        device,
                        &mut ctx,
                        config.use_shared_irq.unwrap_or(mgr.use_shared_irq),
                        config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                    )
                    .map_err(VhostNetDeviceError::RegisterNetDevice)?;
                    ctx.insert_hotplug_mmio_device(&mmio_dev, None)
                        .map_err(VhostNetDeviceError::DeviceManager)?;
                    // live-upgrade need save/restore device from info.device.
                    mgr.info_list[device_index].set_device(mmio_dev);
                }
                Err(err) => {
                    mgr.info_list.remove(device_index);
                    return Err(VhostNetDeviceError::Virtio(err));
                }
            }
        }

        Ok(())
    }

    /// Attach all configured vhost-net device to the virtual machine instance.
    pub fn attach_devices(&mut self, ctx: &mut DeviceOpContext) -> Result<(), VhostNetDeviceError> {
        for info in self.info_list.iter_mut() {
            slog::info!(
                ctx.logger(),
                "attach vhost-net device";
                "subsystem" => "vhost_net_dev_mgr",
                "id" => &info.config.iface_id,
                "host_dev_name" => &info.config.host_dev_name,
            );

            let device = Self::create_device(&info.config, ctx)
                .map_err(VhostNetDeviceError::CreateNetDevice)?;
            let mmio_dev = DeviceManager::create_mmio_virtio_device(
                device,
                ctx,
                info.config.use_shared_irq.unwrap_or(self.use_shared_irq),
                info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
            )
            .map_err(VhostNetDeviceError::RegisterNetDevice)?;
            info.set_device(mmio_dev);
        }
        Ok(())
    }

    /// Remove all vhost-net devices.
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

impl Default for VhostNetDeviceMgr {
    fn default() -> Self {
        Self {
            info_list: DeviceConfigInfos::new(),
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}

#[cfg(test)]
mod tests {
    use dbs_utils::net::MacAddr;
    use dbs_virtio_devices::Error as VirtioError;

    use crate::{
        device_manager::{
            vhost_net_dev_mgr::{VhostNetDeviceConfigInfo, VhostNetDeviceError, VhostNetDeviceMgr},
            DeviceManager, DeviceMgrError, DeviceOpContext,
        },
        test_utils::tests::create_vm_for_test,
        vm::VmConfigInfo,
    };

    #[test]
    fn test_create_vhost_net_device() {
        let vm = create_vm_for_test();
        let mgr = DeviceManager::new_test_mgr();
        let id_1 = String::from("id_1");
        let host_dev_name_1 = String::from("dev1");
        let guest_mac_1 = "01:23:45:67:89:0a";

        let netif_1 = VhostNetDeviceConfigInfo {
            iface_id: id_1,
            host_dev_name: host_dev_name_1,
            num_queues: 2,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_1).unwrap()),
            allow_duplicate_mac: false,
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
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        assert!(VhostNetDeviceMgr::create_device(&netif_1, &mut ctx).is_err());
    }

    #[test]
    fn test_attach_vhost_net_device() {
        // Init vm for test.
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

        let id_1 = String::from("id_1");
        let host_dev_name_1 = String::from("dev1");
        let guest_mac_1 = "01:23:45:67:89:0a";

        let netif_1 = VhostNetDeviceConfigInfo {
            iface_id: id_1,
            host_dev_name: host_dev_name_1,
            num_queues: 2,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_1).unwrap()),
            allow_duplicate_mac: false,
            use_shared_irq: None,
            use_generic_irq: None,
        };

        assert!(
            VhostNetDeviceMgr::insert_device(vm.device_manager_mut(), device_op_ctx, netif_1)
                .is_ok()
        );
        assert_eq!(vm.device_manager().vhost_net_manager.info_list.len(), 1);

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
            .vhost_net_manager
            .attach_devices(&mut device_op_ctx)
            .is_ok());
    }

    #[test]
    fn test_insert_vhost_net_device() {
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();

        let id_1 = String::from("id_1");
        let mut host_dev_name_1 = String::from("dev1");
        let mut guest_mac_1 = "01:23:45:67:89:0a";

        // Test create.
        let mut netif_1 = VhostNetDeviceConfigInfo {
            iface_id: id_1,
            host_dev_name: host_dev_name_1,
            num_queues: 2,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_1).unwrap()),
            allow_duplicate_mac: false,
            use_shared_irq: None,
            use_generic_irq: None,
        };

        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        assert!(VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1.clone()).is_ok());
        assert_eq!(mgr.vhost_net_manager.info_list.len(), 1);

        // Test update mac address (this test does not modify the tap).
        guest_mac_1 = "01:23:45:67:89:0b";
        netif_1.guest_mac = Some(MacAddr::parse_str(guest_mac_1).unwrap());
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        assert!(VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1.clone()).is_ok());
        assert_eq!(mgr.vhost_net_manager.info_list.len(), 1);

        // Test update host_dev_name (the tap will be updated).
        host_dev_name_1 = String::from("dev2");
        netif_1.host_dev_name = host_dev_name_1;
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        assert!(VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1).is_ok());
        assert_eq!(mgr.vhost_net_manager.info_list.len(), 1);
    }

    #[test]
    fn test_vhost_net_insert_error_cases() {
        let vm = create_vm_for_test();
        let mut mgr = DeviceManager::new_test_mgr();

        let guest_mac_1 = "01:23:45:67:89:0a";
        let guest_mac_2 = "11:45:45:67:89:0b";

        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );

        // invalid queue num
        let mut netif_1 = VhostNetDeviceConfigInfo {
            iface_id: String::from("id_1"),
            host_dev_name: String::from("dev_1"),
            num_queues: 1,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_1).unwrap()),
            allow_duplicate_mac: false,
            use_shared_irq: None,
            use_generic_irq: None,
        };

        let res = VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1.clone());
        if let Err(VhostNetDeviceError::InvalidQueueNum(1)) = res {
            assert_eq!(mgr.vhost_net_manager.info_list.len(), 0);
        } else {
            panic!();
        }

        // Adding the first valid network config.
        netif_1.num_queues = 2;
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        assert!(VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1.clone()).is_ok());
        assert_eq!(mgr.vhost_net_manager.info_list.len(), 1);

        // Error Cases for CREATE
        // Error Case: Add new network config with the same host_dev_name
        netif_1.iface_id = String::from("id_2");
        netif_1.guest_mac = Some(MacAddr::parse_str(guest_mac_2).unwrap());
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        let res = VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1.clone());
        if let Err(VhostNetDeviceError::HostDeviceNameInUse(_)) = res {
            assert_eq!(mgr.vhost_net_manager.info_list.len(), 1);
        } else {
            panic!();
        }

        // Error Cases for CREATE
        // Error Case: Add new network config with the same guest_address
        netif_1.iface_id = String::from("id_2");
        netif_1.host_dev_name = String::from("dev_2");
        netif_1.guest_mac = Some(MacAddr::parse_str(guest_mac_1).unwrap());
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        let res = VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_1);
        if let Err(VhostNetDeviceError::GuestMacAddressInUse(_)) = res {
            assert_eq!(mgr.vhost_net_manager.info_list.len(), 1);
        } else {
            panic!();
        }

        // Adding the second valid network config.
        let mut netif_2 = VhostNetDeviceConfigInfo {
            iface_id: String::from("id_2"),
            host_dev_name: String::from("dev_2"),
            num_queues: 2,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_2).unwrap()),
            allow_duplicate_mac: false,
            use_shared_irq: None,
            use_generic_irq: None,
        };
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        assert!(VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_2.clone()).is_ok());
        assert_eq!(mgr.vhost_net_manager.info_list.len(), 2);

        // Error Cases for UPDATE
        // Error Case: update netif_2 network config with the same host_dev_name as netif_1
        netif_2.host_dev_name = String::from("dev_1");
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        let res = VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_2.clone());
        if let Err(VhostNetDeviceError::HostDeviceNameInUse(_)) = res {
            assert_eq!(mgr.vhost_net_manager.info_list.len(), 2);
        } else {
            panic!();
        }

        // Error Cases for UPDATE
        // Error Case: update netif_2 network config with the same guest_address as netif_
        netif_2.guest_mac = Some(MacAddr::parse_str(guest_mac_1).unwrap());
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        let res = VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_2);
        if let Err(VhostNetDeviceError::GuestMacAddressInUse(_)) = res {
            assert_eq!(mgr.vhost_net_manager.info_list.len(), 2);
        } else {
            panic!();
        }

        // Adding the third valid network config with same mac.
        let netif_3 = VhostNetDeviceConfigInfo {
            iface_id: String::from("id_3"),
            host_dev_name: String::from("dev_3"),
            num_queues: 2,
            queue_size: 128,
            guest_mac: Some(MacAddr::parse_str(guest_mac_1).unwrap()),
            allow_duplicate_mac: true,
            use_shared_irq: None,
            use_generic_irq: None,
        };
        let ctx = DeviceOpContext::new(
            None,
            &mgr,
            None,
            None,
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        assert!(VhostNetDeviceMgr::insert_device(&mut mgr, ctx, netif_3).is_ok());
        assert_eq!(mgr.vhost_net_manager.info_list.len(), 3);
    }

    #[test]
    fn test_vhost_net_error_display() {
        let err = VhostNetDeviceError::InvalidQueueNum(0);
        let _ = format!("{}{:?}", err, err);

        let err = VhostNetDeviceError::DeviceManager(DeviceMgrError::GetDeviceResource);
        let _ = format!("{}{:?}", err, err);

        let err = VhostNetDeviceError::DeviceIdAlreadyExist(String::from("1"));
        let _ = format!("{}{:?}", err, err);

        let err = VhostNetDeviceError::GuestMacAddressInUse(String::from("1"));
        let _ = format!("{}{:?}", err, err);

        let err = VhostNetDeviceError::HostDeviceNameInUse(String::from("1"));
        let _ = format!("{}{:?}", err, err);

        let err = VhostNetDeviceError::Virtio(VirtioError::DescriptorChainTooShort);
        let _ = format!("{}{:?}", err, err);

        let err = VhostNetDeviceError::UpdateNotAllowedPostBoot;
        let _ = format!("{}{:?}", err, err);
    }
}
