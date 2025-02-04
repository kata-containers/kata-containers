// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::PathBuf;

pub mod ch_api;
pub mod convert;
pub mod net_util;
mod virtio_devices;

use crate::virtio_devices::RateLimiterConfig;
use kata_sys_util::protection::GuestProtection;
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
pub use net_util::MacAddr;

pub const MAX_NUM_PCI_SEGMENTS: u16 = 16;

mod errors;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct BalloonConfig {
    pub size: u64,
    /// Option to deflate the balloon in case the guest is out of memory.
    #[serde(default)]
    pub deflate_on_oom: bool,
    /// Option to enable free page reporting from the guest.
    #[serde(default)]
    pub free_page_reporting: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct CmdlineConfig {
    pub args: String,
}

impl CmdlineConfig {
    fn is_empty(&self) -> bool {
        self.args.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct ConsoleConfig {
    //#[serde(default = "default_consoleconfig_file")]
    pub file: Option<PathBuf>,
    pub mode: ConsoleOutputMode,
    #[serde(default)]
    pub iommu: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum ConsoleOutputMode {
    #[default]
    Off,
    Pty,
    Tty,
    File,
    Null,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct CpuAffinity {
    pub vcpu: u8,
    pub host_cpus: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct CpusConfig {
    pub boot_vcpus: u8,
    pub max_vcpus: u8,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology: Option<CpuTopology>,
    #[serde(default)]
    pub kvm_hyperv: bool,
    #[serde(skip_serializing_if = "u8_is_zero")]
    pub max_phys_bits: u8,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affinity: Option<Vec<CpuAffinity>>,
    #[serde(default)]
    pub features: CpuFeatures,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct CpuFeatures {
    #[cfg(target_arch = "x86_64")]
    #[serde(default)]
    pub amx: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct CpuTopology {
    pub threads_per_core: u8,
    pub cores_per_die: u8,
    pub dies_per_package: u8,
    pub packages: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct DeviceConfig {
    pub path: PathBuf,
    #[serde(default)]
    pub iommu: bool,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub pci_segment: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct DiskConfig {
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub direct: bool,
    #[serde(default)]
    pub iommu: bool,
    //#[serde(default = "default_diskconfig_num_queues")]
    pub num_queues: usize,
    //#[serde(default = "default_diskconfig_queue_size")]
    pub queue_size: u16,
    #[serde(default)]
    pub vhost_user: bool,
    pub vhost_socket: Option<String>,
    #[serde(default)]
    pub rate_limiter_config: Option<RateLimiterConfig>,
    #[serde(default)]
    pub id: Option<String>,
    // For testing use only. Not exposed in API.
    #[serde(default)]
    pub disable_io_uring: bool,
    #[serde(default)]
    pub pci_segment: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct FsConfig {
    pub tag: String,
    pub socket: PathBuf,
    //#[serde(default = "default_fsconfig_num_queues")]
    pub num_queues: usize,
    //#[serde(default = "default_fsconfig_queue_size")]
    pub queue_size: u16,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub pci_segment: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum HotplugMethod {
    #[default]
    Acpi,
    VirtioMem,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct InitramfsConfig {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct KernelConfig {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct MemoryConfig {
    pub size: u64,
    #[serde(default)]
    pub mergeable: bool,
    #[serde(default)]
    pub hotplug_method: HotplugMethod,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotplug_size: Option<u64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotplugged_size: Option<u64>,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub hugepages: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hugepage_size: Option<u64>,
    #[serde(default)]
    pub prefault: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zones: Option<Vec<MemoryZoneConfig>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct MemoryZoneConfig {
    pub id: String,
    pub size: u64,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub hugepages: bool,
    #[serde(default)]
    pub hugepage_size: Option<u64>,
    #[serde(default)]
    pub host_numa_node: Option<u32>,
    #[serde(default)]
    pub hotplug_size: Option<u64>,
    #[serde(default)]
    pub hotplugged_size: Option<u64>,
    #[serde(default)]
    pub prefault: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct NetConfig {
    //#[serde(default = "default_netconfig_tap")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<String>,
    //#[serde(default = "default_netconfig_ip")]
    pub ip: Ipv4Addr,
    //#[serde(default = "default_netconfig_mask")]
    pub mask: Ipv4Addr,
    //#[serde(default = "default_netconfig_mac")]
    pub mac: MacAddr,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_mac: Option<MacAddr>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,
    #[serde(default)]
    pub iommu: bool,
    //#[serde(default = "default_netconfig_num_queues")]
    #[serde(skip_serializing_if = "usize_is_zero")]
    pub num_queues: usize,
    //#[serde(default = "default_netconfig_queue_size")]
    #[serde(skip_serializing_if = "u16_is_zero")]
    pub queue_size: u16,
    #[serde(default)]
    pub vhost_user: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vhost_socket: Option<String>,
    #[serde(default)]
    pub vhost_mode: VhostMode,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fds: Option<Vec<i32>>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limiter_config: Option<RateLimiterConfig>,
    #[serde(default)]
    #[serde(skip_serializing_if = "u16_is_zero")]
    pub pci_segment: u16,
}

impl Default for NetConfig {
    fn default() -> Self {
        NetConfig {
            tap: None,
            ip: Ipv4Addr::new(192, 168, 249, 1),
            mask: Ipv4Addr::new(255, 255, 255, 0),
            mac: MacAddr::default(),
            host_mac: None,
            mtu: None,
            iommu: false,
            num_queues: 0,
            queue_size: 0,
            vhost_user: false,
            vhost_socket: None,
            vhost_mode: VhostMode::default(),
            id: None,
            fds: None,
            rate_limiter_config: None,
            pci_segment: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct NumaConfig {
    #[serde(default)]
    pub guest_numa_id: u32,
    #[serde(default)]
    pub cpus: Option<Vec<u8>>,
    #[serde(default)]
    pub distances: Option<Vec<NumaDistance>>,
    #[serde(default)]
    pub memory_zones: Option<Vec<String>>,
    #[cfg(target_arch = "x86_64")]
    #[serde(default)]
    pub sgx_epc_sections: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct NumaDistance {
    #[serde(default)]
    pub destination: u32,
    #[serde(default)]
    pub distance: u8,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PayloadConfig {
    #[serde(default)]
    pub firmware: Option<PathBuf>,
    #[serde(default)]
    pub kernel: Option<PathBuf>,
    #[serde(default)]
    pub cmdline: Option<String>,
    #[serde(default)]
    pub initramfs: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct PlatformConfig {
    //#[serde(default = "default_platformconfig_num_pci_segments")]
    pub num_pci_segments: u16,
    #[serde(default)]
    pub iommu_segments: Option<Vec<u16>>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub oem_strings: Option<Vec<String>>,
    #[serde(default)]
    pub tdx: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct PmemConfig {
    pub file: PathBuf,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub iommu: bool,
    #[serde(default)]
    pub discard_writes: bool,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub pci_segment: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct RngConfig {
    pub src: PathBuf,
    #[serde(default)]
    pub iommu: bool,
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct SgxEpcConfig {
    pub id: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub prefault: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct UserDeviceConfig {
    pub socket: PathBuf,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub pci_segment: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct VdpaConfig {
    pub path: PathBuf,
    //#[serde(default = "default_vdpaconfig_num_queues")]
    pub num_queues: usize,
    #[serde(default)]
    pub iommu: bool,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub pci_segment: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum VhostMode {
    #[default]
    Client,
    Server,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct VmConfig {
    #[serde(default)]
    pub cpus: CpusConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel: Option<KernelConfig>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initramfs: Option<InitramfsConfig>,
    #[serde(default)]
    #[serde(skip_serializing_if = "CmdlineConfig::is_empty")]
    pub cmdline: CmdlineConfig,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<PayloadConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disks: Option<Vec<DiskConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<Vec<NetConfig>>,
    #[serde(default)]
    pub rng: RngConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balloon: Option<BalloonConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs: Option<Vec<FsConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmem: Option<Vec<PmemConfig>>,
    pub serial: ConsoleConfig,
    pub console: ConsoleConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devices: Option<Vec<DeviceConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_devices: Option<Vec<UserDeviceConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vdpa: Option<Vec<VdpaConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vsock: Option<VsockConfig>,
    #[serde(default)]
    pub iommu: bool,
    #[cfg(target_arch = "x86_64")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sgx_epc: Option<Vec<SgxEpcConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numa: Option<Vec<NumaConfig>>,
    #[serde(default)]
    pub watchdog: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<PlatformConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct VsockConfig {
    pub cid: u64,
    pub socket: PathBuf,
    #[serde(default)]
    pub iommu: bool,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub pci_segment: u16,
}

//--------------------------------------------------------------------
// For serde serialization

#[allow(clippy::trivially_copy_pass_by_ref)]
fn u8_is_zero(v: &u8) -> bool {
    *v == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn usize_is_zero(v: &usize) -> bool {
    *v == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn u16_is_zero(v: &u16) -> bool {
    *v == 0
}

// Type used to simplify conversion from a generic Hypervisor config
// to a CH specific VmConfig.
#[derive(Debug, Clone, Default)]
pub struct NamedHypervisorConfig {
    pub kernel_params: String,
    pub sandbox_path: String,
    pub vsock_socket_path: String,
    pub cfg: HypervisorConfig,

    pub shared_fs_devices: Option<Vec<FsConfig>>,
    pub network_devices: Option<Vec<NetConfig>>,

    // Set to the available guest protection *iff* BOTH of the following
    // conditions are true:
    //
    // - The hardware supports guest protection.
    // - The user has requested that guest protection be used.
    pub guest_protection_to_use: GuestProtection,
}

// Returns true if the enabled guest protection is Intel TDX.
pub fn guest_protection_is_tdx(guest_protection_to_use: GuestProtection) -> bool {
    matches!(guest_protection_to_use, GuestProtection::Tdx(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_sys_util::protection::TDXDetails;

    #[test]
    fn test_guest_protection_is_tdx() {
        let tdx_details = TDXDetails {
            major_version: 1,
            minor_version: 0,
        };

        #[derive(Debug)]
        struct TestData {
            protection: GuestProtection,
            result: bool,
        }

        let tests = &[
            TestData {
                protection: GuestProtection::NoProtection,
                result: false,
            },
            TestData {
                protection: GuestProtection::Pef,
                result: false,
            },
            TestData {
                protection: GuestProtection::Se,
                result: false,
            },
            TestData {
                protection: GuestProtection::Sev,
                result: false,
            },
            TestData {
                protection: GuestProtection::Snp,
                result: false,
            },
            TestData {
                protection: GuestProtection::Tdx(tdx_details),
                result: true,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = guest_protection_is_tdx(d.protection.clone());

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            if d.result {
                assert!(result, "{}", msg);
            } else {
                assert!(!result, "{}", msg);
            }
        }
    }
}
