// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::net_util::MAC_ADDR_LEN;
use crate::NamedHypervisorConfig;
use crate::VmConfig;
use crate::{
    ConsoleConfig, ConsoleOutputMode, CpuFeatures, CpuTopology, CpusConfig, DiskConfig, MacAddr,
    MemoryConfig, PayloadConfig, PlatformConfig, PmemConfig, RngConfig, VsockConfig,
};
use anyhow::{anyhow, Context, Result};
use kata_types::config::default::DEFAULT_CH_ENTROPY_SOURCE;
use kata_types::config::hypervisor::{CpuInfo, MachineInfo, MemoryInfo};
use kata_types::config::BootInfo;
use std::convert::TryFrom;
use std::fmt::Display;
use std::path::PathBuf;

use crate::errors::*;

// 1 MiB
const MIB: u64 = 1024 * 1024;

const PMEM_ALIGN_BYTES: u64 = 2 * MIB;

const DEFAULT_CH_MAX_PHYS_BITS: u8 = 46;

const DEFAULT_VSOCK_CID: u64 = 3;

impl TryFrom<NamedHypervisorConfig> for VmConfig {
    type Error = VmConfigError;

    fn try_from(n: NamedHypervisorConfig) -> Result<Self, Self::Error> {
        let kernel_params = if n.kernel_params.is_empty() {
            None
        } else {
            Some(n.kernel_params)
        };

        let cfg = n.cfg;

        let debug = cfg.debug_info.enable_debug;
        let confidential_guest = cfg.security_info.confidential_guest;

        let tdx_enabled = n.tdx_enabled;

        let vsock_socket_path = if n.vsock_socket_path.is_empty() {
            return Err(VmConfigError::EmptyVsockSocketPath);
        } else {
            n.vsock_socket_path
        };

        let sandbox_path = if n.sandbox_path.is_empty() {
            return Err(VmConfigError::EmptySandboxPath);
        } else {
            n.sandbox_path
        };

        let fs = n.shared_fs_devices;
        let net = n.network_devices;

        let cpus = CpusConfig::try_from(cfg.cpu_info).map_err(VmConfigError::CPUError)?;

        let rng = RngConfig::from(cfg.machine_info);

        // Note how CH handles the different image types:
        //
        // - A standard image is specified in PmemConfig.
        // - An initrd/initramfs is specified in PayloadConfig.
        // - A confidential guest image is specified by a DiskConfig.
        //   - If TDX is enabled, the firmware (`td-shim` [1]) must be
        //     specified in PayloadConfig.
        // - A confidential guest initrd is specified by a PayloadConfig with
        //   firmware.
        //
        // [1] - https://github.com/confidential-containers/td-shim
        let boot_info = cfg.boot_info;

        let use_initrd = !boot_info.initrd.is_empty();
        let use_image = !boot_info.image.is_empty();

        if use_initrd && use_image {
            return Err(VmConfigError::MultipleBootFiles);
        }

        if !use_initrd && !use_image {
            return Err(VmConfigError::NoBootFile);
        }

        let pmem = if use_initrd || confidential_guest {
            None
        } else {
            let pmem = PmemConfig::try_from(&boot_info).map_err(VmConfigError::PmemError)?;

            Some(vec![pmem])
        };

        let payload = Some(
            PayloadConfig::try_from((boot_info.clone(), kernel_params, tdx_enabled))
                .map_err(VmConfigError::PayloadError)?,
        );

        let disks = if confidential_guest && use_image {
            let disk = DiskConfig::try_from(boot_info).map_err(VmConfigError::DiskError)?;

            Some(vec![disk])
        } else {
            None
        };

        let serial = get_serial_cfg(debug, confidential_guest);
        let console = get_console_cfg(debug, confidential_guest);

        let memory = MemoryConfig::try_from((cfg.memory_info, confidential_guest))
            .map_err(VmConfigError::MemoryError)?;

        std::fs::create_dir_all(sandbox_path.clone())
            .map_err(|e| VmConfigError::SandboxError(sandbox_path, e.to_string()))?;

        let vsock = VsockConfig::try_from((vsock_socket_path, DEFAULT_VSOCK_CID))
            .map_err(VmConfigError::VsockError)?;

        let platform = get_platform_cfg(tdx_enabled);

        let cfg = VmConfig {
            cpus,
            memory,
            serial,
            console,
            payload,
            fs,
            net,
            pmem,
            disks,
            vsock: Some(vsock),
            rng,
            platform,

            ..Default::default()
        };

        Ok(cfg)
    }
}

impl TryFrom<(String, u64)> for VsockConfig {
    type Error = VsockConfigError;

    fn try_from(args: (String, u64)) -> Result<Self, Self::Error> {
        let vsock_socket_path = args.0;
        let cid = args.1;

        let path = if vsock_socket_path.is_empty() {
            return Err(VsockConfigError::NoVsockSocketPath);
        } else {
            vsock_socket_path
        };

        let cfg = VsockConfig {
            cid,
            socket: PathBuf::from(path),

            ..Default::default()
        };

        Ok(cfg)
    }
}

impl TryFrom<(MemoryInfo, bool)> for MemoryConfig {
    type Error = MemoryConfigError;

    fn try_from(args: (MemoryInfo, bool)) -> Result<Self, Self::Error> {
        let mem = args.0;
        let confidential_guest = args.1;

        if mem.default_memory == 0 {
            return Err(MemoryConfigError::NoDefaultMemory);
        }

        let sysinfo = nix::sys::sysinfo::sysinfo().map_err(MemoryConfigError::SysInfoFail)?;

        let max_mem_bytes = sysinfo.ram_total();

        let mem_bytes: u64 = MIB
            .checked_mul(mem.default_memory as u64)
            .ok_or(())
            .map_err(|_| MemoryConfigError::BadDefaultMemSize(mem.default_memory))?;

        if mem_bytes > max_mem_bytes {
            return Err(MemoryConfigError::DefaultMemSizeTooBig);
        }

        let hotplug_size = if confidential_guest {
            None
        } else {
            // The amount of memory that can be hot-plugged is the total less the
            // amount allocated at VM start.
            let hotplug_size_bytes = max_mem_bytes
                .checked_sub(mem_bytes)
                .ok_or(())
                .map_err(|_| MemoryConfigError::BadMemSizeForHotplug(max_mem_bytes))?;

            let aligned_hotplug_size_bytes =
                checked_next_multiple_of(hotplug_size_bytes, PMEM_ALIGN_BYTES)
                    .ok_or(())
                    .map_err(|_| MemoryConfigError::BadPmemAlign(hotplug_size_bytes))?;

            Some(aligned_hotplug_size_bytes)
        };

        let cfg = MemoryConfig {
            size: mem_bytes,

            // Required
            shared: true,

            hotplug_size,

            ..Default::default()
        };

        Ok(cfg)
    }
}

// Return the next multiple of 'multiple' starting from the specified value
// (aka align value to multiple).
//
// This is a temporary solution until checked_next_multiple_of() integer
// method is available in the rust language.
//
// See: https://github.com/rust-lang/rust/issues/88581
fn checked_next_multiple_of(value: u64, multiple: u64) -> Option<u64> {
    match value.checked_rem(multiple) {
        None => Some(value),
        Some(r) => value.checked_add(multiple - r),
    }
}

impl TryFrom<CpuInfo> for CpusConfig {
    type Error = CpusConfigError;

    fn try_from(cpu: CpuInfo) -> Result<Self, Self::Error> {
        let boot_vcpus =
            u8::try_from(cpu.default_vcpus).map_err(CpusConfigError::BootVCPUsTooBig)?;

        let max_vcpus =
            u8::try_from(cpu.default_maxvcpus).map_err(CpusConfigError::MaxVCPUsTooBig)?;

        let topology = CpuTopology {
            cores_per_die: max_vcpus,
            threads_per_core: 1,
            dies_per_package: 1,
            packages: 1,
        };

        let max_phys_bits = DEFAULT_CH_MAX_PHYS_BITS;

        let features = CpuFeatures::from(cpu.cpu_features);

        let cfg = CpusConfig {
            boot_vcpus,
            max_vcpus,
            max_phys_bits,
            topology: Some(topology),
            features,

            ..Default::default()
        };

        Ok(cfg)
    }
}

impl From<String> for CpuFeatures {
    #[cfg(target_arch = "x86_64")]
    fn from(s: String) -> Self {
        let amx = s.split(',').any(|x| x == "amx");

        CpuFeatures { amx }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn from(_s: String) -> Self {
        CpuFeatures::default()
    }
}

// - The 2nd tuple element is the space separated final kernel parameters list.
//   It is made up of both the CH specific kernel parameters and the user
//   specified parameters from BootInfo.
//
//   The kernel params cannot be created only from BootInfo since that contains
//   the user-specified kernel parameters only.
//
// - The 3rd tuple element determines if TDX is enabled.
//
impl TryFrom<(BootInfo, Option<String>, bool)> for PayloadConfig {
    type Error = PayloadConfigError;

    fn try_from(args: (BootInfo, Option<String>, bool)) -> Result<Self, Self::Error> {
        let boot_info = args.0;
        let cmdline = args.1;
        let tdx_enabled = args.2;

        // The kernel is always specified here,
        // not in the top level VmConfig.kernel.
        let kernel = if boot_info.kernel.is_empty() {
            return Err(PayloadConfigError::NoKernel);
        } else {
            PathBuf::from(boot_info.kernel)
        };

        let initramfs = if boot_info.initrd.is_empty() {
            None
        } else {
            Some(PathBuf::from(boot_info.initrd))
        };

        let firmware = if tdx_enabled {
            if boot_info.firmware.is_empty() {
                return Err(PayloadConfigError::TDXFirmwareMissing);
            } else {
                Some(PathBuf::from(boot_info.firmware))
            }
        } else if boot_info.firmware.is_empty() {
            None
        } else {
            Some(PathBuf::from(boot_info.firmware))
        };

        let payload = PayloadConfig {
            kernel: Some(kernel),
            initramfs,
            cmdline,
            firmware,
        };

        Ok(payload)
    }
}

impl TryFrom<BootInfo> for DiskConfig {
    type Error = DiskConfigError;

    fn try_from(boot_info: BootInfo) -> Result<Self, Self::Error> {
        let path = if boot_info.image.is_empty() {
            return Err(DiskConfigError::MissingPath);
        } else {
            PathBuf::from(boot_info.image)
        };

        let disk = DiskConfig {
            path: Some(path),
            readonly: true,

            ..Default::default()
        };

        Ok(disk)
    }
}

impl From<MachineInfo> for RngConfig {
    fn from(m: MachineInfo) -> Self {
        let entropy_source = if !m.entropy_source.is_empty() {
            m.entropy_source
        } else {
            DEFAULT_CH_ENTROPY_SOURCE.to_string()
        };

        RngConfig {
            src: PathBuf::from(entropy_source),

            ..Default::default()
        }
    }
}

impl TryFrom<&BootInfo> for PmemConfig {
    type Error = PmemConfigError;

    fn try_from(b: &BootInfo) -> Result<Self, Self::Error> {
        let file = if b.image.is_empty() {
            return Err(PmemConfigError::MissingImage);
        } else {
            b.image.clone()
        };

        let cfg = PmemConfig {
            file: PathBuf::from(file),
            discard_writes: true,

            ..Default::default()
        };

        Ok(cfg)
    }
}

fn get_serial_cfg(debug: bool, confidential_guest: bool) -> ConsoleConfig {
    let mode = if confidential_guest {
        ConsoleOutputMode::Off
    } else if debug {
        ConsoleOutputMode::Tty
    } else {
        ConsoleOutputMode::Off
    };

    ConsoleConfig {
        file: None,
        mode,
        iommu: false,
    }
}

fn get_console_cfg(debug: bool, confidential_guest: bool) -> ConsoleConfig {
    let mode = if confidential_guest {
        if debug {
            ConsoleOutputMode::Tty
        } else {
            ConsoleOutputMode::Off
        }
    } else {
        ConsoleOutputMode::Off
    };

    ConsoleConfig {
        file: None,
        mode,
        iommu: false,
    }
}

fn get_platform_cfg(tdx_enabled: bool) -> Option<PlatformConfig> {
    if tdx_enabled {
        let platform = PlatformConfig {
            tdx: true,

            ..Default::default()
        };

        Some(platform)
    } else {
        None
    }
}

#[allow(dead_code)]
fn parse_mac<S>(s: &S) -> Result<MacAddr>
where
    S: AsRef<str> + ?Sized + Display,
{
    let v: Vec<&str> = s.as_ref().split(':').collect();
    let mut bytes = [0u8; MAC_ADDR_LEN];

    if v.len() != MAC_ADDR_LEN {
        return Err(anyhow!(
            "invalid MAC {} (length {}, expected {})",
            s,
            v.len(),
            MAC_ADDR_LEN
        ));
    }

    for i in 0..MAC_ADDR_LEN {
        if v[i].len() != 2 {
            return Err(anyhow!(
                "invalid MAC {} (segment {} length {}, expected {})",
                s,
                i,
                v.len(),
                2
            ));
        }

        bytes[i] =
            u8::from_str_radix(v[i], 16).context(format!("failed to parse MAC address: {}", s))?;
    }

    Ok(MacAddr { bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_types::config::hypervisor::{Hypervisor as HypervisorConfig, SecurityInfo};

    // Generate a valid generic memory info object and a valid CH specific
    // memory config object.
    fn make_memory_objects(
        default_memory_mib: u32,
        usable_max_mem_bytes: u64,
        confidential_guest: bool,
    ) -> (MemoryInfo, MemoryConfig) {
        let mem_info = MemoryInfo {
            default_memory: default_memory_mib,

            ..Default::default()
        };

        let hotplug_size = if confidential_guest {
            None
        } else {
            checked_next_multiple_of(
                usable_max_mem_bytes - (default_memory_mib as u64 * MIB),
                PMEM_ALIGN_BYTES,
            )
        };

        let mem_cfg = MemoryConfig {
            size: default_memory_mib as u64 * MIB,
            shared: true,
            hotplug_size,

            ..Default::default()
        };

        (mem_info, mem_cfg)
    }

    // The "default" sent to CH but without "cores_per_die"
    // to allow the tests to set that value explicitly.
    fn make_bare_topology() -> CpuTopology {
        CpuTopology {
            threads_per_core: 1,
            dies_per_package: 1,
            packages: 1,

            ..Default::default()
        }
    }

    fn make_cpu_objects(cpu_default: u8, cpu_max: u8) -> (CpuInfo, CpusConfig) {
        let cpu_info = CpuInfo {
            default_vcpus: cpu_default as i32,
            default_maxvcpus: cpu_max as u32,

            ..Default::default()
        };

        let cpus_config = CpusConfig {
            boot_vcpus: cpu_default,
            max_vcpus: cpu_max,
            topology: Some(CpuTopology {
                cores_per_die: cpu_max,

                ..make_bare_topology()
            }),
            max_phys_bits: DEFAULT_CH_MAX_PHYS_BITS,

            ..Default::default()
        };

        (cpu_info, cpus_config)
    }

    fn make_bootinfo_pmemconfig_objects(image: &str) -> (BootInfo, PmemConfig) {
        let boot_info = BootInfo {
            image: image.to_string(),

            ..Default::default()
        };

        let pmem_config = PmemConfig {
            file: PathBuf::from(image),
            discard_writes: true,

            ..Default::default()
        };

        (boot_info, pmem_config)
    }

    fn make_bootinfo_diskconfig_objects(path: &str) -> (BootInfo, DiskConfig) {
        let boot_info = BootInfo {
            image: path.to_string(),

            ..Default::default()
        };

        let disk_config = DiskConfig {
            path: Some(PathBuf::from(path)),
            readonly: true,

            ..Default::default()
        };

        (boot_info, disk_config)
    }

    // Create BootInfo and PayloadConfig objects for non-TDX scenarios.
    fn make_bootinfo_payloadconfig_objects(
        kernel: &str,
        initramfs: &str,
        firmware: Option<&str>,
        cmdline: Option<String>,
    ) -> (BootInfo, PayloadConfig) {
        let boot_info = if let Some(firmware) = firmware {
            BootInfo {
                kernel: kernel.into(),
                initrd: initramfs.into(),
                firmware: firmware.into(),

                ..Default::default()
            }
        } else {
            BootInfo {
                kernel: kernel.into(),
                initrd: initramfs.into(),

                ..Default::default()
            }
        };

        let payload_firmware = firmware.map(PathBuf::from);

        let payload_config = PayloadConfig {
            kernel: Some(PathBuf::from(kernel)),
            initramfs: Some(PathBuf::from(initramfs)),
            firmware: payload_firmware,
            cmdline,
        };

        (boot_info, payload_config)
    }

    fn make_machineinfo_rngconfig_objects(entropy_source: &str) -> (MachineInfo, RngConfig) {
        let machine_info = MachineInfo {
            entropy_source: entropy_source.to_string(),

            ..Default::default()
        };

        let rng_config = RngConfig {
            src: PathBuf::from(entropy_source.to_string()),

            ..Default::default()
        };

        (machine_info, rng_config)
    }

    #[test]
    fn test_get_serial_cfg() {
        #[derive(Debug)]
        struct TestData {
            debug: bool,
            confidential_guest: bool,
            result: ConsoleConfig,
        }

        let tests = &[
            TestData {
                debug: false,
                confidential_guest: false,
                result: ConsoleConfig {
                    file: None,
                    mode: ConsoleOutputMode::Off,
                    iommu: false,
                },
            },
            TestData {
                debug: true,
                confidential_guest: false,
                result: ConsoleConfig {
                    file: None,
                    mode: ConsoleOutputMode::Tty,
                    iommu: false,
                },
            },
            TestData {
                debug: false,
                confidential_guest: true,
                result: ConsoleConfig {
                    file: None,
                    mode: ConsoleOutputMode::Off,
                    iommu: false,
                },
            },
            TestData {
                debug: true,
                confidential_guest: true,
                result: ConsoleConfig {
                    file: None,
                    mode: ConsoleOutputMode::Off,
                    iommu: false,
                },
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = get_serial_cfg(d.debug, d.confidential_guest);

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            assert_eq!(result.file, d.result.file, "{}", msg);
            assert_eq!(result.iommu, d.result.iommu, "{}", msg);
            assert_eq!(result.mode, d.result.mode, "{}", msg);
        }
    }

    #[test]
    fn test_get_console_cfg() {
        #[derive(Debug)]
        struct TestData {
            debug: bool,
            confidential_guest: bool,
            result: ConsoleConfig,
        }

        let tests = &[
            TestData {
                debug: false,
                confidential_guest: false,
                result: ConsoleConfig {
                    file: None,
                    mode: ConsoleOutputMode::Off,
                    iommu: false,
                },
            },
            TestData {
                debug: true,
                confidential_guest: false,
                result: ConsoleConfig {
                    file: None,
                    mode: ConsoleOutputMode::Off,
                    iommu: false,
                },
            },
            TestData {
                debug: false,
                confidential_guest: true,
                result: ConsoleConfig {
                    file: None,
                    mode: ConsoleOutputMode::Off,
                    iommu: false,
                },
            },
            TestData {
                debug: true,
                confidential_guest: true,
                result: ConsoleConfig {
                    file: None,
                    mode: ConsoleOutputMode::Tty,
                    iommu: false,
                },
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = get_console_cfg(d.debug, d.confidential_guest);

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            assert_eq!(result, d.result, "{}", msg);
        }
    }

    #[test]
    fn test_get_platform_cfg() {
        #[derive(Debug)]
        struct TestData {
            tdx_enabled: bool,
            result: Option<PlatformConfig>,
        }

        let tests = &[
            TestData {
                tdx_enabled: false,
                result: None,
            },
            TestData {
                tdx_enabled: true,
                result: Some(PlatformConfig {
                    tdx: true,

                    ..Default::default()
                }),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = get_platform_cfg(d.tdx_enabled);

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            assert_eq!(result, d.result, "{}", msg);
        }
    }

    #[test]
    fn test_bootinfo_to_pmemconfig() {
        #[derive(Debug)]
        struct TestData {
            boot_info: BootInfo,
            result: Result<PmemConfig, PmemConfigError>,
        }

        let image = "/an/image";

        let (boot_info_with_image, pmem_config) = make_bootinfo_pmemconfig_objects(image);

        let tests = &[
            TestData {
                boot_info: BootInfo::default(),
                result: Err(PmemConfigError::MissingImage),
            },
            TestData {
                boot_info: boot_info_with_image,
                result: Ok(pmem_config),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = PmemConfig::try_from(&d.boot_info);

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            if d.result.is_err() {
                assert!(result.is_err(), "{}", msg);

                assert_eq!(
                    &result.unwrap_err(),
                    d.result.as_ref().unwrap_err(),
                    "{}",
                    msg
                );

                continue;
            }

            assert!(result.is_ok(), "{}", msg);
            assert_eq!(&result.unwrap(), d.result.as_ref().unwrap(), "{}", msg);
        }
    }

    #[test]
    fn test_machineinfo_to_rngconfig() {
        #[derive(Debug)]
        struct TestData {
            machine_info: MachineInfo,
            result: RngConfig,
        }

        let entropy_source = "/dev/foo";

        let (machine_info, rng_config) = make_machineinfo_rngconfig_objects(entropy_source);

        let tests = &[
            TestData {
                machine_info: MachineInfo::default(),
                result: RngConfig {
                    src: PathBuf::from(DEFAULT_CH_ENTROPY_SOURCE.to_string()),

                    ..Default::default()
                },
            },
            TestData {
                machine_info: MachineInfo {
                    entropy_source: DEFAULT_CH_ENTROPY_SOURCE.to_string(),

                    ..Default::default()
                },
                result: RngConfig {
                    src: PathBuf::from(DEFAULT_CH_ENTROPY_SOURCE.to_string()),

                    ..Default::default()
                },
            },
            TestData {
                machine_info: MachineInfo {
                    entropy_source: entropy_source.to_string(),

                    ..Default::default()
                },
                result: RngConfig {
                    src: PathBuf::from(entropy_source.to_string()),

                    ..Default::default()
                },
            },
            TestData {
                machine_info,
                result: rng_config,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = RngConfig::from(d.machine_info.clone());

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            assert_eq!(result, d.result, "{}", msg);
        }
    }

    #[test]
    fn test_string_to_cpufeatures() {
        #[derive(Debug)]
        struct TestData<'a> {
            s: &'a str,
            result: CpuFeatures,
        }

        let tests = &[
            TestData {
                s: "",
                result: CpuFeatures::default(),
            },
            #[cfg(target_arch = "x86_64")]
            TestData {
                s: "amx",
                result: CpuFeatures { amx: true },
            },
            #[cfg(target_arch = "x86_64")]
            TestData {
                s: "amxyz",
                result: CpuFeatures { amx: false },
            },
            #[cfg(target_arch = "x86_64")]
            TestData {
                s: "aamx",
                result: CpuFeatures { amx: false },
            },
            #[cfg(not(target_arch = "x86_64"))]
            TestData {
                s: "amx",
                result: CpuFeatures::default(),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = CpuFeatures::from(d.s.to_string());

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            assert_eq!(result, d.result, "{}", msg);
        }
    }

    #[test]
    fn test_bootinfo_to_diskconfig() {
        #[derive(Debug)]
        struct TestData {
            boot_info: BootInfo,
            result: Result<DiskConfig, DiskConfigError>,
        }

        let path = "/some/where";

        let (boot_info, disk_config) = make_bootinfo_diskconfig_objects(path);

        let tests = &[
            TestData {
                boot_info: BootInfo::default(),
                result: Err(DiskConfigError::MissingPath),
            },
            TestData {
                boot_info,
                result: Ok(disk_config),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = DiskConfig::try_from(d.boot_info.clone());

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            assert_eq!(result, d.result, "{}", msg);
        }
    }

    #[test]
    fn test_cpuinfo_to_cpusconfig() {
        #[derive(Debug)]
        struct TestData {
            cpu_info: CpuInfo,
            result: Result<CpusConfig, CpusConfigError>,
        }

        let topology = make_bare_topology();

        let u8_max = std::u8::MAX;

        let (cpu_info, cpus_config) = make_cpu_objects(7, u8_max);

        let tests = &[
            TestData {
                cpu_info: CpuInfo::default(),
                result: Ok(CpusConfig {
                    boot_vcpus: 0,
                    max_vcpus: 0,
                    topology: Some(CpuTopology {
                        cores_per_die: 0,

                        ..topology
                    }),
                    max_phys_bits: DEFAULT_CH_MAX_PHYS_BITS,

                    ..Default::default()
                }),
            },
            TestData {
                cpu_info: CpuInfo {
                    default_vcpus: u8_max as i32,

                    ..Default::default()
                },
                result: Ok(CpusConfig {
                    boot_vcpus: u8_max,
                    max_vcpus: 0,
                    topology: Some(topology.clone()),
                    max_phys_bits: DEFAULT_CH_MAX_PHYS_BITS,

                    ..Default::default()
                }),
            },
            TestData {
                cpu_info: CpuInfo {
                    default_vcpus: u8_max as i32 + 1,

                    ..Default::default()
                },
                result: Err(CpusConfigError::BootVCPUsTooBig(
                    u8::try_from(u8_max as i32 + 1).unwrap_err(),
                )),
            },
            TestData {
                cpu_info: CpuInfo {
                    default_maxvcpus: u8_max as u32 + 1,

                    ..Default::default()
                },
                result: Err(CpusConfigError::MaxVCPUsTooBig(
                    u8::try_from(u8_max as u32 + 1).unwrap_err(),
                )),
            },
            TestData {
                cpu_info: CpuInfo {
                    default_vcpus: u8_max as i32,
                    default_maxvcpus: u8_max as u32,

                    ..Default::default()
                },
                result: Ok(CpusConfig {
                    boot_vcpus: u8_max,
                    max_vcpus: u8_max,
                    topology: Some(CpuTopology {
                        cores_per_die: u8_max,

                        ..topology
                    }),
                    max_phys_bits: DEFAULT_CH_MAX_PHYS_BITS,

                    ..Default::default()
                }),
            },
            TestData {
                cpu_info: CpuInfo {
                    default_vcpus: (u8_max - 1) as i32,
                    default_maxvcpus: u8_max as u32,

                    ..Default::default()
                },
                result: Ok(CpusConfig {
                    boot_vcpus: (u8_max - 1),
                    max_vcpus: u8_max,
                    topology: Some(CpuTopology {
                        cores_per_die: u8_max,

                        ..topology
                    }),
                    max_phys_bits: DEFAULT_CH_MAX_PHYS_BITS,

                    ..Default::default()
                }),
            },
            TestData {
                cpu_info,
                result: Ok(cpus_config),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = CpusConfig::try_from(d.cpu_info.clone());

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            if d.result.is_err() {
                assert!(result.is_err(), "{}", msg);

                assert_eq!(
                    &result.unwrap_err(),
                    d.result.as_ref().unwrap_err(),
                    "{}",
                    msg
                );
                continue;
            }

            assert!(result.is_ok(), "{}", msg);
            assert_eq!(&result.unwrap(), d.result.as_ref().unwrap(), "{}", msg);
        }
    }

    #[test]
    fn test_bootinfo_to_payloadconfig() {
        #[derive(Debug)]
        struct TestData {
            boot_info: BootInfo,
            cmdline: Option<String>,
            tdx: bool,
            result: Result<PayloadConfig, PayloadConfigError>,
        }

        let cmdline = "debug foo a=b c=d";
        let kernel = "kernel";
        let firmware = "firmware";
        let initramfs = "initramfs";

        let (boot_info_with_initrd, payload_config_with_initrd) =
            make_bootinfo_payloadconfig_objects(
                kernel,
                initramfs,
                Some(firmware),
                Some(cmdline.to_string()),
            );

        let boot_info_without_initrd = BootInfo {
            kernel: kernel.into(),
            firmware: firmware.into(),

            ..Default::default()
        };

        let payload_config_without_initrd = PayloadConfig {
            kernel: Some(PathBuf::from(kernel)),
            firmware: Some(PathBuf::from(firmware)),
            cmdline: Some(cmdline.into()),

            ..Default::default()
        };

        let tests = &[
            TestData {
                boot_info: BootInfo::default(),
                cmdline: None,
                tdx: false,
                result: Err(PayloadConfigError::NoKernel),
            },
            TestData {
                boot_info: BootInfo {
                    kernel: kernel.into(),
                    kernel_params: String::new(),
                    initrd: initramfs.into(),

                    ..Default::default()
                },
                cmdline: None,
                tdx: false,
                result: Ok(PayloadConfig {
                    kernel: Some(PathBuf::from(kernel)),
                    cmdline: None,
                    initramfs: Some(PathBuf::from(initramfs)),

                    ..Default::default()
                }),
            },
            TestData {
                boot_info: BootInfo {
                    kernel: kernel.into(),
                    kernel_params: cmdline.to_string(),
                    initrd: initramfs.into(),

                    ..Default::default()
                },
                cmdline: Some(cmdline.to_string()),
                tdx: false,
                result: Ok(PayloadConfig {
                    kernel: Some(PathBuf::from(kernel)),
                    initramfs: Some(PathBuf::from(initramfs)),
                    cmdline: Some(cmdline.to_string()),

                    ..Default::default()
                }),
            },
            TestData {
                boot_info: BootInfo {
                    kernel: kernel.into(),
                    initrd: initramfs.into(),

                    ..Default::default()
                },
                cmdline: None,
                tdx: true,
                result: Err(PayloadConfigError::TDXFirmwareMissing),
            },
            TestData {
                boot_info: boot_info_with_initrd,
                cmdline: Some(cmdline.to_string()),
                tdx: true,
                result: Ok(payload_config_with_initrd),
            },
            TestData {
                boot_info: boot_info_without_initrd,
                cmdline: Some(cmdline.to_string()),
                tdx: true,
                result: Ok(payload_config_without_initrd),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = PayloadConfig::try_from((d.boot_info.clone(), d.cmdline.clone(), d.tdx));

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            if d.result.is_err() {
                assert!(result.is_err(), "{}", msg);

                assert_eq!(
                    &result.unwrap_err(),
                    d.result.as_ref().unwrap_err(),
                    "{}",
                    msg
                );
                continue;
            }

            assert!(result.is_ok(), "{}", msg);
            assert_eq!(&result.unwrap(), d.result.as_ref().unwrap(), "{}", msg);
        }
    }

    #[test]
    fn test_memoryinfo_to_memoryconfig() {
        #[derive(Debug)]
        struct TestData {
            mem_info: MemoryInfo,
            confidential_guest: bool,
            result: Result<MemoryConfig, MemoryConfigError>,
        }

        let sysinfo = nix::sys::sysinfo::sysinfo().unwrap();

        let actual_max_mem_bytes = sysinfo.ram_total();

        // Calculate the available MiB value
        let max_mem_mib = actual_max_mem_bytes.checked_div(MIB).unwrap();

        // Undo the operation to get back to the usable amount of max memory
        // bytes.
        let usable_max_mem_bytes = MIB.checked_mul(max_mem_mib).unwrap();

        let (mem_info_std, mem_cfg_std) = make_memory_objects(79, usable_max_mem_bytes, false);
        let (mem_info_confidential_guest, mem_cfg_confidential_guest) =
            make_memory_objects(79, usable_max_mem_bytes, true);

        let tests = &[
            TestData {
                mem_info: MemoryInfo::default(),
                confidential_guest: false,
                result: Err(MemoryConfigError::NoDefaultMemory),
            },
            TestData {
                mem_info: MemoryInfo {
                    default_memory: 17,

                    ..Default::default()
                },
                confidential_guest: true,
                result: Ok(MemoryConfig {
                    size: (17 * MIB),
                    shared: true,
                    hotplug_size: None,

                    ..Default::default()
                }),
            },
            TestData {
                mem_info: MemoryInfo {
                    default_memory: max_mem_mib as u32,

                    ..Default::default()
                },
                confidential_guest: true,
                result: Ok(MemoryConfig {
                    size: usable_max_mem_bytes,
                    shared: true,
                    hotplug_size: None,

                    ..Default::default()
                }),
            },
            TestData {
                mem_info: MemoryInfo {
                    default_memory: (max_mem_mib + 1) as u32,

                    ..Default::default()
                },
                confidential_guest: true,
                result: Err(MemoryConfigError::DefaultMemSizeTooBig),
            },
            TestData {
                mem_info: MemoryInfo {
                    default_memory: 1024,

                    ..Default::default()
                },
                confidential_guest: false,
                result: Ok(MemoryConfig {
                    size: 1024_u64 * MIB,
                    shared: true,
                    hotplug_size: checked_next_multiple_of(
                        usable_max_mem_bytes - (1024 * MIB),
                        PMEM_ALIGN_BYTES,
                    ),

                    ..Default::default()
                }),
            },
            TestData {
                mem_info: mem_info_std,
                confidential_guest: false,
                result: Ok(mem_cfg_std),
            },
            TestData {
                mem_info: mem_info_confidential_guest,
                confidential_guest: true,
                result: Ok(mem_cfg_confidential_guest),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = MemoryConfig::try_from((d.mem_info.clone(), d.confidential_guest));

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            if d.result.is_err() {
                assert!(result.is_err(), "{}", msg);

                assert_eq!(
                    &result.unwrap_err(),
                    d.result.as_ref().unwrap_err(),
                    "{}",
                    msg
                );
                continue;
            }

            assert!(result.is_ok(), "{}", msg);
            assert_eq!(&result.unwrap(), d.result.as_ref().unwrap(), "{}", msg);
        }
    }

    #[test]
    fn test_vsock_config() {
        #[derive(Debug)]
        struct TestData<'a> {
            vsock_socket_path: &'a str,
            cid: u64,
            result: Result<VsockConfig, VsockConfigError>,
        }

        let tests = &[
            TestData {
                vsock_socket_path: "",
                cid: 0,
                result: Err(VsockConfigError::NoVsockSocketPath),
            },
            TestData {
                vsock_socket_path: "vsock_socket_path",
                cid: DEFAULT_VSOCK_CID,
                result: Ok(VsockConfig {
                    socket: PathBuf::from("vsock_socket_path"),
                    cid: DEFAULT_VSOCK_CID,

                    ..Default::default()
                }),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = VsockConfig::try_from((d.vsock_socket_path.to_string(), d.cid));

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            if d.result.is_err() {
                assert!(result.is_err(), "{}", msg);

                assert_eq!(
                    &result.unwrap_err(),
                    d.result.as_ref().unwrap_err(),
                    "{}",
                    msg
                );
                continue;
            }

            assert!(result.is_ok(), "{}", msg);
            assert_eq!(&result.unwrap(), d.result.as_ref().unwrap(), "{}", msg);
        }
    }

    #[test]
    fn test_named_hypervisor_config_to_vmconfig() {
        #[derive(Debug)]
        struct TestData {
            cfg: NamedHypervisorConfig,
            result: Result<VmConfig, VmConfigError>,
        }

        let u8_max = std::u8::MAX;
        let sysinfo = nix::sys::sysinfo::sysinfo().unwrap();

        let actual_max_mem_bytes = sysinfo.ram_total();

        // Calculate the available MiB value
        let max_mem_mib = actual_max_mem_bytes.checked_div(MIB).unwrap();

        // Undo the operation to get back to the usable amount of max memory
        // bytes.
        let usable_max_mem_bytes = MIB.checked_mul(max_mem_mib).unwrap();

        let image = "image";
        let initramfs = "initramfs";
        let kernel = "kernel";
        let firmware = "firmware";

        let entropy_source = "entropy_source";
        let sandbox_path = "sandbox_path";
        let vsock_socket_path = "vsock_socket_path";

        let valid_vsock =
            VsockConfig::try_from((vsock_socket_path.to_string(), DEFAULT_VSOCK_CID)).unwrap();

        let (cpu_info, cpus_config) = make_cpu_objects(7, u8_max);

        let (memory_info_std, mem_config_std) =
            make_memory_objects(79, usable_max_mem_bytes, false);

        let (memory_info_confidential_guest, mem_config_confidential_guest) =
            make_memory_objects(79, usable_max_mem_bytes, true);

        let (_, pmem_config_with_image) = make_bootinfo_pmemconfig_objects(image);
        let (machine_info, rng_config) = make_machineinfo_rngconfig_objects(entropy_source);

        let payload_firmware = None;

        let (boot_info_with_initrd, payload_config_with_initrd) =
            make_bootinfo_payloadconfig_objects(kernel, initramfs, payload_firmware, None);

        let (boot_info_confidential_guest_image, disk_config_confidential_guest_image) =
            make_bootinfo_diskconfig_objects(image);

        let boot_info_confidential_guest_initrd = BootInfo {
            kernel: kernel.to_string(),
            initrd: initramfs.to_string(),

            ..Default::default()
        };

        let boot_info_tdx_image = BootInfo {
            kernel: kernel.to_string(),
            image: image.to_string(),
            firmware: firmware.to_string(),

            ..Default::default()
        };

        let boot_info_tdx_initrd = BootInfo {
            kernel: kernel.to_string(),
            initrd: initramfs.to_string(),
            firmware: firmware.to_string(),

            ..Default::default()
        };

        let payload_config_confidential_guest_initrd = PayloadConfig {
            kernel: Some(PathBuf::from(kernel)),
            initramfs: Some(PathBuf::from(initramfs)),

            ..Default::default()
        };

        // XXX: Note that the image is defined in a DiskConfig!
        let payload_config_tdx_for_image = PayloadConfig {
            firmware: Some(PathBuf::from(firmware)),
            kernel: Some(PathBuf::from(kernel)),

            ..Default::default()
        };

        let payload_config_tdx_initrd = PayloadConfig {
            firmware: Some(PathBuf::from(firmware)),
            initramfs: Some(PathBuf::from(initramfs)),
            kernel: Some(PathBuf::from(kernel)),

            ..Default::default()
        };

        //------------------------------

        let hypervisor_cfg_with_image_and_kernel = HypervisorConfig {
            cpu_info: cpu_info.clone(),
            memory_info: memory_info_std.clone(),
            boot_info: BootInfo {
                image: image.to_string(),
                kernel: kernel.to_string(),

                ..Default::default()
            },
            machine_info: machine_info.clone(),

            ..Default::default()
        };

        let hypervisor_cfg_with_initrd = HypervisorConfig {
            cpu_info: cpu_info.clone(),
            memory_info: memory_info_std,
            boot_info: boot_info_with_initrd,
            machine_info: machine_info.clone(),

            ..Default::default()
        };

        let security_info_confidential_guest = SecurityInfo {
            confidential_guest: true,

            ..Default::default()
        };

        let hypervisor_cfg_confidential_guest_image = HypervisorConfig {
            cpu_info: cpu_info.clone(),
            memory_info: memory_info_confidential_guest.clone(),
            boot_info: BootInfo {
                kernel: kernel.to_string(),

                ..boot_info_confidential_guest_image
            },
            machine_info: machine_info.clone(),
            security_info: security_info_confidential_guest.clone(),

            ..Default::default()
        };

        let hypervisor_cfg_confidential_guest_initrd = HypervisorConfig {
            cpu_info: cpu_info.clone(),
            memory_info: memory_info_confidential_guest.clone(),
            boot_info: boot_info_confidential_guest_initrd,
            machine_info: machine_info.clone(),
            security_info: security_info_confidential_guest.clone(),

            ..Default::default()
        };

        let hypervisor_cfg_tdx_image = HypervisorConfig {
            cpu_info: cpu_info.clone(),
            memory_info: memory_info_confidential_guest.clone(),
            boot_info: boot_info_tdx_image,
            machine_info: machine_info.clone(),
            security_info: security_info_confidential_guest.clone(),

            ..Default::default()
        };

        let hypervisor_cfg_tdx_initrd = HypervisorConfig {
            cpu_info,
            memory_info: memory_info_confidential_guest,
            boot_info: boot_info_tdx_initrd,
            machine_info,
            security_info: security_info_confidential_guest,

            ..Default::default()
        };

        //------------------------------

        let vmconfig_with_image_and_kernel = VmConfig {
            cpus: cpus_config.clone(),
            memory: mem_config_std.clone(),
            rng: rng_config.clone(),
            vsock: Some(valid_vsock.clone()),

            // rootfs image specific
            pmem: Some(vec![pmem_config_with_image]),

            payload: Some(PayloadConfig {
                kernel: Some(PathBuf::from(kernel)),

                ..Default::default()
            }),

            ..Default::default()
        };

        let vmconfig_with_initrd = VmConfig {
            cpus: cpus_config.clone(),
            memory: mem_config_std,
            rng: rng_config.clone(),
            vsock: Some(valid_vsock.clone()),

            // initrd/initramfs specific
            payload: Some(payload_config_with_initrd),

            ..Default::default()
        };

        let vmconfig_confidential_guest_image = VmConfig {
            cpus: cpus_config.clone(),
            memory: mem_config_confidential_guest.clone(),
            rng: rng_config.clone(),
            vsock: Some(valid_vsock.clone()),

            // Confidential guest image specific
            disks: Some(vec![disk_config_confidential_guest_image.clone()]),

            payload: Some(PayloadConfig {
                kernel: Some(PathBuf::from(kernel)),

                ..Default::default()
            }),

            ..Default::default()
        };

        let vmconfig_confidential_guest_initrd = VmConfig {
            cpus: cpus_config.clone(),
            memory: mem_config_confidential_guest.clone(),
            rng: rng_config.clone(),
            vsock: Some(valid_vsock.clone()),

            // Confidential guest initrd specific
            payload: Some(payload_config_confidential_guest_initrd),

            ..Default::default()
        };

        let platform_config_tdx = get_platform_cfg(true);

        let vmconfig_tdx_image = VmConfig {
            cpus: cpus_config.clone(),
            memory: mem_config_confidential_guest.clone(),
            rng: rng_config.clone(),
            vsock: Some(valid_vsock.clone()),
            platform: platform_config_tdx.clone(),

            // TDX specific
            payload: Some(payload_config_tdx_for_image),

            // Confidential guest + TDX specific
            disks: Some(vec![disk_config_confidential_guest_image]),

            ..Default::default()
        };

        let vmconfig_tdx_initrd = VmConfig {
            cpus: cpus_config,
            memory: mem_config_confidential_guest,
            rng: rng_config,
            vsock: Some(valid_vsock),
            platform: platform_config_tdx,

            // Confidential guest + TDX specific
            payload: Some(payload_config_tdx_initrd),

            ..Default::default()
        };

        //------------------------------

        let named_hypervisor_cfg_with_image_and_kernel = NamedHypervisorConfig {
            sandbox_path: sandbox_path.into(),
            vsock_socket_path: vsock_socket_path.into(),

            cfg: hypervisor_cfg_with_image_and_kernel,

            ..Default::default()
        };

        let named_hypervisor_cfg_with_initrd = NamedHypervisorConfig {
            sandbox_path: sandbox_path.into(),
            vsock_socket_path: vsock_socket_path.into(),

            cfg: hypervisor_cfg_with_initrd,

            ..Default::default()
        };

        let named_hypervisor_cfg_confidential_guest_image = NamedHypervisorConfig {
            sandbox_path: sandbox_path.into(),
            vsock_socket_path: vsock_socket_path.into(),

            cfg: hypervisor_cfg_confidential_guest_image,

            ..Default::default()
        };

        let named_hypervisor_cfg_confidential_guest_initrd = NamedHypervisorConfig {
            sandbox_path: sandbox_path.into(),
            vsock_socket_path: vsock_socket_path.into(),

            cfg: hypervisor_cfg_confidential_guest_initrd,

            ..Default::default()
        };

        let named_hypervisor_cfg_tdx_image = NamedHypervisorConfig {
            sandbox_path: sandbox_path.into(),
            vsock_socket_path: vsock_socket_path.into(),

            cfg: hypervisor_cfg_tdx_image,

            tdx_enabled: true,

            ..Default::default()
        };

        let named_hypervisor_cfg_tdx_initrd = NamedHypervisorConfig {
            sandbox_path: sandbox_path.into(),
            vsock_socket_path: vsock_socket_path.into(),

            cfg: hypervisor_cfg_tdx_initrd,

            tdx_enabled: true,

            ..Default::default()
        };

        //------------------------------

        let tests = &[
            TestData {
                cfg: NamedHypervisorConfig::default(),
                result: Err(VmConfigError::EmptyVsockSocketPath),
            },
            TestData {
                cfg: NamedHypervisorConfig {
                    vsock_socket_path: "vsock_socket_path".into(),

                    ..Default::default()
                },
                result: Err(VmConfigError::EmptySandboxPath),
            },
            TestData {
                cfg: NamedHypervisorConfig {
                    sandbox_path: "sandbox_path".into(),

                    ..Default::default()
                },
                result: Err(VmConfigError::EmptyVsockSocketPath),
            },
            TestData {
                cfg: NamedHypervisorConfig {
                    sandbox_path: "sandbox_path".into(),
                    vsock_socket_path: "vsock_socket_path".into(),
                    cfg: HypervisorConfig::default(),

                    ..Default::default()
                },
                result: Err(VmConfigError::NoBootFile),
            },
            TestData {
                cfg: NamedHypervisorConfig {
                    sandbox_path: "sandbox_path".into(),
                    vsock_socket_path: "vsock_socket_path".into(),
                    cfg: HypervisorConfig {
                        boot_info: BootInfo {
                            initrd: "initrd".into(),
                            image: "image".into(),

                            ..Default::default()
                        },

                        ..Default::default()
                    },

                    ..Default::default()
                },
                result: Err(VmConfigError::MultipleBootFiles),
            },
            TestData {
                cfg: named_hypervisor_cfg_with_image_and_kernel,
                result: Ok(vmconfig_with_image_and_kernel),
            },
            TestData {
                cfg: named_hypervisor_cfg_with_initrd,
                result: Ok(vmconfig_with_initrd),
            },
            TestData {
                cfg: named_hypervisor_cfg_confidential_guest_image,
                result: Ok(vmconfig_confidential_guest_image),
            },
            TestData {
                cfg: named_hypervisor_cfg_confidential_guest_initrd,
                result: Ok(vmconfig_confidential_guest_initrd),
            },
            TestData {
                cfg: named_hypervisor_cfg_tdx_image,
                result: Ok(vmconfig_tdx_image),
            },
            TestData {
                cfg: named_hypervisor_cfg_tdx_initrd,
                result: Ok(vmconfig_tdx_initrd),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = VmConfig::try_from(d.cfg.clone());

            let msg = format!("{}: actual result: {:?}", msg, result);

            if std::env::var("DEBUG").is_ok() {
                eprintln!("DEBUG: {}", msg);
            }

            if d.result.is_err() {
                assert!(result.is_err(), "{}", msg);

                assert_eq!(
                    &result.unwrap_err(),
                    d.result.as_ref().unwrap_err(),
                    "{}",
                    msg
                );
                continue;
            }

            assert!(result.is_ok(), "{}", msg);
            assert_eq!(&result.unwrap(), d.result.as_ref().unwrap(), "{}", msg);
        }
    }
}
