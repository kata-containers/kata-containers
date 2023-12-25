// Copyright 2020 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use dbs_virtio_devices as virtio;
use serde_derive::{Deserialize, Serialize};
use slog::{error, info};
use virtio::balloon::{Balloon, BalloonConfig};
use virtio::Error as VirtioError;

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::config_manager::{ConfigItem, DeviceConfigInfo, DeviceConfigInfos};
use crate::device_manager::DbsMmioV2Device;
use crate::device_manager::{DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::metric::METRICS;

// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = false;

/// Errors associated with `BalloonDeviceConfig`.
#[derive(Debug, thiserror::Error)]
pub enum BalloonDeviceError {
    /// The balloon device was already used.
    #[error("the virtio-balloon ID was already added to a different device")]
    BalloonDeviceAlreadyExists,

    /// Cannot perform the requested operation after booting the microVM.
    #[error("the update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// guest memory error
    #[error("failed to access guest memory, {0}")]
    GuestMemoryError(#[source] vm_memory::mmap::Error),

    /// create balloon device error
    #[error("failed to create virtio-balloon device, {0}")]
    CreateBalloonDevice(#[source] virtio::Error),

    /// hotplug balloon device error
    #[error("cannot hotplug virtio-balloon device, {0}")]
    HotplugDeviceFailed(#[source] DeviceMgrError),

    /// create mmio device error
    #[error("cannot create virtio-balloon mmio device, {0}")]
    CreateMmioDevice(#[source] DeviceMgrError),

    /// Cannot initialize a balloon device or add a device to the MMIO Bus.
    #[error("failure while registering balloon device: {0}")]
    RegisterBalloonDevice(#[source] DeviceMgrError),

    /// resize balloon device error
    #[error("failure while resizing virtio-balloon device, {0}")]
    ResizeFailed(#[source] VirtioError),

    /// The balloon device id doesn't exist.
    #[error("invalid balloon device id '{0}'")]
    InvalidDeviceId(String),

    /// balloon device does not exist
    #[error("balloon device does not exist")]
    NotExist,

    /// The device manager errors.
    #[error("DeviceManager error: {0}")]
    DeviceManager(#[source] DeviceMgrError),
}

/// Configuration information for a virtio-balloon device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BalloonDeviceConfigInfo {
    /// Unique identifier of the balloon device
    pub balloon_id: String,
    /// Resize balloon size in mib
    pub size_mib: u64,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
    /// VIRTIO_BALLOON_F_DEFLATE_ON_OOM
    pub f_deflate_on_oom: bool,
    /// VIRTIO_BALLOON_F_REPORTING
    pub f_reporting: bool,
}

impl ConfigItem for BalloonDeviceConfigInfo {
    type Err = BalloonDeviceError;

    fn id(&self) -> &str {
        &self.balloon_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), BalloonDeviceError> {
        if self.balloon_id.as_str() == other.balloon_id.as_str() {
            Err(BalloonDeviceError::BalloonDeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Balloon Device Info
pub type BalloonDeviceInfo = DeviceConfigInfo<BalloonDeviceConfigInfo>;

impl ConfigItem for BalloonDeviceInfo {
    type Err = BalloonDeviceError;

    fn id(&self) -> &str {
        &self.config.balloon_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), BalloonDeviceError> {
        if self.config.balloon_id.as_str() == other.config.balloon_id.as_str() {
            Err(BalloonDeviceError::BalloonDeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Wrapper for the collection that holds all the Balloon Devices Configs
#[derive(Clone)]
pub struct BalloonDeviceMgr {
    /// A list of `BalloonDeviceConfig` objects.
    info_list: DeviceConfigInfos<BalloonDeviceConfigInfo>,
    pub(crate) use_shared_irq: bool,
}

impl BalloonDeviceMgr {
    /// Inserts `balloon_cfg` in the virtio-balloon device configuration list.
    /// If an entry with the same id already exists, it will attempt to update
    /// the existing entry.
    pub fn insert_or_update_device(
        &mut self,
        mut ctx: DeviceOpContext,
        balloon_cfg: BalloonDeviceConfigInfo,
    ) -> std::result::Result<(), BalloonDeviceError> {
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            error!(ctx.logger(), "hotplug feature has been disabled.";
            "subsystem" => "balloon_dev_mgr",);
            return Err(BalloonDeviceError::UpdateNotAllowedPostBoot);
        }

        let epoll_mgr = ctx
            .get_epoll_mgr()
            .map_err(BalloonDeviceError::DeviceManager)?;

        // If the id of the drive already exists in the list, the operation is update.
        if let Some(index) = self.get_index_of_balloon_dev(&balloon_cfg.balloon_id) {
            // Update an existing balloon device
            if ctx.is_hotplug {
                info!(ctx.logger(), "resize virtio balloon size to {:?}", balloon_cfg.size_mib; "subsystem" => "balloon_dev_mgr");
                self.update_balloon_size(index, balloon_cfg.size_mib)?;
            }
            self.info_list.insert_or_update(&balloon_cfg)?;
        } else {
            // Create a new balloon device
            if !self.info_list.is_empty() {
                error!(ctx.logger(), "only support one balloon device!"; "subsystem" => "balloon_dev_mgr");
                return Err(BalloonDeviceError::BalloonDeviceAlreadyExists);
            }

            if !ctx.is_hotplug {
                self.info_list.insert_or_update(&balloon_cfg)?;
                return Ok(());
            }

            info!(ctx.logger(), "hotplug balloon device: {}", balloon_cfg.balloon_id; "subsystem" => "balloon_dev_mgr");
            let device = Box::new(
                virtio::balloon::Balloon::new(
                    epoll_mgr,
                    BalloonConfig {
                        f_deflate_on_oom: balloon_cfg.f_deflate_on_oom,
                        f_reporting: balloon_cfg.f_reporting,
                    },
                )
                .map_err(BalloonDeviceError::CreateBalloonDevice)?,
            );
            METRICS
                .write()
                .unwrap()
                .balloon
                .insert(balloon_cfg.balloon_id.clone(), device.metrics());

            let mmio_dev =
                DeviceManager::create_mmio_virtio_device_with_device_change_notification(
                    device,
                    &mut ctx,
                    balloon_cfg.use_shared_irq.unwrap_or(self.use_shared_irq),
                    balloon_cfg.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                )
                .map_err(BalloonDeviceError::CreateMmioDevice)?;
            ctx.insert_hotplug_mmio_device(&mmio_dev, None)
                .map_err(|e| {
                    error!(
                        ctx.logger(),
                        "hotplug balloon device {} error: {}",
                        &balloon_cfg.balloon_id, e;
                        "subsystem" => "balloon_dev_mgr"
                    );
                    BalloonDeviceError::HotplugDeviceFailed(e)
                })?;
            let index = self.info_list.insert_or_update(&balloon_cfg)?;
            self.info_list[index].set_device(mmio_dev);
        }
        Ok(())
    }

    /// Attaches all virtio-balloon devices from the BalloonDevicesConfig.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), BalloonDeviceError> {
        let epoll_mgr = ctx
            .get_epoll_mgr()
            .map_err(BalloonDeviceError::DeviceManager)?;

        for info in self.info_list.iter_mut() {
            info!(ctx.logger(), "attach balloon device: {}", info.config.balloon_id; "subsystem" => "balloon_dev_mgr");

            let device = Balloon::new(
                epoll_mgr.clone(),
                BalloonConfig {
                    f_deflate_on_oom: info.config.f_deflate_on_oom,
                    f_reporting: info.config.f_reporting,
                },
            )
            .map_err(BalloonDeviceError::CreateBalloonDevice)?;
            METRICS
                .write()
                .unwrap()
                .balloon
                .insert(info.config.balloon_id.clone(), device.metrics());
            let mmio_dev =
                DeviceManager::create_mmio_virtio_device_with_device_change_notification(
                    Box::new(device),
                    ctx,
                    info.config.use_shared_irq.unwrap_or(self.use_shared_irq),
                    info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                )
                .map_err(BalloonDeviceError::RegisterBalloonDevice)?;
            info.set_device(mmio_dev);
        }

        Ok(())
    }

    fn update_balloon_size(
        &self,
        index: usize,
        size_mib: u64,
    ) -> std::result::Result<(), BalloonDeviceError> {
        let device = self.info_list[index]
            .device
            .as_ref()
            .ok_or_else(|| BalloonDeviceError::NotExist)?;
        if let Some(mmio_dev) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
            let guard = mmio_dev.state();
            let inner_dev = guard.get_inner_device();
            if let Some(balloon_dev) = inner_dev
                .as_any()
                .downcast_ref::<Balloon<GuestAddressSpaceImpl>>()
            {
                return balloon_dev
                    .set_size(size_mib)
                    .map_err(BalloonDeviceError::ResizeFailed);
            }
        }
        Ok(())
    }

    fn get_index_of_balloon_dev(&self, balloon_id: &str) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.balloon_id.eq(balloon_id))
    }
}

impl Default for BalloonDeviceMgr {
    /// Create a new `BalloonDeviceMgr` object..
    fn default() -> Self {
        BalloonDeviceMgr {
            info_list: DeviceConfigInfos::new(),
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}

impl Drop for BalloonDeviceMgr {
    // todo: move METIRCS oprations to remove_device. issue #8207.
    fn drop(&mut self) {
        METRICS.write().unwrap().balloon.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_manager::tests::create_address_space;
    use crate::test_utils::tests::create_vm_for_test;

    impl Default for BalloonDeviceConfigInfo {
        fn default() -> Self {
            BalloonDeviceConfigInfo {
                balloon_id: "".to_string(),
                size_mib: 0,
                use_generic_irq: None,
                use_shared_irq: None,
                f_deflate_on_oom: false,
                f_reporting: false,
            }
        }
    }

    #[test]
    fn test_balloon_config_check_conflicts() {
        let config = BalloonDeviceConfigInfo::default();
        let mut config2 = BalloonDeviceConfigInfo::default();
        assert!(config.check_conflicts(&config2).is_err());
        config2.balloon_id = "dummy_balloon".to_string();
        assert!(config.check_conflicts(&config2).is_ok());
    }

    #[test]
    fn test_create_balloon_devices_configs() {
        let mgr = BalloonDeviceMgr::default();
        assert_eq!(mgr.info_list.len(), 0);
        assert_eq!(mgr.get_index_of_balloon_dev(""), None);
    }

    #[test]
    fn test_balloon_insert_or_update_device() {
        //Init vm for test.
        let mut vm = create_vm_for_test();

        // Test for standard config
        let device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            None,
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );

        let dummy_balloon_device = BalloonDeviceConfigInfo::default();
        vm.device_manager_mut()
            .balloon_manager
            .insert_or_update_device(device_op_ctx, dummy_balloon_device)
            .unwrap();
        assert_eq!(vm.device_manager().balloon_manager.info_list.len(), 1);
    }

    #[test]
    fn test_balloon_attach_device() {
        //Init vm and insert balloon config for test.
        let mut vm = create_vm_for_test();
        let device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            Some(create_address_space()),
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );

        let dummy_balloon_device = BalloonDeviceConfigInfo::default();
        vm.device_manager_mut()
            .balloon_manager
            .insert_or_update_device(device_op_ctx, dummy_balloon_device)
            .unwrap();
        assert_eq!(vm.device_manager().balloon_manager.info_list.len(), 1);

        // Test for standard config
        let mut device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            Some(create_address_space()),
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );
        assert!(vm
            .device_manager_mut()
            .balloon_manager
            .attach_devices(&mut device_op_ctx)
            .is_ok());
        assert_eq!(vm.device_manager().balloon_manager.info_list.len(), 1);
    }

    #[test]
    fn test_balloon_update_device() {
        //Init vm for test.
        let mut vm = create_vm_for_test();
        let device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            Some(create_address_space()),
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );

        let dummy_balloon_device = BalloonDeviceConfigInfo::default();
        vm.device_manager_mut()
            .balloon_manager
            .insert_or_update_device(device_op_ctx, dummy_balloon_device)
            .unwrap();
        assert_eq!(vm.device_manager().balloon_manager.info_list.len(), 1);

        let mut device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            Some(create_address_space()),
            false,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );

        assert!(vm
            .device_manager_mut()
            .balloon_manager
            .attach_devices(&mut device_op_ctx)
            .is_ok());
        assert_eq!(vm.device_manager().balloon_manager.info_list.len(), 1);

        assert!(vm
            .device_manager()
            .balloon_manager
            .update_balloon_size(0, 200)
            .is_ok());
    }
}
