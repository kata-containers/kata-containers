use crate::error::{oci_error, OciSpecError};

use derive_builder::Builder;
use getset::{CopyGetters, Getters, Setters};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::TryFrom, path::PathBuf};

#[derive(Builder, Clone, Debug, Deserialize, Eq, Getters, Setters, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// Linux contains platform-specific configuration for Linux based
/// containers.
pub struct Linux {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// UIDMappings specifies user mappings for supporting user namespaces.
    uid_mappings: Option<Vec<LinuxIdMapping>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// GIDMappings specifies group mappings for supporting user namespaces.
    gid_mappings: Option<Vec<LinuxIdMapping>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Sysctl are a set of key value pairs that are set for the container
    /// on start.
    sysctl: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Resources contain cgroup information for handling resource
    /// constraints for the container.
    resources: Option<LinuxResources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CgroupsPath specifies the path to cgroups that are created and/or
    /// joined by the container. The path is expected to be relative
    /// to the cgroups mountpoint. If resources are specified,
    /// the cgroups at CgroupsPath will be updated based on resources.
    cgroups_path: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Namespaces contains the namespaces that are created and/or joined by
    /// the container.
    namespaces: Option<Vec<LinuxNamespace>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Devices are a list of device nodes that are created for the
    /// container.
    devices: Option<Vec<LinuxDevice>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seccomp specifies the seccomp security settings for the container.
    seccomp: Option<LinuxSeccomp>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// RootfsPropagation is the rootfs mount propagation mode for the
    /// container.
    rootfs_propagation: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// MaskedPaths masks over the provided paths inside the container.
    masked_paths: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// ReadonlyPaths sets the provided paths as RO inside the container.
    readonly_paths: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// MountLabel specifies the selinux context for the mounts in the
    /// container.
    mount_label: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// IntelRdt contains Intel Resource Director Technology (RDT)
    /// information for handling resource constraints (e.g., L3
    /// cache, memory bandwidth) for the container.
    intel_rdt: Option<LinuxIntelRdt>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Personality contains configuration for the Linux personality
    /// syscall.
    personality: Option<LinuxPersonality>,
}

// Default impl for Linux (see funtions for more info)
impl Default for Linux {
    fn default() -> Self {
        Linux {
            // Creates empty Vec
            uid_mappings: Default::default(),
            // Creates empty Vec
            gid_mappings: Default::default(),
            // Empty sysctl Hashmap
            sysctl: Default::default(),
            resources: Some(LinuxResources {
                devices: vec![LinuxDeviceCgroup {
                    access: "rwm".to_string().into(),
                    allow: false,
                    typ: Default::default(),
                    major: Default::default(),
                    minor: Default::default(),
                }]
                .into(),
                memory: Default::default(),
                cpu: Default::default(),
                pids: Default::default(),
                block_io: Default::default(),
                hugepage_limits: Default::default(),
                network: Default::default(),
                rdma: Default::default(),
                unified: Default::default(),
            }),
            // Defaults to None
            cgroups_path: Default::default(),
            namespaces: get_default_namespaces().into(),
            // Empty Vec
            devices: Default::default(),
            // Empty String
            rootfs_propagation: Default::default(),
            masked_paths: get_default_maskedpaths().into(),
            readonly_paths: get_default_readonly_paths().into(),
            // Empty String
            mount_label: Default::default(),
            seccomp: None,
            intel_rdt: None,
            personality: None,
        }
    }
}

#[derive(
    Builder, Clone, Copy, CopyGetters, Debug, Default, Deserialize, Eq, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// LinuxIDMapping specifies UID/GID mappings.
pub struct LinuxIdMapping {
    #[serde(default, rename = "hostID")]
    /// HostID is the starting UID/GID on the host to be mapped to
    /// `container_id`.
    host_id: u32,

    #[serde(default, rename = "containerID")]
    /// ContainerID is the starting UID/GID in the container.
    container_id: u32,

    #[serde(default)]
    /// Size is the number of IDs to be mapped.
    size: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
/// Device types
pub enum LinuxDeviceType {
    /// All
    A,

    /// block (buffered)
    B,

    /// character (unbuffered)
    C,

    /// character (unbufferd)
    U,

    /// FIFO
    P,
}

impl Default for LinuxDeviceType {
    fn default() -> LinuxDeviceType {
        LinuxDeviceType::A
    }
}

impl LinuxDeviceType {
    /// Retrieve a string reference for the device type.
    pub fn as_str(&self) -> &str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::U => "u",
            Self::P => "p",
        }
    }
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// Represents a device rule for the devices specified to the device
/// controller
pub struct LinuxDeviceCgroup {
    #[serde(default)]
    #[getset(get_copy = "pub", set = "pub")]
    /// Allow or deny
    allow: bool,

    #[serde(default, rename = "type")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Device type, block, char, etc.
    typ: Option<LinuxDeviceType>,

    #[getset(get_copy = "pub", set = "pub")]
    /// Device's major number
    major: Option<i64>,

    #[getset(get_copy = "pub", set = "pub")]
    /// Device's minor number
    minor: Option<i64>,

    /// Cgroup access premissions format, rwm.
    #[serde(default)]
    #[getset(get = "pub", set = "pub")]
    access: Option<String>,
}

impl ToString for LinuxDeviceCgroup {
    fn to_string(&self) -> String {
        let major = self
            .major
            .map(|mj| mj.to_string())
            .unwrap_or_else(|| "*".to_string());
        let minor = self
            .minor
            .map(|mi| mi.to_string())
            .unwrap_or_else(|| "*".to_string());
        let access = self.access.as_deref().unwrap_or("");
        format!(
            "{} {}:{} {}",
            &self.typ.unwrap_or_default().as_str(),
            &major,
            &minor,
            &access
        )
    }
}

#[derive(
    Builder, Clone, Copy, CopyGetters, Debug, Default, Deserialize, Eq, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// LinuxMemory for Linux cgroup 'memory' resource management.
pub struct LinuxMemory {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Memory limit (in bytes).
    limit: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Memory reservation or soft_limit (in bytes).
    reservation: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Total memory limit (memory + swap).
    swap: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Kernel memory limit (in bytes).
    kernel: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "kernelTCP")]
    /// Kernel memory limit for tcp (in bytes).
    kernel_tcp: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// How aggressive the kernel will swap memory pages.
    swappiness: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "disableOOMKiller")]
    /// DisableOOMKiller disables the OOM killer for out of memory
    /// conditions.
    disable_oom_killer: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Enables hierarchical memory accounting
    use_hierarchy: Option<bool>,
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxCPU for Linux cgroup 'cpu' resource management.
pub struct LinuxCpu {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// CPU shares (relative weight (ratio) vs. other cgroups with cpu
    /// shares).
    shares: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// CPU hardcap limit (in usecs). Allowed cpu time in a given period.
    quota: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Cgroups are configured with minimum weight, 0: default behavior, 1: SCHED_IDLE.
    idle: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Maximum amount of accumulated time in microseconds for which tasks
    /// in a cgroup can run additionally for burst during one period
    burst: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// CPU period to be used for hardcapping (in usecs).
    period: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// How much time realtime scheduling may use (in usecs).
    realtime_runtime: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// CPU period to be used for realtime scheduling (in usecs).
    realtime_period: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// CPUs to use within the cpuset. Default is to use any CPU available.
    cpus: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// List of memory nodes in the cpuset. Default is to use any available
    /// memory node.
    mems: Option<String>,
}

#[derive(
    Builder,
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Eq,
    CopyGetters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// LinuxPids for Linux cgroup 'pids' resource management (Linux 4.3).
pub struct LinuxPids {
    #[serde(default)]
    /// Maximum number of PIDs. Default is "no limit".
    limit: i64,
}

#[derive(
    Builder, Clone, Copy, CopyGetters, Debug, Default, Deserialize, Eq, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// LinuxWeightDevice struct holds a `major:minor weight` pair for
/// weightDevice.
pub struct LinuxWeightDevice {
    #[serde(default)]
    /// Major is the device's major number.
    major: i64,

    #[serde(default)]
    /// Minor is the device's minor number.
    minor: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Weight is the bandwidth rate for the device.
    weight: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// LeafWeight is the bandwidth rate for the device while competing with
    /// the cgroup's child cgroups, CFQ scheduler only.
    leaf_weight: Option<u16>,
}

#[derive(
    Builder, Clone, Copy, CopyGetters, Debug, Default, Deserialize, Eq, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// LinuxThrottleDevice struct holds a `major:minor rate_per_second` pair.
pub struct LinuxThrottleDevice {
    #[serde(default)]
    /// Major is the device's major number.
    major: i64,

    #[serde(default)]
    /// Minor is the device's minor number.
    minor: i64,

    #[serde(default)]
    /// Rate is the IO rate limit per cgroup per device.
    rate: u64,
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxBlockIO for Linux cgroup 'blkio' resource management.
pub struct LinuxBlockIo {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Specifies per cgroup weight.
    weight: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Specifies tasks' weight in the given cgroup while competing with the
    /// cgroup's child cgroups, CFQ scheduler only.
    leaf_weight: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Weight per cgroup per device, can override BlkioWeight.
    weight_device: Option<Vec<LinuxWeightDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// IO read rate limit per cgroup per device, bytes per second.
    throttle_read_bps_device: Option<Vec<LinuxThrottleDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// IO write rate limit per cgroup per device, bytes per second.
    throttle_write_bps_device: Option<Vec<LinuxThrottleDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// IO read rate limit per cgroup per device, IO per second.
    throttle_read_iops_device: Option<Vec<LinuxThrottleDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// IO write rate limit per cgroup per device, IO per second.
    throttle_write_iops_device: Option<Vec<LinuxThrottleDevice>>,
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxHugepageLimit structure corresponds to limiting kernel hugepages.
pub struct LinuxHugepageLimit {
    #[serde(default)]
    #[getset(get = "pub", set = "pub")]
    /// Pagesize is the hugepage size.
    /// Format: "<size><unit-prefix>B' (e.g. 64KB, 2MB, 1GB, etc.)
    page_size: String,

    #[serde(default)]
    #[getset(get_copy = "pub", set = "pub")]
    /// Limit is the limit of "hugepagesize" hugetlb usage.
    limit: i64,
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxInterfacePriority for network interfaces.
pub struct LinuxInterfacePriority {
    #[serde(default)]
    #[getset(get = "pub", set = "pub")]
    /// Name is the name of the network interface.
    name: String,

    #[serde(default)]
    #[getset(get_copy = "pub", set = "pub")]
    /// Priority for the interface.
    priority: u32,
}

impl ToString for LinuxInterfacePriority {
    fn to_string(&self) -> String {
        format!("{} {}\n", self.name, self.priority)
    }
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxNetwork identification and priority configuration.
pub struct LinuxNetwork {
    #[serde(skip_serializing_if = "Option::is_none", rename = "classID")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Set class identifier for container's network packets
    class_id: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Set priority of network traffic for container.
    priorities: Option<Vec<LinuxInterfacePriority>>,
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// Resource constraints for container
pub struct LinuxResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Devices configures the device allowlist.
    devices: Option<Vec<LinuxDeviceCgroup>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Memory restriction configuration.
    memory: Option<LinuxMemory>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// CPU resource restriction configuration.
    cpu: Option<LinuxCpu>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Task resource restrictions
    pids: Option<LinuxPids>,

    #[serde(default, skip_serializing_if = "Option::is_none", rename = "blockIO")]
    #[getset(get = "pub", set = "pub")]
    /// BlockIO restriction configuration.
    block_io: Option<LinuxBlockIo>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Hugetlb limit (in bytes).
    hugepage_limits: Option<Vec<LinuxHugepageLimit>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Network restriction configuration.
    network: Option<LinuxNetwork>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Rdma resource restriction configuration. Limits are a set of key
    /// value pairs that define RDMA resource limits, where the key
    /// is device name and value is resource limits.
    rdma: Option<HashMap<String, LinuxRdma>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Unified resources.
    unified: Option<HashMap<String, String>>,
}

#[derive(
    Builder, Clone, Copy, CopyGetters, Debug, Default, Deserialize, Eq, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// LinuxRdma for Linux cgroup 'rdma' resource management (Linux 4.11).
pub struct LinuxRdma {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Maximum number of HCA handles that can be opened. Default is "no
    /// limit".
    hca_handles: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Maximum number of HCA objects that can be created. Default is "no
    /// limit".
    hca_objects: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, Hash)]
#[serde(rename_all = "snake_case")]
/// Available Linux namespaces.
pub enum LinuxNamespaceType {
    /// Mount Namespace for isolating mount points
    Mount = 0x00020000,

    /// Cgroup Namespace for isolating cgroup hierarchies
    Cgroup = 0x02000000,

    /// Uts Namespace for isolating hostname and NIS domain name
    Uts = 0x04000000,

    /// Ipc Namespace for isolating System V, IPC, POSIX message queues
    Ipc = 0x08000000,

    /// User Namespace for isolating user and group  ids
    User = 0x10000000,

    /// PID Namespace for isolating process ids
    Pid = 0x20000000,

    /// Network Namespace for isolating network devices, ports, stacks etc.
    Network = 0x40000000,
}

impl TryFrom<&str> for LinuxNamespaceType {
    type Error = OciSpecError;

    fn try_from(namespace: &str) -> Result<Self, Self::Error> {
        match namespace {
            "mnt" => Ok(LinuxNamespaceType::Mount),
            "cgroup" => Ok(LinuxNamespaceType::Cgroup),
            "uts" => Ok(LinuxNamespaceType::Uts),
            "ipc" => Ok(LinuxNamespaceType::Ipc),
            "user" => Ok(LinuxNamespaceType::User),
            "pid" => Ok(LinuxNamespaceType::Pid),
            "net" => Ok(LinuxNamespaceType::Network),
            _ => Err(oci_error(format!(
                "unknown namespace {}, could not convert",
                namespace
            ))),
        }
    }
}

impl Default for LinuxNamespaceType {
    fn default() -> Self {
        Self::Pid
    }
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxNamespace is the configuration for a Linux namespace.
pub struct LinuxNamespace {
    #[serde(rename = "type")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Type is the type of namespace.
    typ: LinuxNamespaceType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Path is a path to an existing namespace persisted on disk that can
    /// be joined and is of the same type
    path: Option<PathBuf>,
}

/// Utility function to get default namespaces.
pub fn get_default_namespaces() -> Vec<LinuxNamespace> {
    vec![
        LinuxNamespace {
            typ: LinuxNamespaceType::Pid,
            path: Default::default(),
        },
        LinuxNamespace {
            typ: LinuxNamespaceType::Network,
            path: Default::default(),
        },
        LinuxNamespace {
            typ: LinuxNamespaceType::Ipc,
            path: Default::default(),
        },
        LinuxNamespace {
            typ: LinuxNamespaceType::Uts,
            path: Default::default(),
        },
        LinuxNamespace {
            typ: LinuxNamespaceType::Mount,
            path: Default::default(),
        },
    ]
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxDevice represents the mknod information for a Linux special device
/// file.
pub struct LinuxDevice {
    #[serde(default)]
    #[getset(get = "pub", set = "pub")]
    /// Path to the device.
    path: PathBuf,

    #[serde(rename = "type")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Device type, block, char, etc..
    typ: LinuxDeviceType,

    #[serde(default)]
    #[getset(get_copy = "pub", set = "pub")]
    /// Major is the device's major number.
    major: i64,

    #[serde(default)]
    #[getset(get_copy = "pub", set = "pub")]
    /// Minor is the device's minor number.
    minor: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// FileMode permission bits for the device.
    file_mode: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// UID of the device.
    uid: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// Gid of the device.
    gid: Option<u32>,
}

impl From<&LinuxDevice> for LinuxDeviceCgroup {
    fn from(linux_device: &LinuxDevice) -> LinuxDeviceCgroup {
        LinuxDeviceCgroup {
            allow: true,
            typ: linux_device.typ.into(),
            major: Some(linux_device.major as i64),
            minor: Some(linux_device.minor as i64),
            access: "rwm".to_string().into(),
        }
    }
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxSeccomp represents syscall restrictions.
pub struct LinuxSeccomp {
    #[getset(get_copy = "pub", set = "pub")]
    /// The default action to be done.
    default_action: LinuxSeccompAction,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// The default error return code to use when the default action is SCMP_ACT_ERRNO.
    default_errno_ret: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Available architectures for the restriction.
    architectures: Option<Vec<Arch>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Flags added to the seccomp restriction.
    flags: Option<Vec<LinuxSeccompFilterFlag>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// The unix domain socket path over which runtime will use for `SCMP_ACT_NOTIFY`.
    listener_path: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// An opaque data to pass to the seccomp agent.
    listener_metadata: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// The syscalls for the restriction.
    syscalls: Option<Vec<LinuxSyscall>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u32)]
/// Available seccomp actions.
pub enum LinuxSeccompAction {
    /// Kill the thread, defined for backward compatibility.
    ScmpActKill = 0x00000000,

    /// Kill the process.
    ScmpActKillProcess = 0x80000000,

    /// Throw a SIGSYS signal.
    ScmpActTrap = 0x00030000,

    /// Return the specified error code.
    ScmpActErrno = 0x00050001,

    /// Notifies userspace.
    ScmpActNotify = 0x7fc00000,

    /// Notify a tracing process with the specified value.
    ScmpActTrace = 0x7ff00001,

    /// Allow the syscall to be executed after the action has been logged.
    ScmpActLog = 0x7ffc0000,

    /// Allow the syscall to be executed.
    ScmpActAllow = 0x7fff0000,
}

impl Default for LinuxSeccompAction {
    fn default() -> Self {
        Self::ScmpActAllow
    }
}

#[allow(clippy::enum_clike_unportable_variant)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
/// Available seccomp architectures.
pub enum Arch {
    /// The native architecture.
    ScmpArchNative = 0x00000000,

    /// The x86 (32-bit) architecture.
    ScmpArchX86 = 0x40000003,

    /// The x86-64 (64-bit) architecture.
    ScmpArchX86_64 = 0xc000003e,

    /// The x32 (32-bit x86_64) architecture.
    ///
    /// This is different from the value used by the kernel because we need to
    /// be able to distinguish between x32 and x86_64.
    ScmpArchX32 = 0x4000003e,

    /// The ARM architecture.
    ScmpArchArm = 0x40000028,

    /// The AArch64 architecture.
    ScmpArchAarch64 = 0xc00000b7,

    /// The MIPS architecture.
    ScmpArchMips = 0x00000008,

    /// The MIPS64 architecture.
    ScmpArchMips64 = 0x80000008,

    /// The MIPS64n32 architecture.
    ScmpArchMips64n32 = 0xa0000008,

    /// The MIPSel architecture.
    ScmpArchMipsel = 0x40000008,

    /// The MIPSel64 architecture.
    ScmpArchMipsel64 = 0xc0000008,

    /// The MIPSel64n32 architecture.
    ScmpArchMipsel64n32 = 0xe0000008,

    /// The PowerPC architecture.
    ScmpArchPpc = 0x00000014,

    /// The PowerPC64 architecture.
    ScmpArchPpc64 = 0x80000015,

    /// The PowerPC64le architecture.
    ScmpArchPpc64le = 0xc0000015,

    /// The S390 architecture.
    ScmpArchS390 = 0x00000016,

    /// The S390x architecture.
    ScmpArchS390x = 0x80000016,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
/// Available seccomp filter flags.
pub enum LinuxSeccompFilterFlag {
    /// All filter return actions except SECCOMP_RET_ALLOW should be logged. An administrator may
    /// override this filter flag by preventing specific actions from being logged via the
    /// /proc/sys/kernel/seccomp/actions_logged file. (since Linux 4.14)
    SeccompFilterFlagLog,

    /// When adding a new filter, synchronize all other threads of the calling process to the same
    /// seccomp filter tree. A "filter tree" is the ordered list of filters attached to a thread.
    /// (Attaching identical filters in separate seccomp() calls results in different filters from this
    /// perspective.)
    ///
    /// If any thread cannot synchronize to the same filter tree, the call will not attach the new
    /// seccomp filter, and will fail, returning the first thread ID found that cannot synchronize.
    /// Synchronization will fail if another thread in the same process is in SECCOMP_MODE_STRICT or if
    /// it has attached new seccomp filters to itself, diverging from the calling thread's filter tree.
    SeccompFilterFlagTsync,

    /// Disable Speculative Store Bypass mitigation. (since Linux 4.17)
    SeccompFilterFlagSpecAllow,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u32)]
/// The seccomp operator to be used for args.
pub enum LinuxSeccompOperator {
    /// Refers to the SCMP_CMP_NE operator (not equal).
    ScmpCmpNe = 1,

    /// Refers to the SCMP_CMP_LT operator (less than).
    ScmpCmpLt = 2,

    /// Refers to the SCMP_CMP_LE operator (less equal).
    ScmpCmpLe = 3,

    /// Refers to the SCMP_CMP_EQ operator (equal to).
    ScmpCmpEq = 4,

    /// Refers to the SCMP_CMP_GE operator (greater equal).
    ScmpCmpGe = 5,

    /// Refers to the SCMP_CMP_GT operator (greater than).
    ScmpCmpGt = 6,

    /// Refers to the SCMP_CMP_MASKED_EQ operator (masked equal).
    ScmpCmpMaskedEq = 7,
}

impl Default for LinuxSeccompOperator {
    fn default() -> Self {
        Self::ScmpCmpEq
    }
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxSyscall is used to match a syscall in seccomp.
pub struct LinuxSyscall {
    #[getset(get = "pub", set = "pub")]
    /// The names of the syscalls.
    names: Vec<String>,

    #[getset(get_copy = "pub", set = "pub")]
    /// The action to be done for the syscalls.
    action: LinuxSeccompAction,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    /// The error return value.
    errno_ret: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// The arguments for the syscalls.
    args: Option<Vec<LinuxSeccompArg>>,
}

#[derive(
    Builder, Clone, Copy, CopyGetters, Debug, Default, Deserialize, Eq, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get_copy = "pub", set = "pub")]
/// LinuxSeccompArg used for matching specific syscall arguments in seccomp.
pub struct LinuxSeccompArg {
    /// The index of the argument.
    index: usize,

    /// The value of the argument.
    value: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The second value of the argument.
    value_two: Option<u64>,

    /// The operator for the argument.
    op: LinuxSeccompOperator,
}

/// Default masks paths, cannot read these host files.
pub fn get_default_maskedpaths() -> Vec<String> {
    vec![
        // For example now host interfaces such as
        // bluetooth cannot be accessed due to /proc/acpi
        "/proc/acpi".to_string(),
        "/proc/asound".to_string(),
        "/proc/kcore".to_string(),
        "/proc/keys".to_string(),
        "/proc/latency_stats".to_string(),
        "/proc/timer_list".to_string(),
        "/proc/timer_stats".to_string(),
        "/proc/sched_debug".to_string(),
        "/sys/firmware".to_string(),
        "/proc/scsi".to_string(),
    ]
}

/// Default readonly paths, for example most containers shouldn't have permission to write to
/// `/proc/sys`.
pub fn get_default_readonly_paths() -> Vec<String> {
    vec![
        "/proc/bus".to_string(),
        "/proc/fs".to_string(),
        "/proc/irq".to_string(),
        "/proc/sys".to_string(),
        "/proc/sysrq-trigger".to_string(),
    ]
}

#[derive(
    Builder, Clone, Debug, Default, Deserialize, Eq, Getters, Setters, PartialEq, Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// LinuxIntelRdt has container runtime resource constraints for Intel RDT
/// CAT and MBA features which introduced in Linux 4.10 and 4.12 kernel.
pub struct LinuxIntelRdt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The identity for RDT Class of Service.
    clos_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The schema for L3 cache id and capacity bitmask (CBM).
    /// Format: "L3:<cache_id0>=<cbm0>;<cache_id1>=<cbm1>;..."
    l3_cache_schema: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The schema of memory bandwidth per L3 cache id.
    /// Format: "MB:<cache_id0>=bandwidth0;<cache_id1>=bandwidth1;..."
    /// The unit of memory bandwidth is specified in "percentages" by
    /// default, and in "MBps" if MBA Software Controller is
    /// enabled.
    mem_bw_schema: Option<String>,
}

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// LinuxPersonality represents the Linux personality syscall input.
pub struct LinuxPersonality {
    #[getset(get_copy = "pub", set = "pub")]
    /// Domain for the personality.
    domain: LinuxPersonalityDomain,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    /// Additional flags
    flags: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// Define domain and flags for LinuxPersonality.
pub enum LinuxPersonalityDomain {
    #[serde(rename = "LINUX")]
    /// PerLinux is the standard Linux personality.
    PerLinux,

    #[serde(rename = "LINUX32")]
    /// PerLinux32 sets personality to 32 bit.
    PerLinux32,
}

impl Default for LinuxPersonalityDomain {
    fn default() -> Self {
        Self::PerLinux
    }
}

#[cfg(feature = "proptests")]
use quickcheck::{Arbitrary, Gen};

#[cfg(feature = "proptests")]
fn some_none_generator_util<T: Arbitrary>(g: &mut Gen) -> Option<T> {
    let choice = g.choose(&[true, false]).unwrap();
    match choice {
        false => None,
        true => Some(T::arbitrary(g)),
    }
}

#[cfg(feature = "proptests")]
impl Arbitrary for LinuxDeviceCgroup {
    fn arbitrary(g: &mut Gen) -> LinuxDeviceCgroup {
        let typ_choices = ["a", "b", "c", "u", "p"];

        let typ_chosen = g.choose(&typ_choices).unwrap();

        let typ = match typ_chosen.to_string().as_str() {
            "a" => LinuxDeviceType::A,
            "b" => LinuxDeviceType::B,
            "c" => LinuxDeviceType::C,
            "u" => LinuxDeviceType::U,
            "p" => LinuxDeviceType::P,
            _ => LinuxDeviceType::A,
        };

        let access_choices = ["rwm", "m"];
        LinuxDeviceCgroup {
            allow: bool::arbitrary(g),
            typ: typ.into(),
            major: some_none_generator_util::<i64>(g),
            minor: some_none_generator_util::<i64>(g),
            access: g.choose(&access_choices).unwrap().to_string().into(),
        }
    }
}

#[cfg(feature = "proptests")]
impl Arbitrary for LinuxMemory {
    fn arbitrary(g: &mut Gen) -> LinuxMemory {
        LinuxMemory {
            kernel: some_none_generator_util::<i64>(g),
            kernel_tcp: some_none_generator_util::<i64>(g),
            limit: some_none_generator_util::<i64>(g),
            reservation: some_none_generator_util::<i64>(g),
            swap: some_none_generator_util::<i64>(g),
            swappiness: some_none_generator_util::<u64>(g),
            disable_oom_killer: some_none_generator_util::<bool>(g),
            use_hierarchy: some_none_generator_util::<bool>(g),
        }
    }
}

#[cfg(feature = "proptests")]
impl Arbitrary for LinuxHugepageLimit {
    fn arbitrary(g: &mut Gen) -> LinuxHugepageLimit {
        let unit_choice = ["KB", "MB", "GB"];
        let unit = g.choose(&unit_choice).unwrap();
        let page_size = u64::arbitrary(g).to_string() + unit;

        LinuxHugepageLimit {
            page_size,
            limit: i64::arbitrary(g),
        }
    }
}
