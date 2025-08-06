// Copyright 2023 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! This module abstract some structs for constructing fdt. Instead of using
//! multiple parameters.

use std::collections::HashMap;

use dbs_arch::{gic::GICDevice, DeviceInfoForFDT, DeviceType, VpmuFeatureLevel};
use vm_memory::mmap::GuestMemoryMmap;

use crate::InitrdConfig;

/// Struct to save vcpu information
pub struct FdtVcpuInfo {
    /// vcpu mpidrs
    vcpu_mpidr: Vec<u64>,
    /// vcpu boot-onlined
    vcpu_boot_onlined: Vec<u32>,
    /// vpmu feature
    vpmu_feature: VpmuFeatureLevel,
    // TODO: #274 cache passthrough
    /// cache passthrough
    cache_passthrough_enabled: bool,
}

impl FdtVcpuInfo {
    /// Generate FdtVcpuInfo
    pub fn new(
        vcpu_mpidr: Vec<u64>,
        vcpu_boot_onlined: Vec<u32>,
        vpmu_feature: VpmuFeatureLevel,
        cache_passthrough_enabled: bool,
    ) -> Self {
        FdtVcpuInfo {
            vcpu_mpidr,
            vcpu_boot_onlined,
            vpmu_feature,
            cache_passthrough_enabled,
        }
    }
}

/// Struct to save vm information.
pub struct FdtVmInfo<'a> {
    /// guest meory
    guest_memory: &'a GuestMemoryMmap,
    /// command line
    cmdline: &'a str,
    /// initrd config
    initrd_config: Option<&'a InitrdConfig>,
    /// vcpu information
    vcpu_info: FdtVcpuInfo,
}

impl FdtVmInfo<'_> {
    /// Generate FdtVmInfo.
    pub fn new<'a>(
        guest_memory: &'a GuestMemoryMmap,
        cmdline: &'a str,
        initrd_config: Option<&'a InitrdConfig>,
        vcpu_info: FdtVcpuInfo,
    ) -> FdtVmInfo<'a> {
        FdtVmInfo {
            guest_memory,
            cmdline,
            initrd_config,
            vcpu_info,
        }
    }

    /// Get guest_memory.
    pub fn get_guest_memory(&self) -> &GuestMemoryMmap {
        self.guest_memory
    }

    /// Get cmdline.
    pub fn get_cmdline(&self) -> &str {
        self.cmdline
    }

    /// Get initrd_config.
    pub fn get_initrd_config(&self) -> Option<&InitrdConfig> {
        self.initrd_config
    }

    /// Get vcpu_mpidr.
    pub fn get_vcpu_mpidr(&self) -> &[u64] {
        self.vcpu_info.vcpu_mpidr.as_slice()
    }

    /// Get vpmu_feature.
    pub fn get_boot_onlined(&self) -> &[u32] {
        self.vcpu_info.vcpu_boot_onlined.as_slice()
    }

    /// Get vpmu_feature.
    pub fn get_vpmu_feature(&self) -> VpmuFeatureLevel {
        self.vcpu_info.vpmu_feature
    }

    /// Get cache_passthrough_enabled.
    pub fn get_cache_passthrough_enabled(&self) -> bool {
        self.vcpu_info.cache_passthrough_enabled
    }
}

// This struct is used for cache passthrough and numa passthrough
// TODO: #274 cache passthrough
// TODO: #275 numa passthrough
/// Struct to save numa information.
#[derive(Default)]
pub struct FdtNumaInfo {
    /// vcpu -> pcpu maps
    cpu_maps: Option<Vec<u8>>,
    /// numa id map vector for memory
    memory_numa_id_map: Option<Vec<u32>>,
    /// numa id map vector for vcpu
    vcpu_numa_id_map: Option<Vec<u32>>,
}

impl FdtNumaInfo {
    /// Generate FdtNumaInfo.
    pub fn new(
        cpu_maps: Option<Vec<u8>>,
        memory_numa_id_map: Option<Vec<u32>>,
        vcpu_numa_id_map: Option<Vec<u32>>,
    ) -> Self {
        FdtNumaInfo {
            cpu_maps,
            memory_numa_id_map,
            vcpu_numa_id_map,
        }
    }

    /// Get cpu_maps struct.
    pub fn get_cpu_maps(&self) -> Option<Vec<u8>> {
        self.cpu_maps.clone()
    }

    /// Get memory_numa_id_map struct.
    pub fn get_memory_numa_id_map(&self) -> Option<&Vec<u32>> {
        self.memory_numa_id_map.as_ref()
    }

    /// Get vcpu_numa_id_map struct.
    pub fn get_vcpu_numa_id_map(&self) -> Option<&Vec<u32>> {
        self.vcpu_numa_id_map.as_ref()
    }
}

/// Struct to save device information.
pub struct FdtDeviceInfo<'a, T: DeviceInfoForFDT> {
    /// mmio device information
    mmio_device_info: Option<&'a HashMap<(DeviceType, String), T>>,
    /// interrupt controller
    irq_chip: &'a dyn GICDevice,
}

impl<T: DeviceInfoForFDT> FdtDeviceInfo<'_, T> {
    /// Generate FdtDeviceInfo.
    pub fn new<'a>(
        mmio_device_info: Option<&'a HashMap<(DeviceType, String), T>>,
        irq_chip: &'a dyn GICDevice,
    ) -> FdtDeviceInfo<'a, T> {
        FdtDeviceInfo {
            mmio_device_info,
            irq_chip,
        }
    }

    /// Get mmio device information.
    pub fn get_mmio_device_info(&self) -> Option<&HashMap<(DeviceType, String), T>> {
        self.mmio_device_info
    }

    /// Get interrupt controller.
    pub fn get_irqchip(&self) -> &dyn GICDevice {
        self.irq_chip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use dbs_arch::gic::create_gic;
    use vm_memory::{GuestAddress, GuestMemory};

    const CMDLINE: &str = "console=tty0";
    const INITRD_CONFIG: InitrdConfig = InitrdConfig {
        address: GuestAddress(0x10000000),
        size: 0x1000,
    };
    const VCPU_MPIDR: [u64; 1] = [0];
    const VCPU_BOOT_ONLINED: [u32; 1] = [1];
    const VPMU_FEATURE: VpmuFeatureLevel = VpmuFeatureLevel::Disabled;
    const CACHE_PASSTHROUGH_ENABLED: bool = false;

    #[inline]
    fn helper_generate_fdt_vm_info(guest_memory: &GuestMemoryMmap) -> FdtVmInfo<'_> {
        FdtVmInfo::new(
            guest_memory,
            CMDLINE,
            Some(&INITRD_CONFIG),
            FdtVcpuInfo::new(
                VCPU_MPIDR.to_vec(),
                VCPU_BOOT_ONLINED.to_vec(),
                VPMU_FEATURE,
                CACHE_PASSTHROUGH_ENABLED,
            ),
        )
    }

    #[test]
    fn test_fdtutils_fdt_vm_info() {
        let ranges = vec![(GuestAddress(0x80000000), 0x40000)];
        let guest_memory: GuestMemoryMmap<()> =
            GuestMemoryMmap::<()>::from_ranges(ranges.as_slice())
                .expect("Cannot initialize memory");
        let vm_info = helper_generate_fdt_vm_info(&guest_memory);

        assert_eq!(
            guest_memory.check_address(GuestAddress(0x80001000)),
            Some(GuestAddress(0x80001000))
        );
        assert_eq!(guest_memory.check_address(GuestAddress(0x80050000)), None);
        assert!(guest_memory.check_range(GuestAddress(0x80000000), 0x40000));
        assert_eq!(vm_info.get_cmdline(), CMDLINE);
        assert_eq!(
            vm_info.get_initrd_config().unwrap().address,
            INITRD_CONFIG.address
        );
        assert_eq!(
            vm_info.get_initrd_config().unwrap().size,
            INITRD_CONFIG.size
        );
        assert_eq!(vm_info.get_vcpu_mpidr(), VCPU_MPIDR.as_slice());
        assert_eq!(vm_info.get_boot_onlined(), VCPU_BOOT_ONLINED.as_slice());
        assert_eq!(vm_info.get_vpmu_feature(), VPMU_FEATURE);
        assert_eq!(
            vm_info.get_cache_passthrough_enabled(),
            CACHE_PASSTHROUGH_ENABLED
        );
    }

    const CPU_MAPS: [u8; 5] = [1, 2, 3, 4, 5];
    const MEMORY_VEC: [u32; 2] = [0, 1];
    const CPU_VEC: [u32; 5] = [0, 0, 0, 1, 1];

    #[inline]
    fn helper_generate_fdt_numa_info() -> FdtNumaInfo {
        FdtNumaInfo::new(
            Some(CPU_MAPS.to_vec()),
            Some(MEMORY_VEC.to_vec()),
            Some(CPU_VEC.to_vec()),
        )
    }

    #[test]
    fn test_fdtutils_fdt_numa_info() {
        // test default
        let numa_info = FdtNumaInfo::default();
        assert_eq!(numa_info.get_cpu_maps(), None);
        assert_eq!(numa_info.get_memory_numa_id_map(), None);
        assert_eq!(numa_info.get_vcpu_numa_id_map(), None);

        let numa_info = helper_generate_fdt_numa_info();
        assert_eq!(
            numa_info.get_cpu_maps().unwrap().as_slice(),
            CPU_MAPS.as_slice()
        );
        assert_eq!(
            numa_info.get_memory_numa_id_map().unwrap().as_slice(),
            MEMORY_VEC.as_slice()
        );
        assert_eq!(
            numa_info.get_vcpu_numa_id_map().unwrap().as_slice(),
            CPU_VEC.as_slice()
        );
    }

    use dbs_arch::gic::its::ItsType;
    use dbs_device::resources::{DeviceResources, Resource};
    use kvm_ioctls::Kvm;

    use super::super::tests::MMIODeviceInfo;

    const MEMORY_SIZE: u64 = 4096;
    const ECAM_SPACE: [Resource; 1] = [Resource::MmioAddressRange {
        base: 0x40000000,
        size: 0x1000,
    }];
    const BAR_SPACE: [Resource; 2] = [
        Resource::MmioAddressRange {
            base: 0x40001000,
            size: 0x1000,
        },
        Resource::MmioAddressRange {
            base: 0x40002000,
            size: 0x1000,
        },
    ];

    #[test]
    fn test_fdtutils_fdt_device_info() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        let gic = create_gic(&vm, 0).unwrap();
        let mmio_device_info: Option<HashMap<(DeviceType, String), MMIODeviceInfo>> = Some(
            [
                (
                    (DeviceType::Serial, DeviceType::Serial.to_string()),
                    MMIODeviceInfo::new(0, 1),
                ),
                (
                    (DeviceType::Virtio(1), "virtio".to_string()),
                    MMIODeviceInfo::new(MEMORY_SIZE, 2),
                ),
                (
                    (DeviceType::RTC, "rtc".to_string()),
                    MMIODeviceInfo::new(2 * MEMORY_SIZE, 3),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
        );
        let mut ecam_space = DeviceResources::new();
        ecam_space.append(ECAM_SPACE.as_slice()[0].clone());

        let mut bar_space = DeviceResources::new();
        bar_space.append(BAR_SPACE.as_slice()[0].clone());
        bar_space.append(BAR_SPACE.as_slice()[1].clone());

        let its_type1 = ItsType::PciMsiIts;
        let its_type2 = ItsType::PlatformMsiIts;

        let device_info = FdtDeviceInfo::new(mmio_device_info.as_ref(), gic.as_ref());
        assert_eq!(
            device_info.get_mmio_device_info(),
            mmio_device_info.as_ref()
        );
        assert_eq!(
            format!("{:?}", device_info.get_irqchip().device_fd()),
            format!("{:?}", gic.as_ref().device_fd())
        );
        assert_eq!(
            device_info.get_irqchip().device_properties(),
            gic.as_ref().device_properties()
        );
        assert_eq!(
            device_info.get_irqchip().fdt_compatibility(),
            gic.as_ref().fdt_compatibility()
        );
        assert_eq!(
            device_info.get_irqchip().fdt_maint_irq(),
            gic.as_ref().fdt_maint_irq()
        );
        assert_eq!(
            device_info.get_irqchip().vcpu_count(),
            gic.as_ref().vcpu_count()
        );
        assert_eq!(
            device_info.get_irqchip().get_its_reg_range(&its_type1),
            gic.as_ref().get_its_reg_range(&its_type1)
        );
        assert_eq!(
            device_info.get_irqchip().get_its_reg_range(&its_type2),
            gic.as_ref().get_its_reg_range(&its_type2)
        );
    }
}
