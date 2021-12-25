// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Result};

use serde::Deserialize;

use crate::config::KataConfig;
use crate::{eother, sl};

/// CRI-containerd specific annotations.
pub mod cri_containerd;

/// CRI-O specific annotations.
pub mod crio;

/// Dockershim specific annotations.
pub mod dockershim;

/// Third-party annotations.
pub mod thirdparty;

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

        if !value.is_empty() {
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
}

/// Generic hypervisor related annotations.
impl Annotation {
    fn check_allowed_hypervisor_annotation(&self, id: &str) -> Result<()> {
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            if hv.security_info.is_annotation_enabled(id) {
                return Ok(());
            }
        }

        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{}{} is not allowed", KATA_ANNO_CONF_HYPERVISOR_PREFIX, id),
        ))
    }

    /// Get and validate the annotation for `config.hypervisor.path`.
    pub fn get_hypervisor_path(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("path")?;
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
        self.check_allowed_hypervisor_annotation("ctlpath")?;
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
        self.check_allowed_hypervisor_annotation("jailer_path")?;
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
        self.check_allowed_hypervisor_annotation("enable_io_threads")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_ENABLE_IO_THREADS))
    }

    /// Get the annotation of the hash algorithm type used for assets verification
    pub fn get_asset_hash_type(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_ASSET_HASH_TYPE)
    }
}

// Hypervisor block storage related annotations.
impl Annotation {
    /// Get the annotation for `config.hypervisor.block_device_driver`
    pub fn get_block_device_driver(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("block_device_driver")?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_DRIVER))
    }

    /// Get the annotation for `config.hypervisor.disable_block_device_use`
    pub fn get_disable_block_device_use(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation("disable_block_device_use")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_DISABLE_BLOCK_DEVICE_USE))
    }

    /// Get the annotation for `config.hypervisor.block_device_cache_set`
    pub fn get_block_device_cache_set(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation("block_device_cache_set")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_SET))
    }

    /// Get the annotation for `config.hypervisor.block_device_cache_direct`
    pub fn get_block_device_cache_direct(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation("block_device_cache_direct")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_DIRECT))
    }

    /// Get the annotation for `config.hypervisor.block_device_cache_noflush`
    pub fn get_block_device_cache_noflush(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation("block_device_cache_direct")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_NOFLUSH))
    }

    /// Get the annotation for `config.hypervisor.disable_image_nvdimm`
    pub fn get_disable_image_nvdimm(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation("disable_image_nvdimm")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_DISABLE_IMAGE_NVDIMM))
    }

    /// Get the annotation for `config.hypervisor.memory_offset`
    pub fn get_memory_offset(&self) -> Result<Option<u64>> {
        self.check_allowed_hypervisor_annotation("memory_offset")?;
        Ok(self.get_u64(KATA_ANNO_CONF_HYPERVISOR_MEMORY_OFFSET))
    }

    /// Get the annotation for `config.hypervisor.enable_vhost_user_store`
    pub fn get_enable_vhost_user_store(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation("enable_vhost_user_store")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_ENABLE_VHOSTUSER_STORE))
    }

    /// Get and validate the annotation for `config.hypervisor.vhost_user_store_path`
    pub fn get_vhost_user_store_path(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("enable_vhost_user_store")?;
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
        self.check_allowed_hypervisor_annotation("kernel")?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_KERNEL_PATH))
    }

    /// Get the annotation for hash value of guest kernel file path.
    pub fn get_kernel_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_KERNEL_HASH)
    }

    /// Get the annotation for `config.hypervisor.kernel_params`.
    pub fn get_kernel_params(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("kernel_params")?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_KERNEL_PARAMS))
    }

    /// Get the annotation for `config.hypervisor.image`.
    pub fn get_image(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("image")?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_IMAGE_PATH))
    }

    /// Get the annotation for hash value of guest boot image file path.
    pub fn get_image_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_IMAGE_HASH)
    }

    /// Get the annotation for `config.hypervisor.initrd`.
    pub fn get_initrd(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("initrd")?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_INITRD_PATH))
    }

    /// Get the annotation for hash value of guest initrd file path.
    pub fn get_initrd_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_INITRD_HASH)
    }

    /// Get the annotation for `config.hypervisor.firmware`.
    pub fn get_firmware(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("firmware")?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_FIRMWARE_PATH))
    }

    /// Get the annotation for hash value of firmware file path.
    pub fn get_firmware_hash(&self) -> Option<String> {
        self.get(KATA_ANNO_CONF_HYPERVISOR_FIRMWARE_HASH)
    }
}

// VM CPU related annotations.
impl Annotation {
    /// Get the annotation for "config.hypervisor.cpu_features".
    pub fn get_cpu_features(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("cpu_features")?;
        Ok(self.get(KATA_ANNO_CONF_HYPERVISOR_CPU_FEATURES))
    }

    /// Get the annotation for "config.hypervisor.default_vcpus".
    pub fn get_default_vcpus(&self) -> Result<Option<i32>> {
        self.check_allowed_hypervisor_annotation("default_vcpus")?;
        Ok(self.get_i32(KATA_ANNO_CONF_HYPERVISOR_DEFAULT_VCPUS))
    }

    /// Get the annotation for "config.hypervisor.default_max_vcpus".
    pub fn get_default_max_vcpus(&self) -> Result<Option<u32>> {
        self.check_allowed_hypervisor_annotation("default_max_vcpus")?;
        Ok(self.get_u32(KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MAX_VCPUS))
    }
}

// VM Network related annotations.
impl Annotation {
    /// Get the annotation for `config.hypervisor.disable_vhost_net`.
    pub fn get_disable_vhost_net(&self) -> Result<Option<bool>> {
        self.check_allowed_hypervisor_annotation("disable_vhost_net")?;
        Ok(self.get_bool(KATA_ANNO_CONF_HYPERVISOR_DISABLE_VHOST_NET))
    }

    /// Get the annotation for `config.hypervisor.rx_rate_limiter_max_rate`.
    pub fn get_rx_rate_limiter_max_rate(&self) -> Result<Option<u64>> {
        self.check_allowed_hypervisor_annotation("rx_rate_limiter_max_rate")?;
        Ok(self.get_u64(KATA_ANNO_CONF_HYPERVISOR_RX_RATE_LIMITER_MAX_RATE))
    }

    /// Get the annotation for `config.hypervisor.tx_rate_limiter_max_rate`.
    pub fn get_tx_rate_limiter_max_rate(&self) -> Result<Option<u64>> {
        self.check_allowed_hypervisor_annotation("tx_rate_limiter_max_rate")?;
        Ok(self.get_u64(KATA_ANNO_CONF_HYPERVISOR_TX_RATE_LIMITER_MAX_RATE))
    }
}

impl Annotation {
    //<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
    /// Get and validate annotation for entropy source.
    pub fn get_entropy_source(&self) -> Result<Option<String>> {
        self.check_allowed_hypervisor_annotation("entropy_source")?;
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
        self.check_allowed_hypervisor_annotation("file_mem_backend")?;
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
    //>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
}
