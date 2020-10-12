// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use serde;
#[macro_use]
use serde_derive;
use serde_json;

use protocols::oci::State as OCIState;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use nix::unistd;

use self::device::{Device, ThrottleDevice, WeightDevice};
use self::namespaces::Namespaces;
use crate::specconv::CreateOpts;

pub mod device;
pub mod namespaces;
pub mod validator;

#[derive(Serialize, Deserialize, Debug)]
pub struct Rlimit {
    #[serde(default)]
    r#type: i32,
    #[serde(default)]
    hard: i32,
    #[serde(default)]
    soft: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IDMap {
    #[serde(default)]
    container_id: i32,
    #[serde(default)]
    host_id: i32,
    #[serde(default)]
    size: i32,
}

type Action = i32;

#[derive(Serialize, Deserialize, Debug)]
pub struct Seccomp {
    #[serde(default)]
    default_action: Action,
    #[serde(default)]
    architectures: Vec<String>,
    #[serde(default)]
    syscalls: Vec<Syscall>,
}

type Operator = i32;

#[derive(Serialize, Deserialize, Debug)]
pub struct Arg {
    #[serde(default)]
    index: u32,
    #[serde(default)]
    value: u64,
    #[serde(default)]
    value_two: u64,
    #[serde(default)]
    op: Operator,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Syscall {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default)]
    action: Action,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<Arg>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config<'a> {
    #[serde(default)]
    no_pivot_root: bool,
    #[serde(default)]
    parent_death_signal: i32,
    #[serde(default)]
    rootfs: String,
    #[serde(default)]
    readonlyfs: bool,
    #[serde(default, rename = "rootPropagation")]
    root_propagation: i32,
    #[serde(default)]
    mounts: Vec<Mount>,
    #[serde(default)]
    devices: Vec<Device>,
    #[serde(default)]
    mount_label: String,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    namespaces: Namespaces,
    #[serde(default)]
    capabilities: Option<Capabilities>,
    #[serde(default)]
    networks: Vec<Network>,
    #[serde(default)]
    routes: Vec<Route>,
    #[serde(default)]
    cgroups: Option<Cgroup<'a>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    apparmor_profile: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    process_label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rlimits: Vec<Rlimit>,
    #[serde(default)]
    oom_score_adj: Option<i32>,
    #[serde(default)]
    uid_mappings: Vec<IDMap>,
    #[serde(default)]
    gid_mappings: Vec<IDMap>,
    #[serde(default)]
    mask_paths: Vec<String>,
    #[serde(default)]
    readonly_paths: Vec<String>,
    #[serde(default)]
    sysctl: HashMap<String, String>,
    #[serde(default)]
    seccomp: Option<Seccomp>,
    #[serde(default)]
    no_new_privileges: bool,
    hooks: Option<Hooks>,
    #[serde(default)]
    version: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    no_new_keyring: bool,
    #[serde(default)]
    intel_rdt: Option<IntelRdt>,
    #[serde(default)]
    rootless_euid: bool,
    #[serde(default)]
    rootless_cgroups: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Hooks {
    prestart: Vec<Box<Hook>>,
    poststart: Vec<Box<Hook>>,
    poststop: Vec<Box<Hook>>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Capabilities {
    bounding: Vec<String>,
    effective: Vec<String>,
    inheritable: Vec<String>,
    permitted: Vec<String>,
    ambient: Vec<String>,
}

pub trait Hook {
    fn run(&self, state: &OCIState) -> Result<()>;
}

pub struct FuncHook {
    // run: fn(&OCIState) -> Result<()>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Command {
    #[serde(default)]
    path: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: Vec<String>,
    #[serde(default)]
    dir: String,
    #[serde(default)]
    timeout: Duration,
}

pub struct CommandHook {
    command: Command,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mount {
    #[serde(default)]
    source: String,
    #[serde(default)]
    destination: String,
    #[serde(default)]
    device: String,
    #[serde(default)]
    flags: i32,
    #[serde(default)]
    propagation_flags: Vec<i32>,
    #[serde(default)]
    data: String,
    #[serde(default)]
    relabel: String,
    #[serde(default)]
    extensions: i32,
    #[serde(default)]
    premount_cmds: Vec<Command>,
    #[serde(default)]
    postmount_cmds: Vec<Command>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HugepageLimit {
    #[serde(default)]
    page_size: String,
    #[serde(default)]
    limit: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IntelRdt {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    l3_cache_schema: String,
    #[serde(
        default,
        rename = "memBwSchema",
        skip_serializing_if = "String::is_empty"
    )]
    mem_bw_schema: String,
}

pub type FreezerState = String;

#[derive(Serialize, Deserialize, Debug)]
pub struct Cgroup<'a> {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    parent: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    scope_prefix: String,
    paths: HashMap<String, String>,
    resource: &'a Resources<'a>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Resources<'a> {
    #[serde(default)]
    allow_all_devices: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allowed_devices: Vec<&'a Device>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    denied_devices: Vec<&'a Device>,
    #[serde(default)]
    devices: Vec<&'a Device>,
    #[serde(default)]
    memory: i64,
    #[serde(default)]
    memory_reservation: i64,
    #[serde(default)]
    memory_swap: i64,
    #[serde(default)]
    kernel_memory: i64,
    #[serde(default)]
    kernel_memory_tcp: i64,
    #[serde(default)]
    cpu_shares: u64,
    #[serde(default)]
    cpu_quota: i64,
    #[serde(default)]
    cpu_period: u64,
    #[serde(default)]
    cpu_rt_quota: i64,
    #[serde(default)]
    cpu_rt_period: u64,
    #[serde(default)]
    cpuset_cpus: String,
    #[serde(default)]
    cpuset_mems: String,
    #[serde(default)]
    pids_limit: i64,
    #[serde(default)]
    blkio_weight: u64,
    #[serde(default)]
    blkio_leaf_weight: u64,
    #[serde(default)]
    blkio_weight_device: Vec<&'a WeightDevice>,
    #[serde(default)]
    blkio_throttle_read_bps_device: Vec<&'a ThrottleDevice>,
    #[serde(default)]
    blkio_throttle_write_bps_device: Vec<&'a ThrottleDevice>,
    #[serde(default)]
    blkio_throttle_read_iops_device: Vec<&'a ThrottleDevice>,
    #[serde(default)]
    blkio_throttle_write_iops_device: Vec<&'a ThrottleDevice>,
    #[serde(default)]
    freezer: FreezerState,
    #[serde(default)]
    hugetlb_limit: Vec<&'a HugepageLimit>,
    #[serde(default)]
    oom_kill_disable: bool,
    #[serde(default)]
    memory_swapiness: u64,
    #[serde(default)]
    net_prio_ifpriomap: Vec<&'a IfPrioMap>,
    #[serde(default)]
    net_cls_classid_u: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Network {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    bridge: String,
    #[serde(default)]
    mac_address: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    gateway: String,
    #[serde(default)]
    ipv6_address: String,
    #[serde(default)]
    ipv6_gateway: String,
    #[serde(default)]
    mtu: i32,
    #[serde(default)]
    txqueuelen: i32,
    #[serde(default)]
    host_interface_name: String,
    #[serde(default)]
    hairpin_mode: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Route {
    #[serde(default)]
    destination: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    gateway: String,
    #[serde(default)]
    interface_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IfPrioMap {
    #[serde(default)]
    interface: String,
    #[serde(default)]
    priority: i32,
}

impl IfPrioMap {
    fn cgroup_string(&self) -> String {
        format!("{} {}", self.interface, self.priority)
    }
}
