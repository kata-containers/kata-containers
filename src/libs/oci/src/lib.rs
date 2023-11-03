// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use libc::{self, mode_t};
use std::collections::HashMap;

mod serialize;
pub use serialize::{to_string, to_writer, Error, Result};

pub const OCI_SPEC_CONFIG_FILE_NAME: &str = "config.json";

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

fn default_seccomp_errno() -> u32 {
    libc::EPERM as u32
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hostname: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub domainname: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zos: Option<ZOS>,
}

impl Spec {
    pub fn load(path: &str) -> Result<Spec> {
        serialize::deserialize(path)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        serialize::serialize(self, path)
    }
}

pub type LinuxRlimit = POSIXRlimit;

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Scheduler {
    #[serde(default)]
    pub policy: LinuxSchedulerPolicy,
    #[serde(default)]
    pub nice: i32,
    #[serde(default)]
    pub priority: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<LinuxSchedulerFlag>,
    #[serde(default)]
    pub runtime: u64,
    #[serde(default)]
    pub deadline: u64,
    #[serde(default)]
    pub period: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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
    #[serde(
        default,
        rename = "commandLine",
        skip_serializing_if = "String::is_empty"
    )]
    pub command_line: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<Scheduler>,
    #[serde(
        default,
        rename = "selinuxLabel",
        skip_serializing_if = "String::is_empty"
    )]
    pub selinux_label: String,
    #[serde(
        default,
        rename = "ioPriority",
        skip_serializing_if = "Option::is_none"
    )]
    pub io_priority: Option<LinuxIOPriority>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxIOPriority {
    #[serde(default)]
    pub class: IOPriorityClass,
    #[serde(default)]
    pub priority: i32,
}

pub type IOPriorityClass = String;

pub const IOPRIO_CLASS_RT: &str = "IOPRIO_CLASS_RT";
pub const IOPRIO_CLASS_BE: &str = "IOPRIO_CLASS_BE";
pub const IOPRIO_CLASS_IDLE: &str = "IOPRIO_CLASS_IDLE";

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Box {
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub width: u32,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct User {
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub umask: Option<u32>,
    #[serde(
        default,
        rename = "additionalGids",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub additional_gids: Vec<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Root {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default)]
    pub readonly: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Mount {
    #[serde(default)]
    pub destination: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub r#type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(
        default,
        rename = "uidMappings",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub uid_mappings: Vec<LinuxIDMapping>,
    #[serde(
        default,
        rename = "gidMappings",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub gid_mappings: Vec<LinuxIDMapping>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Hooks {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prestart: Vec<Hook>,
    #[serde(
        default,
        rename = "createRuntime",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub create_runtime: Vec<Hook>,
    #[serde(
        default,
        rename = "createContainer",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub create_container: Vec<Hook>,
    #[serde(
        default,
        rename = "startContainer",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub start_container: Vec<Hook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poststart: Vec<Hook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poststop: Vec<Hook>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Linux {
    #[serde(
        default,
        rename = "uidMappings",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub uid_mappings: Vec<LinuxIDMapping>,
    #[serde(
        default,
        rename = "gidMappings",
        skip_serializing_if = "Vec::is_empty"
    )]
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
    #[serde(
        default,
        rename = "maskedPaths",
        skip_serializing_if = "Vec::is_empty"
    )]
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
    #[serde(
        default,
        rename = "intelRdt",
        skip_serializing_if = "Option::is_none"
    )]
    pub intel_rdt: Option<LinuxIntelRdt>,
    #[serde(
        default,
        rename = "personality",
        skip_serializing_if = "Option::is_none"
    )]
	pub personality: Option<LinuxPersonality>,
    #[serde(
        default,
        rename = "timeOffsets",
        skip_serializing_if = "HashMap::is_empty"
    )]
	pub time_offsets: HashMap<String, LinuxTimeOffset>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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
pub const TIMENAMESPACE: &str = "time";

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxIDMapping {
    #[serde(default, rename = "containerID")]
    pub container_id: u32,
    #[serde(default, rename = "hostID")]
    pub host_id: u32,
    #[serde(default)]
    pub size: u32,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxTimeOffset {
    #[serde(default)]
    pub secs: i64,
    #[serde(default)]
    pub nanosecs: u32,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct POSIXRlimit {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub hard: u64,
    #[serde(default)]
    pub soft: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxHugepageLimit {
    #[serde(
        default,
        rename = "pageSize",
        skip_serializing_if = "String::is_empty"
    )]
    pub page_size: String,
    #[serde(default)]
    pub limit: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxInterfacePriority {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub priority: u32,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxBlockIODevice {
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxWeightDevice {
    #[serde(flatten)]
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

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxThrottleDevice {
    #[serde(flatten)]
    pub blk: LinuxBlockIODevice,
    #[serde(default)]
    pub rate: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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
        rename = "throttleReadBpsDevice",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub throttle_read_bps_device: Vec<LinuxThrottleDevice>,
    #[serde(
        default,
        rename = "throttleWriteBpsDevice",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub throttle_write_bps_device: Vec<LinuxThrottleDevice>,
    #[serde(
        default,
        rename = "throttleReadIOPSDevice",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub throttle_read_iops_device: Vec<LinuxThrottleDevice>,
    #[serde(
        default,
        rename = "throttleWriteIOPSDevice",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub throttle_write_iops_device: Vec<LinuxThrottleDevice>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxMemory {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<i64>,
    #[serde(
        default,
        rename = "kernelTCP",
        skip_serializing_if = "Option::is_none"
    )]
    pub kernel_tcp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swappiness: Option<u64>,
    #[serde(
        default,
        rename = "disableOOMKiller",
        skip_serializing_if = "Option::is_none"
    )]
    pub disable_oom_killer: Option<bool>,
    #[serde(
        default,
        rename = "useHierachy",
        skip_serializing_if = "Option::is_none"
    )]
    pub use_hierarchy: Option<bool>,
    #[serde(
        default,
        rename = "checkBeforeUpdate",
        skip_serializing_if = "Option::is_none"
    )]
    pub check_before_update: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxCPU {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<u64>,
    #[serde(
        default,
        rename = "realtimeRuntime",
        skip_serializing_if = "Option::is_none"
    )]
    pub realtime_runtime: Option<i64>,
    #[serde(
        default,
        rename = "realtimePeriod",
        skip_serializing_if = "Option::is_none"
    )]
    pub realtime_period: Option<u64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cpus: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mems: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxPids {
    #[serde(default)]
    pub limit: i64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxNetwork {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "classID")]
    pub class_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priorities: Vec<LinuxInterfacePriority>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxRdma {
    #[serde(
        default,
        rename = "hcaHandles",
        skip_serializing_if = "Option::is_none"
    )]
    pub hca_handles: Option<u32>,
    #[serde(
        default,
        rename = "hcaObjects",
        skip_serializing_if = "Option::is_none"
    )]
    pub hca_objects: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxResources {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<LinuxDeviceCgroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<LinuxMemory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<LinuxCPU>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pids: Option<LinuxPids>,
    #[serde(rename = "blockIO", skip_serializing_if = "Option::is_none")]
    pub block_io: Option<LinuxBlockIO>,
    #[serde(
        default,
        rename = "hugepageLimits",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub hugepage_limits: Vec<LinuxHugepageLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<LinuxNetwork>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub rdma: HashMap<String, LinuxRdma>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub unified: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxDevice {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub r#type: String,
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
    #[serde(default, rename = "fileMode", skip_serializing_if = "Option::is_none")]
    pub file_mode: Option<mode_t>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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

pub type LinuxPersonalityDomain = String;
pub type LinuxPersonalityFlag = String;

pub const PERLINUX: &str = "LINUX";
pub const PERLINUX32: &str = "LINUX32";

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxPersonality {
    #[serde(default)]
    pub domain: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Solaris {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub milestone: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub limitpriv: String,
    #[serde(
        default,
        rename = "maxShmMemory",
        skip_serializing_if = "String::is_empty"
    )]
    pub max_shm_memory: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anet: Vec<SolarisAnet>,
    #[serde(
        default,
        rename = "cappedCPU",
        skip_serializing_if = "Option::is_none"
    )]
    pub capped_cpu: Option<SolarisCappedCPU>,
    #[serde(
        default,
        rename = "cappedMemory",
        skip_serializing_if = "Option::is_none"
    )]
    pub capped_memory: Option<SolarisCappedMemory>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct SolarisCappedCPU {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ncpus: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct SolarisCappedMemory {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub physical: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub swap: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct SolarisAnet {
    #[serde(
        default,
        rename = "linkname",
        skip_serializing_if = "String::is_empty"
    )]
    pub link_name: String,
    #[serde(
        default,
        rename = "lowerLink",
        skip_serializing_if = "String::is_empty"
    )]
    pub lower_link: String,
    #[serde(
        default,
        rename = "allowedAddress",
        skip_serializing_if = "String::is_empty"
    )]
    pub allowed_addr: String,
    #[serde(
        default,
        rename = "configureAllowedAddress",
        skip_serializing_if = "String::is_empty"
    )]
    pub config_allowed_addr: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub defrouter: String,
    #[serde(
        default,
        rename = "linkProtection",
        skip_serializing_if = "String::is_empty"
    )]
    pub link_protection: String,
    #[serde(
        default,
        rename = "macAddress",
        skip_serializing_if = "String::is_empty"
    )]
    pub mac_address: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Windows<T> {
    #[serde(
        default,
        rename = "layerFolders",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub layer_folders: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<WindowsDevice>,
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

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct WindowsDevice {
    #[serde(default)]
    pub id: String,
    #[serde(default, rename = "idType")]
    pub id_type: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct WindowsResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<WindowsMemoryResources>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<WindowsCPUResources>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<WindowsStorageResources>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct WindowsMemoryResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct WindowsCPUResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct WindowsStorageResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<u64>,
    #[serde(
        default,
        rename = "sandboxSize",
        skip_serializing_if = "Option::is_none"
    )]
    pub sandbox_size: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct WindowsNetwork {
    #[serde(
        default,
        rename = "endpointList",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub endpoint_list: Vec<String>,
    #[serde(default, rename = "allowUnqualifiedDNSQuery")]
    pub allow_unqualified_dns_query: bool,
    #[serde(
        default,
        rename = "DNSSearchList",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub dns_search_list: Vec<String>,
    #[serde(
        default,
        rename = "networkSharedContainerName",
        skip_serializing_if = "String::is_empty"
    )]
    pub network_shared_container_name: String,
    #[serde(
        default,
        rename = "networkNamespace",
        skip_serializing_if = "String::is_empty"
    )]
    pub network_namespace: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct WindowsHyperV {
    #[serde(
        default,
        rename = "utilityVMPath",
        skip_serializing_if = "String::is_empty"
    )]
    pub utility_vm_path: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct VM {
    pub hypervisor: VMHypervisor,
    pub kernel: VMKernel,
    pub image: VMImage,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct VMHypervisor {
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct VMKernel {
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub initrd: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct VMImage {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub format: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxSeccomp {
    #[serde(default, rename = "defaultAction")]
    pub default_action: LinuxSeccompAction,
    #[serde(
        default,
        rename = "defaultErrnoRet",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_errno_ret: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architectures: Vec<Arch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<LinuxSeccompFlag>,
    #[serde(
        default,
        rename = "listenerPath",
        skip_serializing_if = "String::is_empty"
    )]
    pub listener_path: String,
    #[serde(
        default,
        rename = "listenerMetadata",
        skip_serializing_if = "String::is_empty"
    )]
    pub listener_metadata: String,
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
pub const ARCHRISCV64: &str = "SCMP_ARCH_RISCV64";

pub type LinuxSeccompFlag = String;

pub const LINUXSECCOMPFLAGLOG: &str = "SECCOMP_FILTER_FLAG_LOG";
pub const LINUXSECCOMPFILTERFLAGSPECALLOW: &str = "SECCOMP_FILTER_FLAG_SPEC_ALLOW";
pub const LINUXSECCOMPFILTERFLAGWAITKILLABLERECV: &str = "SECCOMP_FILTER_FLAG_WAIT_KILLABLE_RECV";

pub type LinuxSeccompAction = String;

pub const ACTKILL: &str = "SCMP_ACT_KILL";
pub const ACTKILLPROCESS: &str = "SCMP_ACT_KILL_PROCESS";
pub const ACTKILLTHREAD: &str = "SCMP_ACT_KILL_THREAD";
pub const ACTTRAP: &str = "SCMP_ACT_TRAP";
pub const ACTERRNO: &str = "SCMP_ACT_ERRNO";
pub const ACTTRACE: &str = "SCMP_ACT_TRACE";
pub const ACTALLOW: &str = "SCMP_ACT_ALLOW";
pub const ACTLOG: &str = "SCMP_ACT_LOG";
pub const ACTNOTIFY: &str = "SCMP_ACT_NOTIFY";

pub type LinuxSeccompOperator = String;

pub const OPNOTEQUAL: &str = "SCMP_CMP_NE";
pub const OPLESSTHAN: &str = "SCMP_CMP_LT";
pub const OPLESSEQUAL: &str = "SCMP_CMP_LE";
pub const OPEQUALTO: &str = "SCMP_CMP_EQ";
pub const OPGREATEREQUAL: &str = "SCMP_CMP_GE";
pub const OPGREATERTHAN: &str = "SCMP_CMP_GT";
pub const OPMASKEDEQUAL: &str = "SCMP_CMP_MASKED_EQ";

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxSyscall {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub action: LinuxSeccompAction,
    #[serde(default = "default_seccomp_errno", rename = "errnoRet")]
    pub errno_ret: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<LinuxSeccompArg>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct LinuxIntelRdt {
    #[serde(
        default,
        rename = "closID",
        skip_serializing_if = "String::is_empty"
    )]
    pub clos_id: String,
    #[serde(
        default,
        rename = "l3CacheSchema",
        skip_serializing_if = "String::is_empty"
    )]
    pub l3_cache_schema: String,
    #[serde(
        default,
        rename = "memBwSchema",
        skip_serializing_if = "String::is_empty"
    )]
    pub mem_bw_schema: String,
    #[serde(default, rename = "enableCMT")]
    pub enable_cmt: bool,
    #[serde(default, rename = "enableMBM")]
    pub enable_mbm: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct ZOS {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<ZOSDevice>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct ZOSDevice {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub r#type: String,
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
    #[serde(
        default,
        rename = "fileMode",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_mode: Option<mode_t>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
}

pub type LinuxSchedulerPolicy = String;

pub const SCHEDOTHER: &str = "SCHED_OTHER";
pub const SCHEDFIFO: &str = "SCHED_FIFO";
pub const SCHEDRR: &str = "SCHED_RR";
pub const SCHEDBATCH: &str = "SCHED_BATCH";
pub const SCHEDISO: &str = "SCHED_ISO";
pub const SCHEDIDLE: &str = "SCHED_IDLE";
pub const SCHEDDEADLINE: &str = "SCHED_DEADLINE";

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContainerState {
    Creating,
    Created,
    Running,
    Stopped,
    Paused,
}

pub type LinuxSchedulerFlag = String;

pub const SCHEDFLAGRESETONFORK: &str = "SCHED_FLAG_RESET_ON_FORK";
pub const SCHEDFLAGRECLAIM: &str = "SCHED_FLAG_RECLAIM";
pub const SCHEDFLAGDLOVERRUN: &str = "SCHED_FLAG_DL_OVERRUN";
pub const SCHEDFLAGKEEPPOLICY: &str = "SCHED_FLAG_KEEP_POLICY";
pub const SCHEDFLAGKEEPPARAMS: &str = "SCHED_FLAG_KEEP_PARAMS";
pub const SCHEDFLAGUTILCLAMPMIN: &str = "SCHED_FLAG_UTIL_CLAMP_MIN";
pub const SCHEDFLAGUTILCLAMPMAX: &str = "SCHED_FLAG_UTIL_CLAMP_MAX";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct State {
    #[serde(
        default,
        rename = "ociVersion",
        skip_serializing_if = "String::is_empty"
    )]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    pub status: ContainerState,
    #[serde(default)]
    pub pid: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bundle: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    #[test]
    fn test_deserialize_state() {
        let data = r#"{
            "ociVersion": "0.2.0",
            "id": "oci-container1",
            "status": "running",
            "pid": 4422,
            "bundle": "/containers/redis",
            "annotations": {
                "myKey": "myValue"
            }
        }"#;
        let expected = State {
            version: "0.2.0".to_string(),
            id: "oci-container1".to_string(),
            status: ContainerState::Running,
            pid: 4422,
            bundle: "/containers/redis".to_string(),
            annotations: [("myKey".to_string(), "myValue".to_string())]
                .iter()
                .cloned()
                .collect(),
        };

        let current: crate::State = serde_json::from_str(data).unwrap();
        assert_eq!(expected, current);
    }

    #[test]
    fn test_deserialize_spec() {
        let data = r#"{
            "ociVersion": "1.0.1",
            "process": {
                "terminal": true,
                "user": {
                    "uid": 1,
                    "gid": 1,
                    "additionalGids": [
                        5,
                        6
                    ]
                },
                "args": [
                    "sh"
                ],
                "env": [
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    "TERM=xterm"
                ],
                "cwd": "/",
                "capabilities": {
                    "bounding": [
                        "CAP_AUDIT_WRITE",
                        "CAP_KILL",
                        "CAP_NET_BIND_SERVICE"
                    ],
                    "permitted": [
                        "CAP_AUDIT_WRITE",
                        "CAP_KILL",
                        "CAP_NET_BIND_SERVICE"
                    ],
                    "inheritable": [
                        "CAP_AUDIT_WRITE",
                        "CAP_KILL",
                        "CAP_NET_BIND_SERVICE"
                    ],
                    "effective": [
                        "CAP_AUDIT_WRITE",
                        "CAP_KILL"
                    ],
                    "ambient": [
                        "CAP_NET_BIND_SERVICE"
                    ]
                },
                "rlimits": [
                    {
                        "type": "RLIMIT_CORE",
                        "hard": 1024,
                        "soft": 1024
                    },
                    {
                        "type": "RLIMIT_NOFILE",
                        "hard": 1024,
                        "soft": 1024
                    }
                ],
                "apparmorProfile": "acme_secure_profile",
                "oomScoreAdj": 100,
                "selinuxLabel": "system_u:system_r:svirt_lxc_net_t:s0:c124,c675",
                "noNewPrivileges": true
            },
            "root": {
                "path": "rootfs",
                "readonly": true
            },
            "hostname": "slartibartfast",
            "mounts": [
                {
                    "destination": "/proc",
                    "type": "proc",
                    "source": "proc"
                },
                {
                    "destination": "/dev",
                    "type": "tmpfs",
                    "source": "tmpfs",
                    "options": [
                        "nosuid",
                        "strictatime",
                        "mode=755",
                        "size=65536k"
                    ]
                },
                {
                    "destination": "/dev/pts",
                    "type": "devpts",
                    "source": "devpts",
                    "options": [
                        "nosuid",
                        "noexec",
                        "newinstance",
                        "ptmxmode=0666",
                        "mode=0620",
                        "gid=5"
                    ]
                },
                {
                    "destination": "/dev/shm",
                    "type": "tmpfs",
                    "source": "shm",
                    "options": [
                        "nosuid",
                        "noexec",
                        "nodev",
                        "mode=1777",
                        "size=65536k"
                    ]
                },
                {
                    "destination": "/dev/mqueue",
                    "type": "mqueue",
                    "source": "mqueue",
                    "options": [
                        "nosuid",
                        "noexec",
                        "nodev"
                    ]
                },
                {
                    "destination": "/sys",
                    "type": "sysfs",
                    "source": "sysfs",
                    "options": [
                        "nosuid",
                        "noexec",
                        "nodev"
                    ]
                },
                {
                    "destination": "/sys/fs/cgroup",
                    "type": "cgroup",
                    "source": "cgroup",
                    "options": [
                        "nosuid",
                        "noexec",
                        "nodev",
                        "relatime",
                        "ro"
                    ]
                }
            ],
            "hooks": {
                "prestart": [
                    {
                        "path": "/usr/bin/fix-mounts",
                        "args": [
                            "fix-mounts",
                            "arg1",
                            "arg2"
                        ],
                        "env": [
                            "key1=value1"
                        ]
                    },
                    {
                        "path": "/usr/bin/setup-network"
                    }
                ],
                "createRuntime": [
                    {
                        "path": "/usr/local/bin/nerdctl"
                    }
                ],
                "poststart": [
                    {
                        "path": "/usr/bin/notify-start",
                        "timeout": 5
                    }
                ],
                "poststop": [
                    {
                        "path": "/usr/sbin/cleanup.sh",
                        "args": [
                            "cleanup.sh",
                            "-f"
                        ]
                    }
                ]
            },
            "linux": {
                "devices": [
                    {
                        "path": "/dev/fuse",
                        "type": "c",
                        "major": 10,
                        "minor": 229,
                        "fileMode": 438,
                        "uid": 0,
                        "gid": 0
                    },
                    {
                        "path": "/dev/sda",
                        "type": "b",
                        "major": 8,
                        "minor": 0,
                        "fileMode": 432,
                        "uid": 0,
                        "gid": 0
                    }
                ],
                "uidMappings": [
                    {
                        "containerID": 0,
                        "hostID": 1000,
                        "size": 32000
                    }
                ],
                "gidMappings": [
                    {
                        "containerID": 0,
                        "hostID": 1000,
                        "size": 32000
                    }
                ],
                "sysctl": {
                    "net.ipv4.ip_forward": "1",
                    "net.core.somaxconn": "256"
                },
                "cgroupsPath": "/myRuntime/myContainer",
                "resources": {
                    "network": {
                        "classID": 1048577,
                        "priorities": [
                            {
                                "name": "eth0",
                                "priority": 500
                            },
                            {
                                "name": "eth1",
                                "priority": 1000
                            }
                        ]
                    },
                    "pids": {
                        "limit": 32771
                    },
                    "hugepageLimits": [
                        {
                            "pageSize": "2MB",
                            "limit": 9223372036854772000
                        },
                        {
                            "pageSize": "64KB",
                            "limit": 1000000
                        }
                    ],
                    "memory": {
                        "limit": 536870912,
                        "reservation": 536870912,
                        "swap": 536870912,
                        "kernel": -1,
                        "kernelTCP": -1,
                        "swappiness": 0,
                        "disableOOMKiller": false
                    },
                    "cpu": {
                        "shares": 1024,
                        "quota": 1000000,
                        "period": 500000,
                        "realtimeRuntime": 950000,
                        "realtimePeriod": 1000000,
                        "cpus": "2-3",
                        "mems": "0-7"
                    },
                    "devices": [
                        {
                            "allow": false,
                            "access": "rwm"
                        },
                        {
                            "allow": true,
                            "type": "c",
                            "major": 10,
                            "minor": 229,
                            "access": "rw"
                        },
                        {
                            "allow": true,
                            "type": "b",
                            "major": 8,
                            "minor": 0,
                            "access": "r"
                        }
                    ],
                    "blockIO": {
                        "weight": 10,
                        "leafWeight": 10,
                        "weightDevice": [
                            {
                                "major": 8,
                                "minor": 0,
                                "weight": 500,
                                "leafWeight": 300
                            },
                            {
                                "major": 8,
                                "minor": 16,
                                "weight": 500
                            }
                        ],
                        "throttleReadBpsDevice": [
                            {
                                "major": 8,
                                "minor": 0,
                                "rate": 600
                            }
                        ],
                        "throttleWriteIOPSDevice": [
                            {
                                "major": 8,
                                "minor": 16,
                                "rate": 300
                            }
                        ]
                    }
                },
                "rootfsPropagation": "slave",
                "seccomp": {
                    "defaultAction": "SCMP_ACT_ALLOW",
                    "architectures": [
                        "SCMP_ARCH_X86",
                        "SCMP_ARCH_X32"
                    ],
                    "syscalls": [
                        {
                            "names": [
                                "getcwd",
                                "chmod"
                            ],
                            "action": "SCMP_ACT_ERRNO"
                        }
                    ]
                },
                "namespaces": [
                    {
                        "type": "pid"
                    },
                    {
                        "type": "network"
                    },
                    {
                        "type": "ipc"
                    },
                    {
                        "type": "uts"
                    },
                    {
                        "type": "mount"
                    },
                    {
                        "type": "user"
                    },
                    {
                        "type": "cgroup"
                    }
                ],
                "maskedPaths": [
                    "/proc/kcore",
                    "/proc/latency_stats",
                    "/proc/timer_stats",
                    "/proc/sched_debug"
                ],
                "readonlyPaths": [
                    "/proc/asound",
                    "/proc/bus",
                    "/proc/fs",
                    "/proc/irq",
                    "/proc/sys",
                    "/proc/sysrq-trigger"
                ],
                "mountLabel": "system_u:object_r:svirt_sandbox_file_t:s0:c715,c811"
            },
            "annotations": {
                "com.example.key1": "value1",
                "com.example.key2": "value2"
            }
        }"#;
        let expected = crate::Spec {
            version: "1.0.1".to_string(),
            process: Option::from(crate::Process {
                terminal: true,
                console_size: None,
                user: crate::User {
                    uid: 1,
                    gid: 1,
                    umask: None,
                    // incompatible with oci
                    additional_gids: vec![5, 6],
                    username: "".to_string(),
                },
                command_line: "".to_string(),
                args: vec!["sh".to_string()],
                env: vec![
                    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                    "TERM=xterm".to_string(),
                ],
                cwd: "/".to_string(),
                capabilities: Some(crate::LinuxCapabilities {
                    bounding: vec![
                        "CAP_AUDIT_WRITE".to_string(),
                        "CAP_KILL".to_string(),
                        "CAP_NET_BIND_SERVICE".to_string(),
                    ],
                    effective: vec!["CAP_AUDIT_WRITE".to_string(), "CAP_KILL".to_string()],
                    inheritable: vec![
                        "CAP_AUDIT_WRITE".to_string(),
                        "CAP_KILL".to_string(),
                        "CAP_NET_BIND_SERVICE".to_string(),
                    ],
                    permitted: vec![
                        "CAP_AUDIT_WRITE".to_string(),
                        "CAP_KILL".to_string(),
                        "CAP_NET_BIND_SERVICE".to_string(),
                    ],
                    ambient: vec!["CAP_NET_BIND_SERVICE".to_string()],
                }),
                rlimits: vec![
                    crate::POSIXRlimit {
                        r#type: "RLIMIT_CORE".to_string(),
                        hard: 1024,
                        soft: 1024,
                    },
                    crate::POSIXRlimit {
                        r#type: "RLIMIT_NOFILE".to_string(),
                        hard: 1024,
                        soft: 1024,
                    },
                ],
                no_new_privileges: true,
                apparmor_profile: "acme_secure_profile".to_string(),
                oom_score_adj: Some(100),
                scheduler: None,
                selinux_label: "system_u:system_r:svirt_lxc_net_t:s0:c124,c675".to_string(),
                io_priority: None,
            }),
            root: Some(crate::Root {
                path: "rootfs".to_string(),
                readonly: true,
            }),
            domainname: "".to_string(),
            hostname: "slartibartfast".to_string(),
            mounts: vec![
                crate::Mount {
                    destination: "/proc".to_string(),
                    r#type: "proc".to_string(),
                    source: "proc".to_string(),
                    options: vec![],
                    uid_mappings: vec![],
                    gid_mappings: vec![],
                },
                crate::Mount {
                    destination: "/dev".to_string(),
                    r#type: "tmpfs".to_string(),
                    source: "tmpfs".to_string(),
                    options: vec![
                        "nosuid".to_string(),
                        "strictatime".to_string(),
                        "mode=755".to_string(),
                        "size=65536k".to_string(),
                    ],
                    uid_mappings: vec![],
                    gid_mappings: vec![],
                },
                crate::Mount {
                    destination: "/dev/pts".to_string(),
                    r#type: "devpts".to_string(),
                    source: "devpts".to_string(),
                    options: vec![
                        "nosuid".to_string(),
                        "noexec".to_string(),
                        "newinstance".to_string(),
                        "ptmxmode=0666".to_string(),
                        "mode=0620".to_string(),
                        "gid=5".to_string(),
                    ],
                    uid_mappings: vec![],
                    gid_mappings: vec![],
                },
                crate::Mount {
                    destination: "/dev/shm".to_string(),
                    r#type: "tmpfs".to_string(),
                    source: "shm".to_string(),
                    options: vec![
                        "nosuid".to_string(),
                        "noexec".to_string(),
                        "nodev".to_string(),
                        "mode=1777".to_string(),
                        "size=65536k".to_string(),
                    ],
                    uid_mappings: vec![],
                    gid_mappings: vec![],
                },
                crate::Mount {
                    destination: "/dev/mqueue".to_string(),
                    r#type: "mqueue".to_string(),
                    source: "mqueue".to_string(),
                    options: vec![
                        "nosuid".to_string(),
                        "noexec".to_string(),
                        "nodev".to_string(),
                    ],
                    uid_mappings: vec![],
                    gid_mappings: vec![],
                },
                crate::Mount {
                    destination: "/sys".to_string(),
                    r#type: "sysfs".to_string(),
                    source: "sysfs".to_string(),
                    options: vec![
                        "nosuid".to_string(),
                        "noexec".to_string(),
                        "nodev".to_string(),
                    ],
                    uid_mappings: vec![],
                    gid_mappings: vec![],
                },
                crate::Mount {
                    destination: "/sys/fs/cgroup".to_string(),
                    r#type: "cgroup".to_string(),
                    source: "cgroup".to_string(),
                    options: vec![
                        "nosuid".to_string(),
                        "noexec".to_string(),
                        "nodev".to_string(),
                        "relatime".to_string(),
                        "ro".to_string(),
                    ],
                    uid_mappings: vec![],
                    gid_mappings: vec![],
                },
            ],
            hooks: Some(crate::Hooks {
                prestart: vec![
                    crate::Hook {
                        path: "/usr/bin/fix-mounts".to_string(),
                        args: vec![
                            "fix-mounts".to_string(),
                            "arg1".to_string(),
                            "arg2".to_string(),
                        ],
                        env: vec!["key1=value1".to_string()],
                        timeout: None,
                    },
                    crate::Hook {
                        path: "/usr/bin/setup-network".to_string(),
                        args: vec![],
                        env: vec![],
                        timeout: None,
                    },
                ],
                create_runtime: vec![crate::Hook {
                    path: "/usr/local/bin/nerdctl".to_string(),
                    args: vec![],
                    env: vec![],
                    timeout: None,
                }],
                poststart: vec![crate::Hook {
                    path: "/usr/bin/notify-start".to_string(),
                    args: vec![],
                    env: vec![],
                    timeout: Some(5),
                }],
                poststop: vec![crate::Hook {
                    path: "/usr/sbin/cleanup.sh".to_string(),
                    args: vec!["cleanup.sh".to_string(), "-f".to_string()],
                    env: vec![],
                    timeout: None,
                }],
                ..Default::default()
            }),
            annotations: [
                ("com.example.key1".to_string(), "value1".to_string()),
                ("com.example.key2".to_string(), "value2".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
            linux: Some(crate::Linux {
                uid_mappings: vec![crate::LinuxIDMapping {
                    container_id: 0,
                    host_id: 1000,
                    size: 32000,
                }],
                gid_mappings: vec![crate::LinuxIDMapping {
                    container_id: 0,
                    host_id: 1000,
                    size: 32000,
                }],
                sysctl: [
                    ("net.ipv4.ip_forward".to_string(), "1".to_string()),
                    ("net.core.somaxconn".to_string(), "256".to_string()),
                ]
                .iter()
                .cloned()
                .collect(),
                resources: Some(crate::LinuxResources {
                    devices: vec![
                        crate::LinuxDeviceCgroup {
                            allow: false,
                            r#type: "".to_string(),
                            major: None,
                            minor: None,
                            access: "rwm".to_string(),
                        },
                        crate::LinuxDeviceCgroup {
                            allow: true,
                            r#type: "c".to_string(),
                            major: Some(10),
                            minor: Some(229),
                            access: "rw".to_string(),
                        },
                        crate::LinuxDeviceCgroup {
                            allow: true,
                            r#type: "b".to_string(),
                            major: Some(8),
                            minor: Some(0),
                            access: "r".to_string(),
                        },
                    ],
                    memory: Some(crate::LinuxMemory {
                        limit: Some(536870912),
                        reservation: Some(536870912),
                        swap: Some(536870912),
                        kernel: Some(-1),
                        kernel_tcp: Some(-1),
                        swappiness: Some(0),
                        disable_oom_killer: Some(false),
                        use_hierarchy: None,
                        check_before_update: None,
                    }),
                    cpu: Some(crate::LinuxCPU {
                        shares: Some(1024),
                        quota: Some(1000000),
                        burst: None,
                        period: Some(500000),
                        realtime_runtime: Some(950000),
                        realtime_period: Some(1000000),
                        cpus: "2-3".to_string(),
                        mems: "0-7".to_string(),
                        idle: None,
                    }),
                    pids: Some(crate::LinuxPids { limit: 32771 }),
                    block_io: Some(crate::LinuxBlockIO {
                        weight: Some(10),
                        leaf_weight: Some(10),
                        weight_device: vec![
                            crate::LinuxWeightDevice {
                                blk: crate::LinuxBlockIODevice { major: 8, minor: 0 },
                                weight: Some(500),
                                leaf_weight: Some(300),
                            },
                            crate::LinuxWeightDevice {
                                blk: crate::LinuxBlockIODevice {
                                    major: 8,
                                    minor: 16,
                                },
                                weight: Some(500),
                                leaf_weight: None,
                            },
                        ],
                        throttle_read_bps_device: vec![crate::LinuxThrottleDevice {
                            blk: crate::LinuxBlockIODevice { major: 8, minor: 0 },
                            rate: 600,
                        }],
                        throttle_write_bps_device: vec![],
                        throttle_read_iops_device: vec![],
                        throttle_write_iops_device: vec![crate::LinuxThrottleDevice {
                            blk: crate::LinuxBlockIODevice {
                                major: 8,
                                minor: 16,
                            },
                            rate: 300,
                        }],
                    }),
                    hugepage_limits: vec![
                        crate::LinuxHugepageLimit {
                            page_size: "2MB".to_string(),
                            limit: 9223372036854772000,
                        },
                        crate::LinuxHugepageLimit {
                            page_size: "64KB".to_string(),
                            limit: 1000000,
                        },
                    ],
                    network: Some(crate::LinuxNetwork {
                        class_id: Some(1048577),
                        priorities: vec![
                            crate::LinuxInterfacePriority {
                                name: "eth0".to_string(),
                                priority: 500,
                            },
                            crate::LinuxInterfacePriority {
                                name: "eth1".to_string(),
                                priority: 1000,
                            },
                        ],
                    }),
                    rdma: Default::default(),
                    unified: HashMap::new(),
                }),
                cgroups_path: "/myRuntime/myContainer".to_string(),
                namespaces: vec![
                    crate::LinuxNamespace {
                        r#type: "pid".to_string(),
                        path: "".to_string(),
                    },
                    crate::LinuxNamespace {
                        r#type: "network".to_string(),
                        path: "".to_string(),
                    },
                    crate::LinuxNamespace {
                        r#type: "ipc".to_string(),
                        path: "".to_string(),
                    },
                    crate::LinuxNamespace {
                        r#type: "uts".to_string(),
                        path: "".to_string(),
                    },
                    crate::LinuxNamespace {
                        r#type: "mount".to_string(),
                        path: "".to_string(),
                    },
                    crate::LinuxNamespace {
                        r#type: "user".to_string(),
                        path: "".to_string(),
                    },
                    crate::LinuxNamespace {
                        r#type: "cgroup".to_string(),
                        path: "".to_string(),
                    },
                ],
                devices: vec![
                    crate::LinuxDevice {
                        path: "/dev/fuse".to_string(),
                        r#type: "c".to_string(),
                        major: 10,
                        minor: 229,
                        file_mode: Some(438),
                        uid: Some(0),
                        gid: Some(0),
                    },
                    crate::LinuxDevice {
                        path: "/dev/sda".to_string(),
                        r#type: "b".to_string(),
                        major: 8,
                        minor: 0,
                        file_mode: Some(432),
                        uid: Some(0),
                        gid: Some(0),
                    },
                ],
                seccomp: Some(crate::LinuxSeccomp {
                    default_action: "SCMP_ACT_ALLOW".to_string(),
                    default_errno_ret: None,
                    architectures: vec!["SCMP_ARCH_X86".to_string(), "SCMP_ARCH_X32".to_string()],
                    flags: vec![],
                    listener_path: "".to_string(),
                    listener_metadata: "".to_string(),
                    syscalls: vec![crate::LinuxSyscall {
                        names: vec!["getcwd".to_string(), "chmod".to_string()],
                        action: "SCMP_ACT_ERRNO".to_string(),
                        errno_ret: crate::default_seccomp_errno(),
                        args: vec![],
                    }],
                }),
                rootfs_propagation: "slave".to_string(),
                masked_paths: vec![
                    "/proc/kcore".to_string(),
                    "/proc/latency_stats".to_string(),
                    "/proc/timer_stats".to_string(),
                    "/proc/sched_debug".to_string(),
                ],
                readonly_paths: vec![
                    "/proc/asound".to_string(),
                    "/proc/bus".to_string(),
                    "/proc/fs".to_string(),
                    "/proc/irq".to_string(),
                    "/proc/sys".to_string(),
                    "/proc/sysrq-trigger".to_string(),
                ],
                mount_label: "system_u:object_r:svirt_sandbox_file_t:s0:c715,c811".to_string(),
                intel_rdt: None,
                personality: None,
                time_offsets: HashMap::new(),
            }),
            solaris: None,
            windows: None,
            vm: None,
            zos: None,
        };

        let current: crate::Spec = serde_json::from_str(data).unwrap();
        assert_eq!(expected, current);
    }
}
