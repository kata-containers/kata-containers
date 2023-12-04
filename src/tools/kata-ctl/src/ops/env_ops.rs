// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Contains checks that are not architecture-specific

use crate::arch::arch_specific;
use crate::args::EnvArgument;
use crate::ops::version;
use crate::utils;
use kata_sys_util::protection;
use kata_types::config::TomlConfig;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::process::Command;
use sys_info;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HostInfo {
    #[serde(default)]
    available_guest_protection: String,
    #[serde(default)]
    kernel: String,
    #[serde(default)]
    architecture: String,
    #[serde(default)]
    vm_container_capable: bool,
    #[serde(default)]
    support_vsocks: bool,
    #[serde(default)]
    distro: DistroInfo,
    #[serde(default)]
    cpu: CPUInfo,
    #[serde(default)]
    memory: MemoryInfo,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DistroInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct CPUInfo {
    #[serde(default)]
    vendor: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    cpus: usize,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct MemoryInfo {
    #[serde(default)]
    total: u64,
    #[serde(default)]
    available: u64,
    #[serde(default)]
    free: u64,
}

// Semantic version for the output of the command.
//
// XXX: Increment for every change to the output format
// (meaning any change to the EnvInfo type).
const FORMAT_VERSION: &str = "0.0.1-kata-ctl";

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct MetaInfo {
    #[serde(default)]
    version: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct VersionInfo {
    #[serde(default)]
    semver: String,
    #[serde(default)]
    commit: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct RuntimeConfigInfo {
    #[serde(default)]
    path: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct RuntimeInfo {
    #[serde(default)]
    path: String,
    #[serde(default)]
    guest_selinux_label: String,
    #[serde(default)]
    pub experimental: Vec<String>,
    #[serde(default)]
    debug: bool,
    #[serde(default)]
    trace: bool,
    #[serde(default)]
    disable_guest_seccomp: bool,
    #[serde(default)]
    disable_new_net_ns: bool,
    #[serde(default)]
    sandbox_cgroup_only: bool,
    #[serde(default)]
    static_sandbox_resource_mgmt: bool,
    #[serde(default)]
    config: RuntimeConfigInfo,
    #[serde(default)]
    version: VersionInfo,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AgentInfo {
    #[serde(default)]
    debug: bool,
    #[serde(default)]
    trace: bool,
}
// KernelInfo stores kernel details
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct KernelInfo {
    #[serde(default)]
    path: String,
    #[serde(default)]
    parameters: String,
}

// InitrdInfo stores initrd image details
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct InitrdInfo {
    #[serde(default)]
    path: String,
}

// ImageInfo stores root filesystem image details
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ImageInfo {
    #[serde(default)]
    path: String,
}

// SecurityInfo stores the hypervisor security details
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SecurityInfo {
    #[serde(default)]
    rootless: bool,
    #[serde(default)]
    disable_seccomp: bool,
    #[serde(default)]
    guest_hook_path: String,
    #[serde(default)]
    enable_annotations: Vec<String>,
    #[serde(default)]
    confidential_guest: bool,
}

// HypervisorInfo stores hypervisor details
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HypervisorInfo {
    #[serde(default)]
    machine_type: String,
    #[serde(default)]
    machine_accelerators: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    block_device_driver: String,
    #[serde(default)]
    entropy_source: String,
    #[serde(default)]
    shared_fs: String,
    #[serde(default)]
    virtio_fs_daemon: String,
    #[serde(default)]
    msize_9p: u32,
    #[serde(default)]
    memory_slots: u32,
    #[serde(default)]
    pcie_root_port: u32,
    #[serde(default)]
    hotplug_vfio_on_rootbus: bool,
    #[serde(default)]
    debug: bool,
    #[serde(default)]
    enable_iommu: bool,
    #[serde(default)]
    enable_iommu_platform: bool,
    #[serde(default)]
    default_vcpus: i32,
    #[serde(default)]
    cpu_features: String,
    #[serde(default)]
    security_info: SecurityInfo,
}

// EnvInfo collects all information that will be displayed by the
// env command.
//
// XXX: Any changes must be coupled with a change to formatVersion.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct EnvInfo {
    #[serde(default)]
    kernel: KernelInfo,
    #[serde(default)]
    meta: MetaInfo,
    #[serde(default)]
    image: ImageInfo,
    #[serde(default)]
    initrd: InitrdInfo,
    #[serde(default)]
    hypervisor: HypervisorInfo,
    #[serde(default)]
    runtime: RuntimeInfo,
    #[serde(default)]
    host: HostInfo,
    #[serde(default)]
    agent: AgentInfo,
}

pub fn get_meta_info() -> MetaInfo {
    MetaInfo {
        version: String::from(FORMAT_VERSION),
    }
}

pub fn get_memory_info() -> Result<MemoryInfo> {
    let mem_info = sys_info::mem_info().context("get host memory information")?;
    Ok(MemoryInfo {
        total: mem_info.total,
        available: mem_info.avail,
        free: mem_info.free,
    })
}

fn get_host_info() -> Result<HostInfo> {
    let host_kernel_version = utils::get_kernel_version(utils::PROC_VERSION_FILE)?;
    let (host_distro_name, host_distro_version) =
        utils::get_distro_details(utils::OS_RELEASE, utils::OS_RELEASE_CLR)?;
    let (cpu_vendor, cpu_model) = arch_specific::get_cpu_details()?;

    let host_distro = DistroInfo {
        name: host_distro_name,
        version: host_distro_version,
    };

    let cores: usize = std::thread::available_parallelism()
        .context("get available parallelism")?
        .into();

    let host_cpu = CPUInfo {
        vendor: cpu_vendor,
        model: cpu_model,
        cpus: cores,
    };

    let memory_info = get_memory_info()?;

    let guest_protection =
        protection::available_guest_protection().map_err(|e| anyhow!(e.to_string()))?;

    let guest_protection = guest_protection.to_string();

    let mut vm_container_capable = true;

    if arch_specific::host_is_vmcontainer_capable().is_err() {
        vm_container_capable = false;
    }

    let support_vsocks = utils::supports_vsocks(utils::VHOST_VSOCK_DEVICE)?;

    Ok(HostInfo {
        kernel: host_kernel_version,
        architecture: String::from(std::env::consts::ARCH),
        distro: host_distro,
        cpu: host_cpu,
        memory: memory_info,
        available_guest_protection: guest_protection,
        vm_container_capable,
        support_vsocks,
    })
}

pub fn get_runtime_info(toml_config: &TomlConfig) -> Result<RuntimeInfo> {
    let version = VersionInfo {
        semver: String::from(version::VERSION),
        commit: String::from(version::COMMIT),
    };

    let config_path = TomlConfig::get_default_config_file();
    let mut toml_path = String::new();
    if config_path.is_ok() {
        let p = config_path?;
        let path_str = p.to_str();
        toml_path = match path_str {
            Some(s) => String::from(s),
            None => String::new(),
        };
    }

    Ok(RuntimeInfo {
        // TODO: Needs to be implemented: https://github.com/kata-containers/kata-containers/issues/6518
        path: String::from("not implemented yet. See: https://github.com/kata-containers/kata-containers/issues/6518"),
        version,
        experimental: toml_config.runtime.experimental.clone(),
        // TODO: See https://github.com/kata-containers/kata-containers/issues/6667
        guest_selinux_label: String::from("not implemented yet: See https://github.com/kata-containers/kata-containers/issues/6667"),
        debug: toml_config.runtime.debug,
        trace: toml_config.runtime.enable_tracing,
        disable_guest_seccomp: toml_config.runtime.disable_guest_seccomp,
        disable_new_net_ns: toml_config.runtime.disable_new_netns,
        sandbox_cgroup_only: toml_config.runtime.sandbox_cgroup_only,
        static_sandbox_resource_mgmt: toml_config.runtime.static_sandbox_resource_mgmt,
        config: RuntimeConfigInfo { path: toml_path },
    })
}

pub fn get_agent_info(toml_config: &TomlConfig) -> Result<AgentInfo> {
    // Assign the first entry to the agent config, to make this
    // work for configs where agent_name is absent.
    // This is a workaround for https://github.com/kata-containers/kata-containers/issues/5954
    let key_val = toml_config.agent.iter().next();
    let mut agent_config = match key_val {
        Some(x) => Ok(x.1),
        None => Err(anyhow!("Missing agent config")),
    }?;

    // If the agent_name config is present, use that
    if !&toml_config.runtime.agent_name.is_empty() {
        agent_config = toml_config
            .agent
            .get(&toml_config.runtime.agent_name)
            .ok_or("could not find agent config in configuration")
            .map_err(|e| anyhow!(e))?;
    }

    Ok(AgentInfo {
        debug: agent_config.debug,
        trace: agent_config.enable_tracing,
    })
}

pub fn get_command_version(cmd: &str) -> Result<String> {
    // Path is empty in case of dragonball hypervisor
    if cmd.is_empty() {
        return Ok("unknown".to_string());
    }
    let output = Command::new(cmd)
        .arg("--version")
        .output()
        .map_err(|e| anyhow!(e))?;

    let version = String::from_utf8(output.stdout).map_err(|e| anyhow!(e))?;

    Ok(version)
}

pub fn get_hypervisor_info(
    toml_config: &TomlConfig,
) -> Result<(HypervisorInfo, ImageInfo, KernelInfo, InitrdInfo)> {
    // Assign the first entry in the hashmap to the hypervisor config, to make this
    // work for configs where hypervisor_name is absent.
    // This is a workaround for https://github.com/kata-containers/kata-containers/issues/5954
    let key_val = toml_config.hypervisor.iter().next();
    let mut hypervisor_config = match key_val {
        Some(x) => Ok(x.1),
        None => Err(anyhow!("Missing hypervisor config")),
    }?;

    // If hypervisor_name config is present, use that
    if !&toml_config.runtime.hypervisor_name.is_empty() {
        hypervisor_config = toml_config
            .hypervisor
            .get(&toml_config.runtime.hypervisor_name)
            .ok_or("could not find hypervisor config in configuration")
            .map_err(|e| anyhow!(e))?;
    }

    let version =
        get_command_version(&hypervisor_config.path).context("error getting hypervisor version")?;

    let security_info = SecurityInfo {
        rootless: hypervisor_config.security_info.rootless,
        disable_seccomp: hypervisor_config.security_info.disable_seccomp,
        guest_hook_path: hypervisor_config.security_info.guest_hook_path.clone(),
        enable_annotations: hypervisor_config.security_info.enable_annotations.clone(),
        confidential_guest: hypervisor_config.security_info.confidential_guest,
    };

    let hypervisor_info = HypervisorInfo {
        machine_type: hypervisor_config.machine_info.machine_type.to_string(),
        machine_accelerators: hypervisor_config
            .machine_info
            .machine_accelerators
            .to_string(),
        version,
        path: hypervisor_config.path.to_string(),
        block_device_driver: hypervisor_config
            .blockdev_info
            .block_device_driver
            .to_string(),
        entropy_source: hypervisor_config.machine_info.entropy_source.to_string(),
        shared_fs: hypervisor_config
            .shared_fs
            .shared_fs
            .clone()
            .unwrap_or_else(|| String::from("none")),
        virtio_fs_daemon: hypervisor_config.shared_fs.virtio_fs_daemon.to_string(),
        msize_9p: hypervisor_config.shared_fs.msize_9p,
        memory_slots: hypervisor_config.memory_info.memory_slots,
        pcie_root_port: hypervisor_config.device_info.pcie_root_port,
        hotplug_vfio_on_rootbus: hypervisor_config.device_info.hotplug_vfio_on_root_bus,
        debug: hypervisor_config.debug_info.enable_debug,
        enable_iommu: hypervisor_config.device_info.enable_iommu,
        enable_iommu_platform: hypervisor_config.device_info.enable_iommu_platform,
        default_vcpus: hypervisor_config.cpu_info.default_vcpus,
        cpu_features: hypervisor_config.cpu_info.cpu_features.to_string(),
        security_info,
    };

    let image_info = ImageInfo {
        path: hypervisor_config.boot_info.image.clone(),
    };

    let kernel_info = KernelInfo {
        path: hypervisor_config.boot_info.kernel.to_string(),
        parameters: hypervisor_config.boot_info.kernel_params.to_string(),
    };

    let initrd_info = InitrdInfo {
        path: hypervisor_config.boot_info.initrd.to_string(),
    };

    Ok((hypervisor_info, image_info, kernel_info, initrd_info))
}

pub fn get_env_info(toml_config: &TomlConfig) -> Result<EnvInfo> {
    let metainfo = get_meta_info();

    let runtime_info = get_runtime_info(toml_config).context("get runtime info")?;

    let agent_info = get_agent_info(toml_config).context("get agent configuration")?;

    let host_info = get_host_info().context("get host information")?;

    let (hypervisor_info, _image_info, kernel_info, initrd_info) =
        get_hypervisor_info(toml_config).context("get hypervisor configuration")?;

    let env_info = EnvInfo {
        meta: metainfo,
        runtime: runtime_info,
        kernel: kernel_info,
        image: _image_info,
        initrd: initrd_info,
        hypervisor: hypervisor_info,
        host: host_info,
        agent: agent_info,
    };

    Ok(env_info)
}

pub fn handle_env(env_args: EnvArgument) -> Result<()> {
    let mut file: Box<dyn Write> = if let Some(path) = env_args.file {
        Box::new(
            File::create(path.as_str()).with_context(|| format!("Error creating file {}", path))?,
        )
    } else {
        Box::new(io::stdout())
    };

    let (toml_config, _) = TomlConfig::load_from_default().context("load toml config")?;

    let env_info = get_env_info(&toml_config)?;

    if env_args.json {
        let serialized_json = serde_json::to_string_pretty(&env_info)?;
        write!(file, "{}", serialized_json)?;
    } else {
        let toml = toml::to_string(&env_info)?;
        write!(file, "{}", toml)?;
    }

    Ok(())
}
