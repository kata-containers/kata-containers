// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::types::{Config, CopyFileInput, Options};
use anyhow::{anyhow, Result};
use oci::{
    Linux as ociLinux, Mount as ociMount, Process as ociProcess, Root as ociRoot, Spec as ociSpec,
};
use protocols::agent::CopyFileRequest;
use protocols::oci::{
    Box as ttrpcBox, Linux as ttrpcLinux, LinuxBlockIO as ttrpcLinuxBlockIO,
    LinuxCPU as ttrpcLinuxCPU, LinuxCapabilities as ttrpcLinuxCapabilities,
    LinuxDevice as ttrpcLinuxDevice, LinuxDeviceCgroup as ttrpcLinuxDeviceCgroup,
    LinuxHugepageLimit as ttrpcLinuxHugepageLimit, LinuxIDMapping as ttrpcLinuxIDMapping,
    LinuxIntelRdt as ttrpcLinuxIntelRdt, LinuxInterfacePriority as ttrpcLinuxInterfacePriority,
    LinuxMemory as ttrpcLinuxMemory, LinuxNamespace as ttrpcLinuxNamespace,
    LinuxNetwork as ttrpcLinuxNetwork, LinuxPids as ttrpcLinuxPids,
    LinuxResources as ttrpcLinuxResources, LinuxSeccomp as ttrpcLinuxSeccomp,
    LinuxSeccompArg as ttrpcLinuxSeccompArg, LinuxSyscall as ttrpcLinuxSyscall,
    LinuxThrottleDevice as ttrpcLinuxThrottleDevice, LinuxWeightDevice as ttrpcLinuxWeightDevice,
    Mount as ttrpcMount, Process as ttrpcProcess, Root as ttrpcRoot, Spec as ttrpcSpec,
    User as ttrpcUser,
};
use rand::Rng;
use serde::de::DeserializeOwned;
use slog::{debug, warn};
use std::collections::HashMap;
use std::fs::{self, File};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// Length of a sandbox identifier
const SANDBOX_ID_LEN: u8 = 64;

const FILE_URI: &str = "file://";

// Length of the guests hostname
const MIN_HOSTNAME_LEN: u8 = 8;

// Name of the OCI configuration file found at the root of an OCI bundle.
const CONFIG_FILE: &str = "config.json";

lazy_static! {
    // Create a mutable hash map statically
    static ref SIGNALS: Arc<Mutex<HashMap<&'static str, u8>>> = {

        let mut m: HashMap<&'static str, u8> = HashMap::new();

        m.insert("SIGHUP", 1);
        m.insert("SIGINT", 2);
        m.insert("SIGQUIT", 3);
        m.insert("SIGILL", 4);
        m.insert("SIGTRAP", 5);
        m.insert("SIGABRT", 6);
        m.insert("SIGBUS", 7);
        m.insert("SIGFPE", 8);
        m.insert("SIGKILL", 9);
        m.insert("SIGUSR1", 10);
        m.insert("SIGSEGV", 11);
        m.insert("SIGUSR2", 12);
        m.insert("SIGPIPE", 13);
        m.insert("SIGALRM", 14);
        m.insert("SIGTERM", 15);
        m.insert("SIGSTKFLT", 16);

        // XXX:
        m.insert("SIGCHLD", 17);
        m.insert("SIGCLD", 17);

        m.insert("SIGCONT", 18);
        m.insert("SIGSTOP", 19);
        m.insert("SIGTSTP", 20);
        m.insert("SIGTTIN", 21);
        m.insert("SIGTTOU", 22);
        m.insert("SIGURG", 23);
        m.insert("SIGXCPU", 24);
        m.insert("SIGXFSZ", 25);
        m.insert("SIGVTALRM", 26);
        m.insert("SIGPROF", 27);
        m.insert("SIGWINCH", 28);
        m.insert("SIGIO", 29);
        m.insert("SIGPWR", 30);
        m.insert("SIGSYS", 31);

        Arc::new(Mutex::new(m))
    };
}

pub fn signame_to_signum(name: &str) -> Result<u8> {
    if name.is_empty() {
        return Err(anyhow!("invalid signal"));
    }

    // "fall through" on error as we assume the name is not a number, but
    // a signal name.
    if let Ok(n) = name.parse::<u8>() {
        return Ok(n);
    }

    let mut search_term = if name.starts_with("SIG") {
        name.to_string()
    } else {
        format!("SIG{}", name)
    };

    search_term = search_term.to_uppercase();

    // Access the hashmap
    let signals_ref = SIGNALS.clone();
    let m = signals_ref.lock().unwrap();

    match m.get(&*search_term) {
        Some(value) => Ok(*value),
        None => Err(anyhow!(format!("invalid signal name: {:?}", name))),
    }
}

// Convert a human time fornat (like "2s") into the equivalent number
// of nano seconds.
pub fn human_time_to_ns(human_time: &str) -> Result<i64> {
    if human_time.is_empty() || human_time.eq("0") {
        return Ok(0);
    }

    let d: humantime::Duration = human_time
        .parse::<humantime::Duration>()
        .map_err(|e| anyhow!(e))?;

    Ok(d.as_nanos() as i64)
}

// Look up the specified option name and return its value.
//
// - The function looks for the appropriate option value in the specified
//   'args' first.
// - 'args' is assumed to be a space-separated set of "name=value" pairs).
// - If not found in the args, the function looks in the global options hash.
// - If found in neither location, certain well-known options are auto-generated.
// - All other options values default to an empty string.
// - All options are saved in the global hash before being returned for future
//   use.
pub fn get_option(name: &str, options: &mut Options, args: &str) -> Result<String> {
    let words: Vec<&str> = args.split_whitespace().collect();

    for word in words {
        let fields: Vec<String> = word.split('=').map(|s| s.to_string()).collect();

        if fields.len() < 2 {
            continue;
        }

        if fields[0].is_empty() {
            continue;
        }

        let key = fields[0].clone();

        let mut value = fields[1..].join("=");

        // Expand "spec=file:///some/where/config.json"
        if key.eq("spec") && value.starts_with(FILE_URI) {
            let (_, spec_file) = split_uri(&value)?;

            if !spec_file.is_empty() {
                value = match spec_file_to_string(spec_file) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(sl!(), "failed to load spec file: {:}", e);

                        "".to_string()
                    }
                };
            }
        }

        // Command args take priority over any previous value,
        // so update the global set of options for this and all
        // subsequent commands.
        options.insert(key, value);
    }

    // Explains briefly how the option value was determined
    let mut msg = "cached";

    // If the option exists in the hash, return it
    if let Some(value) = options.get(name) {
        debug!(sl!(), "using option {:?}={:?} ({})", name, value, msg);

        return Ok(value.into());
    }

    msg = "generated";

    // Handle option values that can be auto-generated
    let value = match name {
        "cid" => random_container_id(),
        "sid" => random_sandbox_id(),

        // Default to CID
        "exec_id" => {
            msg = "derived";

            options.get("cid").unwrap_or(&"".to_string()).into()
        }
        _ => "".into(),
    };

    debug!(sl!(), "using option {:?}={:?} ({})", name, value, msg);

    // Store auto-generated value
    options.insert(name.to_string(), value.to_string());

    Ok(value)
}

pub fn generate_random_hex_string(len: u32) -> String {
    const CHARSET: &[u8] = b"abcdef0123456789";
    let mut rng = rand::thread_rng();

    let str: String = (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    str
}

pub fn random_sandbox_id() -> String {
    generate_random_hex_string(SANDBOX_ID_LEN as u32)
}

pub fn random_container_id() -> String {
    // Containers and sandboxes have same ID types
    random_sandbox_id()
}

fn config_file_from_bundle_dir(bundle_dir: &str) -> Result<String> {
    if bundle_dir.is_empty() {
        return Err(anyhow!("missing bundle directory"));
    }

    let config_path = PathBuf::from(&bundle_dir).join(CONFIG_FILE);

    config_path
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}", e).context("failed to construct config file path"))
}

fn root_oci_to_ttrpc(bundle_dir: &str, root: &ociRoot) -> Result<ttrpcRoot> {
    let root_dir = root.path.clone();

    let path = if root_dir.starts_with('/') {
        root_dir
    } else {
        // Expand the root directory into an absolute value
        let abs_root_dir = PathBuf::from(&bundle_dir).join(&root_dir);

        abs_root_dir
            .into_os_string()
            .into_string()
            .map_err(|e| anyhow!("{:?}", e).context("failed to construct bundle path"))?
    };

    let ttrpc_root = ttrpcRoot {
        Path: path,
        Readonly: root.readonly,
        ..Default::default()
    };

    Ok(ttrpc_root)
}

fn process_oci_to_ttrpc(p: &ociProcess) -> ttrpcProcess {
    let console_size = match &p.console_size {
        Some(s) => {
            let mut b = ttrpcBox::new();
            b.set_Width(s.width);
            b.set_Height(s.height);
            protobuf::MessageField::some(b)
        }
        None => protobuf::MessageField::none(),
    };

    let oom_score_adj: i64 = match p.oom_score_adj {
        Some(s) => s.into(),
        None => 0,
    };

    let mut user = ttrpcUser::new();
    user.set_UID(p.user.uid);
    user.set_GID(p.user.gid);
    user.set_AdditionalGids(p.user.additional_gids.clone());

    // FIXME: Implement RLimits OCI spec handling (copy from p.rlimits)
    //let rlimits = vec![ttrpcPOSIXRlimit::new()];
    let rlimits = Vec::new();

    let capabilities = match &p.capabilities {
        Some(c) => {
            let mut gc = ttrpcLinuxCapabilities::new();
            gc.set_Bounding(c.bounding.clone());
            gc.set_Effective(c.effective.clone());
            gc.set_Inheritable(c.inheritable.clone());
            gc.set_Permitted(c.permitted.clone());
            gc.set_Ambient(c.ambient.clone());

            protobuf::MessageField::some(gc)
        }
        None => protobuf::MessageField::none(),
    };

    let mut env = Vec::new();
    for pair in &p.env {
        env.push(pair.to_string());
    }

    ttrpcProcess {
        Terminal: p.terminal,
        ConsoleSize: console_size,
        User: protobuf::MessageField::some(user),
        Args: p.args.clone(),
        Env: env,
        Cwd: p.cwd.clone(),
        Capabilities: capabilities,
        Rlimits: rlimits,
        NoNewPrivileges: p.no_new_privileges,
        ApparmorProfile: p.apparmor_profile.clone(),
        OOMScoreAdj: oom_score_adj,
        SelinuxLabel: p.selinux_label.clone(),
        ..Default::default()
    }
}

fn mount_oci_to_ttrpc(m: &ociMount) -> ttrpcMount {
    let mut ttrpc_options = Vec::new();
    for op in &m.options {
        ttrpc_options.push(op.to_string());
    }

    ttrpcMount {
        destination: m.destination.clone(),
        source: m.source.clone(),
        type_: m.r#type.clone(),
        options: ttrpc_options,
        ..Default::default()
    }
}

fn idmaps_oci_to_ttrpc(res: &[oci::LinuxIdMapping]) -> Vec<ttrpcLinuxIDMapping> {
    let mut ttrpc_idmaps = Vec::new();
    for m in res.iter() {
        let mut idmapping = ttrpcLinuxIDMapping::default();
        idmapping.set_HostID(m.host_id);
        idmapping.set_ContainerID(m.container_id);
        idmapping.set_Size(m.size);
        ttrpc_idmaps.push(idmapping);
    }
    ttrpc_idmaps
}

fn devices_oci_to_ttrpc(res: &[oci::LinuxDeviceCgroup]) -> Vec<ttrpcLinuxDeviceCgroup> {
    let mut ttrpc_devices = Vec::new();
    for d in res.iter() {
        let mut device = ttrpcLinuxDeviceCgroup::default();
        device.set_Major(d.major.unwrap_or(0));
        device.set_Minor(d.minor.unwrap_or(0));
        device.set_Access(d.access.clone());
        device.set_Type(d.r#type.clone());
        device.set_Allow(d.allow);
        ttrpc_devices.push(device);
    }
    ttrpc_devices
}

fn memory_oci_to_ttrpc(res: &Option<oci::LinuxMemory>) -> protobuf::MessageField<ttrpcLinuxMemory> {
    let memory = if res.is_some() {
        let mem = res.as_ref().unwrap();
        protobuf::MessageField::some(ttrpcLinuxMemory {
            Limit: mem.limit.unwrap_or(0),
            Reservation: mem.reservation.unwrap_or(0),
            Swap: mem.swap.unwrap_or(0),
            Kernel: mem.kernel.unwrap_or(0),
            KernelTCP: mem.kernel_tcp.unwrap_or(0),
            Swappiness: mem.swappiness.unwrap_or(0),
            DisableOOMKiller: mem.disable_oom_killer.unwrap_or(false),
            ..Default::default()
        })
    } else {
        protobuf::MessageField::none()
    };
    memory
}

fn cpu_oci_to_ttrpc(res: &Option<oci::LinuxCpu>) -> protobuf::MessageField<ttrpcLinuxCPU> {
    match &res {
        Some(s) => {
            let mut cpu = ttrpcLinuxCPU::default();
            cpu.set_Shares(s.shares.unwrap_or(0));
            cpu.set_Quota(s.quota.unwrap_or(0));
            cpu.set_Period(s.period.unwrap_or(0));
            cpu.set_RealtimeRuntime(s.realtime_runtime.unwrap_or(0));
            cpu.set_RealtimePeriod(s.realtime_period.unwrap_or(0));
            protobuf::MessageField::some(cpu)
        }
        None => protobuf::MessageField::none(),
    }
}

fn pids_oci_to_ttrpc(res: &Option<oci::LinuxPids>) -> protobuf::MessageField<ttrpcLinuxPids> {
    match &res {
        Some(s) => {
            let mut b = ttrpcLinuxPids::new();
            b.set_Limit(s.limit);
            protobuf::MessageField::some(b)
        }
        None => protobuf::MessageField::none(),
    }
}

fn hugepage_limits_oci_to_ttrpc(res: &[oci::LinuxHugepageLimit]) -> Vec<ttrpcLinuxHugepageLimit> {
    let mut ttrpc_hugepage_limits = Vec::new();
    for h in res.iter() {
        let mut hugepage_limit = ttrpcLinuxHugepageLimit::default();
        hugepage_limit.set_Limit(h.limit);
        hugepage_limit.set_Pagesize(h.page_size.clone());
        ttrpc_hugepage_limits.push(hugepage_limit);
    }
    ttrpc_hugepage_limits
}

fn network_oci_to_ttrpc(
    res: &Option<oci::LinuxNetwork>,
) -> protobuf::MessageField<ttrpcLinuxNetwork> {
    match &res {
        Some(s) => {
            let mut b = ttrpcLinuxNetwork::new();
            b.set_ClassID(s.class_id.unwrap_or(0));
            let mut priorities = Vec::new();
            for pr in s.priorities.iter() {
                let mut lip = ttrpcLinuxInterfacePriority::new();
                lip.set_Name(pr.name.clone());
                lip.set_Priority(pr.priority);
                priorities.push(lip);
            }
            protobuf::MessageField::some(b)
        }
        None => protobuf::MessageField::none(),
    }
}

fn weight_devices_oci_to_ttrpc(res: &[oci::LinuxWeightDevice]) -> Vec<ttrpcLinuxWeightDevice> {
    let mut ttrpc_weight_devices = Vec::new();
    for dev in res.iter() {
        let mut device = ttrpcLinuxWeightDevice::default();
        device.set_Major(dev.blk.major);
        device.set_Minor(dev.blk.minor);
        let weight: u32 = match dev.weight {
            Some(s) => s.into(),
            None => 0,
        };
        device.set_Weight(weight);
        let leaf_weight: u32 = match dev.leaf_weight {
            Some(s) => s.into(),
            None => 0,
        };
        device.set_LeafWeight(leaf_weight);
        ttrpc_weight_devices.push(device);
    }
    ttrpc_weight_devices
}

fn throttle_devices_oci_to_ttrpc(
    res: &[oci::LinuxThrottleDevice],
) -> Vec<ttrpcLinuxThrottleDevice> {
    let mut ttrpc_throttle_devices = Vec::new();
    for dev in res.iter() {
        let mut device = ttrpcLinuxThrottleDevice::default();
        device.set_Major(dev.blk.major);
        device.set_Minor(dev.blk.minor);
        device.set_Rate(dev.rate);
        ttrpc_throttle_devices.push(device);
    }
    ttrpc_throttle_devices
}

fn block_io_oci_to_ttrpc(
    res: &Option<oci::LinuxBlockIo>,
) -> protobuf::MessageField<ttrpcLinuxBlockIO> {
    match &res {
        Some(s) => {
            let mut b = ttrpcLinuxBlockIO::new();
            let weight: u32 = match s.weight {
                Some(s) => s.into(),
                None => 0,
            };
            let leaf_weight: u32 = match s.leaf_weight {
                Some(s) => s.into(),
                None => 0,
            };

            b.set_Weight(weight);
            b.set_LeafWeight(leaf_weight);
            b.set_WeightDevice(weight_devices_oci_to_ttrpc(&s.weight_device));
            b.set_ThrottleReadBpsDevice(throttle_devices_oci_to_ttrpc(&s.throttle_read_bps_device));
            b.set_ThrottleReadIOPSDevice(throttle_devices_oci_to_ttrpc(
                &s.throttle_read_iops_device,
            ));
            b.set_ThrottleWriteBpsDevice(throttle_devices_oci_to_ttrpc(
                &s.throttle_write_bps_device,
            ));
            b.set_ThrottleWriteIOPSDevice(throttle_devices_oci_to_ttrpc(
                &s.throttle_write_iops_device,
            ));
            protobuf::MessageField::some(b)
        }
        None => protobuf::MessageField::none(),
    }
}

fn resources_oci_to_ttrpc(res: &oci::LinuxResources) -> ttrpcLinuxResources {
    let devices = devices_oci_to_ttrpc(&res.devices);
    let memory = memory_oci_to_ttrpc(&res.memory);
    let cpu = cpu_oci_to_ttrpc(&res.cpu);
    let pids = pids_oci_to_ttrpc(&res.pids);
    let hugepage_limits = hugepage_limits_oci_to_ttrpc(&res.hugepage_limits);
    let block_io = block_io_oci_to_ttrpc(&res.block_io);

    let network = network_oci_to_ttrpc(&res.network);
    ttrpcLinuxResources {
        Devices: devices,
        Memory: memory,
        CPU: cpu,
        Pids: pids,
        BlockIO: block_io,
        HugepageLimits: hugepage_limits,
        Network: network,
        ..Default::default()
    }
}

fn namespace_oci_to_ttrpc(res: &[oci::LinuxNamespace]) -> Vec<ttrpcLinuxNamespace> {
    let mut ttrpc_namespace = Vec::new();
    for n in res.iter() {
        let mut ns = ttrpcLinuxNamespace::default();
        ns.set_Path(n.path.clone());
        ns.set_Type(n.r#type.clone());
        ttrpc_namespace.push(ns);
    }
    ttrpc_namespace
}

fn linux_devices_oci_to_ttrpc(res: &[oci::LinuxDevice]) -> Vec<ttrpcLinuxDevice> {
    let mut ttrpc_linux_devices = Vec::new();
    for n in res.iter() {
        let mut ld = ttrpcLinuxDevice::default();
        ld.set_FileMode(n.file_mode.unwrap_or(0));
        ld.set_GID(n.gid.unwrap_or(0));
        ld.set_UID(n.uid.unwrap_or(0));
        ld.set_Major(n.major);
        ld.set_Minor(n.minor);
        ld.set_Path(n.path.clone());
        ld.set_Type(n.r#type.clone());
        ttrpc_linux_devices.push(ld);
    }
    ttrpc_linux_devices
}

fn seccomp_oci_to_ttrpc(sec: &oci::LinuxSeccomp) -> ttrpcLinuxSeccomp {
    let mut ttrpc_seccomp = ttrpcLinuxSeccomp::default();
    let mut ttrpc_arch = Vec::new();
    for a in &sec.architectures {
        ttrpc_arch.push(std::string::String::from(a));
    }
    ttrpc_seccomp.set_Architectures(ttrpc_arch);
    ttrpc_seccomp.set_DefaultAction(sec.default_action.clone());
    let mut ttrpc_flags = Vec::new();
    for f in &sec.flags {
        ttrpc_flags.push(std::string::String::from(f));
    }
    ttrpc_seccomp.set_Flags(ttrpc_flags);
    let mut ttrpc_syscalls = Vec::new();
    for sys in &sec.syscalls {
        let mut ttrpc_sys = ttrpcLinuxSyscall::default();
        ttrpc_sys.set_Action(sys.action.clone());
        let mut ttrpc_args = Vec::new();
        for arg in &sys.args {
            let mut a = ttrpcLinuxSeccompArg::default();
            a.set_Index(arg.index as u64);
            a.set_Op(arg.op.clone());
            a.set_Value(arg.value);
            a.set_ValueTwo(arg.value_two);
            ttrpc_args.push(a);
        }
        ttrpc_sys.set_Args(ttrpc_args);
        ttrpc_syscalls.push(ttrpc_sys);
    }
    ttrpc_seccomp.set_Syscalls(ttrpc_syscalls);
    ttrpc_seccomp
}
fn intel_rdt_oci_to_ttrpc(ir: &oci::LinuxIntelRdt) -> ttrpcLinuxIntelRdt {
    let mut ttrpc_intel_rdt = ttrpcLinuxIntelRdt::default();
    ttrpc_intel_rdt.set_L3CacheSchema(ir.l3_cache_schema.clone());
    ttrpc_intel_rdt
}
fn linux_oci_to_ttrpc(l: &ociLinux) -> ttrpcLinux {
    let uid_mappings = idmaps_oci_to_ttrpc(&l.uid_mappings);
    let gid_mappings = idmaps_oci_to_ttrpc(&l.gid_mappings);

    let ttrpc_linux_resources = match &l.resources {
        Some(s) => {
            let b = resources_oci_to_ttrpc(s);
            protobuf::MessageField::some(b)
        }
        None => protobuf::MessageField::none(),
    };

    let ttrpc_namespaces = namespace_oci_to_ttrpc(&l.namespaces);
    let ttrpc_linux_devices = linux_devices_oci_to_ttrpc(&l.devices);
    let ttrpc_seccomp = match &l.seccomp {
        Some(s) => {
            let b = seccomp_oci_to_ttrpc(s);
            protobuf::MessageField::some(b)
        }
        None => protobuf::MessageField::none(),
    };

    let ttrpc_intel_rdt = match &l.intel_rdt {
        Some(s) => {
            let b = intel_rdt_oci_to_ttrpc(s);
            protobuf::MessageField::some(b)
        }
        None => protobuf::MessageField::none(),
    };

    ttrpcLinux {
        UIDMappings: uid_mappings,
        GIDMappings: gid_mappings,
        Sysctl: l.sysctl.clone(),
        Resources: ttrpc_linux_resources,
        CgroupsPath: l.cgroups_path.clone(),
        Namespaces: ttrpc_namespaces,
        Devices: ttrpc_linux_devices,
        Seccomp: ttrpc_seccomp,
        RootfsPropagation: l.rootfs_propagation.clone(),
        MaskedPaths: l.masked_paths.clone(),
        ReadonlyPaths: l.readonly_paths.clone(),
        MountLabel: l.mount_label.clone(),
        IntelRdt: ttrpc_intel_rdt,
        ..Default::default()
    }
}

fn oci_to_ttrpc(bundle_dir: &str, cid: &str, oci: &ociSpec) -> Result<ttrpcSpec> {
    let process = match &oci.process {
        Some(p) => protobuf::MessageField::some(process_oci_to_ttrpc(p)),
        None => protobuf::MessageField::none(),
    };

    let root = match &oci.root {
        Some(r) => {
            let ttrpc_root = root_oci_to_ttrpc(bundle_dir, r)?;

            protobuf::MessageField::some(ttrpc_root)
        }
        None => protobuf::MessageField::none(),
    };

    let mut mounts = Vec::new();
    for m in &oci.mounts {
        mounts.push(mount_oci_to_ttrpc(m));
    }

    let linux = match &oci.linux {
        Some(l) => protobuf::MessageField::some(linux_oci_to_ttrpc(l)),
        None => protobuf::MessageField::none(),
    };

    if cid.len() < MIN_HOSTNAME_LEN as usize {
        return Err(anyhow!("container ID too short for hostname"));
    }

    // FIXME: Implement setting a custom (and unique!) hostname (requires uts ns setup)
    //let hostname = cid[0..MIN_HOSTNAME_LEN as usize].to_string();
    let hostname = "".to_string();

    let ttrpc_spec = ttrpcSpec {
        Version: oci.version.clone(),
        Process: process,
        Root: root,
        Hostname: hostname,
        Mounts: mounts,
        Hooks: protobuf::MessageField::none(),
        Annotations: HashMap::new(),
        Linux: linux,
        Solaris: protobuf::MessageField::none(),
        Windows: protobuf::MessageField::none(),
        ..Default::default()
    };

    Ok(ttrpc_spec)
}

// Split a URI and return a tuple comprising the scheme and the data.
//
// Note that we have to use our own parsing since "json://" is not
// an official schema ;(
fn split_uri(uri: &str) -> Result<(String, String)> {
    const URI_DELIMITER: &str = "://";

    let fields: Vec<&str> = uri.split(URI_DELIMITER).collect();

    if fields.len() != 2 {
        return Err(anyhow!("invalid URI: {:?}", uri));
    }

    Ok((fields[0].into(), fields[1].into()))
}

pub fn spec_file_to_string(spec_file: String) -> Result<String> {
    let oci_spec = ociSpec::load(&spec_file).map_err(|e| anyhow!(e))?;

    serde_json::to_string(&oci_spec).map_err(|e| anyhow!(e))
}

pub fn get_oci_spec_json(cfg: &Config) -> Result<String> {
    let spec_file = config_file_from_bundle_dir(&cfg.bundle_dir)?;

    spec_file_to_string(spec_file)
}

pub fn get_ttrpc_spec(options: &mut Options, cid: &str) -> Result<ttrpcSpec> {
    let bundle_dir = get_option("bundle-dir", options, "")?;

    let json_spec = get_option("spec", options, "")?;
    assert_ne!(json_spec, "");

    let oci_spec: ociSpec = serde_json::from_str(&json_spec).map_err(|e| anyhow!(e))?;

    oci_to_ttrpc(&bundle_dir, cid, &oci_spec)
}

pub fn str_to_bytes(s: &str) -> Result<Vec<u8>> {
    let prefix = "hex:";

    if s.starts_with(prefix) {
        let hex_str = s.trim_start_matches(prefix);

        let decoded = hex::decode(hex_str).map_err(|e| anyhow!(e))?;

        Ok(decoded)
    } else {
        Ok(s.as_bytes().to_vec())
    }
}

// Returns a request object of the type requested.
//
// Call as:
//
// ```rust
// let req1: SomeType = make_request(args)?;
// let req2: AnotherType = make_request(args)?;
// ```
//
// The args string can take a number of forms:
//
// - A file URI:
//
//   The string is expected to start with 'file://' with the full path to
//   a local file containing a complete or partial JSON document.
//
//   Example: 'file:///some/where/foo.json'
//
// - A JSON URI:
//
//   This invented 'json://{ ...}' URI allows either a complete JSON document
//   or a partial JSON fragment to be specified. The JSON takes the form of
//   the JSON serialised protocol buffers files that specify the Kata Agent
//   API.
//
//   - If the complete document for the specified type is provided, the values
//     specified are deserialised into the returned
//     type.
//
//   - If a partial document is provided, the values specified are
//     deserialised into the returned type and all remaining elements take their
//     default values.
//
//   - If no values are specified, all returned type will be created as
//     if TypeName::default() had been specified instead.
//
//   Example 1 (Complete and valid empty JSON document): 'json://{}'
//   Example 2 (Valid partial JSON document): 'json://{"foo": true, "bar": "hello"}'
//   Example 3 (GetGuestDetails API example):
//
//     let args = r#"json://{"mem_block_size": true, "mem_hotplug_probe": true}"#;
//
//     let req: GetGuestDetailsRequest = make_request(args)?;
//
pub fn make_request<T: Default + DeserializeOwned>(args: &str) -> Result<T> {
    if args.is_empty() {
        return Ok(Default::default());
    }

    let (scheme, data) = split_uri(args)?;

    match scheme.as_str() {
        "json" => Ok(serde_json::from_str(&data)?),
        "file" => {
            let file = File::open(data)?;

            Ok(serde_json::from_reader(file)?)
        }
        // Don't error since the args may contain key=value pairs which
        // are not handled by this functionz.
        _ => Ok(Default::default()),
    }
}

pub fn make_copy_file_request(input: &CopyFileInput) -> Result<CopyFileRequest> {
    // create dir mode permissions
    // Dir mode | 750
    let perms = 0o20000000750;

    let src_meta: fs::Metadata = fs::symlink_metadata(&input.src)?;

    let mut req = CopyFileRequest::default();

    req.set_path(input.dest.clone());
    req.set_dir_mode(perms);
    req.set_file_mode(src_meta.mode());
    req.set_uid(src_meta.uid() as i32);
    req.set_gid(src_meta.gid() as i32);
    req.set_offset(0);
    req.set_file_size(0);

    if src_meta.is_symlink() {
        match fs::read_link(&input.src)?.into_os_string().into_string() {
            Ok(path) => {
                req.set_data(path.into_bytes());
            }
            Err(_) => {
                return Err(anyhow!("failed to read link for {}", input.src));
            }
        }
    } else if src_meta.is_file() {
        req.set_file_size(src_meta.size() as i64);
    }

    Ok(req)
}
