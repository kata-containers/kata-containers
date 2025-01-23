// Copyright (c) 2019-2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Configuration information for hypervisors.
//!
//! The configuration information for hypervisors is complex, and different hypervisor requires
//! different configuration information. To make it flexible and extensible, we build a multi-layer
//! architecture to manipulate hypervisor configuration information.
//! - the vendor layer. The `HypervisorVendor` structure provides hook points for vendors to
//!   customize the configuration for its deployment.
//! - the hypervisor plugin layer. The hypervisor plugin layer provides hook points for different
//!   hypervisors to manipulate the configuration information.
//! - the hypervisor common layer. This layer handles generic logic for all types of hypervisors.
//!
//! These three layers are applied in order. So changes made by the vendor layer will be visible
//! to the hypervisor plugin layer and the common layer. And changes made by the plugin layer will
//! only be visible to the common layer.
//!
//! Ideally the hypervisor configuration information should be split into hypervisor specific
//! part and common part. But the Kata 2.0 has adopted a policy to build a superset for all
//! hypervisors, so let's contain it...

use super::{default, ConfigOps, ConfigPlugin, TomlConfig};
use crate::annotations::KATA_ANNO_CFG_HYPERVISOR_PREFIX;
use crate::{eother, resolve_path, sl, validate_path};
use byte_unit::{Byte, Unit};
use lazy_static::lazy_static;
use regex::RegexSet;
use serde_enum_str::{Deserialize_enum_str, Serialize_enum_str};
use std::collections::HashMap;
use std::io::{self, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};
use sysinfo::System;

mod dragonball;
pub use self::dragonball::{DragonballConfig, HYPERVISOR_NAME_DRAGONBALL};

mod qemu;
pub use self::qemu::{QemuConfig, HYPERVISOR_NAME_QEMU};

mod ch;
pub use self::ch::{CloudHypervisorConfig, HYPERVISOR_NAME_CH};

mod remote;
pub use self::remote::{RemoteConfig, HYPERVISOR_NAME_REMOTE};

/// Virtual PCI block device driver.
pub const VIRTIO_BLK_PCI: &str = "virtio-blk-pci";

/// Virtual MMIO block device driver.
pub const VIRTIO_BLK_MMIO: &str = "virtio-blk-mmio";

/// Virtual CCW block device driver.
pub const VIRTIO_BLK_CCW: &str = "virtio-blk-ccw";

/// Virtual SCSI block device driver.
pub const VIRTIO_SCSI: &str = "virtio-scsi";

/// Virtual PMEM device driver.
pub const VIRTIO_PMEM: &str = "virtio-pmem";

mod firecracker;
pub use self::firecracker::{FirecrackerConfig, HYPERVISOR_NAME_FIRECRACKER};

const VIRTIO_9P: &str = "virtio-9p";
const VIRTIO_FS: &str = "virtio-fs";
const VIRTIO_FS_INLINE: &str = "inline-virtio-fs";
const MAX_BRIDGE_SIZE: u32 = 5;

const KERNEL_PARAM_DELIMITER: &str = " ";

lazy_static! {
    static ref HYPERVISOR_PLUGINS: Mutex<HashMap<String, Arc<dyn ConfigPlugin>>> =
        Mutex::new(HashMap::new());
}

/// Register a hypervisor plugin with `name`.
pub fn register_hypervisor_plugin(name: &str, plugin: Arc<dyn ConfigPlugin>) {
    let mut hypervisors = HYPERVISOR_PLUGINS.lock().unwrap();
    hypervisors.insert(name.to_string(), plugin);
}

/// Get the hypervisor plugin with `name`.
pub fn get_hypervisor_plugin(name: &str) -> Option<Arc<dyn ConfigPlugin>> {
    let hypervisors = HYPERVISOR_PLUGINS.lock().unwrap();
    hypervisors.get(name).cloned()
}

/// Configuration information for block device.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BlockDeviceInfo {
    /// Disable block device from being used for a container's rootfs.
    ///
    /// In case of a storage driver like devicemapper where a container's root file system is
    /// backed by a block device, the block device is passed directly to the hypervisor for
    /// performance reasons. This flag prevents the block device from being passed to the
    /// hypervisor, shared fs is used instead to pass the rootfs.
    #[serde(default)]
    pub disable_block_device_use: bool,

    /// Block storage driver to be used for the hypervisor in case the container rootfs is backed
    /// by a block device. This is virtio-scsi, virtio-blk or nvdimm.
    #[serde(default)]
    pub block_device_driver: String,

    /// Specifies cache-related options will be set to block devices or not.
    #[serde(default)]
    pub block_device_cache_set: bool,

    /// Specifies cache-related options for block devices.
    ///
    /// Denotes whether use of O_DIRECT (bypass the host page cache) is enabled.
    #[serde(default)]
    pub block_device_cache_direct: bool,

    /// Specifies cache-related options for block devices.
    /// Denotes whether flush requests for the device are ignored.
    #[serde(default)]
    pub block_device_cache_noflush: bool,

    /// If false and nvdimm is supported, use nvdimm device to plug guest image.
    #[serde(default)]
    pub disable_image_nvdimm: bool,

    /// The size in MiB will be plused to max memory of hypervisor.
    ///
    /// It is the memory address space for the NVDIMM devie. If set block storage driver
    /// (block_device_driver) to "nvdimm", should set memory_offset to the size of block device.
    #[serde(default)]
    pub memory_offset: u64,

    /// Enable vhost-user storage device, default false
    ///
    /// Enabling this will result in some Linux reserved block type major range 240-254 being
    /// chosen to represent vhost-user devices.
    #[serde(default)]
    pub enable_vhost_user_store: bool,

    /// The base directory specifically used for vhost-user devices.
    ///
    /// Its sub-path "block" is used for block devices; "block/sockets" is where we expect
    /// vhost-user sockets to live; "block/devices" is where simulated block device nodes for
    /// vhost-user devices to live.
    #[serde(default)]
    pub vhost_user_store_path: String,

    /// List of valid annotations values for the vhost user store path.
    ///
    /// The default if not set is empty (all annotations rejected.)
    #[serde(default)]
    pub valid_vhost_user_store_paths: Vec<String>,
}

impl BlockDeviceInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        if self.disable_block_device_use {
            self.block_device_driver = "".to_string();
            self.enable_vhost_user_store = false;
            self.memory_offset = 0;
            return Ok(());
        }

        if self.block_device_driver.is_empty() {
            self.block_device_driver = default::DEFAULT_BLOCK_DEVICE_TYPE.to_string();
        }
        if self.memory_offset == 0 {
            self.memory_offset = default::DEFAULT_BLOCK_NVDIMM_MEM_OFFSET;
        }
        if !self.enable_vhost_user_store {
            self.vhost_user_store_path = String::new();
        } else if self.vhost_user_store_path.is_empty() {
            self.vhost_user_store_path = default::DEFAULT_VHOST_USER_STORE_PATH.to_string();
        }
        resolve_path!(
            self.vhost_user_store_path,
            "Invalid vhost-user-store-path {}: {}"
        )?;

        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        if self.disable_block_device_use {
            return Ok(());
        }
        let l = [
            VIRTIO_BLK_PCI,
            VIRTIO_BLK_CCW,
            VIRTIO_BLK_MMIO,
            VIRTIO_PMEM,
            VIRTIO_SCSI,
        ];
        if !l.contains(&self.block_device_driver.as_str()) {
            return Err(eother!(
                "{} is unsupported block device type.",
                self.block_device_driver
            ));
        }
        validate_path!(
            self.vhost_user_store_path,
            "Invalid vhost-user-store-path {}: {}"
        )?;

        Ok(())
    }

    /// Validate path of vhost-user storage backend.
    pub fn validate_vhost_user_store_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_vhost_user_store_paths, path)
    }
}

/// Guest kernel boot information.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BootInfo {
    /// Path to guest kernel file on host
    #[serde(default)]
    pub kernel: String,
    /// Guest kernel commandline.
    #[serde(default)]
    pub kernel_params: String,
    /// Path to initrd file on host
    #[serde(default)]
    pub initrd: String,
    /// Path to root device on host
    #[serde(default)]
    pub image: String,
    /// Rootfs filesystem type.
    #[serde(default)]
    pub rootfs_type: String,
    /// Path to the firmware.
    ///
    /// If you want that qemu uses the default firmware leave this option empty.
    #[serde(default)]
    pub firmware: String,
    /// Block storage driver to be used for the VM rootfs is backed
    /// by a block device. This is virtio-pmem, virtio-blk-pci or virtio-blk-mmio
    #[serde(default)]
    pub vm_rootfs_driver: String,
}

impl BootInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        resolve_path!(self.kernel, "guest kernel image file {} is invalid: {}")?;
        resolve_path!(self.image, "guest boot image file {} is invalid: {}")?;
        resolve_path!(self.initrd, "guest initrd image file {} is invalid: {}")?;
        resolve_path!(self.firmware, "firmware image file {} is invalid: {}")?;

        if self.vm_rootfs_driver.is_empty() {
            self.vm_rootfs_driver = default::DEFAULT_BLOCK_DEVICE_TYPE.to_string();
        }

        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        validate_path!(self.kernel, "guest kernel image file {} is invalid: {}")?;
        validate_path!(self.image, "guest boot image file {} is invalid: {}")?;
        validate_path!(self.initrd, "guest initrd image file {} is invalid: {}")?;
        validate_path!(self.firmware, "firmware image file {} is invalid: {}")?;
        if !self.image.is_empty() && !self.initrd.is_empty() {
            return Err(eother!("Can not configure both initrd and image for boot"));
        }

        let l = [
            VIRTIO_BLK_PCI,
            VIRTIO_BLK_CCW,
            VIRTIO_BLK_MMIO,
            VIRTIO_PMEM,
            VIRTIO_SCSI,
        ];
        if !l.contains(&self.vm_rootfs_driver.as_str()) {
            return Err(eother!(
                "{} is unsupported block device type.",
                self.vm_rootfs_driver
            ));
        }

        Ok(())
    }

    /// Add kernel parameters to bootinfo. It is always added before the original
    /// to let the original one takes priority
    pub fn add_kernel_params(&mut self, params: Vec<String>) {
        let mut p = params;
        if !self.kernel_params.is_empty() {
            p.push(self.kernel_params.clone()); // [new_params0, new_params1, ..., original_params]
        }
        self.kernel_params = p.join(KERNEL_PARAM_DELIMITER);
    }

    /// Validate guest kernel image annotaion
    pub fn validate_boot_path(&self, path: &str) -> Result<()> {
        validate_path!(path, "path {} is invalid{}")?;
        Ok(())
    }
}

/// Virtual CPU configuration information.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CpuInfo {
    /// CPU features, comma-separated list of cpu features to pass to the cpu.
    /// For example, `cpu_features = "pmu=off,vmx=off"
    #[serde(default)]
    pub cpu_features: String,

    /// Default number of vCPUs per SB/VM:
    /// - unspecified or 0                --> will be set to @DEFVCPUS@
    /// - < 0                             --> will be set to the actual number of physical cores
    ///  > 0 <= number of physical cores --> will be set to the specified number
    ///  > number of physical cores      --> will be set to the actual number of physical cores
    #[serde(default)]
    pub default_vcpus: i32,

    /// Default maximum number of vCPUs per SB/VM:
    /// - unspecified or == 0             --> will be set to the actual number of physical cores or
    ///                                       to the maximum number of vCPUs supported by KVM
    ///                                       if that number is exceeded
    /// - > 0 <= number of physical cores --> will be set to the specified number
    /// - > number of physical cores      --> will be set to the actual number of physical cores or
    ///                                       to the maximum number of vCPUs supported by KVM
    ///                                       if that number is exceeded
    ///
    /// WARNING: Depending of the architecture, the maximum number of vCPUs supported by KVM is used
    /// when the actual number of physical cores is greater than it.
    ///
    /// WARNING: Be aware that this value impacts the virtual machine's memory footprint and CPU
    /// the hotplug functionality. For example, `default_maxvcpus = 240` specifies that until 240
    /// vCPUs can be added to a SB/VM, but the memory footprint will be big. Another example, with
    /// `default_maxvcpus = 8` the memory footprint will be small, but 8 will be the maximum number
    /// of vCPUs supported by the SB/VM. In general, we recommend that you do not edit this
    /// variable, unless you know what are you doing.
    ///
    /// NOTICE: on arm platform with gicv2 interrupt controller, set it to 8.
    #[serde(default)]
    pub default_maxvcpus: u32,
}

impl CpuInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        let features: Vec<&str> = self.cpu_features.split(',').map(|v| v.trim()).collect();
        self.cpu_features = features.join(",");

        let cpus = num_cpus::get() as u32;

        // adjust default_maxvcpus
        if self.default_maxvcpus == 0 || self.default_maxvcpus > cpus {
            self.default_maxvcpus = cpus;
        }

        // adjust default_vcpus
        if self.default_vcpus < 0 || self.default_vcpus as u32 > cpus {
            self.default_vcpus = cpus as i32;
        } else if self.default_vcpus == 0 {
            self.default_vcpus = default::DEFAULT_GUEST_VCPUS as i32;
        }

        if self.default_vcpus > self.default_maxvcpus as i32 {
            self.default_vcpus = self.default_maxvcpus as i32;
        }

        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        if self.default_vcpus > self.default_maxvcpus as i32 {
            return Err(eother!(
                "The default_vcpus({}) is greater than default_maxvcpus({})",
                self.default_vcpus,
                self.default_maxvcpus
            ));
        }
        Ok(())
    }
}

/// Configuration information for debug
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DebugInfo {
    /// This option changes the default hypervisor and kernel parameters to enable debug output
    /// where available.
    #[serde(default)]
    pub enable_debug: bool,

    /// The log log level will be applied to hypervisor.
    /// Possible values are:
    /// - trace
    /// - debug
    /// - info
    /// - warn
    /// - error
    /// - critical
    #[serde(default = "default_hypervisor_log_level")]
    pub log_level: String,

    /// Enable dumping information about guest page structures if true.
    #[serde(default)]
    pub guest_memory_dump_paging: bool,

    /// Set where to save the guest memory dump file.
    ///
    /// If set, when GUEST_PANICKED event occurred, guest memory will be dumped to host filesystem
    /// under guest_memory_dump_path. This directory will be created automatically if it does not
    /// exist. The dumped file(also called vmcore) can be processed with crash or gdb.
    ///
    /// # WARNING:
    ///  Dump guest's memory can take very long depending on the amount of guest memory and use
    /// much disk space.
    #[serde(default)]
    pub guest_memory_dump_path: String,

    /// This option allows to add a debug monitor socket when `enable_debug = true`
    /// WARNING: Anyone with access to the monitor socket can take full control of
    /// Qemu. This is for debugging purpose only and must *NEVER* be used in
    /// production.
    /// Valid values are :
    /// - "hmp"
    /// - "qmp"
    /// - "qmp-pretty" (same as "qmp" with pretty json formatting)
    /// If set to the empty string "", no debug monitor socket is added. This is
    /// the default.
    /// dbg_monitor_socket = "hmp"
    #[serde(default)]
    pub dbg_monitor_socket: String,
}

impl DebugInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

fn default_hypervisor_log_level() -> String {
    String::from("info")
}

/// Virtual machine device configuration information.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DeviceInfo {
    /// Bridges can be used to hot plug devices.
    ///
    /// Limitations:
    /// - Currently only pci bridges are supported
    /// - Until 30 devices per bridge can be hot plugged.
    /// - Until 5 PCI bridges can be cold plugged per VM.
    ///
    /// This limitation could be a bug in qemu or in the kernel
    /// Default number of bridges per SB/VM:
    /// - unspecified or 0   --> will be set to @DEFBRIDGES@
    /// - > 1 <= 5           --> will be set to the specified number
    /// - > 5                --> will be set to 5
    #[serde(default)]
    pub default_bridges: u32,

    /// VFIO devices are hotplugged on a bridge by default.
    ///
    /// Enable hotplugging on root bus. This may be required for devices with a large PCI bar,
    /// as this is a current limitation with hotplugging on a bridge.
    #[serde(default)]
    pub hotplug_vfio_on_root_bus: bool,

    /// Before hot plugging a PCIe device, you need to add a pcie_root_port device.
    ///
    /// Use this parameter when using some large PCI bar devices, such as Nvidia GPU.
    /// The value means the number of pcie_root_port.
    /// This value is valid when hotplug_vfio_on_root_bus is true and machine_type is "q35"
    #[serde(default)]
    pub pcie_root_port: u32,

    /// Enable vIOMMU, default false
    ///
    /// Enabling this will result in the VM having a vIOMMU device. This will also add the
    /// following options to the kernel's command line: intel_iommu=on,iommu=pt
    #[serde(default)]
    pub enable_iommu: bool,

    /// Enable IOMMU_PLATFORM, default false
    ///
    /// Enabling this will result in the VM device having iommu_platform=on set
    #[serde(default)]
    pub enable_iommu_platform: bool,

    /// Enable balloon f_reporting, default false
    ///
    /// Enabling this will result in the VM balloon device having f_reporting=on set
    #[serde(default)]
    pub reclaim_guest_freed_memory: bool,
}

impl DeviceInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        if self.default_bridges > MAX_BRIDGE_SIZE {
            self.default_bridges = MAX_BRIDGE_SIZE;
        }

        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        if self.default_bridges > MAX_BRIDGE_SIZE {
            return Err(eother!(
                "The configured PCI bridges {} are too many",
                self.default_bridges
            ));
        }
        Ok(())
    }
}

/// Virtual machine PCIe Topology configuration.
#[derive(Clone, Debug, Default)]
pub struct TopologyConfigInfo {
    /// Hypervisor name
    pub hypervisor_name: String,
    /// Device Info
    pub device_info: DeviceInfo,
}

impl TopologyConfigInfo {
    /// Initialize the topology config info from toml config
    pub fn new(toml_config: &TomlConfig) -> Option<Self> {
        // Firecracker does not support PCIe Devices, so we should not initialize such a PCIe topology for it.
        // If the case of fc hit, just return None.
        let hypervisor_names = [
            HYPERVISOR_NAME_QEMU,
            HYPERVISOR_NAME_CH,
            HYPERVISOR_NAME_DRAGONBALL,
            HYPERVISOR_NAME_FIRECRACKER,
            HYPERVISOR_NAME_REMOTE,
        ];
        let hypervisor_name = toml_config.runtime.hypervisor_name.as_str();
        if !hypervisor_names.contains(&hypervisor_name) {
            return None;
        }

        let hv = toml_config.hypervisor.get(hypervisor_name)?;
        Some(Self {
            hypervisor_name: hypervisor_name.to_string(),
            device_info: hv.device_info.clone(),
        })
    }
}

/// Configuration information for virtual machine.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MachineInfo {
    /// Virtual machine model/type.
    #[serde(default)]
    pub machine_type: String,

    /// Machine accelerators.
    /// Comma-separated list of machine accelerators to pass to the hypervisor.
    /// For example, `machine_accelerators = "nosmm,nosmbus,nosata,nopit,static-prt,nofw"`
    #[serde(default)]
    pub machine_accelerators: String,

    /// Add flash image file to VM.
    ///
    /// The arguments of it should be in format of ["/path/to/flash0.img", "/path/to/flash1.img"].
    #[serde(default)]
    pub pflashes: Vec<String>,

    /// Default entropy source.
    /// The path to a host source of entropy (including a real hardware RNG).
    /// `/dev/urandom` and `/dev/random` are two main options. Be aware that `/dev/random` is a
    /// blocking source of entropy.  If the host runs out of entropy, the VMs boot time will
    /// increase leading to get startup timeouts. The source of entropy `/dev/urandom` is
    /// non-blocking and provides a generally acceptable source of entropy. It should work well
    /// for pretty much all practical purposes.
    #[serde(default)]
    pub entropy_source: String,

    /// List of valid annotations values for entropy_source.
    /// The default if not set is empty (all annotations rejected.)
    #[serde(default)]
    pub valid_entropy_sources: Vec<String>,
}

impl MachineInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        let accelerators: Vec<&str> = self
            .machine_accelerators
            .split(',')
            .map(|v| v.trim())
            .collect();
        self.machine_accelerators = accelerators.join(",");

        for pflash in self.pflashes.iter_mut() {
            resolve_path!(*pflash, "Flash image file {} is invalide: {}")?;
        }
        resolve_path!(self.entropy_source, "Entropy source {} is invalid: {}")?;

        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        for pflash in self.pflashes.iter() {
            validate_path!(*pflash, "Flash image file {} is invalid: {}")?;
        }
        validate_path!(self.entropy_source, "Entropy source {} is invalid: {}")?;
        Ok(())
    }

    /// Validate path of entropy source.
    pub fn validate_entropy_source<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_entropy_sources, path)
    }
}

/// Huge page type for VM RAM backend
#[derive(Clone, Debug, Deserialize_enum_str, Serialize_enum_str, PartialEq, Eq)]
pub enum HugePageType {
    /// This will result in the VM memory being allocated using hugetlbfs backend. This is useful
    /// when you want to use vhost-user network stacks within the container. This will automatically
    /// result in memory pre allocation.
    #[serde(rename = "hugetlbfs")]
    Hugetlbfs,
    /// This will result in the VM memory being allocated using transparant huge page backend.
    #[serde(rename = "thp")]
    THP,
}

impl Default for HugePageType {
    fn default() -> Self {
        Self::Hugetlbfs
    }
}

/// Virtual machine memory configuration information.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MemoryInfo {
    /// Default memory size in MiB for SB/VM.
    #[serde(default)]
    pub default_memory: u32,

    /// Default maximum memory in MiB per SB / VM
    /// unspecified or == 0           --> will be set to the actual amount of physical RAM
    /// > 0 <= amount of physical RAM --> will be set to the specified number
    /// > amount of physical RAM      --> will be set to the actual amount of physical RAM
    #[serde(default)]
    pub default_maxmemory: u32,

    /// Default memory slots per SB/VM.
    ///
    /// This is will determine the times that memory will be hotadded to sandbox/VM.
    #[serde(default)]
    pub memory_slots: u32,

    /// Enable file based guest memory support.
    ///
    /// The default is an empty string which will disable this feature. In the case of virtio-fs,
    /// this is enabled automatically and '/dev/shm' is used as the backing folder. This option
    /// will be ignored if VM templating is enabled.
    #[serde(default)]
    pub file_mem_backend: String,

    /// List of valid annotations values for the file_mem_backend annotation
    ///
    /// The default if not set is empty (all annotations rejected.)
    #[serde(default)]
    pub valid_file_mem_backends: Vec<String>,

    /// Enable pre allocation of VM RAM, default false
    ///
    /// Enabling this will result in lower container density as all of the memory will be allocated
    /// and locked. This is useful when you want to reserve all the memory upfront or in the cases
    /// where you want memory latencies to be very predictable
    #[serde(default)]
    pub enable_mem_prealloc: bool,

    /// Enable huge pages for VM RAM, default false
    ///
    /// Enabling this will result in the VM memory being allocated using huge pages.
    /// Its backend type is specified by item "hugepage_type"
    #[serde(default)]
    pub enable_hugepages: bool,

    /// Select huge page type, default "hugetlbfs"
    /// Following huge types are supported:
    /// - hugetlbfs
    /// - thp
    #[serde(default)]
    pub hugepage_type: HugePageType,

    /// Specifies virtio-mem will be enabled or not.
    ///
    /// Please note that this option should be used with the command
    /// "echo 1 > /proc/sys/vm/overcommit_memory".
    #[serde(default)]
    pub enable_virtio_mem: bool,

    /// Enable swap in the guest. Default false.
    ///
    /// When enable_guest_swap is enabled, insert a raw file to the guest as the swap device if the
    /// swappiness of a container (set by annotation "io.katacontainers.container.resource.swappiness")
    /// is bigger than 0.
    ///
    /// The size of the swap device should be swap_in_bytes (set by annotation
    /// "io.katacontainers.container.resource.swap_in_bytes") - memory_limit_in_bytes.
    /// If swap_in_bytes is not set, the size should be memory_limit_in_bytes.
    /// If swap_in_bytes and memory_limit_in_bytes is not set, the size should be default_memory.
    #[serde(default)]
    pub enable_guest_swap: bool,
}

impl MemoryInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        resolve_path!(
            self.file_mem_backend,
            "Memory backend file {} is invalid: {}"
        )?;
        if self.default_maxmemory == 0 {
            let s = System::new_all();
            self.default_maxmemory = Byte::from_u64(s.total_memory())
                .get_adjusted_unit(Unit::MiB)
                .get_value() as u32;
        }
        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        validate_path!(
            self.file_mem_backend,
            "Memory backend file {} is invalid: {}"
        )?;
        if self.default_memory == 0 {
            return Err(eother!("Configured memory size for guest VM is zero"));
        }
        if self.memory_slots == 0 {
            return Err(eother!("Configured memory slots for guest VM are zero"));
        }

        Ok(())
    }

    /// Validate path of memory backend files.
    pub fn validate_memory_backend_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_file_mem_backends, path)
    }
}

/// Configuration information for network.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct NetworkInfo {
    /// If vhost-net backend for virtio-net is not desired, set to true.
    ///
    /// Default is false, which trades off security (vhost-net runs ring0) for network I/O
    /// performance.
    #[serde(default)]
    pub disable_vhost_net: bool,

    /// Use rx Rate Limiter to control network I/O inbound bandwidth(size in bits/sec for SB/VM).
    ///
    /// In Qemu, we use classful qdiscs HTB(Hierarchy Token Bucket) to discipline traffic.
    /// Default 0-sized value means unlimited rate.
    #[serde(default)]
    pub rx_rate_limiter_max_rate: u64,

    /// Use tx Rate Limiter to control network I/O outbound bandwidth(size in bits/sec for SB/VM).
    ///
    /// In Qemu, we use classful qdiscs HTB(Hierarchy Token Bucket) and ifb(Intermediate Functional
    /// Block) to discipline traffic.
    /// Default 0-sized value means unlimited rate.
    #[serde(default)]
    pub tx_rate_limiter_max_rate: u64,

    /// network queues
    #[serde(default)]
    pub network_queues: u32,
}

impl NetworkInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Configuration information for security.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SecurityInfo {
    /// Enable running QEMU VMM as a non-root user.
    ///
    /// By default QEMU VMM run as root. When this is set to true, QEMU VMM process runs as
    /// a non-root random user. See documentation for the limitations of this mode.
    #[serde(default)]
    pub rootless: bool,

    /// Disable seccomp.
    #[serde(default)]
    pub disable_seccomp: bool,

    /// Enable confidential guest support.
    ///
    /// Toggling that setting may trigger different hardware features, ranging from memory
    /// encryption to both memory and CPU-state encryption and integrity.The Kata Containers
    /// runtime dynamically detects the available feature set and aims at enabling the largest
    /// possible one.
    #[serde(default)]
    pub confidential_guest: bool,

    /// Path to OCI hook binaries in the *guest rootfs*.
    ///
    /// This does not affect host-side hooks which must instead be added to the OCI spec passed to
    /// the runtime.
    ///
    /// You can create a rootfs with hooks by customizing the osbuilder scripts:
    /// https://github.com/kata-containers/kata-containers/tree/main/tools/osbuilder
    ///
    /// Hooks must be stored in a subdirectory of guest_hook_path according to their hook type,
    /// i.e. "guest_hook_path/{prestart,poststart,poststop}". The agent will scan these directories
    /// for executable files and add them, in lexicographical order, to the lifecycle of the guest
    /// container.
    ///
    /// Hooks are executed in the runtime namespace of the guest. See the official documentation:
    /// https://github.com/opencontainers/runtime-spec/blob/v1.0.1/config.md#posix-platform-hooks
    ///
    /// Warnings will be logged if any error is encountered while scanning for hooks, but it will
    /// not abort container execution.
    #[serde(default)]
    pub guest_hook_path: String,

    /// List of valid annotation names for the hypervisor.
    ///
    /// Each member of the list is a regular expression, which is the base name of the annotation,
    /// e.g. "path" for io.katacontainers.config.hypervisor.path"
    #[serde(default)]
    pub enable_annotations: Vec<String>,
}

impl SecurityInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        if self.guest_hook_path.is_empty() {
            self.guest_hook_path = default::DEFAULT_GUEST_HOOK_PATH.to_string();
        }
        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }

    /// Check whether annotation key is enabled or not.
    pub fn is_annotation_enabled(&self, path: &str) -> bool {
        if !path.starts_with(KATA_ANNO_CFG_HYPERVISOR_PREFIX) {
            return false;
        }
        let pos = KATA_ANNO_CFG_HYPERVISOR_PREFIX.len();
        let key = &path[pos..];
        if let Ok(set) = RegexSet::new(&self.enable_annotations) {
            return set.is_match(key);
        }
        false
    }

    /// Validate path
    pub fn validate_path(&self, path: &str) -> Result<()> {
        validate_path!(path, "path {} is invalid{}")?;
        Ok(())
    }
}

/// Configuration information for shared filesystem, such virtio-9p and virtio-fs.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SharedFsInfo {
    /// Shared file system type:
    /// - virtio-fs (default)
    /// - virtio-9p`
    pub shared_fs: Option<String>,

    /// Path to vhost-user-fs daemon.
    #[serde(default)]
    pub virtio_fs_daemon: String,

    /// List of valid annotations values for the virtiofs daemon
    /// The default if not set is empty (all annotations rejected.)
    #[serde(default)]
    pub valid_virtio_fs_daemon_paths: Vec<String>,

    /// Extra args for virtiofsd daemon
    ///
    /// Format example:
    ///    ["-o", "arg1=xxx,arg2", "-o", "hello world", "--arg3=yyy"]
    ///
    ///  see `virtiofsd -h` for possible options.
    #[serde(default)]
    pub virtio_fs_extra_args: Vec<String>,

    /// Cache mode:
    /// - never: Metadata, data, and pathname lookup are not cached in guest. They are always
    ///   fetched from host and any changes are immediately pushed to host.
    /// - auto: Metadata and pathname lookup cache expires after a configured amount of time
    ///   (default is 1 second). Data is cached while the file is open (close to open consistency).
    /// - always: Metadata, data, and pathname lookup are cached in guest and never expire.
    #[serde(default)]
    pub virtio_fs_cache: String,

    /// Default size of DAX cache in MiB
    #[serde(default)]
    pub virtio_fs_cache_size: u32,

    /// Default size of virtqueues
    #[serde(default)]
    pub virtio_fs_queue_size: u32,

    /// Enable virtio-fs DAX window if true.
    #[serde(default)]
    pub virtio_fs_is_dax: bool,

    /// This is the msize used for 9p shares. It is the number of bytes used for 9p packet payload.
    #[serde(default)]
    pub msize_9p: u32,
}

impl SharedFsInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        if self.shared_fs.as_deref() == Some("") {
            self.shared_fs = Some(default::DEFAULT_SHARED_FS_TYPE.to_string());
        }
        match self.shared_fs.as_deref() {
            Some(VIRTIO_FS) => self.adjust_virtio_fs(false)?,
            Some(VIRTIO_FS_INLINE) => self.adjust_virtio_fs(true)?,
            Some(VIRTIO_9P) => {
                if self.msize_9p == 0 {
                    self.msize_9p = default::DEFAULT_SHARED_9PFS_SIZE_MB;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        match self.shared_fs.as_deref() {
            None => Ok(()),
            Some(VIRTIO_FS) => self.validate_virtio_fs(false),
            Some(VIRTIO_FS_INLINE) => self.validate_virtio_fs(true),
            Some(VIRTIO_9P) => {
                if self.msize_9p < default::MIN_SHARED_9PFS_SIZE_MB
                    || self.msize_9p > default::MAX_SHARED_9PFS_SIZE_MB
                {
                    return Err(eother!(
                        "Invalid 9p configuration msize 0x{:x}, min value is 0x{:x}, max value is 0x{:x}",
                        self.msize_9p,default::MIN_SHARED_9PFS_SIZE_MB, default::MAX_SHARED_9PFS_SIZE_MB
                    ));
                }
                Ok(())
            }
            Some(v) => Err(eother!("Invalid shared_fs type {}", v)),
        }
    }

    /// Validate path of virtio-fs daemon, especially for annotations.
    pub fn validate_virtiofs_daemon_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_virtio_fs_daemon_paths, path)
    }

    fn adjust_virtio_fs(&mut self, inline: bool) -> Result<()> {
        // inline mode doesn't need external virtiofsd daemon
        if !inline {
            resolve_path!(
                self.virtio_fs_daemon,
                "Virtio-fs daemon path {} is invalid: {}"
            )?;
        }

        if self.virtio_fs_cache.is_empty() {
            self.virtio_fs_cache = default::DEFAULT_VIRTIO_FS_CACHE_MODE.to_string();
        }
        if self.virtio_fs_cache == *"none" {
            warn!(sl!(), "virtio-fs cache mode `none` is deprecated since Kata Containers 2.5.0 and will be removed in the future release, please use `never` instead. For more details please refer to https://github.com/kata-containers/kata-containers/issues/4234.");
            self.virtio_fs_cache = default::DEFAULT_VIRTIO_FS_CACHE_MODE.to_string();
        }
        if self.virtio_fs_is_dax && self.virtio_fs_cache_size == 0 {
            self.virtio_fs_cache_size = default::DEFAULT_VIRTIO_FS_DAX_SIZE_MB;
        }
        if !self.virtio_fs_is_dax && self.virtio_fs_cache_size != 0 {
            self.virtio_fs_is_dax = true;
        }
        Ok(())
    }

    fn validate_virtio_fs(&self, inline: bool) -> Result<()> {
        // inline mode doesn't need external virtiofsd daemon
        if !inline {
            validate_path!(
                self.virtio_fs_daemon,
                "Virtio-fs daemon path {} is invalid: {}"
            )?;
        }

        let l = ["never", "auto", "always"];

        if !l.contains(&self.virtio_fs_cache.as_str()) {
            return Err(eother!(
                "Invalid virtio-fs cache mode: {}",
                &self.virtio_fs_cache
            ));
        }
        if self.virtio_fs_is_dax && self.virtio_fs_cache_size == 0 {
            return Err(eother!(
                "Invalid virtio-fs DAX window size: {}",
                &self.virtio_fs_cache_size
            ));
        }
        Ok(())
    }
}

/// Configuration information for remote hypervisor type.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RemoteInfo {
    /// Remote hypervisor socket path
    #[serde(default)]
    pub hypervisor_socket: String,

    /// Remote hyperisor timeout of creating (in seconds)
    #[serde(default)]
    pub hypervisor_timeout: i32,
}

/// Common configuration information for hypervisors.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Hypervisor {
    /// Path to the hypervisor executable.
    #[serde(default)]
    pub path: String,
    /// List of valid annotations values for the hypervisor.
    ///
    /// Each member of the list is a path pattern as described by glob(3). The default if not set
    /// is empty (all annotations rejected.)
    #[serde(default)]
    pub valid_hypervisor_paths: Vec<String>,

    /// Hypervisor control executable path.
    #[serde(default)]
    pub ctlpath: String,
    /// List of valid annotations values for the hypervisor control executable.
    ///
    /// Each member of the list is a path pattern as described by glob(3). The default if not set
    /// is empty (all annotations rejected.)
    #[serde(default)]
    pub valid_ctlpaths: Vec<String>,

    /// Control channel path.
    #[serde(default)]
    pub jailer_path: String,
    /// List of valid annotations values for the hypervisor jailer path.
    ///
    /// Each member of the list is a path pattern as described by glob(3). The default if not set
    /// is empty (all annotations rejected.)
    #[serde(default)]
    pub valid_jailer_paths: Vec<String>,

    /// Disable the customizations done in the runtime when it detects that it is running on top
    /// a VMM. This will result in the runtime behaving as it would when running on bare metal.
    #[serde(default)]
    pub disable_nesting_checks: bool,

    /// Enable iothreads (data-plane) to be used. This causes IO to be handled in a separate IO
    /// thread. This is currently only implemented for SCSI.
    #[serde(default)]
    pub enable_iothreads: bool,

    /// Block device configuration information.
    #[serde(default, flatten)]
    pub blockdev_info: BlockDeviceInfo,

    /// Guest system boot information.
    #[serde(default, flatten)]
    pub boot_info: BootInfo,

    /// Guest virtual CPU configuration information.
    #[serde(default, flatten)]
    pub cpu_info: CpuInfo,

    /// Debug configuration information.
    #[serde(default, flatten)]
    pub debug_info: DebugInfo,

    /// Device configuration information.
    #[serde(default, flatten)]
    pub device_info: DeviceInfo,

    /// Virtual machine configuration information.
    #[serde(default, flatten)]
    pub machine_info: MachineInfo,

    /// Virtual machine memory configuration information.
    #[serde(default, flatten)]
    pub memory_info: MemoryInfo,

    /// Network configuration information.
    #[serde(default, flatten)]
    pub network_info: NetworkInfo,

    /// Security configuration information.
    #[serde(default, flatten)]
    pub security_info: SecurityInfo,

    /// Shared file system configuration information.
    #[serde(default, flatten)]
    pub shared_fs: SharedFsInfo,

    /// Remote hypervisor configuration information.
    #[serde(default, flatten)]
    pub remote_info: RemoteInfo,

    /// A sandbox annotation used to specify prefetch_files.list host path container image
    /// being used, and runtime will pass it to Hypervisor to  search for corresponding
    /// prefetch list file:
    ///   prefetch_list_path = /path/to/<uid>/xyz.com/fedora:36/prefetch_file.list
    #[serde(default)]
    pub prefetch_list_path: String,

    /// Vendor customized runtime configuration.
    #[serde(default, flatten)]
    pub vendor: HypervisorVendor,

    /// Disable applying SELinux on the container process.
    #[serde(default = "yes")]
    pub disable_guest_selinux: bool,
}

fn yes() -> bool {
    true
}

impl Hypervisor {
    /// Validate path of hypervisor executable.
    pub fn validate_hypervisor_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_hypervisor_paths, path)
    }

    /// Validate path of hypervisor control executable.
    pub fn validate_hypervisor_ctlpath<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_ctlpaths, path)
    }

    /// Validate path of jailer executable.
    pub fn validate_jailer_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_jailer_paths, path)
    }
}

impl ConfigOps for Hypervisor {
    fn adjust_config(conf: &mut TomlConfig) -> Result<()> {
        HypervisorVendor::adjust_config(conf)?;
        let hypervisors: Vec<String> = conf.hypervisor.keys().cloned().collect();
        info!(
            sl!(),
            "Adjusting hypervisor configuration {:?}", hypervisors
        );
        for hypervisor in hypervisors.iter() {
            if let Some(plugin) = get_hypervisor_plugin(hypervisor) {
                plugin.adjust_config(conf)?;
                // Safe to unwrap() because `hypervisor` is a valid key in the hash map.
                let hv = conf.hypervisor.get_mut(hypervisor).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "hypervisor not found".to_string())
                })?;
                hv.blockdev_info.adjust_config()?;
                hv.boot_info.adjust_config()?;
                hv.cpu_info.adjust_config()?;
                hv.debug_info.adjust_config()?;
                hv.device_info.adjust_config()?;
                hv.machine_info.adjust_config()?;
                hv.memory_info.adjust_config()?;
                hv.network_info.adjust_config()?;
                hv.security_info.adjust_config()?;
                hv.shared_fs.adjust_config()?;
                resolve_path!(
                    hv.prefetch_list_path,
                    "prefetch_list_path `{}` is invalid: {}"
                )?;
            } else {
                return Err(eother!("Can not find plugin for hypervisor {}", hypervisor));
            }
        }

        Ok(())
    }

    fn validate(conf: &TomlConfig) -> Result<()> {
        HypervisorVendor::validate(conf)?;

        let hypervisors: Vec<String> = conf.hypervisor.keys().cloned().collect();
        for hypervisor in hypervisors.iter() {
            if let Some(plugin) = get_hypervisor_plugin(hypervisor) {
                plugin.validate(conf)?;

                // Safe to unwrap() because `hypervisor` is a valid key in the hash map.
                let hv = conf.hypervisor.get(hypervisor).unwrap();
                hv.blockdev_info.validate()?;
                hv.boot_info.validate()?;
                hv.cpu_info.validate()?;
                hv.debug_info.validate()?;
                hv.device_info.validate()?;
                hv.machine_info.validate()?;
                hv.memory_info.validate()?;
                hv.network_info.validate()?;
                hv.security_info.validate()?;
                hv.shared_fs.validate()?;
                validate_path!(hv.path, "Hypervisor binary path `{}` is invalid: {}")?;
                validate_path!(
                    hv.ctlpath,
                    "Hypervisor control executable `{}` is invalid: {}"
                )?;
                validate_path!(hv.jailer_path, "Hypervisor jailer path `{}` is invalid: {}")?;
                validate_path!(
                    hv.prefetch_list_path,
                    "prefetch_files.list path `{}` is invalid: {}"
                )?;
            } else {
                return Err(eother!("Can not find plugin for hypervisor {}", hypervisor));
            }
        }

        Ok(())
    }
}

#[cfg(not(feature = "enable-vendor"))]
mod vendor {
    use super::*;

    /// Vendor customization runtime configuration.
    #[derive(Clone, Debug, Default, Deserialize, Serialize)]
    pub struct HypervisorVendor {}

    impl ConfigOps for HypervisorVendor {}
}

#[cfg(feature = "enable-vendor")]
#[path = "vendor.rs"]
mod vendor;

pub use self::vendor::HypervisorVendor;
use crate::config::validate_path_pattern;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_plugin() {
        let db = DragonballConfig::new();
        db.register();

        let db = Arc::new(DragonballConfig::new());
        register_hypervisor_plugin("dragonball", db);

        assert!(get_hypervisor_plugin("dragonball").is_some());
        assert!(get_hypervisor_plugin("dragonball2").is_none());
    }

    #[test]
    fn test_add_kernel_params() {
        let mut boot_info = BootInfo {
            ..Default::default()
        };
        let params = vec![
            String::from("foo"),
            String::from("bar"),
            String::from("baz=faz"),
        ];
        boot_info.add_kernel_params(params);

        assert_eq!(boot_info.kernel_params, String::from("foo bar baz=faz"));

        let new_params = vec![
            String::from("boo=far"),
            String::from("a"),
            String::from("b=c"),
        ];
        boot_info.add_kernel_params(new_params);

        assert_eq!(
            boot_info.kernel_params,
            String::from("boo=far a b=c foo bar baz=faz")
        );
    }

    #[test]
    fn test_cpu_info_adjust_config() {
        // get CPU cores of the test node
        let node_cpus = num_cpus::get() as u32;
        let default_vcpus = default::DEFAULT_GUEST_VCPUS as i32;

        struct TestData<'a> {
            desc: &'a str,
            input: &'a mut CpuInfo,
            output: CpuInfo,
        }

        let tests = &mut [
            TestData {
                desc: "all with default values",
                input: &mut CpuInfo {
                    cpu_features: "".to_string(),
                    default_vcpus: 0,
                    default_maxvcpus: 0,
                },
                output: CpuInfo {
                    cpu_features: "".to_string(),
                    default_vcpus,
                    default_maxvcpus: node_cpus,
                },
            },
            TestData {
                desc: "all with big values",
                input: &mut CpuInfo {
                    cpu_features: "a,b,c".to_string(),
                    default_vcpus: 9999999,
                    default_maxvcpus: 9999999,
                },
                output: CpuInfo {
                    cpu_features: "a,b,c".to_string(),
                    default_vcpus: node_cpus as i32,
                    default_maxvcpus: node_cpus,
                },
            },
            TestData {
                desc: "default_vcpus lager than default_maxvcpus",
                input: &mut CpuInfo {
                    cpu_features: "a, b ,c".to_string(),
                    default_vcpus: -1,
                    default_maxvcpus: 1,
                },
                output: CpuInfo {
                    cpu_features: "a,b,c".to_string(),
                    default_vcpus: 1,
                    default_maxvcpus: 1,
                },
            },
        ];

        for (_, tc) in tests.iter_mut().enumerate() {
            // we can ensure that unwrap will not panic
            tc.input.adjust_config().unwrap();

            assert_eq!(
                tc.input.cpu_features, tc.output.cpu_features,
                "test[{}] cpu_features",
                tc.desc
            );
            assert_eq!(
                tc.input.default_vcpus, tc.output.default_vcpus,
                "test[{}] default_vcpus",
                tc.desc
            );
            assert_eq!(
                tc.input.default_maxvcpus, tc.output.default_maxvcpus,
                "test[{}] default_maxvcpus",
                tc.desc
            );
        }
    }
}
