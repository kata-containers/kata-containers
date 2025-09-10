// Copyright (c) 2019-2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Configuration information for hypervisors.
//!
//! The configuration information for hypervisors is complex, and different hypervisors require
//! different configuration information. To make it flexible and extensible, we build a multi-layer
//! architecture to manipulate hypervisor configuration information.
//!
//! - **Vendor layer**: The `HypervisorVendor` structure provides hook points for vendors to
//!   customize the configuration for its deployment.
//! - **Hypervisor plugin layer**: Provides hook points for different hypervisors to manipulate
//!   the configuration information.
//! - **Hypervisor common layer**: Handles generic logic for all types of hypervisors.
//!
//! These three layers are applied in order. Changes made by the vendor layer will be visible
//! to the hypervisor plugin layer and the common layer. Changes made by the plugin layer will
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
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

mod dragonball;
pub use self::dragonball::{DragonballConfig, HYPERVISOR_NAME_DRAGONBALL};

mod qemu;
pub use self::qemu::{QemuConfig, HYPERVISOR_NAME_QEMU};

mod ch;
pub use self::ch::{CloudHypervisorConfig, HYPERVISOR_NAME_CH};

mod remote;
pub use self::remote::{RemoteConfig, HYPERVISOR_NAME_REMOTE};

mod rate_limiter;
pub use self::rate_limiter::RateLimiterConfig;

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

const NO_VIRTIO_FS: &str = "none";
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
    /// by a block device. Options include:
    /// - `virtio-scsi`
    /// - `virtio-blk`
    /// - `nvdimm`
    #[serde(default)]
    pub block_device_driver: String,

    /// Block device AIO is the I/O mechanism specially for Qemu
    /// Options:
    ///
    ///   - threads
    ///     Pthread based disk I/O.
    ///
    ///   - native
    ///     Native Linux I/O.
    ///
    ///   - io_uring
    ///     Linux io_uring API. This provides the fastest I/O operations on Linux, requires kernel > 5.1 and
    ///     qemu >= 5.0.
    #[serde(default)]
    pub block_device_aio: String,

    /// Specifies cache-related options will be set to block devices or not.
    #[serde(default)]
    pub block_device_cache_set: bool,

    /// Specifies cache-related options for block devices.
    ///
    /// Denotes whether use of O_DIRECT (bypass the host page cache) is enabled.
    #[serde(default)]
    pub block_device_cache_direct: bool,

    /// Specifies cache-related options for block devices.
    ///
    /// Denotes whether flush requests for the device are ignored.
    #[serde(default)]
    pub block_device_cache_noflush: bool,

    /// If false and nvdimm is supported, use nvdimm device to plug guest image.
    #[serde(default)]
    pub disable_image_nvdimm: bool,

    /// The size in MiB will be plused to max memory of hypervisor.
    ///
    /// It is the memory address space for the NVDIMM device. If set block storage driver
    /// (`block_device_driver`) to `nvdimm`, should set `memory_offset` to the size of block device.
    #[serde(default)]
    pub memory_offset: u64,

    /// Enable vhost-user storage device, default false.
    ///
    /// Enabling this will result in some Linux reserved block type major range 240-254 being
    /// chosen to represent vhost-user devices.
    #[serde(default)]
    pub enable_vhost_user_store: bool,

    /// The base directory specifically used for vhost-user devices.
    ///
    /// Its sub-path `block` is used for block devices; `block/sockets` is where we expect
    /// vhost-user sockets to live; `block/devices` is where simulated block device nodes for
    /// vhost-user devices to live.
    #[serde(default)]
    pub vhost_user_store_path: String,

    /// List of valid annotations values for the vhost user store path.
    ///
    /// The default if not set is empty (all annotations rejected.)
    #[serde(default)]
    pub valid_vhost_user_store_paths: Vec<String>,

    /// controls disk I/O bandwidth (size in bits/sec)
    #[serde(default)]
    pub disk_rate_limiter_bw_max_rate: u64,
    /// increases the initial max rate
    #[serde(default)]
    pub disk_rate_limiter_bw_one_time_burst: Option<u64>,
    /// controls disk I/O bandwidth (size in ops/sec)
    #[serde(default)]
    pub disk_rate_limiter_ops_max_rate: u64,
    /// increases the initial max rate
    #[serde(default)]
    pub disk_rate_limiter_ops_one_time_burst: Option<u64>,
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
        if self.block_device_aio.is_empty() {
            self.block_device_aio = default::DEFAULT_BLOCK_DEVICE_AIO.to_string();
        } else {
            const VALID_BLOCK_DEVICE_AIO: &[&str] = &[
                default::DEFAULT_BLOCK_DEVICE_AIO,
                default::DEFAULT_BLOCK_DEVICE_AIO_NATIVE,
                default::DEFAULT_BLOCK_DEVICE_AIO_THREADS,
            ];
            if !VALID_BLOCK_DEVICE_AIO.contains(&self.block_device_aio.as_str()) {
                return Err(eother!(
                    "{} is unsupported block device AIO mode.",
                    self.block_device_aio
                ));
            }
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
    /// Path to guest kernel file on host.
    #[serde(default)]
    pub kernel: String,

    /// Guest kernel commandline.
    #[serde(default)]
    pub kernel_params: String,

    /// Path to initrd file on host.
    #[serde(default)]
    pub initrd: String,

    /// Path to root device on host.
    #[serde(default)]
    pub image: String,

    /// Rootfs filesystem type.
    #[serde(default)]
    pub rootfs_type: String,

    /// Path to the firmware.
    ///
    /// If you want that qemu uses the default firmware, leave this option empty.
    #[serde(default)]
    pub firmware: String,

    /// Block storage driver to be used for the VM rootfs when backed by a block device.
    /// Options include:
    /// - `virtio-pmem`
    /// - `virtio-blk-pci`
    /// - `virtio-blk-mmio`
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

    /// Add kernel parameters to bootinfo.
    ///
    /// New parameters are added before the original to let the original ones take priority.
    pub fn add_kernel_params(&mut self, params: Vec<String>) {
        let mut p = params;
        if !self.kernel_params.is_empty() {
            p.push(self.kernel_params.clone());
        }
        self.kernel_params = p.join(KERNEL_PARAM_DELIMITER);
    }

    /// Validate guest kernel image annotation.
    pub fn validate_boot_path(&self, path: &str) -> Result<()> {
        validate_path!(path, "path {} is invalid{}")?;
        Ok(())
    }
}

/// Virtual CPU configuration information.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CpuInfo {
    /// CPU features, comma-separated list of cpu features to pass to the cpu.
    ///
    /// Example: `cpu_features = "pmu=off,vmx=off"`
    #[serde(default)]
    pub cpu_features: String,

    /// Default number of vCPUs per SB/VM:
    /// - Unspecified or `0`: Set to `@DEFVCPUS@`
    /// - `< 0`: Set to the actual number of physical cores
    /// - `> 0` and `<= number of physical cores`: Set to specified number
    /// - `> number of physical cores`: Set to actual number of physical cores
    #[serde(default)]
    pub default_vcpus: f32,

    /// Default maximum number of vCPUs per SB/VM:
    /// - Unspecified or `0`: Set to actual number of physical cores or
    ///   maximum vCPUs supported by KVM if exceeded
    /// - `> 0` and `<= number of physical cores`: Set to specified number
    /// - `> number of physical cores`: Set to actual number of physical cores or
    ///   maximum vCPUs supported by KVM if exceeded
    ///
    /// # WARNING
    ///
    /// - This impacts memory footprint and CPU hotplug functionality
    /// - On ARM with GICv2, max is 8
    #[serde(default)]
    pub default_maxvcpus: u32,
}

impl CpuInfo {
    /// Adjust the configuration information after loading from configuration file.
    pub fn adjust_config(&mut self) -> Result<()> {
        let features: Vec<&str> = self.cpu_features.split(',').map(|v| v.trim()).collect();
        self.cpu_features = features.join(",");

        let cpus = num_cpus::get() as f32;

        // adjust default_maxvcpus
        if self.default_maxvcpus == 0 || self.default_maxvcpus as f32 > cpus {
            self.default_maxvcpus = cpus as u32;
        }

        // adjust default_vcpus
        if self.default_vcpus < 0.0 || self.default_vcpus > cpus {
            self.default_vcpus = cpus;
        } else if self.default_vcpus == 0.0 {
            self.default_vcpus = default::DEFAULT_GUEST_VCPUS as f32;
        }

        if self.default_vcpus > self.default_maxvcpus as f32 {
            self.default_vcpus = self.default_maxvcpus as f32;
        }

        Ok(())
    }

    /// Validate the configuration information.
    pub fn validate(&self) -> Result<()> {
        if self.default_vcpus > self.default_maxvcpus as f32 {
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
    /// Enable debug output for hypervisor and kernel parameters.
    #[serde(default)]
    pub enable_debug: bool,

    /// Log level for hypervisor. Possible values:
    /// - `trace`
    /// - `debug`
    /// - `info`
    /// - `warn`
    /// - `error`
    /// - `critical`
    #[serde(default = "default_hypervisor_log_level")]
    pub log_level: String,

    /// Enable dumping information about guest page structures.
    #[serde(default)]
    pub guest_memory_dump_paging: bool,

    /// Path to save guest memory dump files.
    ///
    /// When `GUEST_PANICKED` event occurs, guest memory will be dumped here.
    ///
    /// # WARNING
    ///
    /// Dumping guest memory can be time-consuming and use significant disk space.
    #[serde(default)]
    pub guest_memory_dump_path: String,

    /// Add a debug monitor socket when `enable_debug = true`.
    ///
    /// # WARNING
    ///
    /// Anyone with access to the monitor socket can take full control of Qemu.
    /// **Never** use in production.
    ///
    /// Valid values:
    /// - `"hmp"`
    /// - `"qmp"`
    /// - `"qmp-pretty"` (formatted JSON)
    ///
    /// Empty string disables this feature (default).
    ///
    /// Example usage in configuration:
    /// ```toml
    /// dbg_monitor_socket = "hmp"
    /// ```
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
    /// Number of bridges for hot plugging devices.
    ///
    /// # Limitations
    ///
    /// - Only PCI bridges supported
    /// - Max 30 devices per bridge
    /// - Max 5 PCI bridges per VM
    ///
    /// # Configuration
    ///
    /// - Unspecified or `0`: Set to `@DEFBRIDGES@`
    /// - `> 1` and `<= 5`: Set to specified number
    /// - `> 5`: Set to 5
    #[serde(default)]
    pub default_bridges: u32,

    /// Enable hotplugging on root bus for devices with large PCI bars.
    #[serde(default)]
    pub hotplug_vfio_on_root_bus: bool,

    /// Number of PCIe root ports to create during VM creation.
    ///
    /// Valid when `hotplug_vfio_on_root_bus = true` and `machine_type = "q35"`.
    #[serde(default)]
    pub pcie_root_port: u32,

    /// Number of PCIe switch ports to create during VM creation.
    ///
    /// Valid when `hotplug_vfio_on_root_bus = true` and `machine_type = "q35"`.
    #[serde(default)]
    pub pcie_switch_port: u32,

    /// Enable vIOMMU device.
    ///
    /// Adds kernel parameters: `intel_iommu=on,iommu=pt`
    #[serde(default)]
    pub enable_iommu: bool,

    /// Set `iommu_platform=on` for VM devices.
    #[serde(default)]
    pub enable_iommu_platform: bool,

    /// Enable balloon device reporting.
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
        // Root Port and Switch Port cannot be set simultaneously
        if self.pcie_root_port > 0 && self.pcie_switch_port > 0 {
            return Err(eother!(
                "Root Port and Switch Port set at the same time is forbidden."
            ));
        }

        Ok(())
    }
}

/// Virtual machine PCIe Topology configuration.
#[derive(Clone, Debug, Default)]
pub struct TopologyConfigInfo {
    /// Hypervisor name.
    pub hypervisor_name: String,

    /// Device information.
    pub device_info: DeviceInfo,
}

impl TopologyConfigInfo {
    /// Initialize the topology config info from TOML config.
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

    /// Machine accelerators as comma-separated list.
    ///
    /// Example: `machine_accelerators = "nosmm,nosmbus,nosata,nopit,static-prt,nofw"`
    #[serde(default)]
    pub machine_accelerators: String,

    /// Flash image files for VM.
    ///
    /// Format: `["/path/to/flash0.img", "/path/to/flash1.img"]`
    #[serde(default)]
    pub pflashes: Vec<String>,

    /// Default entropy source path.
    ///
    /// Options:
    /// - `/dev/urandom` (non-blocking, recommended)
    /// - `/dev/random` (blocking, may cause boot delays)
    #[serde(default)]
    pub entropy_source: String,

    /// List of valid entropy source paths for annotations.
    ///
    /// Default: empty (all annotations rejected)
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
            resolve_path!(*pflash, "Flash image file {} is invalid: {}")?;
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
    /// Memory allocated using hugetlbfs backend
    #[serde(rename = "hugetlbfs")]
    Hugetlbfs,

    /// Memory allocated using transparent huge pages
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

    /// Default maximum memory in MiB per SB/VM:
    /// - Unspecified or `0`: Set to actual physical RAM
    /// - `> 0` and `<= physical RAM`: Set to specified number
    /// - `> physical RAM`: Set to actual physical RAM
    #[serde(default)]
    pub default_maxmemory: u32,

    /// Default memory slots per SB/VM.
    ///
    /// Determines how many times memory can be hot-added.
    #[serde(default)]
    pub memory_slots: u32,

    /// File-based guest memory support path.
    ///
    /// Disabled by default. Automatically set to `/dev/shm` for virtio-fs.
    #[serde(default)]
    pub file_mem_backend: String,

    /// Valid file memory backends for annotations.
    ///
    /// Default: empty (all annotations rejected)
    #[serde(default)]
    pub valid_file_mem_backends: Vec<String>,

    /// Pre-allocate VM RAM (reduces container density).
    #[serde(default)]
    pub enable_mem_prealloc: bool,

    /// Use huge pages for VM RAM.
    #[serde(default)]
    pub enable_hugepages: bool,

    /// Huge page type:
    /// - `hugetlbfs`
    /// - `thp`
    #[serde(default)]
    pub hugepage_type: HugePageType,

    /// Enable virtio-mem.
    ///
    /// Requires `echo 1 > /proc/sys/vm/overcommit_memory`
    #[serde(default)]
    pub enable_virtio_mem: bool,

    /// Enable swap in guest.
    #[serde(default)]
    pub enable_guest_swap: bool,

    /// Swap device path in guest (when `enable_guest_swap = true`).
    #[serde(default = "default_guest_swap_path")]
    pub guest_swap_path: String,

    /// Swap size as percentage of total memory.
    #[serde(default = "default_guest_swap_size_percent")]
    pub guest_swap_size_percent: u64,

    /// Threshold in seconds before creating swap device.
    #[serde(default = "default_guest_swap_create_threshold_secs")]
    pub guest_swap_create_threshold_secs: u64,
}

fn default_guest_swap_size_percent() -> u64 {
    100
}

fn default_guest_swap_path() -> String {
    "/run/kata-containers/swap".to_string()
}

fn default_guest_swap_create_threshold_secs() -> u64 {
    60
}

impl MemoryInfo {
    /// Adjusts the configuration information after loading from a configuration file.
    ///
    /// This method resolves the path for the file memory backend and
    /// sets `default_maxmemory` if it's currently zero, calculating it
    /// from the total system memory.
    pub fn adjust_config(&mut self) -> Result<()> {
        resolve_path!(
            self.file_mem_backend,
            "Memory backend file {} is invalid: {}"
        )?;
        if self.default_maxmemory == 0 {
            let s = System::new_with_specifics(
                RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
            );
            self.default_maxmemory = Byte::from_u64(s.total_memory())
                .get_adjusted_unit(Unit::MiB)
                .get_value() as u32;
        }
        Ok(())
    }

    /// Validates the memory configuration information.
    ///
    /// This ensures that critical memory parameters like `default_memory`
    /// and `memory_slots` are non-zero, and checks the validity of
    /// the memory backend file path.
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

    /// Validates the path of memory backend files against configured patterns.
    pub fn validate_memory_backend_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_file_mem_backends, path)
    }
}

/// Configuration information for network settings.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct NetworkInfo {
    /// If set to `true`, disables the `vhost-net` backend for `virtio-net`.
    ///
    /// The default is `false`, which prioritizes network I/O performance
    /// over security (as `vhost-net` runs in ring0).
    #[serde(default)]
    pub disable_vhost_net: bool,

    /// Sets the maximum inbound bandwidth for network I/O in bits/sec for the sandbox/VM.
    ///
    /// In QEMU, this is implemented using classful `qdiscs HTB` (Hierarchy Token Bucket)
    /// to manage traffic. A value of `0` indicates an unlimited rate (default).
    #[serde(default)]
    pub rx_rate_limiter_max_rate: u64,

    /// Sets the maximum outbound bandwidth for network I/O in bits/sec for the sandbox/VM.
    ///
    /// In QEMU, this is implemented using classful `qdiscs HTB` (Hierarchy Token Bucket)
    /// and `ifb` (Intermediate Functional Block) to manage traffic. A value of `0`
    /// indicates an unlimited rate (default).
    #[serde(default)]
    pub tx_rate_limiter_max_rate: u64,

    /// Configures the number of network queues.
    #[serde(default)]
    pub network_queues: u32,
}

impl NetworkInfo {
    /// Adjusts the network configuration information after loading from a configuration file.
    /// (Currently, this method performs no adjustments.)
    pub fn adjust_config(&mut self) -> Result<()> {
        Ok(())
    }

    /// Validates the network configuration information.
    /// (Currently, this method performs no specific validations beyond basic type checks.)
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Configuration information for security settings.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SecurityInfo {
    /// Enables running the QEMU VMM as a non-root user.
    ///
    /// By default, the QEMU VMM runs as root. When this is set to `true`,
    /// the QEMU VMM process runs as a non-root, randomly generated user.
    /// Refer to the documentation for limitations of this mode.
    #[serde(default)]
    pub rootless: bool,

    /// Disables `seccomp` for the guest VM.
    #[serde(default)]
    pub disable_seccomp: bool,

    /// Enables confidential guest support.
    ///
    /// Toggling this setting may activate different hardware features, ranging from
    /// memory encryption to both memory and CPU-state encryption and integrity.
    /// The Kata Containers runtime dynamically detects the available feature set
    /// and aims to enable the largest possible one.
    #[serde(default)]
    pub confidential_guest: bool,

    /// If `false`, SEV (Secure Encrypted Virtualization) is preferred even if
    /// SEV-SNP (Secure Nested Paging) is also available.
    #[serde(default)]
    pub sev_snp_guest: bool,

    /// Path to OCI hook binaries in the *guest rootfs*.
    ///
    /// This setting does not affect host-side hooks, which must instead be
    /// added to the OCI spec passed to the runtime.
    ///
    /// To create a rootfs with hooks, you can customize the osbuilder scripts:
    /// <https://github.com/kata-containers/kata-containers/tree/main/tools/osbuilder>
    ///
    /// Hooks must be stored in a subdirectory of `guest_hook_path` according to
    /// their hook type, e.g., `guest_hook_path/{prestart,poststart,poststop}`.
    /// The agent will scan these directories for executable files and add them,
    /// in lexicographical order, to the lifecycle of the guest container.
    ///
    /// Hooks are executed in the runtime namespace of the guest. See the official
    /// Open Containers Initiative (OCI) documentation for more details:
    /// <https://github.com/opencontainers/runtime-spec/blob/v1.0.1/config.md#posix-platform-hooks>
    ///
    /// Warnings will be logged if any error is encountered while scanning for hooks,
    /// but it will not abort container execution.
    #[serde(default)]
    pub guest_hook_path: String,

    /// Initdata provides dynamic configuration (such as policies, configs, and identity files)
    /// in an encoded format that users inject into the TEE Guest upon CVM launch.
    ///
    /// It is implemented based on the `InitData Specification`:
    /// <https://github.com/confidential-containers/trustee/blob/61c1dc60ee1f926c2eb95d69666c2430c3fea808/kbs/docs/initdata.md>
    #[serde(default)]
    pub initdata: String,

    /// List of valid annotation names for the hypervisor.
    ///
    /// Each member of the list is a regular expression, representing the base name
    /// of the annotation (e.g., "path" for "io.katacontainers.config.hypervisor.path").
    #[serde(default)]
    pub enable_annotations: Vec<String>,

    /// Defines the Intel Quote Generation Service (QGS) port exposed from the host.
    #[serde(
        default = "default_qgs_port",
        rename = "tdx_quote_generation_service_socket_port"
    )]
    pub qgs_port: u32,

    /// Qemu seccomp sandbox feature
    /// comma-separated list of seccomp sandbox features to control the syscall access.
    /// For example, `seccompsandbox= "on,obsolete=deny,spawn=deny,resourcecontrol=deny"`
    /// Note: "elevateprivileges=deny" doesn't work with daemonize option, so it's removed from the seccomp sandbox
    /// Another note: enabling this feature may reduce performance, you may enable
    /// /proc/sys/net/core/bpf_jit_enable to reduce the impact. see https://man7.org/linux/man-pages/man8/bpfc.8.html
    pub seccomp_sandbox: Option<String>,
}

fn default_qgs_port() -> u32 {
    4050
}

impl SecurityInfo {
    /// Adjusts the security configuration information after loading from a configuration file.
    ///
    /// Sets `guest_hook_path` to its default value if it is empty.
    pub fn adjust_config(&mut self) -> Result<()> {
        Ok(())
    }

    /// Validates the security configuration information.
    /// (Currently, this method performs no specific validations.)
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }

    /// Checks whether a given annotation key is enabled based on the `enable_annotations` list.
    ///
    /// Returns `true` if the annotation key (after removing the `KATA_ANNO_CFG_HYPERVISOR_PREFIX`)
    /// matches any of the regular expressions in `enable_annotations`.
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

    /// Validates a given file system path.
    pub fn validate_path(&self, path: &str) -> Result<()> {
        validate_path!(path, "path {} is invalid{}")?;
        Ok(())
    }
}

/// Configuration information for shared filesystems, such as virtio-9p and virtio-fs.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SharedFsInfo {
    /// Type of shared file system to use:
    /// - `virtio-fs` (default)
    /// - `virtio-9p`
    /// - `none` (disables shared filesystem)
    pub shared_fs: Option<String>,

    /// Path to the `vhost-user-fs` daemon executable.
    #[serde(default)]
    pub virtio_fs_daemon: String,

    /// List of valid annotation values for the `virtiofsd` daemon.
    ///
    /// If not set, the default is an empty list, meaning all annotations are rejected.
    #[serde(default)]
    pub valid_virtio_fs_daemon_paths: Vec<String>,

    /// Extra arguments for the `virtiofsd` daemon.
    ///
    /// Format example: `["-o", "arg1=xxx,arg2", "-o", "hello world", "--arg3=yyy"]`
    ///
    /// Refer to `virtiofsd -h` for possible options.
    #[serde(default)]
    pub virtio_fs_extra_args: Vec<String>,

    /// Cache mode for `virtio-fs`:
    /// - `never`: Metadata, data, and pathname lookups are not cached in the guest.
    ///   They are always fetched from the host, and any changes are immediately pushed to the host.
    /// - `auto`: Metadata and pathname lookup cache expires after a configured amount of time
    ///   (default is 1 second). Data is cached while the file is open (close-to-open consistency).
    /// - `always`: Metadata, data, and pathname lookups are cached in the guest and never expire.
    #[serde(default)]
    pub virtio_fs_cache: String,

    /// Default size of the DAX cache in MiB for `virtio-fs`.
    #[serde(default)]
    pub virtio_fs_cache_size: u32,

    /// Default size of virtqueues for `virtio-fs`.
    #[serde(default)]
    pub virtio_fs_queue_size: u32,

    /// Enables `virtio-fs` DAX (Direct Access) window if `true`.
    #[serde(default)]
    pub virtio_fs_is_dax: bool,

    /// This is the `msize` used for 9p shares. It represents the number of bytes
    /// used for the 9p packet payload.
    #[serde(default)]
    pub msize_9p: u32,
}

impl SharedFsInfo {
    /// Adjusts the shared filesystem configuration after loading from a configuration file.
    ///
    /// Handles default values for `shared_fs` type, `virtio-fs` specific settings
    /// (daemon path, cache mode, DAX), and `virtio-9p` msize.
    pub fn adjust_config(&mut self) -> Result<()> {
        if self.shared_fs.as_deref() == Some(NO_VIRTIO_FS) {
            self.shared_fs = None;
            return Ok(());
        }

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

    /// Validates the shared filesystem configuration.
    ///
    /// Checks the validity of the selected `shared_fs` type and
    /// performs specific validations for `virtio-fs` and `virtio-9p` settings.
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

    /// Validates the path of the virtio-fs daemon, especially for annotations.
    pub fn validate_virtiofs_daemon_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_virtio_fs_daemon_paths, path)
    }

    /// Adjusts virtio-fs specific configuration settings.
    ///
    /// Handles `virtio_fs_daemon` path resolution (unless in inline mode),
    /// default `virtio_fs_cache` mode, and `virtio_fs_is_dax` with `virtio_fs_cache_size`.
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

    /// Validates virtio-fs specific configuration settings.
    ///
    /// Checks the validity of the `virtio_fs_daemon` path (unless in inline mode),
    /// `virtio_fs_cache` mode, and `virtio_fs_is_dax` with `virtio_fs_cache_size`.
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

/// Configuration information for a remote hypervisor type.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RemoteInfo {
    /// Socket path for the remote hypervisor.
    #[serde(default)]
    pub hypervisor_socket: String,

    /// Timeout (in seconds) for creating the remote hypervisor.
    #[serde(default)]
    pub hypervisor_timeout: i32,

    /// GPU specific annotations (currently only applicable for Remote Hypervisor).
    /// Specifies the number of GPUs required for the Kata VM.
    #[serde(default)]
    pub default_gpus: u32,
    /// Specifies the GPU model, e.g., "tesla", "h100", "a100", "radeon", etc.
    #[serde(default)]
    pub default_gpu_model: String,
}

/// Common configuration information for hypervisors.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Hypervisor {
    /// Path to the hypervisor executable.
    #[serde(default)]
    pub path: String,
    /// List of valid annotation values for the hypervisor path.
    ///
    /// Each member of the list is a path pattern as described by `glob(3)`.
    /// The default is an empty list (all annotations rejected) if not set.
    #[serde(default)]
    pub valid_hypervisor_paths: Vec<String>,

    /// Hypervisor control executable path.
    #[serde(default)]
    pub ctlpath: String,
    /// List of valid annotation values for the hypervisor control executable path.
    ///
    /// Each member of the list is a path pattern as described by `glob(3)`.
    /// The default is an empty list (all annotations rejected) if not set.
    #[serde(default)]
    pub valid_ctlpaths: Vec<String>,

    /// Path to the jailer executable.
    #[serde(default)]
    pub jailer_path: String,
    /// List of valid annotation values for the hypervisor jailer path.
    ///
    /// Each member of the list is a path pattern as described by `glob(3)`.
    /// The default is an empty list (all annotations rejected) if not set.
    #[serde(default)]
    pub valid_jailer_paths: Vec<String>,

    /// Disables the runtime customizations applied when running on top of a VMM.
    ///
    /// Setting this to `true` will make the runtime behave as it would when running on bare metal.
    #[serde(default)]
    pub disable_nesting_checks: bool,

    /// Enables the use of iothreads (data-plane).
    ///
    /// When enabled, I/O operations are handled in a separate I/O thread.
    /// This is currently only implemented for SCSI devices.
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

    /// A sandbox annotation used to specify the host path to the `prefetch_files.list`
    /// for the container image being used. The runtime will pass this path to the
    /// Hypervisor to search for the corresponding prefetch list file.
    ///
    /// Example: `/path/to/<uid>/xyz.com/fedora:36/prefetch_file.list`
    #[serde(default)]
    pub prefetch_list_path: String,

    /// Vendor customized runtime configuration.
    #[serde(default, flatten)]
    pub vendor: HypervisorVendor,

    /// Disables applying SELinux on the container process within the guest.
    #[serde(default = "yes")]
    pub disable_guest_selinux: bool,
}

fn yes() -> bool {
    true
}

impl Hypervisor {
    /// Validates the path of the hypervisor executable against configured patterns.
    pub fn validate_hypervisor_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_hypervisor_paths, path)
    }

    /// Validates the path of the hypervisor control executable against configured patterns.
    pub fn validate_hypervisor_ctlpath<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_ctlpaths, path)
    }

    /// Validates the path of the jailer executable against configured patterns.
    pub fn validate_jailer_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        validate_path_pattern(&self.valid_jailer_paths, path)
    }
}

impl ConfigOps for Hypervisor {
    /// Adjusts the overall hypervisor configuration after loading from the configuration file.
    ///
    /// This method iterates through configured hypervisors, calls their respective
    /// plugin adjustments, and then recursively adjusts nested configuration structs
    /// like `blockdev_info`, `boot_info`, etc. It also resolves paths for
    /// `prefetch_list_path`.
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

    /// Validates the overall hypervisor configuration.
    ///
    /// This method iterates through configured hypervisors, calls their respective
    /// plugin validations, and then recursively validates nested configuration structs
    /// and various paths (`path`, `ctlpath`, `jailer_path`, `prefetch_list_path`).
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
        let node_cpus = num_cpus::get() as f32;
        let default_vcpus = default::DEFAULT_GUEST_VCPUS as f32;

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
                    default_vcpus: 0.0,
                    default_maxvcpus: 0,
                },
                output: CpuInfo {
                    cpu_features: "".to_string(),
                    default_vcpus,
                    default_maxvcpus: node_cpus as u32,
                },
            },
            TestData {
                desc: "all with big values",
                input: &mut CpuInfo {
                    cpu_features: "a,b,c".to_string(),
                    default_vcpus: 9999999.0,
                    default_maxvcpus: 9999999,
                },
                output: CpuInfo {
                    cpu_features: "a,b,c".to_string(),
                    default_vcpus: node_cpus,
                    default_maxvcpus: node_cpus as u32,
                },
            },
            TestData {
                desc: "default_vcpus lager than default_maxvcpus",
                input: &mut CpuInfo {
                    cpu_features: "a, b ,c".to_string(),
                    default_vcpus: -1.0,
                    default_maxvcpus: 1,
                },
                output: CpuInfo {
                    cpu_features: "a,b,c".to_string(),
                    default_vcpus: 1.0,
                    default_maxvcpus: 1,
                },
            },
        ];

        for tc in tests.iter_mut() {
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
