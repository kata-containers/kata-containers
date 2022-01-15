// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Result};
<<<<<<< HEAD
use std::u32;

use serde::Deserialize;

use crate::config::hypervisor::get_hypervisor_plugin;
use crate::config::KataConfig;
use crate::config::TomlConfig;
use crate::{eother, sl};
<<<<<<< HEAD
use std::path::Path;
use std::sync::Arc;
=======

<<<<<<< HEAD
use serde::Deserialize;
=======
>>>>>>> 4fd1f433 (add more branch for hypervisor annotation)

use crate::config::KataConfig;
use crate::{eother, sl};

>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
=======
>>>>>>> 32fd6cde (add functionalities to modify config info of hypervisor and agent)
/// CRI-containerd specific annotations.
pub mod cri_containerd;

/// CRI-O specific annotations.
pub mod crio;

/// Dockershim specific annotations.
pub mod dockershim;

/// Third-party annotations.
pub mod thirdparty;

<<<<<<< HEAD
macro_rules! change_hypervisor_config {
    ($result:expr,$var:expr) => {
        match ($result, &mut $var) {
            (result_val, var_val) => match result_val {
                Err(e) => Err(e),
                Ok(r) => match r {
                    Some(v) => {
                        *var_val = v;
                        Ok(())
                    }
                    None => Ok(()),
                },
            },
        }
    };
}
<<<<<<< HEAD
=======
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
=======

macro_rules! change_runtime_config {
    ($result:expr,$var:expr) => {
        match ($result, &mut $var) {
            (result_val, var_val) => match result_val {
                Some(v) => {
                    *var_val = v;
                }
                None => (),
            },
        }
    };
}

>>>>>>> 8cba8f93 (add runtime anno:)
// Common section
/// Prefix for Kata specific annotations
pub const KATA_ANNO_PREFIX: &str = "io.katacontainers.";
/// Prefix for Kata configuration annotations
pub const KATA_ANNO_CONF_PREFIX: &str = "io.katacontainers.config.";
/// Prefix for Kata container annotations
pub const KATA_ANNO_CONTAINER_PREFIX: &str = "io.katacontainers.container.";
/// The annotation key to fetch runtime configuration file.
pub const SANDBOX_CONFIG_PATH_KEY: &str = "io.katacontainers.config_path";

// OCI section
/// The annotation key to fetch the OCI configuration file path.
pub const BUNDLE_PATH_KEY: &str = "io.katacontainers.pkg.oci.bundle_path";
/// The annotation key to fetch container type.
pub const CONTAINER_TYPE_KEY: &str = "io.katacontainers.pkg.oci.container_type";

// Container resource related annotations
/// Prefix for Kata container resource related annotations.
pub const KATA_ANNO_CONTAINER_RESOURCE_PREFIX: &str = "io.katacontainers.container.resource";
/// A container annotation to specify the Resources.Memory.Swappiness.
pub const KATA_ANNO_CONTAINER_RESOURCE_SWAPPINESS: &str =
    "io.katacontainers.container.resource.swappiness";
/// A container annotation to specify the Resources.Memory.Swap.
pub const KATA_ANNO_CONTAINER_RESOURCE_SWAP_IN_BYTES: &str =
    "io.katacontainers.container.resource.swap_in_bytes";

// Agent related annotations
/// Prefix for Agent configurations.
pub const KATA_ANNO_CONF_AGENT_PREFIX: &str = "io.katacontainers.config.agent.";
/// KernelModules is the annotation key for passing the list of kernel modules and their parameters
/// that will be loaded in the guest kernel.
///
/// Semicolon separated list of kernel modules and their parameters. These modules will be loaded
/// in the guest kernel using modprobe(8).
/// The following example can be used to load two kernel modules with parameters
///
///   annotations:
///     io.katacontainers.config.agent.kernel_modules: "e1000e InterruptThrottleRate=3000,3000,3000 EEE=1; i915 enable_ppgtt=0"
///
/// The first word is considered as the module name and the rest as its parameters.
pub const KATA_ANNO_CONF_KERNEL_MODULES: &str = "io.katacontainers.config.agent.kernel_modules";
/// A sandbox annotation to enable tracing for the agent.
pub const KATA_ANNO_CONF_AGENT_TRACE: &str = "io.katacontainers.config.agent.enable_tracing";
/// An annotation to specify the size of the pipes created for containers.
pub const KATA_ANNO_CONF_AGENT_CONTAINER_PIPE_SIZE: &str =
    "io.katacontainers.config.agent.container_pipe_size";
/// An annotation key to specify the size of the pipes created for containers.
pub const CONTAINER_PIPE_SIZE_KERNEL_PARAM: &str = "agent.container_pipe_size";

//	Hypervisor related annotations
/// Prefix for Hypervisor configurations.
pub const KATA_ANNO_CONF_HYPERVISOR_PREFIX: &str = "io.katacontainers.config.hypervisor.";
/// A sandbox annotation for passing a per container path pointing at the hypervisor that will run
/// the container VM.
pub const KATA_ANNO_CONF_HYPERVISOR_PATH: &str = "io.katacontainers.config.hypervisor.path";
/// A sandbox annotation for passing a container hypervisor binary SHA-512 hash value.
pub const KATA_ANNO_CONF_HYPERVISOR_HASH: &str = "io.katacontainers.config.hypervisor.path_hash";
/// A sandbox annotation for passing a per container path pointing at the hypervisor control binary
/// that will run the container VM.
pub const KATA_ANNO_CONF_HYPERVISOR_CTLPATH: &str = "io.katacontainers.config.hypervisor.ctlpath";
/// A sandbox annotation for passing a container hypervisor control binary SHA-512 hash value.
pub const KATA_ANNO_CONF_HYPERVISOR_CTLHASH: &str =
    "io.katacontainers.config.hypervisor.hypervisorctl_hash";
/// A sandbox annotation for passing a per container path pointing at the jailer that will constrain
/// the container VM.
pub const KATA_ANNO_CONF_HYPERVISOR_JAILER_PATH: &str =
    "io.katacontainers.config.hypervisor.jailer_path";
/// A sandbox annotation for passing a jailer binary SHA-512 hash value.
pub const KATA_ANNO_CONF_HYPERVISOR_JAILER_HASH: &str =
    "io.katacontainers.config.hypervisor.jailer_hash";
/// A sandbox annotation to enable IO to be processed in a separate thread.
/// Supported currently for virtio-scsi driver.
pub const KATA_ANNO_CONF_HYPERVISOR_ENABLE_IO_THREADS: &str =
    "io.katacontainers.config.hypervisor.enable_iothreads";
/// The hash type used for assets verification
pub const KATA_ANNO_CONF_HYPERVISOR_ASSET_HASH_TYPE: &str =
    "io.katacontainers.config.hypervisor.asset_hash_type";
/// SHA512 is the SHA-512 (64) hash algorithm
pub const SHA512: &str = "sha512";

//	Hypervisor Block Device related annotations
/// Specify the driver to be used for block device either VirtioSCSI or VirtioBlock
pub const KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_DRIVER: &str =
    "io.katacontainers.config.hypervisor.block_device_driver";
/// A sandbox annotation that disallows a block device from being used.
pub const KATA_ANNO_CONF_HYPERVISOR_DISABLE_BLOCK_DEVICE_USE: &str =
    "io.katacontainers.config.hypervisor.disable_block_device_use";
/// A sandbox annotation that specifies cache-related options will be set to block devices or not.
pub const KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_SET: &str =
    "io.katacontainers.config.hypervisor.block_device_cache_set";
/// A sandbox annotation that specifies cache-related options for block devices.
/// Denotes whether use of O_DIRECT (bypass the host page cache) is enabled.
pub const KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_DIRECT: &str =
    "io.katacontainers.config.hypervisor.block_device_cache_direct";
/// A sandbox annotation that specifies cache-related options for block devices.
/// Denotes whether flush requests for the device are ignored.
pub const KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_NOFLUSH: &str =
    "io.katacontainers.config.hypervisor.block_device_cache_noflush";
/// A sandbox annotation to specify use of nvdimm device for guest rootfs image.
pub const KATA_ANNO_CONF_HYPERVISOR_DISABLE_IMAGE_NVDIMM: &str =
    "io.katacontainers.config.hypervisor.disable_image_nvdimm";
/// A sandbox annotation that specifies the memory space used for nvdimm device by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_MEMORY_OFFSET: &str =
    "io.katacontainers.config.hypervisor.memory_offset";
/// A sandbox annotation to specify if vhost-user-blk/scsi is abailable on the host
pub const KATA_ANNO_CONF_HYPERVISOR_ENABLE_VHOSTUSER_STORE: &str =
    "io.katacontainers.config.hypervisor.enable_vhost_user_store";
/// A sandbox annotation to specify the directory path where vhost-user devices related folders,
/// sockets and device nodes should be.
pub const KATA_ANNO_CONF_HYPERVISOR_VHOSTUSER_STORE_PATH: &str =
    "io.katacontainers.config.hypervisor.vhost_user_store_path";

//	Hypervisor Guest Boot related annotations
/// A sandbox annotation for passing a per container path pointing at the kernel needed to boot
/// the container VM.
pub const KATA_ANNO_CONF_HYPERVISOR_KERNEL_PATH: &str =
    "io.katacontainers.config.hypervisor.kernel";
/// A sandbox annotation for passing a container kernel image SHA-512 hash value.
pub const KATA_ANNO_CONF_HYPERVISOR_KERNEL_HASH: &str =
    "io.katacontainers.config.hypervisor.kernel_hash";
/// A sandbox annotation for passing a per container path pointing at the guest image that will run
/// in the container VM.
/// A sandbox annotation for passing additional guest kernel parameters.
pub const KATA_ANNO_CONF_HYPERVISOR_KERNEL_PARAMS: &str =
    "io.katacontainers.config.hypervisor.kernel_params";
/// A sandbox annotation for passing a container guest image path.
pub const KATA_ANNO_CONF_HYPERVISOR_IMAGE_PATH: &str = "io.katacontainers.config.hypervisor.image";
/// A sandbox annotation for passing a container guest image SHA-512 hash value.
pub const KATA_ANNO_CONF_HYPERVISOR_IMAGE_HASH: &str =
    "io.katacontainers.config.hypervisor.image_hash";
/// A sandbox annotation for passing a per container path pointing at the initrd that will run
/// in the container VM.
pub const KATA_ANNO_CONF_HYPERVISOR_INITRD_PATH: &str =
    "io.katacontainers.config.hypervisor.initrd";
/// A sandbox annotation for passing a container guest initrd SHA-512 hash value.
pub const KATA_ANNO_CONF_HYPERVISOR_INITRD_HASH: &str =
    "io.katacontainers.config.hypervisor.initrd_hash";
/// A sandbox annotation for passing a per container path pointing at the guest firmware that will
/// run the container VM.
pub const KATA_ANNO_CONF_HYPERVISOR_FIRMWARE_PATH: &str =
    "io.katacontainers.config.hypervisor.firmware";
/// A sandbox annotation for passing a container guest firmware SHA-512 hash value.
pub const KATA_ANNO_CONF_HYPERVISOR_FIRMWARE_HASH: &str =
    "io.katacontainers.config.hypervisor.firmware_hash";

//	Hypervisor CPU related annotations
/// A sandbox annotation to specify cpu specific features.
pub const KATA_ANNO_CONF_HYPERVISOR_CPU_FEATURES: &str =
    "io.katacontainers.config.hypervisor.cpu_features";
/// A sandbox annotation for passing the default vcpus assigned for a VM by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_DEFAULT_VCPUS: &str =
    "io.katacontainers.config.hypervisor.default_vcpus";
/// A sandbox annotation that specifies the maximum number of vCPUs allocated for the VM by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MAX_VCPUS: &str =
    "io.katacontainers.config.hypervisor.default_max_vcpus";

//<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
//	Hypervisor Device related annotations
/// A sandbox annotation used to indicate if devices need to be hotplugged on the root bus instead
/// of a bridge.
pub const KATA_ANNO_CONF_HYPERVISOR_HOTPLUG_VFIO_ON_ROOT_BUS: &str =
    "io.katacontainers.config.hypervisor.hotplug_vfio_on_root_bus";
/// PCIeRootPort is used to indicate the number of PCIe Root Port devices
pub const KATA_ANNO_CONF_HYPERVISOR_PCIE_ROOT_PORT: &str =
    "io.katacontainers.config.hypervisor.pcie_root_port";
/// A sandbox annotation to specify if the VM should have a vIOMMU device.
pub const KATA_ANNO_CONF_HYPERVISOR_IOMMU: &str =
    "io.katacontainers.config.hypervisor.enable_iommu";
/// Enable Hypervisor Devices IOMMU_PLATFORM
pub const KATA_ANNO_CONF_HYPERVISOR_IOMMU_PLATFORM: &str =
    "io.katacontainers.config.hypervisor.enable_iommu_platform";

//	Hypervisor Machine related annotations
/// A sandbox annotation to specify the type of machine being emulated by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_MACHINE_TYPE: &str =
    "io.katacontainers.config.hypervisor.machine_type";
/// A sandbox annotation to specify machine specific accelerators for the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_MACHINE_ACCELERATORS: &str =
    "io.katacontainers.config.hypervisor.machine_accelerators";
/// EntropySource is a sandbox annotation to specify the path to a host source of
/// entropy (/dev/random, /dev/urandom or real hardware RNG device)
pub const KATA_ANNO_CONF_HYPERVISOR_ENTROPY_SOURCE: &str =
    "io.katacontainers.config.hypervisor.entropy_source";

//	Hypervisor Memory related annotations
/// A sandbox annotation for the memory assigned for a VM by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MEMORY: &str =
    "io.katacontainers.config.hypervisor.default_memory";
/// A sandbox annotation to specify the memory slots assigned to the VM by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_MEMORY_SLOTS: &str =
    "io.katacontainers.config.hypervisor.memory_slots";
/// A sandbox annotation that specifies the memory space used for nvdimm device by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_MEMORY_PREALLOC: &str =
    "io.katacontainers.config.hypervisor.enable_mem_prealloc";
/// A sandbox annotation to specify if the memory should be pre-allocated from huge pages.
pub const KATA_ANNO_CONF_HYPERVISOR_HUGE_PAGES: &str =
    "io.katacontainers.config.hypervisor.enable_hugepages";
/// A sandbox annotation to soecify file based memory backend root directory.
pub const KATA_ANNO_CONF_HYPERVISOR_FILE_BACKED_MEM_ROOT_DIR: &str =
    "io.katacontainers.config.hypervisor.file_mem_backend";
/// A sandbox annotation that is used to enable/disable virtio-mem.
pub const KATA_ANNO_CONF_HYPERVISOR_VIRTIO_MEM: &str =
    "io.katacontainers.config.hypervisor.enable_virtio_mem";
/// A sandbox annotation to enable swap of vm memory.
pub const KATA_ANNO_CONF_HYPERVISOR_ENABLE_SWAP: &str =
    "io.katacontainers.config.hypervisor.enable_swap";
/// A sandbox annotation to enable swap in the guest.
pub const KATA_ANNO_CONF_HYPERVISOR_ENABLE_GUEST_SWAP: &str =
    "io.katacontainers.config.hypervisor.enable_guest_swap";
//>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>

//	Hypervisor Network related annotations
/// A sandbox annotation to specify if vhost-net is not available on the host.
pub const KATA_ANNO_CONF_HYPERVISOR_DISABLE_VHOST_NET: &str =
    "io.katacontainers.config.hypervisor.disable_vhost_net";
/// A sandbox annotation that specifies max rate on network I/O inbound bandwidth.
pub const KATA_ANNO_CONF_HYPERVISOR_RX_RATE_LIMITER_MAX_RATE: &str =
    "io.katacontainers.config.hypervisor.rx_rate_limiter_max_rate";
/// A sandbox annotation that specifies max rate on network I/O outbound bandwidth.
pub const KATA_ANNO_CONF_HYPERVISOR_TX_RATE_LIMITER_MAX_RATE: &str =
    "io.katacontainers.config.hypervisor.tx_rate_limiter_max_rate";

//<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
//	Hypervisor Security related annotations
/// A sandbox annotation to specify the path within the VM that will be used for 'drop-in' hooks.
pub const KATA_ANNO_CONF_HYPERVISOR_GUEST_HOOK_PATH: &str =
    "io.katacontainers.config.hypervisor.guest_hook_path";
/// A sandbox annotation to enable rootless hypervisor (only supported in QEMU currently).
pub const KATA_ANNO_CONF_HYPERVISOR_ENABLE_ROOTLESS_HYPERVISOR: &str =
    "io.katacontainers.config.hypervisor.rootless";

//	Hypervisor Shared File System related annotations
/// A sandbox annotation to specify the shared file system type, either virtio-9p or virtio-fs.
pub const KATA_ANNO_CONF_HYPERVISOR_SHARED_FS: &str =
    "io.katacontainers.config.hypervisor.shared_fs";
/// A sandbox annotations to specify virtio-fs vhost-user daemon path.
pub const KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_DAEMON: &str =
    "io.katacontainers.config.hypervisor.virtio_fs_daemon";
/// A sandbox annotation to specify the cache mode for fs version cache or "none".
pub const KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_CACHE: &str =
    "io.katacontainers.config.hypervisor.virtio_fs_cache";
/// A sandbox annotation to specify the DAX cache size in MiB.
pub const KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_CACHE_SIZE: &str =
    "io.katacontainers.config.hypervisor.virtio_fs_cache_size";
/// A sandbox annotation to pass options to virtiofsd daemon.
pub const KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_EXTRA_ARGS: &str =
    "io.katacontainers.config.hypervisor.virtio_fs_extra_args";
/// A sandbox annotation to specify as the msize for 9p shares.
pub const KATA_ANNO_CONF_HYPERVISOR_MSIZE_9P: &str = "io.katacontainers.config.hypervisor.msize_9p";

// Runtime related annotations
/// Prefix for Runtime configurations.
pub const KATA_ANNO_CONF_RUNTIME_PREFIX: &str = "io.katacontainers.config.runtime.";
/// A sandbox annotation that determines if seccomp should be applied inside guest.
pub const KATA_ANNO_CONF_DISABLE_GUEST_SECCOMP: &str =
    "io.katacontainers.config.runtime.disable_guest_seccomp";
/// A sandbox annotation that determines if pprof enabled.
pub const KATA_ANNO_CONF_ENABLE_PPROF: &str = "io.katacontainers.config.runtime.enable_pprof";
/// A sandbox annotation that determines if experimental features enabled.
pub const KATA_ANNO_CONF_EXPERIMENTAL: &str = "io.katacontainers.config.runtime.experimental";
/// A sandbox annotaion that determines how the VM should be connected to the the container network
/// interface.
pub const KATA_ANNO_CONF_INTER_NETWORK_MODEL: &str =
    "io.katacontainers.config.runtime.internetworking_model";
/// SandboxCgroupOnly is a sandbox annotation that determines if kata processes are managed only in sandbox cgroup.
pub const KATA_ANNO_CONF_SANDBOX_CGROUP_ONLY: &str =
    "io.katacontainers.config.runtime.sandbox_cgroup_only";
/// A sandbox annotation that determines if create a netns for hypervisor process.
pub const KATA_ANNO_CONF_DISABLE_NEW_NETNS: &str =
    "io.katacontainers.config.runtime.disable_new_netns";
/// A sandbox annotation to specify how attached VFIO devices should be treated.
pub const KATA_ANNO_CONF_VFIO_MODE: &str = "io.katacontainers.config.runtime.vfio_mode";
//>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>

/// A helper structure to query configuration information by check annotations.
#[derive(Debug, Default, Deserialize)]
pub struct Annotation {
    annotations: HashMap<String, String>,
}

impl From<HashMap<String, String>> for Annotation {
    fn from(annotations: HashMap<String, String>) -> Self {
        Annotation { annotations }
    }
}

impl Into<HashMap<String, String>> for Annotation {
    fn into(self) -> HashMap<String, String> {
        self.annotations
    }
}

impl Annotation {
    /// Create a new instance of [`Annotation`].
    pub fn new(annotations: HashMap<String, String>) -> Annotation {
        Annotation { annotations }
    }

    /// Deserialize an object from a json string.
    pub fn deserialize<T>(path: &str) -> Result<T>
    where
        for<'a> T: Deserialize<'a>,
    {
        let f = BufReader::new(File::open(path)?);
        Ok(serde_json::from_reader(f)?)
    }

    /// Get an immutable reference to the annotation hashmap.
    pub fn get_annotation(&self) -> &HashMap<String, String> {
        &self.annotations
    }

    /// Get a mutable reference to the annotation hashmap.
    pub fn get_annotation_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.annotations
    }

    /// Get the value of annotation with `key` as string.
    pub fn get(&self, key: &str) -> Option<String> {
        let v = self.annotations.get(key)?;
        let value = v.trim();

        if value != "" {
            Some(String::from(value))
        } else {
            None
        }
    }

    /// Get the value of annotation with `key` as bool.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        if let Some(value) = self.get(key) {
            let value = value.trim();
            if value == "true" {
                return Some(true);
            } else if value == "false" {
                return Some(false);
            } else {
                warn!(sl!(), "failed to parse bool value from {}", value);
            }
        }

        None
    }

    /// Get the value of annotation with `key` as u32.
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        let s = self.get(key)?;
        match s.parse::<u32>() {
            Ok(nums) => {
                if nums > 0 {
                    Some(nums)
                } else {
                    None
                }
            }

            Err(e) => {
                warn!(
                    sl!(),
                    "failed to parse u32 value from {}, error: {:?}", s, e
                );
                None
            }
        }
    }

    /// Get the value of annotation with `key` as i32.
    pub fn get_i32(&self, key: &str) -> Option<i32> {
        let s = self.get(key)?;
        match s.parse::<i32>() {
            Ok(nums) => {
                if nums > 0 {
                    Some(nums)
                } else {
                    None
                }
            }

            Err(e) => {
                warn!(
                    sl!(),
                    "failed to parse u32 value from {}, error: {:?}", s, e
                );
                None
            }
        }
    }

    /// Get the value of annotation with `key` as u64.
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        let s = self.get(key)?;
        match s.parse::<u64>() {
            Ok(nums) => {
                if nums > 0 {
                    Some(nums)
                } else {
                    None
                }
            }

            Err(e) => {
                warn!(
                    sl!(),
                    "failed to parse u64 value from {}, error: {:?}", s, e
                );
                None
            }
        }
    }
<<<<<<< HEAD
    /*
    /// add annotation for config
    pub fn add_annotation() -> Reuslt<()>{

    }
    */
=======
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
}

// Miscellaneous annotations.
impl Annotation {
    /// Get the annotation of sandbox configuration file path.
    pub fn get_sandbox_config_path(&self) -> Option<String> {
        self.get(SANDBOX_CONFIG_PATH_KEY)
    }

    /// Get the annotation of bundle path.
    pub fn get_bundle_path(&self) -> Option<String> {
        self.get(BUNDLE_PATH_KEY)
    }

    /// Get the annotation of container type.
    pub fn get_container_type(&self) -> Option<String> {
        self.get(CONTAINER_TYPE_KEY)
    }

    /// Get the annotation to specify the Resources.Memory.Swappiness.
    pub fn get_container_resource_swappiness(&self) -> Option<u32> {
        let v = self.get_u32(KATA_ANNO_CONTAINER_RESOURCE_SWAPPINESS)?;
        if v > 100 {
            None
        } else {
            Some(v)
        }
    }

    /// Get the annotation to specify the Resources.Memory.Swap.
    pub fn get_container_resource_swap_in_bytes(&self) -> Option<String> {
        self.get(KATA_ANNO_CONTAINER_RESOURCE_SWAP_IN_BYTES)
    }
}

// Agent related annotations.
impl Annotation {
    /// Get the annotation for "config.agent.kernel_modules`.
    pub fn get_agent_kernel_module(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_KERNEL_MODULES)
    }

    /// Get the annotation for `config.agent.enable_tracing`.
    pub fn get_agent_enable_trace(&self) -> Option<bool> {
        self.get_bool(KATA_ANNO_CONF_AGENT_TRACE)
    }

    /// Get the annotation of agent container pipe size.
    pub fn get_agent_container_pipe_size(&self) -> Option<u32> {
        self.get_u32(KATA_ANNO_CONF_AGENT_CONTAINER_PIPE_SIZE)
    }
<<<<<<< HEAD

    /// add annotation kernel modules of agent to config
    pub fn add_agent_annotation(&self, config: &mut TomlConfig, agent_name: &String) {
        let agent = config.agent.get_mut(agent_name).unwrap();
        let kernel_mods_option = self.get_agent_kernel_module();
        match kernel_mods_option {
            Some(k) => {
                let kernel_mod: Vec<String> = k.split(';').map(str::to_string).collect();
                for modules in kernel_mod {
                    agent.kernel_modules.push(modules.to_string());
                }
            }
            None => (),
        }
    }

    /// add annotation enable_tracing of agent to config
    pub fn add_agent_enable_trace(&self, config: &mut TomlConfig, agent_name: &String) {
        let agent = config.agent.get_mut(agent_name).unwrap();
        let trace = self.get_agent_enable_trace();
        match trace {
            Some(t) => agent.enable_tracing = t,
            None => (),
        }
    }

    /// add annotation container pipe size of agent to config
    pub fn add_agent_container_pipe_size(&self, config: &mut TomlConfig, agent_name: &String) {
        let agent = config.agent.get_mut(agent_name).unwrap();
        let pipe_size = self.get_agent_container_pipe_size();
        match pipe_size {
            Some(s) => agent.container_pipe_size = s,
            None => (),
        }
    }
=======
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
}

/// Generic hypervisor related annotations.
impl Annotation {
    fn check_allowed_hypervisor_annotation(&self, id: &str) -> Result<()> {
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            match self.get(id) {
                Some(_a) => {
                    if hv.security_info.is_annotation_enabled(id) {
                        return Ok(());
                    }
                }
                None => {
                    return Ok(());
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{}{} is not allowed", KATA_ANNO_CONF_HYPERVISOR_PREFIX, id),
        ))
    }

    /// Get and validate the annotation for `config.hypervisor.path`.
    pub fn get_hypervisor_path(&self) -> Result<Option<String>> {
<<<<<<< HEAD
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_PATH)?;
=======
        self.check_allowed_hypervisor_annotation("path")?;
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
        match self.annotations.get(KATA_ANNO_CONF_HYPERVISOR_PATH) {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .validate_hypervisor_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation of hash value for hypervisor path.
    pub fn get_hypervisor_path_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_HASH)
    }

    /// Get and validate the annotation for "config.hypervisor.ctlpath`.
    pub fn get_hypervisor_ctlpath(&self) -> Result<Option<String>> {
<<<<<<< HEAD
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_CTLPATH)?;
=======
        self.check_allowed_hypervisor_annotation("ctlpath")?;
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
        match self.annotations.get(KATA_ANNO_CONF_HYPERVISOR_CTLPATH) {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .validate_hypervisor_ctlpath(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation of hash value for hypervisor ctlpath.
    pub fn get_hypervisor_path_ctlhash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_CTLHASH)
    }

    /// Get and validate the annotation for `config.hypervisor.jailer_path`.
    pub fn get_jailer_path(&self) -> Result<Option<String>> {
<<<<<<< HEAD
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_JAILER_PATH)?;
=======
        self.check_allowed_hypervisor_annotation("jailer_path")?;
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
        match self.annotations.get(KATA_ANNO_CONF_HYPERVISOR_JAILER_PATH) {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .validate_jailer_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation of hash value for jailer.
    pub fn get_jailer_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_JAILER_HASH)
    }

    /// Get the annotation for `config.hypervisor.enable_io_threads`.
    pub fn get_enable_io_threads(&self) -> Result<Option<bool>> {
<<<<<<< HEAD
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_ENABLE_IO_THREADS)?;
=======
        self.check_allowed_hypervisor_annotation("enable_io_threads")?;
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_ENABLE_IO_THREADS))
    }

    /// Get the annotation of the hash algorithm type used for assets verification
    pub fn get_asset_hash_type(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_ASSET_HASH_TYPE)
    }
<<<<<<< HEAD

    /// add the annotation for `config.hypervisor.enable_io_threads`.
    pub fn add_enable_io_threads(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        let enable_io_threads_option = self.get_enable_io_threads();
        match enable_io_threads_option {
            Ok(enable_io_threads) => match enable_io_threads {
                Some(j) => {
                    config
                        .hypervisor
                        .get_mut(hypervisor_name)
                        .unwrap()
                        .enable_iothreads = j;
                    Ok(())
                }
                None => Ok(()),
            },
            Err(e) => Err(e),
        }
    }

    /// add hypervisor path annotation
    pub fn add_hypervisor_path(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        let path_option = self.get_hypervisor_path();
        match path_option {
            Ok(path) => match path {
                Some(s) => {
                    config.hypervisor.get_mut(hypervisor_name).unwrap().path = s;
                    Ok(())
                }
                None => Ok(()),
            },
            Err(e) => Err(e),
        }
    }

    /// add hypervisor ctlpath annotation
    pub fn add_hypervisor_ctlpath(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        let ctlpath_option = self.get_hypervisor_ctlpath();
        match ctlpath_option {
            Ok(ctlpath) => match ctlpath {
                Some(j) => {
                    config.hypervisor.get_mut(hypervisor_name).unwrap().ctlpath = j;
                    Ok(())
                }
                None => Ok(()),
            },
            Err(e) => Err(e),
        }
    }

    /// add hypervisor jailer path
    pub fn add_hypervisor_jailer_path(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        let jailer_path_option = self.get_jailer_path();
        match jailer_path_option {
            Ok(jailer_path) => match jailer_path {
                Some(j) => {
                    config
                        .hypervisor
                        .get_mut(hypervisor_name)
                        .unwrap()
                        .jailer_path = j;
                    Ok(())
                }
                None => Ok(()),
            },
            Err(e) => Err(e),
        }
    }
=======
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
}

// Hypervisor block storage related annotations.
impl Annotation {
    /// Get the annotation for `config.hypervisor.block_device_driver`
    pub fn get_block_device_driver(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_DRIVER)?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_DRIVER))
    }

    /// Get the annotation for `config.hypervisor.disable_block_device_use`
    pub fn get_disable_block_device_use(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(
            KATA_ANNO_CONF_HYPERVISOR_DISABLE_BLOCK_DEVICE_USE,
        )?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_DISABLE_BLOCK_DEVICE_USE))
    }

    /// Get the annotation for `config.hypervisor.block_device_cache_set`
    pub fn get_block_device_cache_set(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_SET)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_SET))
    }

    /// Get the annotation for `config.hypervisor.block_device_cache_direct`
    pub fn get_block_device_cache_direct(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(
            KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_DIRECT,
        )?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_DIRECT))
    }

    /// Get the annotation for `config.hypervisor.block_device_cache_noflush`
    pub fn get_block_device_cache_noflush(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation("block_device_cache_direct")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_NOFLUSH))
    }

    /// Get the annotation for `config.hypervisor.disable_image_nvdimm`
    pub fn get_disable_image_nvdimm(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_DISABLE_IMAGE_NVDIMM)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_DISABLE_IMAGE_NVDIMM))
    }

    /// Get the annotation for `config.hypervisor.memory_offset`
    pub fn get_memory_offset(&self) -> Result<Option<u64>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_MEMORY_OFFSET)?;
        Ok(self.get_u64(KATA_ANNO_CONF_HYPERVISOR_MEMORY_OFFSET))
    }

    /// Get the annotation for `config.hypervisor.enable_vhost_user_store`
    pub fn get_enable_vhost_user_store(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_ENABLE_VHOSTUSER_STORE)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_ENABLE_VHOSTUSER_STORE))
    }

    /// Get and validate the annotation for `config.hypervisor.vhost_user_store_path`
    pub fn get_vhost_user_store_path(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_VHOSTUSER_STORE_PATH)?;
        match self
            .annotations
            .get(KATA_ANNO_CONF_HYPERVISOR_VHOSTUSER_STORE_PATH)
        {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .blockdev_info
                .validate_vhost_user_store_path(v)
                .map(|_| Some(v.to_string())),
        }
    }
}

// VM boot related annotations.
impl Annotation {
    /// Get the annotation for "config.hypervisor.kernel".
    pub fn get_kernel(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_KERNEL_PATH)?;
        match self.annotations.get(KATA_ANNO_CONF_HYPERVISOR_KERNEL_PATH) {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .boot_info
                .validate_boot_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation for hash value of guest kernel file path.
    pub fn get_kernel_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_KERNEL_HASH)
    }

    /// Get the annotation for `config.hypervisor.kernel_params`.
    pub fn get_kernel_params(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_KERNEL_PARAMS)?;
        match self
            .annotations
            .get(KATA_ANNO_CONF_HYPERVISOR_KERNEL_PARAMS)
        {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .boot_info
                .validate_boot_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation for `config.hypervisor.image`.
    pub fn get_image(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_IMAGE_PATH)?;
        match self.annotations.get(KATA_ANNO_CONF_HYPERVISOR_IMAGE_PATH) {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .boot_info
                .validate_boot_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation for hash value of guest boot image file path.
    pub fn get_image_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_IMAGE_HASH)
    }

    /// Get the annotation for `config.hypervisor.initrd`.
    pub fn get_initrd(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_INITRD_PATH)?;
        match self.annotations.get(KATA_ANNO_CONF_HYPERVISOR_INITRD_PATH) {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .boot_info
                .validate_boot_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation for hash value of guest initrd file path.
    pub fn get_initrd_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_INITRD_HASH)
    }

    /// Get the annotation for `config.hypervisor.firmware`.
    pub fn get_firmware(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_FIRMWARE_PATH)?;
        match self
            .annotations
            .get(KATA_ANNO_CONF_HYPERVISOR_FIRMWARE_PATH)
        {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .boot_info
                .validate_boot_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation for hash value of firmware file path.
    pub fn get_firmware_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_FIRMWARE_HASH)
    }

    /// kernel path
    pub fn add_annotation_kernel_path(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_kernel(),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .boot_info
                .kernel
        )
    }
}

// VM CPU related annotations.
impl Annotation {
    /// Get the annotation for "config.hypervisor.cpu_features".
    pub fn get_cpu_features(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_CPU_FEATURES)?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_CPU_FEATURES))
    }

    /// Get the annotation for "config.hypervisor.default_vcpus".
    pub fn get_default_vcpus(&self, hypervisor_name: &String) -> Result<Option<i32>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_DEFAULT_VCPUS)?;
        match self.get_i32(KATA_ANNO_CONF_HYPERVISOR_DEFAULT_VCPUS) {
            None => Ok(None),
            Some(v) => {
                if v > get_hypervisor_plugin(hypervisor_name)
                    .unwrap()
                    .get_max_cpus() as i32
                {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "Vcpus specified in annotation {} is more than maximum limitation {}",
                            v,
                            get_hypervisor_plugin(hypervisor_name)
                                .unwrap()
                                .get_max_cpus()
                        ),
                    ))
                } else {
                    Ok(Some(v))
                }
            }
        }
    }

    /// Get the annotation for "config.hypervisor.default_max_vcpus".
    pub fn get_default_max_vcpus(&self) -> Result<Option<u32>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MAX_VCPUS)?;
        Ok(self.get_u32(KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MAX_VCPUS))
    }

    /// add hypervisor defualt vcpus annotation
    pub fn add_hypervisor_defualt_vcpus(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_default_vcpus(hypervisor_name),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .cpu_info
                .default_vcpus
        )
    }
}

<<<<<<< HEAD
// Vm device related annotation
impl Annotation {
    ///Get the annotatoin for "config.hypervisor.DeviceInfo.hotplug_vfio_on_root_bus".
    pub fn get_hotplug_vfio_on_root_bus(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(
            KATA_ANNO_CONF_HYPERVISOR_HOTPLUG_VFIO_ON_ROOT_BUS,
        )?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_HOTPLUG_VFIO_ON_ROOT_BUS))
    }

    ///Get the annotatoin for "config.hypervisor.DeviceInfo.pice_root_port".
    pub fn get_pice_root_port(&self) -> Result<Option<u32>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_PCIE_ROOT_PORT)?;
        Ok(self.get_u32(KATA_ANNO_CONF_HYPERVISOR_PCIE_ROOT_PORT))
    }

    ///Get the annotatoin for "config.hypervisor.DeviceInfo.enable_iommu".
    pub fn get_enable_iommu(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_IOMMU)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_IOMMU))
    }

    ///Get the annotatoin for "config.hypervisor.DeviceInfo.enable_iommu_platform".
    pub fn get_enable_iommu_platform(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_IOMMU_PLATFORM)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_IOMMU_PLATFORM))
    }
}

// VM Machine related annotations
impl Annotation {
    ///Get the annotation for "config.hypervisor.MachineInfo.machine_type"
    pub fn get_machine_type(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_MACHINE_TYPE)?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_MACHINE_TYPE))
    }

    ///Get the annotation for "config.hypervisor.MachineInfo.accelerators"
    pub fn get_machine_acclereates(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_MACHINE_ACCELERATORS)?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_MACHINE_ACCELERATORS))
    }
}
// VM Memory related annotations
impl Annotation {
    ///Get the annotaion for "config.hypervisor.MemoryInfo.default_memory"
    pub fn get_default_memory(&self, hypervisor_name: &String) -> Result<Option<u32>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MEMORY)?;
        match self.get_u32(KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MEMORY) {
            None => Ok(None),
            Some(v) => {
                if v < get_hypervisor_plugin(hypervisor_name)
                    .unwrap()
                    .get_min_memory()
                {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "Memory specified in annotation {} is less than minmum required {}",
                            v,
                            get_hypervisor_plugin(hypervisor_name)
                                .unwrap()
                                .get_min_memory()
                        ),
                    ))
                } else {
                    Ok(Some(v))
                }
            }
        }
    }

    ///Get the annotation for "config.hypervisor.MemoryInfo.memory_slots"
    pub fn get_memory_slots(&self) -> Result<Option<u32>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_MEMORY_SLOTS)?;
        match self
            .get_annotation()
            .get(KATA_ANNO_CONF_HYPERVISOR_MEMORY_SLOTS)
        {
            None => Ok(None),
            Some(_a) => match self.get_u32(KATA_ANNO_CONF_HYPERVISOR_MEMORY_SLOTS) {
                Some(v) => Ok(Some(v)),
                None => Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("Memory slots in annotation is less than zero"),
                )),
            },
        }
    }

    /// Get the annotation for "config.hypervisor.MemoryInfo.enable_mem_prealloc"
    pub fn get_enable_mem_prealloc(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_MEMORY_PREALLOC)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_MEMORY_PREALLOC))
    }

    /// Get the annotaion for "config.hypervisor.MemoryInfo.enable_hugepages"
    pub fn get_enable_hugepages(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_HUGE_PAGES)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_HUGE_PAGES))
    }

    /// Get the annotaion for "config.hypervisor.MemoryInfo.file_mem_backend"
    pub fn get_file_mem_backend(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(
            KATA_ANNO_CONF_HYPERVISOR_FILE_BACKED_MEM_ROOT_DIR,
        )?;
        match self
            .annotations
            .get(KATA_ANNO_CONF_HYPERVISOR_FILE_BACKED_MEM_ROOT_DIR)
        {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .memory_info
                .validate_memory_backend_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the annotation for "config.hypervisor.MemoryInfo.enable_virtio_mem"
    pub fn get_enable_virtio_mem(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_MEM)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_MEM))
    }

    /// Get the annotaion for "config.hypervisor.MemoryInfo.enable_swap"
    pub fn get_enable_swap(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_ENABLE_SWAP)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_ENABLE_SWAP))
    }

    /// Get the annotaion for "config.hypervisor.MemoryInfo.enable_guest_swap"
    pub fn get_enable_guest_swap(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_ENABLE_GUEST_SWAP)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_ENABLE_GUEST_SWAP))
    }

    /// add hypervisor default memory annotaion
    pub fn add_hypervisor_default_memory(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_default_memory(hypervisor_name),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .memory_info
                .default_memory
        )
    }

    /// add hypervisor member slots
    pub fn add_hypervisor_mem_slots(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_memory_slots(),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .memory_info
                .memory_slots
        )
    }

    /// add hypervisor memory prealloc
    pub fn add_hypervisor_memory_prealloc(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_enable_mem_prealloc(),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .memory_info
                .enable_mem_prealloc
        )
    }

    /// add hypervisor huge pages
    pub fn add_hypervisor_enable_hugepages(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_enable_hugepages(),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .memory_info
                .enable_hugepages
        )
    }

    /// add hypervisor file mem backend
    pub fn add_hypervisor_file_mem_backend(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_file_mem_backend(),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .memory_info
                .file_mem_backend
        )
    }

    /// add hypervisor enable virtio member
    pub fn add_hypervisor_virtio_mem(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_enable_virtio_mem(),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .memory_info
                .enable_virtio_mem
        )
    }

    /// add hypervisor enable swap
    pub fn add_hypervisor_enable_swap(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_enable_swap(),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .memory_info
                .enable_swap
        )
    }

    /// add hypervisor enable guest swap
    pub fn add_hypervisor_enable_guest_swap(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        change_hypervisor_config!(
            self.get_enable_guest_swap(),
            config
                .hypervisor
                .get_mut(hypervisor_name)
                .unwrap()
                .memory_info
                .enable_guest_swap
        )
    }
}

// VM Network related annotations.

=======
// VM Network related annotations.
>>>>>>> 65a31d44 (libs/types: define annotation keys for Kata)
impl Annotation {
    /// Get the annotation for `config.hypervisor.disable_vhost_net`.
    pub fn get_disable_vhost_net(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_DISABLE_VHOST_NET)?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_DISABLE_VHOST_NET))
    }

    /// Get the annotation for `config.hypervisor.rx_rate_limiter_max_rate`.
    pub fn get_rx_rate_limiter_max_rate(&self) -> Result<Option<u64>> {
        self.check_allowed_hypervisor_annotation(
            KATA_ANNO_CONF_HYPERVISOR_RX_RATE_LIMITER_MAX_RATE,
        )?;
        Ok(self.get_u64(KATA_ANNO_CONF_HYPERVISOR_RX_RATE_LIMITER_MAX_RATE))
    }

    /// Get the annotation for `config.hypervisor.tx_rate_limiter_max_rate`.
    pub fn get_tx_rate_limiter_max_rate(&self) -> Result<Option<u64>> {
        self.check_allowed_hypervisor_annotation(
            KATA_ANNO_CONF_HYPERVISOR_TX_RATE_LIMITER_MAX_RATE,
        )?;
        Ok(self.get_u64(KATA_ANNO_CONF_HYPERVISOR_TX_RATE_LIMITER_MAX_RATE))
    }
}

impl Annotation {
    /// Get and validate annotation for guest book path
    pub fn get_guest_hook_path(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_GUEST_HOOK_PATH)?;
        match self.get(KATA_ANNO_CONF_HYPERVISOR_GUEST_HOOK_PATH) {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .security_info
                .validate_path(&v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// get and validate annotaion for enable rootless hypervisor
    pub fn get_enable_rootless_hypervisor(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation(
            KATA_ANNO_CONF_HYPERVISOR_ENABLE_ROOTLESS_HYPERVISOR,
        )?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_ENABLE_ROOTLESS_HYPERVISOR))
    }
}
impl Annotation {
    //<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
    /// Get and validate annotation for entropy source.
    pub fn get_entropy_source(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_ENTROPY_SOURCE)?;
        match self
            .annotations
            .get(KATA_ANNO_CONF_HYPERVISOR_ENTROPY_SOURCE)
        {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .machine_info
                .validate_entropy_source(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get and validate annotation for memory backend.
    pub fn get_memory_backend_path(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_ENTROPY_SOURCE)?;
        match self
            .annotations
            .get(KATA_ANNO_CONF_HYPERVISOR_ENTROPY_SOURCE)
        {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .memory_info
                .validate_memory_backend_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the file share system type
    pub fn get_share_fs(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_SHARED_FS)?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_SHARED_FS))
    }

    /// Get the virtio fs daemon path
    pub fn get_virtio_fs_daemon(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_DAEMON)?;
        match self
            .annotations
            .get(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_DAEMON)
        {
            None => Ok(None),
            Some(v) => KataConfig::get_default_config()
                .get_hypervisor()
                .ok_or_else(|| eother!("No active hypervisor configuration"))?
                .shared_fs
                .validate_virtiofs_daemon_path(v)
                .map(|_| Some(v.to_string())),
        }
    }

    /// Get the virtio fs cache
    pub fn get_virtio_fs_cache(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_CACHE)?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_CACHE))
    }

    /// Get the virtio fs ncache size
    pub fn get_virtio_fs_cache_size(&self) -> Result<Option<u32>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_CACHE_SIZE)?;
        Ok(self.get_u32(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_CACHE_SIZE))
    }

    /// Get the virtio fs extra args
    pub fn get_virtio_fs_extra_args(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_EXTRA_ARGS)?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_EXTRA_ARGS))
    }

    /// Get the hypervisor msize 9p
    pub fn get_hypervisor_msize_9p(&self) -> Result<Option<u32>> {
        self.check_allowed_hypervisor_annotation(KATA_ANNO_CONF_HYPERVISOR_MSIZE_9P)?;
        Ok(self.get_u32(KATA_ANNO_CONF_HYPERVISOR_MSIZE_9P))
    }
    /// add hypervisor share fs
    pub fn add_share_fs(&self, config: &mut TomlConfig, hypervisor_name: &String) -> Result<()> {
        match self.get_share_fs() {
            Err(e) => Err(e),
            Ok(a) => {
                config
                    .hypervisor
                    .get_mut(hypervisor_name)
                    .unwrap()
                    .shared_fs
                    .shared_fs = a;
                Ok(())
            }
        }
    }
    /// add hypervisor virtio fs extra args
    pub fn add_virtio_fs_extra_args(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
    ) -> Result<()> {
        match self.get_virtio_fs_extra_args() {
            Err(e) => Err(e),
            Ok(a) => match a {
                Some(j) => {
                    let args: Vec<String> = j.split(',').map(str::to_string).collect();
                    for arg in args {
                        config
                            .hypervisor
                            .get_mut(hypervisor_name)
                            .unwrap()
                            .shared_fs
                            .virtio_fs_extra_args
                            .push(arg.to_string());
                    }
                    Ok(())
                }
                None => Ok(()),
            },
        }
    }
    //>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
}

// runtime
impl Annotation {
    /// get the annotaion for disable guest seccomp
    pub fn get_disable_guest_seccomp(&self) -> Option<bool> {
        self.get_bool(KATA_ANNO_CONF_DISABLE_GUEST_SECCOMP)
    }
    /// get the annotation enable_pprof
    pub fn get_enable_pprof(&self) -> Option<bool> {
        self.get_bool(KATA_ANNO_CONF_ENABLE_PPROF)
    }
    /// get the annotaion experimental
    pub fn get_experimental(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_EXPERIMENTAL)
    }
    /// get the annotation network model
    pub fn get_network_model(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_INTER_NETWORK_MODEL)
    }
    /// get the annotaion for sanndbox cgroup only
    pub fn get_sandbox_cgroup_only(&self) -> Option<bool> {
        self.get_bool(KATA_ANNO_CONF_SANDBOX_CGROUP_ONLY)
    }
    /// get the annotaion for disable new netns
    pub fn get_disable_new_netns(&self) -> Option<bool> {
        self.get_bool(KATA_ANNO_CONF_DISABLE_NEW_NETNS)
    }
    /// get the annotation for conf vfio mode
    pub fn get_vfio_mode(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_VFIO_MODE)
    }
    /// add annotation for experimental
    pub fn add_annotation_experimental(&self, config: &mut TomlConfig) {
        match self.get_experimental() {
            Some(j) => {
                let args: Vec<String> = j.split(',').map(str::to_string).collect();
                for arg in args {
                    config.runtime.experimental.push(arg.to_string());
                }
            }
            None => (),
        }
    }
}
//  add annotations
impl Annotation {
    /// add annotaion information to config
    pub fn add_config_annotation(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &String,
        agent_name: &String,
    ) -> Result<()> {
        // add agent annotaion
        self.add_agent_annotation(config, agent_name);
        self.add_agent_enable_trace(config, agent_name);
        self.add_agent_container_pipe_size(config, agent_name);
        // add hypervisor annotaion
        if self.add_share_fs(config, hypervisor_name).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor get share fs is not allowed"),
            ));
        }

        if self
            .add_virtio_fs_extra_args(config, hypervisor_name)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor virtio fs extra args is not allowed"),
            ));
        }

        let hv = config.hypervisor.get_mut(hypervisor_name).unwrap();
        if change_hypervisor_config!(self.get_enable_io_threads(), hv.enable_iothreads).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("enable io threads is not allowed"),
            ));
        }
        if change_hypervisor_config!(self.get_hypervisor_path(), hv.path).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor path is not allowed"),
            ));
        }
        if change_hypervisor_config!(self.get_jailer_path(), hv.jailer_path).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor jailer path is not allowed"),
            ));
        }
        if change_hypervisor_config!(self.get_hypervisor_ctlpath(), hv.ctlpath).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor ctl path is not allowed"),
            ));
        }
        // add hypervisor block device related annotations
        if change_hypervisor_config!(
            self.get_block_device_driver(),
            hv.blockdev_info.block_device_driver
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor block device driver is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_disable_block_device_use(),
            hv.blockdev_info.disable_block_device_use
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor disable block device use is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_block_device_cache_direct(),
            hv.blockdev_info.block_device_cache_direct
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor block device cache direct is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_block_device_cache_noflush(),
            hv.blockdev_info.block_device_cache_noflush
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor block device cache no flush is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_disable_image_nvdimm(),
            hv.blockdev_info.disable_image_nvdimm
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor block device disable image nvdimm is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_memory_offset(), hv.blockdev_info.memory_offset)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor block device memory offset is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_enable_vhost_user_store(),
            hv.blockdev_info.enable_vhost_user_store
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor block device enable vhost user store is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_vhost_user_store_path(),
            hv.blockdev_info.vhost_user_store_path
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor block device vhost user store path is not allowed"),
            ));
        }

        // add hypervisor boot related annotations
        if change_hypervisor_config!(self.get_kernel(), hv.boot_info.kernel).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor boot info kernel is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_kernel_params(), hv.boot_info.kernel_params).is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor boot info kernel params is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_image(), hv.boot_info.image).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor boot info image is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_initrd(), hv.boot_info.initrd).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor boot info initrd is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_firmware(), hv.boot_info.firmware).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor boot info firmware is not allowed"),
            ));
        }

        // add hypervisor cpu related annotaion
        if change_hypervisor_config!(self.get_cpu_features(), hv.cpu_info.cpu_features).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor cpu features is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_default_vcpus(hypervisor_name),
            hv.cpu_info.default_vcpus
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor defualt cpus is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_default_max_vcpus(), hv.cpu_info.default_maxvcpus)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor defualt max cpus is not allowed"),
            ));
        }

        // add hypervisor device realted annotaion
        if change_hypervisor_config!(
            self.get_hotplug_vfio_on_root_bus(),
            hv.device_info.hotplug_vfio_on_root_bus
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor hotplug cfio on root bus is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_pice_root_port(), hv.device_info.pcie_root_port)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor pice root port is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_enable_iommu(), hv.device_info.enable_iommu).is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor enable iommu is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_enable_iommu_platform(),
            hv.device_info.enable_iommu_platform
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor enable iommu platform is not allowed"),
            ));
        }

        // add hypervisor machine related annotation
        if change_hypervisor_config!(self.get_machine_type(), hv.machine_info.machine_type).is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor machine type is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_machine_acclereates(),
            hv.machine_info.machine_accelerators
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor machine accelerators is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_entropy_source(), hv.machine_info.entropy_source)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor entropy source is not allowed"),
            ));
        }

        // add memory related annotaion
        if change_hypervisor_config!(
            self.get_default_memory(hypervisor_name),
            hv.memory_info.default_memory
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor defualt memory is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_memory_slots(), hv.memory_info.memory_slots).is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor memory slot is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_enable_mem_prealloc(),
            hv.memory_info.enable_mem_prealloc
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor enable memory prealloc is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_enable_hugepages(), hv.memory_info.enable_hugepages)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor enable huge page is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_file_mem_backend(), hv.memory_info.file_mem_backend)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor memory backend is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_enable_virtio_mem(),
            hv.memory_info.enable_virtio_mem
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor enable virtio mem is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_enable_swap(), hv.memory_info.enable_swap).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor enable swap is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_enable_guest_swap(),
            hv.memory_info.enable_guest_swap
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor enable guest swap is not allowed"),
            ));
        }

        // add hypervisor network related annotation
        if change_hypervisor_config!(
            self.get_disable_vhost_net(),
            hv.network_info.disable_vhost_net
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor disable vhost net is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_rx_rate_limiter_max_rate(),
            hv.network_info.rx_rate_limiter_max_rate
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor rx rate limiter max rate is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_tx_rate_limiter_max_rate(),
            hv.network_info.tx_rate_limiter_max_rate
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor tx rate limiter max rate is not allowed"),
            ));
        }
        // add hypervisor security info related annotation
        if change_hypervisor_config!(self.get_guest_hook_path(), hv.security_info.guest_hook_path)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor guest hook path is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_enable_rootless_hypervisor(),
            hv.security_info.rootless
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor enable rootless hypervisor is not allowed"),
            ));
        }

        // add hypervisor shared file system related annotaion

        if change_hypervisor_config!(self.get_virtio_fs_daemon(), hv.shared_fs.virtio_fs_daemon)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor get share fs is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_virtio_fs_cache(), hv.shared_fs.virtio_fs_cache)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor virtio fs cache is not allowed"),
            ));
        }

        if change_hypervisor_config!(
            self.get_virtio_fs_cache_size(),
            hv.shared_fs.virtio_fs_cache_size
        )
        .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor virtio fs cache size is not allowed"),
            ));
        }

        if change_hypervisor_config!(self.get_hypervisor_msize_9p(), hv.shared_fs.msize_9p).is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("hypervisor virtio fs cache size is not allowed"),
            ));
        }

        // add runtime annotation
        let rt = &mut config.runtime;
        change_runtime_config!(self.get_disable_guest_seccomp(), rt.disable_guest_seccomp);
        change_runtime_config!(self.get_enable_pprof(), rt.enable_pprof);
        change_runtime_config!(self.get_network_model(), rt.internetworking_model);
        change_runtime_config!(self.get_sandbox_cgroup_only(), rt.disable_new_netns);
        change_runtime_config!(self.get_disable_new_netns(), rt.disable_new_netns);
        change_runtime_config!(self.get_vfio_mode(), rt.vfio_mode);
        self.add_annotation_experimental(config);
        Ok(())
    }
}
