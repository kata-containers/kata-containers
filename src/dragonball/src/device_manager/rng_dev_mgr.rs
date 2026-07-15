// Copyright 2026 Ant Group. All Rights Reserved.
// Copyright 2021 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use dbs_virtio_devices as virtio;
use serde_derive::{Deserialize, Serialize};
use slog::{error, info};
use virtio::rng::Rng;

use crate::config_manager::{ConfigItem, DeviceConfigInfo, DeviceConfigInfos};
use crate::device_manager::{DeviceManager, DeviceMgrError, DeviceOpContext};

// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq. It is for the virtio-mmio MSI
// extension, which is not supported by the guest kernels in use, so it is
// disabled by default.
const USE_GENERIC_IRQ: bool = false;

/// Errors associated with `RngDeviceConfig`.
#[derive(Debug, thiserror::Error)]
pub enum RngDeviceError {
    /// The rng device was already used.
    #[error("the virtio-rng device was already added to a different device")]
    RngDeviceAlreadyExists,

    /// Cannot perform the requested operation after booting the microVM.
    #[error("the update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// create rng device error
    #[error("failed to create virtio-rng device, {0}")]
    CreateRngDevice(#[source] virtio::Error),

    /// Cannot initialize a rng device or add a device to the MMIO Bus.
    #[error("failure while registering rng device: {0}")]
    RegisterRngDevice(#[source] DeviceMgrError),

    /// The device manager errors.
    #[error("DeviceManager error: {0}")]
    DeviceManager(#[source] DeviceMgrError),
}

/// Configuration information for a virtio-rng device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RngDeviceConfigInfo {
    /// Path to the host entropy source, which doubles as the rng device id.
    pub src: String,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl ConfigItem for RngDeviceConfigInfo {
    type Err = RngDeviceError;

    fn id(&self) -> &str {
        &self.src
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), RngDeviceError> {
        if self.src.as_str() == other.src.as_str() {
            Err(RngDeviceError::RngDeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Rng Device Info
pub type RngDeviceInfo = DeviceConfigInfo<RngDeviceConfigInfo>;

impl ConfigItem for RngDeviceInfo {
    type Err = RngDeviceError;

    fn id(&self) -> &str {
        &self.config.src
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), RngDeviceError> {
        if self.config.src.as_str() == other.config.src.as_str() {
            Err(RngDeviceError::RngDeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Wrapper for the collection that holds all the Rng Devices Configs
#[derive(Clone)]
pub struct RngDeviceMgr {
    /// A list of `RngDeviceConfig` objects.
    info_list: DeviceConfigInfos<RngDeviceConfigInfo>,
}

impl RngDeviceMgr {
    /// Inserts `rng_cfg` in the virtio-rng device configuration list.
    /// The virtio-rng device is a cold-plug only device, so hotplug requests
    /// are rejected.
    pub fn insert_or_update_device(
        &mut self,
        ctx: DeviceOpContext,
        rng_cfg: RngDeviceConfigInfo,
    ) -> std::result::Result<(), RngDeviceError> {
        if ctx.is_hotplug {
            error!(ctx.logger(), "no support of virtio-rng device hotplug";
                "subsystem" => "rng_dev_mgr",
                "src" => &rng_cfg.src,
            );
            return Err(RngDeviceError::UpdateNotAllowedPostBoot);
        }

        info!(ctx.logger(), "add virtio-rng device configuration";
            "subsystem" => "rng_dev_mgr",
            "src" => &rng_cfg.src,
        );
        self.info_list.insert_or_update(&rng_cfg)?;

        Ok(())
    }

    /// Attaches all virtio-rng devices from the RngDevicesConfig.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), RngDeviceError> {
        let epoll_mgr = ctx.get_epoll_mgr().map_err(RngDeviceError::DeviceManager)?;

        for info in self.info_list.iter_mut() {
            info!(ctx.logger(), "attach virtio-rng device";
                "subsystem" => "rng_dev_mgr",
                "src" => &info.config.src,
            );

            let device = Rng::new(info.config.src.clone(), epoll_mgr.clone())
                .map_err(RngDeviceError::CreateRngDevice)?;
            let mmio_dev = DeviceManager::create_mmio_virtio_device(
                Box::new(device),
                ctx,
                info.config.use_shared_irq.unwrap_or(USE_SHARED_IRQ),
                info.config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
            )
            .map_err(RngDeviceError::RegisterRngDevice)?;
            info.set_device(mmio_dev);
        }

        Ok(())
    }
}

impl Default for RngDeviceMgr {
    /// Create a new `RngDeviceMgr` object..
    fn default() -> Self {
        RngDeviceMgr {
            info_list: DeviceConfigInfos::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_manager::tests::create_address_space;
    use crate::test_utils::tests::create_vm_for_test;
    use test_utils::skip_if_kvm_unaccessable;

    impl Default for RngDeviceConfigInfo {
        fn default() -> Self {
            RngDeviceConfigInfo {
                src: "".to_string(),
                use_shared_irq: None,
                use_generic_irq: None,
            }
        }
    }

    #[test]
    fn test_rng_config_check_conflicts() {
        let config = RngDeviceConfigInfo::default();
        let mut config2 = RngDeviceConfigInfo::default();
        assert!(config.check_conflicts(&config2).is_err());
        config2.src = "/dev/urandom".to_string();
        assert!(config.check_conflicts(&config2).is_ok());
    }

    #[test]
    fn test_rng_insert_or_update_device() {
        skip_if_kvm_unaccessable!();
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

        let dummy_rng_device = RngDeviceConfigInfo {
            src: "/dev/urandom".to_string(),
            use_shared_irq: None,
            use_generic_irq: None,
        };
        vm.device_manager_mut()
            .rng_manager
            .insert_or_update_device(device_op_ctx, dummy_rng_device)
            .unwrap();
        assert_eq!(vm.device_manager().rng_manager.info_list.len(), 1);

        // Hotplug is rejected
        let device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            None,
            true,
            Some(vm.vm_config().clone()),
            vm.shared_info().clone(),
        );
        let dummy_rng_device = RngDeviceConfigInfo {
            src: "/dev/random".to_string(),
            use_shared_irq: None,
            use_generic_irq: None,
        };
        assert!(matches!(
            vm.device_manager_mut()
                .rng_manager
                .insert_or_update_device(device_op_ctx, dummy_rng_device),
            Err(RngDeviceError::UpdateNotAllowedPostBoot)
        ));
        assert_eq!(vm.device_manager().rng_manager.info_list.len(), 1);
    }

    #[test]
    fn test_rng_attach_device() {
        skip_if_kvm_unaccessable!();
        //Init vm and insert rng config for test.
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

        let dummy_rng_device = RngDeviceConfigInfo {
            src: "/dev/urandom".to_string(),
            use_shared_irq: None,
            use_generic_irq: None,
        };
        vm.device_manager_mut()
            .rng_manager
            .insert_or_update_device(device_op_ctx, dummy_rng_device)
            .unwrap();
        assert_eq!(vm.device_manager().rng_manager.info_list.len(), 1);

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
            .rng_manager
            .attach_devices(&mut device_op_ctx)
            .is_ok());
        assert_eq!(vm.device_manager().rng_manager.info_list.len(), 1);
    }
}
