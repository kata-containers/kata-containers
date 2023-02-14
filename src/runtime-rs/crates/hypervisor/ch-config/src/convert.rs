// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::net_util::MAC_ADDR_LEN;
use crate::NamedHypervisorConfig;
use crate::VmConfig;
use crate::{
    ConsoleConfig, ConsoleOutputMode, CpuFeatures, CpuTopology, CpusConfig, MacAddr, MemoryConfig,
    PayloadConfig, RngConfig, VsockConfig,
};
use anyhow::{anyhow, Context, Result};
use kata_types::config::default::DEFAULT_CH_ENTROPY_SOURCE;
use kata_types::config::hypervisor::{CpuInfo, MachineInfo, MemoryInfo};
use kata_types::config::BootInfo;
use std::convert::TryFrom;
use std::fmt::Display;
use std::path::PathBuf;

// 1 MiB
const MIB: u64 = 1024 * 1024;

const DEFAULT_CH_MAX_PHYS_BITS: u8 = 46;

impl TryFrom<NamedHypervisorConfig> for VmConfig {
    type Error = anyhow::Error;

    fn try_from(n: NamedHypervisorConfig) -> Result<Self, Self::Error> {
        let kernel_params = n.kernel_params;
        let cfg = n.cfg;
        let vsock_socket_path = n.vsock_socket_path;
        let sandbox_path = n.sandbox_path;
        let fs = n.shared_fs_devices;
        let pmem = n.pmem_devices;

        let cpus = CpusConfig::try_from(cfg.cpu_info)?;

        let rng = RngConfig::try_from(cfg.machine_info)?;

        // Note that PmemConfig replaces the PayloadConfig.initrd.
        let payload = PayloadConfig::try_from((cfg.boot_info, kernel_params))?;

        let serial = get_serial_cfg()?;
        let console = get_console_cfg()?;

        let memory = MemoryConfig::try_from(cfg.memory_info)?;

        std::fs::create_dir_all(sandbox_path).context("failed to create sandbox path")?;

        let vsock = VsockConfig {
            cid: 3,
            socket: PathBuf::from(vsock_socket_path),
            ..Default::default()
        };

        let cfg = VmConfig {
            cpus,
            memory,
            serial,
            console,
            payload: Some(payload),
            fs,
            pmem,
            vsock: Some(vsock),
            rng,
            ..Default::default()
        };

        Ok(cfg)
    }
}

impl TryFrom<MemoryInfo> for MemoryConfig {
    type Error = anyhow::Error;

    fn try_from(mem: MemoryInfo) -> Result<Self, Self::Error> {
        let sysinfo = nix::sys::sysinfo::sysinfo()?;

        let max_mem_bytes = sysinfo.ram_total();

        let mem_bytes: u64 = MIB
            .checked_mul(mem.default_memory as u64)
            .ok_or("cannot convert default memory to bytes")
            .map_err(|e| anyhow!(e))?;

        // The amount of memory that can be hot-plugged is the total less the
        // amount allocated at VM start.
        let hotplug_size_bytes = max_mem_bytes
            .checked_sub(mem_bytes)
            .ok_or("failed to calculate max hotplug size for CH")
            .map_err(|e| anyhow!(e))?;

        let cfg = MemoryConfig {
            size: mem_bytes,

            // Required
            shared: true,

            hotplug_size: Some(hotplug_size_bytes),

            ..Default::default()
        };

        Ok(cfg)
    }
}

impl TryFrom<CpuInfo> for CpusConfig {
    type Error = anyhow::Error;

    fn try_from(cpu: CpuInfo) -> Result<Self, Self::Error> {
        let boot_vcpus = u8::try_from(cpu.default_vcpus)?;
        let max_vcpus = u8::try_from(cpu.default_maxvcpus)?;

        let topology = CpuTopology {
            threads_per_core: 1,
            cores_per_die: max_vcpus,
            dies_per_package: 1,
            packages: 1,
        };

        let max_phys_bits = DEFAULT_CH_MAX_PHYS_BITS;

        let cfg = CpusConfig {
            boot_vcpus,
            max_vcpus,
            max_phys_bits,
            topology: Some(topology),

            ..Default::default()
        };

        Ok(cfg)
    }
}

impl TryFrom<String> for CpuFeatures {
    type Error = anyhow::Error;

    #[cfg(target_arch = "x86_64")]
    fn try_from(s: String) -> Result<Self, Self::Error> {
        let amx = s.split(',').any(|x| x == "amx");

        let cpu_features = CpuFeatures { amx };

        Ok(cpu_features)
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn try_from(_s: String) -> Result<Self, Self::Error> {
        Ok(CpuFeatures::default())
    }
}

// The 2nd tuple element is the space separated kernel parameters list.
// This cannot be created only from BootInfo since that contains the
// user-specified kernel parameters only.
impl TryFrom<(BootInfo, String)> for PayloadConfig {
    type Error = anyhow::Error;

    fn try_from(args: (BootInfo, String)) -> Result<Self, Self::Error> {
        let b = args.0;
        let cmdline = args.1;

        let kernel = PathBuf::from(b.kernel);

        let payload = PayloadConfig {
            kernel: Some(kernel),
            cmdline: Some(cmdline),

            ..Default::default()
        };

        Ok(payload)
    }
}

impl TryFrom<MachineInfo> for RngConfig {
    type Error = anyhow::Error;

    fn try_from(m: MachineInfo) -> Result<Self, Self::Error> {
        let entropy_source = if !m.entropy_source.is_empty() {
            m.entropy_source
        } else {
            DEFAULT_CH_ENTROPY_SOURCE.to_string()
        };

        let rng = RngConfig {
            src: PathBuf::from(entropy_source),

            ..Default::default()
        };

        Ok(rng)
    }
}

fn get_serial_cfg() -> Result<ConsoleConfig> {
    let cfg = ConsoleConfig {
        file: None,
        mode: ConsoleOutputMode::Tty,
        iommu: false,
    };

    Ok(cfg)
}

fn get_console_cfg() -> Result<ConsoleConfig> {
    let cfg = ConsoleConfig {
        file: None,
        mode: ConsoleOutputMode::Off,
        iommu: false,
    };

    Ok(cfg)
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
