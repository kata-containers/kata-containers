// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Result};
use std::u32;

use serde::Deserialize;

use crate::config::hypervisor::get_hypervisor_plugin;
use crate::config::TomlConfig;

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

// Hypervisor related annotations
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

// Hypervisor Block Device related annotations
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

// Hypervisor Guest Boot related annotations
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

// Hypervisor CPU related annotations
/// A sandbox annotation to specify cpu specific features.
pub const KATA_ANNO_CONF_HYPERVISOR_CPU_FEATURES: &str =
    "io.katacontainers.config.hypervisor.cpu_features";
/// A sandbox annotation for passing the default vcpus assigned for a VM by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_DEFAULT_VCPUS: &str =
    "io.katacontainers.config.hypervisor.default_vcpus";
/// A sandbox annotation that specifies the maximum number of vCPUs allocated for the VM by the hypervisor.
pub const KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MAX_VCPUS: &str =
    "io.katacontainers.config.hypervisor.default_max_vcpus";

// Hypervisor Device related annotations
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

// Hypervisor Memory related annotations
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

// Hypervisor Network related annotations
/// A sandbox annotation to specify if vhost-net is not available on the host.
pub const KATA_ANNO_CONF_HYPERVISOR_DISABLE_VHOST_NET: &str =
    "io.katacontainers.config.hypervisor.disable_vhost_net";
/// A sandbox annotation that specifies max rate on network I/O inbound bandwidth.
pub const KATA_ANNO_CONF_HYPERVISOR_RX_RATE_LIMITER_MAX_RATE: &str =
    "io.katacontainers.config.hypervisor.rx_rate_limiter_max_rate";
/// A sandbox annotation that specifies max rate on network I/O outbound bandwidth.
pub const KATA_ANNO_CONF_HYPERVISOR_TX_RATE_LIMITER_MAX_RATE: &str =
    "io.katacontainers.config.hypervisor.tx_rate_limiter_max_rate";

// Hypervisor Security related annotations
/// A sandbox annotation to specify the path within the VM that will be used for 'drop-in' hooks.
pub const KATA_ANNO_CONF_HYPERVISOR_GUEST_HOOK_PATH: &str =
    "io.katacontainers.config.hypervisor.guest_hook_path";
/// A sandbox annotation to enable rootless hypervisor (only supported in QEMU currently).
pub const KATA_ANNO_CONF_HYPERVISOR_ENABLE_ROOTLESS_HYPERVISOR: &str =
    "io.katacontainers.config.hypervisor.rootless";

// Hypervisor Shared File System related annotations
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
    pub fn get_annotations(&self) -> &HashMap<String, String> {
        &self.annotations
    }

    /// Get a mutable reference to the annotation hashmap.
    pub fn get_annotations_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.annotations
    }

    /// Get the value of annotation with `key` as string.
    pub fn get(&self, key: &str) -> Option<String> {
        self.annotations.get(key).map(|v| String::from(v.trim()))
    }

    /// Get the value of annotation with `key` as bool.
    pub fn get_bool(&self, key: &str) -> Result<Option<bool>> {
        if let Some(value) = self.get(key) {
            return value
                .parse::<bool>()
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid input {} for bool", key),
                    )
                })
                .map(Some);
        }
        Ok(None)
    }

    /// Get the value of annotation with `key` as u32.
    pub fn get_u32(&self, key: &str) -> Result<Option<u32>> {
        if let Some(value) = self.get(key) {
            return value
                .parse::<u32>()
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid input {} for u32", key),
                    )
                })
                .map(Some);
        }
        Ok(None)
    }

    /// Get the value of annotation with `key` as i32.
    pub fn get_i32(&self, key: &str) -> Result<Option<i32>> {
        if let Some(value) = self.get(key) {
            return value
                .parse::<i32>()
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid input {} for i32", key),
                    )
                })
                .map(Some);
        }
        Ok(None)
    }

    /// Get the value of annotation with `key` as u64.
    pub fn get_u64(&self, key: &str) -> Result<Option<u64>> {
        if let Some(value) = self.get(key) {
            return value
                .parse::<u64>()
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid input {} for u64", key),
                    )
                })
                .map(Some);
        }
        Ok(None)
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
    pub fn get_container_resource_swappiness(&self) -> Result<Option<u32>> {
        match self.get_u32(KATA_ANNO_CONTAINER_RESOURCE_SWAPPINESS) {
            Ok(r) => {
                if r.unwrap_or_default() > 100 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} greater than 100", r.unwrap_or_default()),
                    ));
                } else {
                    Ok(r)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Get the annotation to specify the Resources.Memory.Swap.
    pub fn get_container_resource_swap_in_bytes(&self) -> Option<String> {
        self.get(KATA_ANNO_CONTAINER_RESOURCE_SWAP_IN_BYTES)
    }
}

impl Annotation {
    /// update config info by annotation
    pub fn update_config_by_annotation(
        &self,
        config: &mut TomlConfig,
        hypervisor_name: &str,
        agent_name: &str,
    ) -> Result<()> {
        if config.hypervisor.get_mut(hypervisor_name).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("hypervisor {} not found", hypervisor_name),
            ));
        }

        if config.agent.get_mut(agent_name).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("agent {} not found", agent_name),
            ));
        }
        let mut hv = config.hypervisor.get_mut(hypervisor_name).unwrap();
        let mut ag = config.agent.get_mut(agent_name).unwrap();
        for (key, value) in &self.annotations {
            if hv.security_info.is_annotation_enabled(key) {
                match key.as_str() {
                    // update hypervisor config
                    //	Hypervisor related annotations
                    KATA_ANNO_CONF_HYPERVISOR_PATH => {
                        hv.validate_hypervisor_path(value)?;
                        hv.path = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_CTLPATH => {
                        hv.validate_hypervisor_ctlpath(value)?;
                        hv.ctlpath = value.to_string();
                    }

                    KATA_ANNO_CONF_HYPERVISOR_JAILER_PATH => {
                        hv.validate_jailer_path(value)?;
                        hv.jailer_path = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_ENABLE_IO_THREADS => match self.get_bool(key) {
                        Ok(r) => {
                            hv.enable_iothreads = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    //	Hypervisor Block Device related annotations
                    KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_DRIVER => {
                        hv.blockdev_info.block_device_driver = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_DISABLE_BLOCK_DEVICE_USE => {
                        match self.get_bool(key) {
                            Ok(r) => {
                                hv.blockdev_info.disable_block_device_use = r.unwrap_or_default();
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_SET => match self.get_bool(key) {
                        Ok(r) => {
                            hv.blockdev_info.block_device_cache_set = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_DIRECT => match self.get_bool(key)
                    {
                        Ok(r) => {
                            hv.blockdev_info.block_device_cache_direct = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_BLOCK_DEVICE_CACHE_NOFLUSH => {
                        match self.get_bool(key) {
                            Ok(r) => {
                                hv.blockdev_info.block_device_cache_noflush = r.unwrap_or_default();
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    KATA_ANNO_CONF_HYPERVISOR_DISABLE_IMAGE_NVDIMM => match self.get_bool(key) {
                        Ok(r) => {
                            hv.blockdev_info.disable_image_nvdimm = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_MEMORY_OFFSET => match self.get_u64(key) {
                        Ok(r) => {
                            hv.blockdev_info.memory_offset = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_ENABLE_VHOSTUSER_STORE => match self.get_bool(key) {
                        Ok(r) => {
                            hv.blockdev_info.enable_vhost_user_store = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_VHOSTUSER_STORE_PATH => {
                        hv.blockdev_info.validate_vhost_user_store_path(value)?;
                        hv.blockdev_info.vhost_user_store_path = value.to_string();
                    }
                    // Hypervisor Guest Boot related annotations
                    KATA_ANNO_CONF_HYPERVISOR_KERNEL_PATH => {
                        hv.boot_info.validate_boot_path(value)?;
                        hv.boot_info.kernel = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_KERNEL_PARAMS => {
                        hv.boot_info.kernel_params = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_IMAGE_PATH => {
                        hv.boot_info.validate_boot_path(value)?;
                        hv.boot_info.image = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_INITRD_PATH => {
                        hv.boot_info.validate_boot_path(value)?;
                        hv.boot_info.initrd = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_FIRMWARE_PATH => {
                        hv.boot_info.validate_boot_path(value)?;
                        hv.boot_info.firmware = value.to_string();
                    }
                    //	Hypervisor CPU related annotations
                    KATA_ANNO_CONF_HYPERVISOR_CPU_FEATURES => {
                        hv.cpu_info.cpu_features = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_DEFAULT_VCPUS => match self.get_i32(key) {
                        Ok(num_cpus) => {
                            let num_cpus = num_cpus.unwrap_or_default();
                            if num_cpus
                                > get_hypervisor_plugin(hypervisor_name)
                                    .unwrap()
                                    .get_max_cpus() as i32
                            {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!(
                                        "Vcpus specified in annotation {} is more than maximum limitation {}",
                                        num_cpus,
                                        get_hypervisor_plugin(hypervisor_name)
                                            .unwrap()
                                            .get_max_cpus()
                                    ),
                                ));
                            } else {
                                hv.cpu_info.default_vcpus = num_cpus;
                            }
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MAX_VCPUS => match self.get_u32(key) {
                        Ok(r) => {
                            hv.cpu_info.default_maxvcpus = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    //	Hypervisor Device related annotations
                    KATA_ANNO_CONF_HYPERVISOR_HOTPLUG_VFIO_ON_ROOT_BUS => {
                        match self.get_bool(key) {
                            Ok(r) => {
                                hv.device_info.hotplug_vfio_on_root_bus = r.unwrap_or_default();
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    KATA_ANNO_CONF_HYPERVISOR_PCIE_ROOT_PORT => match self.get_u32(key) {
                        Ok(r) => {
                            hv.device_info.pcie_root_port = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_IOMMU => match self.get_bool(key) {
                        Ok(r) => {
                            hv.device_info.enable_iommu = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_IOMMU_PLATFORM => match self.get_bool(key) {
                        Ok(r) => {
                            hv.device_info.enable_iommu_platform = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    //	Hypervisor Machine related annotations
                    KATA_ANNO_CONF_HYPERVISOR_MACHINE_TYPE => {
                        hv.machine_info.machine_type = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_MACHINE_ACCELERATORS => {
                        hv.machine_info.machine_accelerators = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_ENTROPY_SOURCE => {
                        hv.machine_info.validate_entropy_source(value)?;
                        hv.machine_info.entropy_source = value.to_string();
                    }
                    //	Hypervisor Memory related annotations
                    KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MEMORY => match self.get_u32(key) {
                        Ok(r) => {
                            let mem = r.unwrap_or_default();
                            if mem
                                < get_hypervisor_plugin(hypervisor_name)
                                    .unwrap()
                                    .get_min_memory()
                            {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!(
                                        "Memory specified in annotation {} is less than minimum required {}",
                                        mem,
                                        get_hypervisor_plugin(hypervisor_name)
                                            .unwrap()
                                            .get_min_memory()
                                    ),
                                ));
                            } else {
                                hv.memory_info.default_memory = mem;
                            }
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_MEMORY_SLOTS => match self.get_u32(key) {
                        Ok(v) => {
                            hv.memory_info.memory_slots = v.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },

                    KATA_ANNO_CONF_HYPERVISOR_MEMORY_PREALLOC => match self.get_bool(key) {
                        Ok(r) => {
                            hv.memory_info.enable_mem_prealloc = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_HUGE_PAGES => match self.get_bool(key) {
                        Ok(r) => {
                            hv.memory_info.enable_hugepages = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_FILE_BACKED_MEM_ROOT_DIR => {
                        hv.memory_info.validate_memory_backend_path(value)?;
                        hv.memory_info.file_mem_backend = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_VIRTIO_MEM => match self.get_bool(key) {
                        Ok(r) => {
                            hv.memory_info.enable_virtio_mem = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_ENABLE_SWAP => match self.get_bool(key) {
                        Ok(r) => {
                            hv.memory_info.enable_swap = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_ENABLE_GUEST_SWAP => match self.get_bool(key) {
                        Ok(r) => {
                            hv.memory_info.enable_guest_swap = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    //	Hypervisor Network related annotations
                    KATA_ANNO_CONF_HYPERVISOR_DISABLE_VHOST_NET => match self.get_bool(key) {
                        Ok(r) => {
                            hv.network_info.disable_vhost_net = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_RX_RATE_LIMITER_MAX_RATE => match self.get_u64(key) {
                        Ok(r) => {
                            hv.network_info.rx_rate_limiter_max_rate = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_TX_RATE_LIMITER_MAX_RATE => match self.get_u64(key) {
                        Ok(r) => {
                            hv.network_info.tx_rate_limiter_max_rate = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    //	Hypervisor Security related annotations
                    KATA_ANNO_CONF_HYPERVISOR_GUEST_HOOK_PATH => {
                        hv.security_info.validate_path(value)?;
                        hv.security_info.guest_hook_path = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_ENABLE_ROOTLESS_HYPERVISOR => {
                        match self.get_bool(key) {
                            Ok(r) => {
                                hv.security_info.rootless = r.unwrap_or_default();
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    //	Hypervisor Shared File System related annotations
                    KATA_ANNO_CONF_HYPERVISOR_SHARED_FS => {
                        hv.shared_fs.shared_fs = self.get(key);
                    }

                    KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_DAEMON => {
                        hv.shared_fs.validate_virtiofs_daemon_path(value)?;
                        hv.shared_fs.virtio_fs_daemon = value.to_string();
                    }

                    KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_CACHE => {
                        hv.shared_fs.virtio_fs_cache = value.to_string();
                    }
                    KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_CACHE_SIZE => match self.get_u32(key) {
                        Ok(r) => {
                            hv.shared_fs.virtio_fs_cache_size = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_HYPERVISOR_VIRTIO_FS_EXTRA_ARGS => {
                        let args: Vec<String> =
                            value.to_string().split(',').map(str::to_string).collect();
                        for arg in args {
                            hv.shared_fs.virtio_fs_extra_args.push(arg.to_string());
                        }
                    }
                    KATA_ANNO_CONF_HYPERVISOR_MSIZE_9P => match self.get_u32(key) {
                        Ok(v) => {
                            hv.shared_fs.msize_9p = v.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },

                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("Invalid annotation type {}", key),
                        ));
                    }
                }
            } else {
                match key.as_str() {
                    //update agent config
                    KATA_ANNO_CONF_KERNEL_MODULES => {
                        let kernel_mod: Vec<String> =
                            value.to_string().split(';').map(str::to_string).collect();
                        for modules in kernel_mod {
                            ag.kernel_modules.push(modules.to_string());
                        }
                    }
                    KATA_ANNO_CONF_AGENT_TRACE => match self.get_bool(key) {
                        Ok(r) => {
                            ag.enable_tracing = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_AGENT_CONTAINER_PIPE_SIZE => match self.get_u32(key) {
                        Ok(v) => {
                            ag.container_pipe_size = v.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    //update runtume config
                    KATA_ANNO_CONF_DISABLE_GUEST_SECCOMP => match self.get_bool(key) {
                        Ok(r) => {
                            config.runtime.disable_guest_seccomp = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_ENABLE_PPROF => match self.get_bool(key) {
                        Ok(r) => {
                            config.runtime.enable_pprof = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_EXPERIMENTAL => {
                        let args: Vec<String> =
                            value.to_string().split(',').map(str::to_string).collect();
                        for arg in args {
                            config.runtime.experimental.push(arg.to_string());
                        }
                    }
                    KATA_ANNO_CONF_INTER_NETWORK_MODEL => {
                        config.runtime.internetworking_model = value.to_string();
                    }
                    KATA_ANNO_CONF_SANDBOX_CGROUP_ONLY => match self.get_bool(key) {
                        Ok(r) => {
                            config.runtime.sandbox_cgroup_only = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_DISABLE_NEW_NETNS => match self.get_bool(key) {
                        Ok(r) => {
                            config.runtime.disable_new_netns = r.unwrap_or_default();
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    KATA_ANNO_CONF_VFIO_MODE => {
                        config.runtime.vfio_mode = value.to_string();
                    }
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Annotation {} not enabled", key),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
