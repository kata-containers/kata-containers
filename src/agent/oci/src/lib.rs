// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use libc::mode_t;
use std::collections::HashMap;

pub mod serialize;

#[allow(dead_code)]
fn is_false(b: bool) -> bool {
    !b
}

#[allow(dead_code)]
fn is_default<T>(d: &T) -> bool
where
    T: Default + PartialEq,
{
    *d == T::default()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Spec {
    #[serde(
        default,
        rename = "ociVersion",
        skip_serializing_if = "String::is_empty"
    )]
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<Root>,
    #[serde(default, skip_serializing_if = "String:: is_empty")]
    pub hostname: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mounts: Vec<Mount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Hooks>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<Linux>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solaris: Option<Solaris>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<Windows<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm: Option<VM>,
}

impl Spec {
    pub fn load(path: &str) -> Result<Spec, serialize::SerializeError> {
        serialize::deserialize(path)
    }

    pub fn save(&self, path: &str) -> Result<(), serialize::SerializeError> {
        serialize::serialize(self, path)
    }
}

pub type LinuxRlimit = POSIXRlimit;

#[derive(Serialize, Deserialize, Debug)]
pub struct Process {
    #[serde(default)]
    pub terminal: bool,
    #[serde(
        default,
        rename = "consoleSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub console_size: Option<Box>,
    pub user: User,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<LinuxCapabilities>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rlimits: Vec<POSIXRlimit>,
    #[serde(default, rename = "noNewPrivileges")]
    pub no_new_privileges: bool,
    #[serde(
        default,
        rename = "apparmorProfile",
        skip_serializing_if = "String::is_empty"
    )]
    pub apparmor_profile: String,
    #[serde(
        default,
        rename = "oomScoreAdj",
        skip_serializing_if = "Option::is_none"
    )]
    pub oom_score_adj: Option<i32>,
    #[serde(
        default,
        rename = "selinuxLabel",
        skip_serializing_if = "String::is_empty"
    )]
    pub selinux_label: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxCapabilities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounding: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effective: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inheritable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permitted: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ambient: Vec<String>,
}

#[derive(Default, PartialEq, Serialize, Deserialize, Debug)]
pub struct Box {
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub width: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(
        default,
        rename = "addtionalGids",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additional_gids: Vec<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Root {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default)]
    pub readonly: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mount {
    #[serde(default)]
    pub destination: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub r#type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Hook {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Hooks {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prestart: Vec<Hook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poststart: Vec<Hook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poststop: Vec<Hook>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Linux {
    #[serde(default, rename = "uidMappings", skip_serializing_if = "Vec::is_empty")]
    pub uid_mappings: Vec<LinuxIDMapping>,
    #[serde(default, rename = "gidMappings", skip_serializing_if = "Vec::is_empty")]
    pub gid_mappings: Vec<LinuxIDMapping>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub sysctl: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<LinuxResources>,
    #[serde(
        default,
        rename = "cgroupsPath",
        skip_serializing_if = "String::is_empty"
    )]
    pub cgroups_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub namespaces: Vec<LinuxNamespace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<LinuxDevice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seccomp: Option<LinuxSeccomp>,
    #[serde(
        default,
        rename = "rootfsPropagation",
        skip_serializing_if = "String::is_empty"
    )]
    pub rootfs_propagation: String,
    #[serde(default, rename = "maskedPaths", skip_serializing_if = "Vec::is_empty")]
    pub masked_paths: Vec<String>,
    #[serde(
        default,
        rename = "readonlyPaths",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub readonly_paths: Vec<String>,
    #[serde(
        default,
        rename = "mountLabel",
        skip_serializing_if = "String::is_empty"
    )]
    pub mount_label: String,
    #[serde(default, rename = "intelRdt", skip_serializing_if = "Option::is_none")]
    pub intel_rdt: Option<LinuxIntelRdt>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxNamespace {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub r#type: LinuxNamespaceType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
}

pub type LinuxNamespaceType = String;

pub const PIDNAMESPACE: &str = "pid";
pub const NETWORKNAMESPACE: &str = "network";
pub const MOUNTNAMESPACE: &str = "mount";
pub const IPCNAMESPACE: &str = "ipc";
pub const USERNAMESPACE: &str = "user";
pub const UTSNAMESPACE: &str = "uts";
pub const CGROUPNAMESPACE: &str = "cgroup";

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxIDMapping {
    #[serde(default, rename = "containerID")]
    pub container_id: u32,
    #[serde(default, rename = "hostID")]
    pub host_id: u32,
    #[serde(default)]
    pub size: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct POSIXRlimit {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub hard: u64,
    #[serde(default)]
    pub soft: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxHugepageLimit {
    #[serde(default, rename = "pageSize", skip_serializing_if = "String::is_empty")]
    pub page_size: String,
    #[serde(default)]
    pub limit: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxInterfacePriority {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub priority: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxBlockIODevice {
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxWeightDevice {
    pub blk: LinuxBlockIODevice,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    #[serde(
        default,
        rename = "leafWeight",
        skip_serializing_if = "Option::is_none"
    )]
    pub leaf_weight: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxThrottleDevice {
    pub blk: LinuxBlockIODevice,
    #[serde(default)]
    pub rate: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxBlockIO {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    #[serde(
        default,
        rename = "leafWeight",
        skip_serializing_if = "Option::is_none"
    )]
    pub leaf_weight: Option<u16>,
    #[serde(
        default,
        rename = "weightDevice",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub weight_device: Vec<LinuxWeightDevice>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "throttleReadBpsDevice"
    )]
    pub throttle_read_bps_device: Vec<LinuxThrottleDevice>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "throttleWriteBpsDevice"
    )]
    pub throttle_write_bps_device: Vec<LinuxThrottleDevice>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "throttleReadIOPSDevice"
    )]
    pub throttle_read_iops_device: Vec<LinuxThrottleDevice>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "throttleWriteIOPSDevice"
    )]
    pub throttle_write_iops_device: Vec<LinuxThrottleDevice>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxMemory {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "kernelTCP")]
    pub kernel_tcp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swapiness: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "disableOOMKiller"
    )]
    pub disable_oom_killer: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxCPU {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "realtimeRuntime"
    )]
    pub realtime_runtime: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "realtimePeriod"
    )]
    pub realtime_period: Option<u64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cpus: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mems: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxPids {
    #[serde(default)]
    pub limit: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxNetwork {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "classID")]
    pub class_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priorities: Vec<LinuxInterfacePriority>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxRdma {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "hcaHandles"
    )]
    pub hca_handles: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "hcaObjects"
    )]
    pub hca_objects: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxResources {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<LinuxDeviceCgroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<LinuxMemory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<LinuxCPU>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pids: Option<LinuxPids>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "blockIO")]
    pub block_io: Option<LinuxBlockIO>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "hugepageLimits"
    )]
    pub hugepage_limits: Vec<LinuxHugepageLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<LinuxNetwork>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub rdma: HashMap<String, LinuxRdma>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxDevice {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub r#type: String,
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "fileMode")]
    pub file_mode: Option<mode_t>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxDeviceCgroup {
    #[serde(default)]
    pub allow: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub r#type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub major: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minor: Option<i64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub access: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Solaris {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub milestone: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub limitpriv: String,
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "maxShmMemory"
    )]
    pub max_shm_memory: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anet: Vec<SolarisAnet>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "cappedCPU")]
    pub capped_cpu: Option<SolarisCappedCPU>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "cappedMemory"
    )]
    pub capped_memory: Option<SolarisCappedMemory>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SolarisCappedCPU {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ncpus: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SolarisCappedMemory {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub physical: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub swap: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SolarisAnet {
    #[serde(default, skip_serializing_if = "String::is_empty", rename = "linkname")]
    pub link_name: String,
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "lowerLink"
    )]
    pub lower_link: String,
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "allowdAddress"
    )]
    pub allowed_addr: String,
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "configureAllowedAddress"
    )]
    pub config_allowed_addr: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub defrouter: String,
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "linkProtection"
    )]
    pub link_protection: String,
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "macAddress"
    )]
    pub mac_address: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Windows<T> {
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "layerFolders"
    )]
    pub layer_folders: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<WindowsResources>,
    #[serde(default, rename = "credentialSpec")]
    pub credential_spec: T,
    #[serde(default)]
    pub servicing: bool,
    #[serde(default, rename = "ignoreFlushesDuringBoot")]
    pub ignore_flushes_during_boot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperv: Option<WindowsHyperV>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<WindowsNetwork>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowsResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<WindowsMemoryResources>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<WindowsCPUResources>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<WindowsStorageResources>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowsMemoryResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowsCPUResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowsStorageResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "sandboxSize"
    )]
    pub sandbox_size: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowsNetwork {
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "endpointList"
    )]
    pub endpoint_list: Vec<String>,
    #[serde(default, rename = "allowUnqualifiedDNSQuery")]
    pub allow_unqualified_dns_query: bool,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "DNSSearchList"
    )]
    pub dns_search_list: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "nwtworkSharedContainerName"
    )]
    pub network_shared_container_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowsHyperV {
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "utilityVMPath"
    )]
    pub utility_vm_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VM {
    pub hypervisor: VMHypervisor,
    pub kernel: VMKernel,
    pub image: VMImage,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VMHypervisor {
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parameters: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VMKernel {
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parameters: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub initrd: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VMImage {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub format: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxSeccomp {
    #[serde(default, rename = "defaultAction")]
    pub default_action: LinuxSeccompAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architectures: Vec<Arch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub syscalls: Vec<LinuxSyscall>,
}

pub type Arch = String;

pub const ARCHX86: &str = "SCMP_ARCH_X86";
pub const ARCHX86_64: &str = "SCMP_ARCH_X86_64";
pub const ARCHX32: &str = "SCMP_ARCH_X32";
pub const ARCHARM: &str = "SCMP_ARCH_ARM";
pub const ARCHAARCH64: &str = "SCMP_ARCH_AARCH64";
pub const ARCHMIPS: &str = "SCMP_ARCH_MIPS";
pub const ARCHMIPS64: &str = "SCMP_ARCH_MIPS64";
pub const ARCHMIPS64N32: &str = "SCMP_ARCH_MIPS64N32";
pub const ARCHMIPSEL: &str = "SCMP_ARCH_MIPSEL";
pub const ARCHMIPSEL64: &str = "SCMP_ARCH_MIPSEL64";
pub const ARCHMIPSEL64N32: &str = "SCMP_ARCH_MIPSEL64N32";
pub const ARCHPPC: &str = "SCMP_ARCH_PPC";
pub const ARCHPPC64: &str = "SCMP_ARCH_PPC64";
pub const ARCHPPC64LE: &str = "SCMP_ARCH_PPC64LE";
pub const ARCHS390: &str = "SCMP_ARCH_S390";
pub const ARCHS390X: &str = "SCMP_ARCH_S390X";
pub const ARCHPARISC: &str = "SCMP_ARCH_PARISC";
pub const ARCHPARISC64: &str = "SCMP_ARCH_PARISC64";

pub type LinuxSeccompAction = String;

pub const ACTKILL: &str = "SCMP_ACT_KILL";
pub const ACTTRAP: &str = "SCMP_ACT_TRAP";
pub const ACTERRNO: &str = "SCMP_ACT_ERRNO";
pub const ACTTRACE: &str = "SCMP_ACT_TRACE";
pub const ACTALLOW: &str = "SCMP_ACT_ALLOW";

pub type LinuxSeccompOperator = String;

pub const OPNOTEQUAL: &str = "SCMP_CMP_NE";
pub const OPLESSTHAN: &str = "SCMP_CMP_LT";
pub const OPLESSEQUAL: &str = "SCMP_CMP_LE";
pub const OPEQUALTO: &str = "SCMP_CMP_EQ";
pub const OPGREATEREQUAL: &str = "SCMP_CMP_GE";
pub const OPGREATERTHAN: &str = "SCMP_CMP_GT";
pub const OPMASKEDEQUAL: &str = "SCMP_CMP_MASKED_EQ";

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxSeccompArg {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub value: u64,
    #[serde(default, rename = "valueTwo")]
    pub value_two: u64,
    #[serde(default)]
    pub op: LinuxSeccompOperator,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxSyscall {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub action: LinuxSeccompAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<LinuxSeccompArg>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinuxIntelRdt {
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "l3CacheSchema"
    )]
    pub l3_cache_schema: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct State {
    #[serde(
        default,
        skip_serializing_if = "String::is_empty",
        rename = "ociVersion"
    )]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(default)]
    pub pid: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bundle: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
