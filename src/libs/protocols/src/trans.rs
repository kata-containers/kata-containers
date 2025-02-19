// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashSet;
use std::convert::From;
use std::convert::TryFrom;
use std::path::PathBuf;

use crate::oci as grpc;
use oci_spec::runtime as oci;

// translate from interface to ttprc tools
pub fn from_option<F: Sized, T: From<F>>(from: Option<F>) -> protobuf::MessageField<T> {
    match from {
        Some(f) => protobuf::MessageField::some(f.into()),
        None => protobuf::MessageField::none(),
    }
}

pub fn from_option_vec<F: Sized + Clone, T: From<F>>(from: Option<Vec<F>>) -> Vec<T> {
    match from {
        Some(f) => f.into_iter().map(|f| f.into()).collect(),
        None => vec![],
    }
}

fn cap_hashset2vec(hash_set: &Option<HashSet<oci::Capability>>) -> Vec<String> {
    match hash_set {
        Some(set) => set
            .iter()
            .map(|cap: &oci::Capability| cap.to_string())
            .collect::<Vec<_>>(),
        None => Vec::new(),
    }
}

fn cap_vec2hashset(caps: Vec<String>) -> HashSet<oci::Capability> {
    caps.iter()
        .map(|cap: &String| {
            // cap might be JSON-encoded
            let decoded: &str = serde_json::from_str(cap).unwrap_or(cap);
            decoded.strip_prefix("CAP_").unwrap_or(decoded)
                .parse::<oci::Capability>()
                .unwrap_or_else(|_| panic!("Failed to parse {:?} to Enum Capability", cap))
        })
        .collect()
}

fn option_vec_to_vec<T>(option_vec: &Option<Vec<T>>) -> Vec<T>
where
    T: Clone,
{
    match option_vec {
        Some(vec) => vec.clone(),
        None => Vec::new(),
    }
}

impl From<oci::Box> for grpc::Box {
    fn from(b: oci::Box) -> Self {
        grpc::Box {
            Height: b.height() as u32,
            Width: b.width() as u32,
            ..Default::default()
        }
    }
}

impl From<oci::User> for grpc::User {
    fn from(from: oci::User) -> Self {
        grpc::User {
            UID: from.uid(),
            GID: from.gid(),
            AdditionalGids: option_vec_to_vec(from.additional_gids()),
            Username: from
                .username()
                .as_ref()
                .map_or(String::new(), |x| x.clone()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxCapabilities> for grpc::LinuxCapabilities {
    fn from(from: oci::LinuxCapabilities) -> Self {
        grpc::LinuxCapabilities {
            Bounding: cap_hashset2vec(from.bounding()),
            Effective: cap_hashset2vec(from.effective()),
            Inheritable: cap_hashset2vec(from.inheritable()),
            Permitted: cap_hashset2vec(from.permitted()),
            Ambient: cap_hashset2vec(from.ambient()),
            ..Default::default()
        }
    }
}

// TODO(burgerdev): remove condition here and below after upgrading to oci_spec > 0.7.
#[cfg(target_os = "linux")]
impl From<oci::PosixRlimit> for grpc::POSIXRlimit {
    fn from(from: oci::PosixRlimit) -> Self {
        grpc::POSIXRlimit {
            Type: from.typ().to_string(),
            Hard: from.hard(),
            Soft: from.soft(),
            ..Default::default()
        }
    }
}

impl From<oci::Process> for grpc::Process {
    fn from(from: oci::Process) -> Self {
        grpc::Process {
            Terminal: from.terminal().map_or(false, |t| t),
            ConsoleSize: from_option(from.console_size()),
            User: from_option(Some(from.user().clone())),
            Args: option_vec_to_vec(from.args()),
            Env: option_vec_to_vec(from.env()),
            Cwd: from.cwd().display().to_string(),
            Capabilities: from_option(from.capabilities().clone()),
            #[cfg(target_os = "linux")]
            Rlimits: from_option_vec(from.rlimits().clone()),
            NoNewPrivileges: from.no_new_privileges().unwrap_or_default(),
            ApparmorProfile: from
                .apparmor_profile()
                .as_ref()
                .map_or(String::new(), |x| x.clone()),
            OOMScoreAdj: from.oom_score_adj().map_or(0, |t| t as i64),
            SelinuxLabel: from
                .selinux_label()
                .as_ref()
                .map_or(String::new(), |x| x.clone()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxDeviceCgroup> for grpc::LinuxDeviceCgroup {
    fn from(from: oci::LinuxDeviceCgroup) -> Self {
        grpc::LinuxDeviceCgroup {
            Allow: from.allow(),
            Type: from.typ().map_or(String::new(), |x| x.as_str().to_string()),
            Major: from.major().map_or(0, |t| t),
            Minor: from.minor().map_or(0, |t| t),
            Access: from.access().as_ref().map_or(String::new(), |x| x.clone()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxMemory> for grpc::LinuxMemory {
    fn from(from: oci::LinuxMemory) -> Self {
        grpc::LinuxMemory {
            Limit: from.limit().map_or(0, |t| t),
            Reservation: from.reservation().map_or(0, |t| t),
            Swap: from.swap().map_or(0, |t| t),
            Kernel: from.kernel().map_or(0, |t| t),
            KernelTCP: from.kernel_tcp().map_or(0, |t| t),
            Swappiness: from.swappiness().map_or(0, |t| t),
            DisableOOMKiller: from.disable_oom_killer().map_or(false, |t| t),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxCpu> for grpc::LinuxCPU {
    fn from(from: oci::LinuxCpu) -> Self {
        grpc::LinuxCPU {
            Shares: from.shares().map_or(0, |t| t),
            Quota: from.quota().map_or(0, |t| t),
            Period: from.period().map_or(0, |t| t),
            RealtimeRuntime: from.realtime_runtime().map_or(0, |t| t),
            RealtimePeriod: from.realtime_period().map_or(0, |t| t),
            Cpus: from.cpus().as_ref().map_or(String::new(), |x| x.clone()),
            Mems: from.mems().as_ref().map_or(String::new(), |m| m.clone()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxPids> for grpc::LinuxPids {
    fn from(from: oci::LinuxPids) -> Self {
        grpc::LinuxPids {
            Limit: from.limit(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxWeightDevice> for grpc::LinuxWeightDevice {
    fn from(from: oci::LinuxWeightDevice) -> Self {
        grpc::LinuxWeightDevice {
            Major: from.major(),
            Minor: from.minor(),
            Weight: from.weight().map_or(0u32, |t| t as u32),
            LeafWeight: from.leaf_weight().map_or(0u32, |t| t as u32),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxThrottleDevice> for grpc::LinuxThrottleDevice {
    fn from(from: oci::LinuxThrottleDevice) -> Self {
        grpc::LinuxThrottleDevice {
            Major: from.major(),
            Minor: from.minor(),
            Rate: from.rate(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxBlockIo> for grpc::LinuxBlockIO {
    fn from(block_io: oci::LinuxBlockIo) -> Self {
        grpc::LinuxBlockIO {
            Weight: block_io.weight().map_or(0u32, |w| w as u32),
            LeafWeight: block_io.leaf_weight().map_or(0u32, |w| w as u32),
            WeightDevice: from_option_vec(block_io.weight_device().clone()),
            ThrottleReadBpsDevice: from_option_vec(block_io.throttle_read_bps_device().clone()),
            ThrottleWriteBpsDevice: from_option_vec(block_io.throttle_write_bps_device().clone()),
            ThrottleReadIOPSDevice: from_option_vec(block_io.throttle_read_iops_device().clone()),
            ThrottleWriteIOPSDevice: from_option_vec(block_io.throttle_write_iops_device().clone()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxHugepageLimit> for grpc::LinuxHugepageLimit {
    fn from(from: oci::LinuxHugepageLimit) -> Self {
        grpc::LinuxHugepageLimit {
            Pagesize: from.page_size().to_owned(),
            Limit: from.limit() as u64,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxInterfacePriority> for grpc::LinuxInterfacePriority {
    fn from(from: oci::LinuxInterfacePriority) -> Self {
        grpc::LinuxInterfacePriority {
            Name: from.name().to_owned(),
            Priority: from.priority(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxNetwork> for grpc::LinuxNetwork {
    fn from(from: oci::LinuxNetwork) -> Self {
        grpc::LinuxNetwork {
            ClassID: from.class_id().map_or(0, |t| t),
            Priorities: from_option_vec(from.priorities().clone()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxResources> for grpc::LinuxResources {
    fn from(from: oci::LinuxResources) -> Self {
        grpc::LinuxResources {
            Devices: from_option_vec(from.devices().clone()),
            Memory: from_option(*from.memory()),
            CPU: from_option(from.cpu().clone()),
            Pids: from_option(*from.pids()),
            BlockIO: from_option(from.block_io().clone()),
            HugepageLimits: from_option_vec(from.hugepage_limits().clone()),
            Network: from_option(from.network().clone()),
            ..Default::default()
        }
    }
}

impl From<oci::Root> for grpc::Root {
    fn from(from: oci::Root) -> Self {
        grpc::Root {
            Path: from.path().display().to_string(),
            Readonly: from.readonly().unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::Mount> for grpc::Mount {
    fn from(from: oci::Mount) -> Self {
        grpc::Mount {
            destination: from.destination().display().to_string(),
            source: from
                .source()
                .as_ref()
                .map_or(String::new(), |x| x.clone().display().to_string()),
            type_: from.typ().as_ref().map_or(String::new(), |x| x.clone()),
            options: option_vec_to_vec(from.options()),
            ..Default::default()
        }
    }
}

impl From<oci::Hook> for grpc::Hook {
    fn from(from: oci::Hook) -> Self {
        grpc::Hook {
            Path: from.path().display().to_string(),
            Args: option_vec_to_vec(from.args()),
            Env: option_vec_to_vec(from.env()),
            Timeout: from.timeout().as_ref().map_or(0i64, |x| *x),
            ..Default::default()
        }
    }
}

impl From<oci::Hooks> for grpc::Hooks {
    fn from(from: oci::Hooks) -> Self {
        grpc::Hooks {
            Prestart: from_option_vec(from.prestart().clone()),
            CreateRuntime: from_option_vec(from.create_runtime().clone()),
            CreateContainer: from_option_vec(from.create_container().clone()),
            StartContainer: from_option_vec(from.start_container().clone()),
            Poststart: from_option_vec(from.poststart().clone()),
            Poststop: from_option_vec(from.poststop().clone()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxIdMapping> for grpc::LinuxIDMapping {
    fn from(from: oci::LinuxIdMapping) -> Self {
        grpc::LinuxIDMapping {
            HostID: from.host_id(),
            ContainerID: from.container_id(),
            Size: from.size(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxNamespace> for grpc::LinuxNamespace {
    fn from(from: oci::LinuxNamespace) -> Self {
        grpc::LinuxNamespace {
            Type: from.typ().to_string(),
            Path: from
                .path()
                .as_ref()
                .map_or(String::new(), |x| x.clone().display().to_string()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxDevice> for grpc::LinuxDevice {
    fn from(from: oci::LinuxDevice) -> Self {
        grpc::LinuxDevice {
            Path: from.path().display().to_string(),
            Type: from.typ().as_str().to_owned(),
            Major: from.major(),
            Minor: from.minor(),
            // FIXME(issue #10071): To ensure compatibility with
            // libc on non-Linux platforms, we have to temporarily
            // use the into() function for automatic conversion.
            // However, to eliminate the useless conversion warnings
            // when compiling on Linux platforms. We have to disable it.
            #[allow(clippy::useless_conversion)]
            FileMode: from.file_mode().map_or(0, |v| v.into()),
            UID: from.uid().map_or(0, |v| v),
            GID: from.gid().map_or(0, |v| v),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxSeccompArg> for grpc::LinuxSeccompArg {
    fn from(from: oci::LinuxSeccompArg) -> Self {
        grpc::LinuxSeccompArg {
            Index: from.index() as u64,
            Value: from.value(),
            ValueTwo: from.value_two().unwrap_or_default(),
            Op: from.op().to_string(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxSyscall> for grpc::LinuxSyscall {
    fn from(from: oci::LinuxSyscall) -> Self {
        grpc::LinuxSyscall {
            Names: from.names().to_vec(),
            Action: from.action().to_string(),
            Args: from_option_vec(from.args().clone()),
            ErrnoRet: Some(grpc::linux_syscall::ErrnoRet::Errnoret(
                from.errno_ret().map_or(0, |e| e),
            )),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxSeccomp> for grpc::LinuxSeccomp {
    fn from(from: oci::LinuxSeccomp) -> Self {
        grpc::LinuxSeccomp {
            DefaultAction: from.default_action().to_string(),
            Architectures: from
                .architectures()
                .as_ref()
                .map(|arches| {
                    arches
                        .iter()
                        .map(|&arch| arch.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            Syscalls: from
                .syscalls()
                .as_ref()
                .map(|syscalls| {
                    syscalls
                        .iter()
                        .cloned()
                        .map(|syscall| syscall.into())
                        .collect()
                })
                .unwrap_or_default(),
            Flags: from
                .flags()
                .as_ref()
                .map(|flags| flags.iter().map(|&flag| flag.to_string()).collect())
                .unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::Linux> for grpc::Linux {
    fn from(from: oci::Linux) -> Self {
        grpc::Linux {
            UIDMappings: from_option_vec(from.uid_mappings().clone()),
            GIDMappings: from_option_vec(from.gid_mappings().clone()),
            Sysctl: from.sysctl().clone().unwrap_or_default(),
            Resources: from_option(from.resources().clone()),
            CgroupsPath: from
                .cgroups_path()
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            Namespaces: from_option_vec(from.namespaces().clone()),
            Devices: from_option_vec(from.devices().clone()),
            Seccomp: from_option(from.seccomp().clone()),
            RootfsPropagation: from.rootfs_propagation().clone().unwrap_or_default(),
            MaskedPaths: from.masked_paths().clone().unwrap_or_default(),
            ReadonlyPaths: from.readonly_paths().clone().unwrap_or_default(),
            MountLabel: from.mount_label().clone().unwrap_or_default(),
            IntelRdt: from_option(from.intel_rdt().clone()),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxIntelRdt> for grpc::LinuxIntelRdt {
    fn from(from: oci::LinuxIntelRdt) -> Self {
        grpc::LinuxIntelRdt {
            L3CacheSchema: from.l3_cache_schema().clone().unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::Spec> for grpc::Spec {
    fn from(from: oci::Spec) -> Self {
        grpc::Spec {
            Version: from.version().to_string(),
            Process: from_option(from.process().clone()),
            Root: from_option(from.root().clone()),
            Hostname: from.hostname().clone().unwrap_or_default(),
            Mounts: from_option_vec(from.mounts().clone()),
            Hooks: from_option(from.hooks().clone()),
            Annotations: from.annotations().clone().unwrap_or_default(),
            Linux: from_option(from.linux().clone()),
            ..Default::default()
        }
    }
}

impl From<grpc::Root> for oci::Root {
    fn from(from: grpc::Root) -> Self {
        let mut oci_root = oci::Root::default();

        oci_root.set_path(PathBuf::from(&from.Path));
        oci_root.set_readonly(from.Readonly.into());

        oci_root
    }
}

// gRPC -> oci
impl From<grpc::Mount> for oci::Mount {
    fn from(mnt: grpc::Mount) -> Self {
        let mut oci_mount = oci::Mount::default();

        oci_mount.set_destination(PathBuf::from(&mnt.destination()));
        if !mnt.type_.is_empty() {
            oci_mount.set_typ(Some(mnt.type_().to_string()));
        }
        if !mnt.source.is_empty() {
            oci_mount.set_source(Some(PathBuf::from(mnt.source())));
        }
        let options = mnt.options().to_vec();
        if !options.is_empty() {
            oci_mount.set_options(Some(options));
        }

        oci_mount
    }
}

impl From<grpc::LinuxIDMapping> for oci::LinuxIdMapping {
    fn from(mapping: grpc::LinuxIDMapping) -> Self {
        oci::LinuxIdMappingBuilder::default()
            .host_id(mapping.HostID())
            .container_id(mapping.ContainerID())
            .size(mapping.Size())
            .build()
            .unwrap()
    }
}

impl From<grpc::LinuxDeviceCgroup> for oci::LinuxDeviceCgroup {
    fn from(from: grpc::LinuxDeviceCgroup) -> Self {
        let mut oci_devcgrp = oci::LinuxDeviceCgroup::default();

        oci_devcgrp.set_allow(from.Allow());
        let devcgrp_type = from
            .Type()
            .parse::<oci::LinuxDeviceType>()
            .unwrap_or_default();
        oci_devcgrp.set_typ(Some(devcgrp_type));
        if from.Major() > 0 {
            oci_devcgrp.set_major(Some(from.Major()));
        }
        if from.Minor() > 0 {
            oci_devcgrp.set_minor(Some(from.Minor()));
        }
        if !from.Access().is_empty() {
            oci_devcgrp.set_access(Some(from.Access().to_string()));
        }

        oci_devcgrp
    }
}

impl From<grpc::LinuxMemory> for oci::LinuxMemory {
    fn from(from: grpc::LinuxMemory) -> Self {
        let mut linux_mem_builder = oci::LinuxMemoryBuilder::default();

        if from.Limit() > 0 {
            linux_mem_builder = linux_mem_builder.limit(from.Limit());
        }
        if from.Reservation() > 0 {
            linux_mem_builder = linux_mem_builder.reservation(from.Reservation());
        }
        if from.Swap() > 0 {
            linux_mem_builder = linux_mem_builder.swap(from.Swap());
        }
        if from.Kernel() > 0 {
            linux_mem_builder = linux_mem_builder.kernel(from.Kernel());
        }
        if from.KernelTCP() > 0 {
            linux_mem_builder = linux_mem_builder.kernel_tcp(from.KernelTCP());
        }
        if from.Swappiness() > 0 {
            linux_mem_builder = linux_mem_builder.swappiness(from.Swappiness());
        }
        linux_mem_builder = linux_mem_builder.disable_oom_killer(from.DisableOOMKiller());

        linux_mem_builder.build().unwrap()
    }
}

impl From<grpc::LinuxCPU> for oci::LinuxCpu {
    fn from(from: grpc::LinuxCPU) -> Self {
        let mut oci_lcpu = oci::LinuxCpu::default();

        if from.Shares() > 0 {
            oci_lcpu.set_shares(Some(from.Shares()));
        }
        if from.Quota() > 0 {
            oci_lcpu.set_quota(Some(from.Quota()));
        }
        if from.Period() > 0 {
            oci_lcpu.set_period(Some(from.Period()));
        }
        if from.RealtimeRuntime() > 0 {
            oci_lcpu.set_realtime_runtime(Some(from.RealtimeRuntime()));
        }
        if from.RealtimePeriod() > 0 {
            oci_lcpu.set_realtime_period(Some(from.RealtimePeriod()));
        }
        if !from.Cpus().is_empty() {
            oci_lcpu.set_cpus(Some(from.Cpus().to_string()));
        }
        if !from.Mems().is_empty() {
            oci_lcpu.set_mems(Some(from.Mems().to_string()));
        }

        oci_lcpu
    }
}

impl From<grpc::LinuxPids> for oci::LinuxPids {
    fn from(from: grpc::LinuxPids) -> Self {
        oci::LinuxPidsBuilder::default()
            .limit(from.Limit())
            .build()
            .unwrap()
    }
}

impl From<grpc::LinuxBlockIO> for oci::LinuxBlockIo {
    fn from(from: grpc::LinuxBlockIO) -> Self {
        let mut oci_blkio = oci::LinuxBlockIo::default();

        if from.Weight() > 0 {
            oci_blkio.set_weight(Some(from.Weight() as u16));
        }
        if from.LeafWeight() > 0 {
            oci_blkio.set_leaf_weight(Some(from.LeafWeight() as u16));
        }
        let weight_device = from
            .WeightDevice()
            .iter()
            .cloned()
            .map(|dev| dev.into())
            .collect();
        let throttle_read_bps_device = from
            .ThrottleReadBpsDevice()
            .iter()
            .cloned()
            .map(|dev| dev.into())
            .collect();
        let throttle_write_bps_device = from
            .ThrottleWriteBpsDevice()
            .iter()
            .cloned()
            .map(|dev| dev.into())
            .collect();
        let throttle_read_iops_device = from
            .ThrottleReadIOPSDevice()
            .iter()
            .cloned()
            .map(|dev| dev.into())
            .collect();
        let throttle_write_iops_device = from
            .ThrottleWriteIOPSDevice()
            .iter()
            .cloned()
            .map(|dev| dev.into())
            .collect();
        oci_blkio.set_weight_device(Some(weight_device));
        oci_blkio.set_throttle_read_bps_device(Some(throttle_read_bps_device));
        oci_blkio.set_throttle_write_bps_device(Some(throttle_write_bps_device));
        oci_blkio.set_throttle_read_iops_device(Some(throttle_read_iops_device));
        oci_blkio.set_throttle_write_iops_device(Some(throttle_write_iops_device));

        oci_blkio
    }
}

impl From<grpc::LinuxThrottleDevice> for oci::LinuxThrottleDevice {
    fn from(from: grpc::LinuxThrottleDevice) -> Self {
        oci::LinuxThrottleDeviceBuilder::default()
            .major(from.Major)
            .minor(from.Minor)
            .rate(from.Rate)
            .build()
            .unwrap()
    }
}

impl From<grpc::LinuxWeightDevice> for oci::LinuxWeightDevice {
    fn from(from: grpc::LinuxWeightDevice) -> Self {
        oci::LinuxWeightDeviceBuilder::default()
            .major(from.Major)
            .minor(from.Minor)
            .weight(from.Weight as u16)
            .leaf_weight(from.LeafWeight as u16)
            .build()
            .unwrap()
    }
}

impl From<grpc::LinuxInterfacePriority> for oci::LinuxInterfacePriority {
    fn from(priority: grpc::LinuxInterfacePriority) -> Self {
        let mut oci_iface_prio = oci::LinuxInterfacePriority::default();

        oci_iface_prio.set_name(priority.Name().to_string());
        oci_iface_prio.set_priority(priority.Priority());

        oci_iface_prio
    }
}

impl From<grpc::LinuxNetwork> for oci::LinuxNetwork {
    fn from(network: grpc::LinuxNetwork) -> Self {
        let mut oci_network = oci::LinuxNetwork::default();

        if network.ClassID() > 0 {
            oci_network.set_class_id(Some(network.ClassID()));
        }
        let priorities: Vec<oci::LinuxInterfacePriority> = network
            .Priorities()
            .iter()
            .cloned()
            .map(|pri| pri.into())
            .collect();
        if !priorities.is_empty() {
            oci_network.set_priorities(Some(priorities));
        }

        oci_network
    }
}

impl From<grpc::LinuxHugepageLimit> for oci::LinuxHugepageLimit {
    fn from(from: grpc::LinuxHugepageLimit) -> Self {
        let mut oci_hugelimit = oci::LinuxHugepageLimit::default();

        oci_hugelimit.set_page_size(from.Pagesize().to_string());
        oci_hugelimit.set_limit(from.Limit() as i64);

        oci_hugelimit
    }
}

impl From<grpc::LinuxResources> for oci::LinuxResources {
    fn from(resources: grpc::LinuxResources) -> Self {
        let mut oci_resources = oci::LinuxResources::default();

        if !resources.Devices().is_empty() {
            oci_resources.set_devices(Some(
                resources
                    .Devices()
                    .iter()
                    .cloned()
                    .map(|dev| dev.into())
                    .collect(),
            ));
        }
        if resources.has_Memory() {
            oci_resources.set_memory(Some(resources.Memory().clone().into()));
        }
        if resources.has_CPU() {
            oci_resources.set_cpu(Some(resources.CPU().clone().into()));
        }
        if !resources.has_Pids() {
            oci_resources.set_pids(Some(resources.Pids().clone().into()));
        }
        if resources.has_BlockIO() {
            oci_resources.set_block_io(Some(resources.BlockIO().clone().into()));
        }
        if resources.has_Network() {
            oci_resources.set_network(Some(resources.Network().clone().into()));
        }
        if !resources.HugepageLimits().is_empty() {
            oci_resources.set_hugepage_limits(Some(
                resources
                    .HugepageLimits()
                    .iter()
                    .cloned()
                    .map(|dev| dev.into())
                    .collect(),
            ));
        }

        oci_resources
    }
}

// grpc -> oci
impl From<grpc::LinuxDevice> for oci::LinuxDevice {
    fn from(device: grpc::LinuxDevice) -> Self {
        let dev_type = device
            .Type()
            .parse::<oci::LinuxDeviceType>()
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to parse LinuxDevice {:?} to Enum LinuxDeviceType",
                    device.Type()
                )
            });

        let mut oci_linuxdev = oci::LinuxDevice::default();

        oci_linuxdev.set_path(PathBuf::from(&device.Path()));
        oci_linuxdev.set_typ(dev_type);
        oci_linuxdev.set_major(device.Major());
        oci_linuxdev.set_minor(device.Minor());
        #[allow(clippy::useless_conversion)]
        oci_linuxdev.set_file_mode(Some(device.FileMode().into()));
        oci_linuxdev.set_uid(Some(device.UID()));
        oci_linuxdev.set_gid(Some(device.GID()));

        oci_linuxdev
    }
}

impl From<grpc::LinuxSeccompArg> for oci::LinuxSeccompArg {
    fn from(from: grpc::LinuxSeccompArg) -> Self {
        oci::LinuxSeccompArgBuilder::default()
            .index(from.Index() as usize)
            .value(from.Value())
            .value_two(from.ValueTwo())
            .op(from
                .Op()
                .parse::<oci::LinuxSeccompOperator>()
                .unwrap_or_else(|_| {
                    panic!(
                        "Failed to parse LinuxSeccompArg {:?} to Enum LinuxSeccompOperator",
                        from.Op()
                    )
                }))
            .build()
            .unwrap()
    }
}

impl From<grpc::LinuxSyscall> for oci::LinuxSyscall {
    fn from(syscall: grpc::LinuxSyscall) -> Self {
        let args: Vec<oci::LinuxSeccompArg> = syscall
            .Args()
            .iter()
            .cloned()
            .map(|seccomp| seccomp.into())
            .collect();

        let mut oci_syscall = oci::LinuxSyscall::default();
        oci_syscall.set_names(syscall.Names().to_vec());
        oci_syscall.set_action(
            syscall
                .Action()
                .parse::<oci::LinuxSeccompAction>()
                .unwrap_or_else(|_| {
                    panic!(
                        "Failed to parse {:?} to Enum LinuxSeccompAction",
                        syscall.Action()
                    )
                }),
        );
        oci_syscall.set_errno_ret(Some(syscall.errnoret()));
        oci_syscall.set_args(if args.is_empty() { None } else { Some(args) });

        oci_syscall
    }
}

impl From<grpc::LinuxSeccomp> for oci::LinuxSeccomp {
    fn from(proto: grpc::LinuxSeccomp) -> Self {
        let archs: Vec<oci::Arch> = proto
            .Architectures()
            .iter()
            .map(|arg0: &String| {
                arg0.parse::<oci::Arch>().unwrap_or_else(|_| {
                    panic!("Failed to parse LinuxSeccomp {:?} to Enum Arch", arg0)
                })
            })
            .collect();
        let flags: Vec<oci::LinuxSeccompFilterFlag> = proto
            .Flags()
            .iter()
            .map(|arg0: &String| {
                arg0.parse::<oci::LinuxSeccompFilterFlag>()
                    .unwrap_or_else(|_| {
                        panic!(
                            "Failed to parse LinuxSeccomp {:?} to Enum LinuxSeccompFilterFlag",
                            arg0
                        )
                    })
            })
            .collect();
        let syscalls: Vec<oci::LinuxSyscall> = proto
            .Syscalls()
            .iter()
            .cloned()
            .map(|syscall| syscall.into())
            .collect();

        let mut oci_seccomp = oci::LinuxSeccomp::default();

        oci_seccomp.set_default_action(
            proto
                .DefaultAction()
                .parse::<oci::LinuxSeccompAction>()
                .unwrap_or_else(|_| {
                    panic!(
                        "Failed to parse LinuxSeccomp {:?} to Enum LinuxSeccompAction",
                        proto.DefaultAction()
                    )
                }),
        );
        oci_seccomp.set_architectures(Some(archs));
        oci_seccomp.set_flags(Some(flags));
        oci_seccomp.set_syscalls(Some(syscalls));

        oci_seccomp
    }
}

impl From<grpc::LinuxNamespace> for oci::LinuxNamespace {
    fn from(ns: grpc::LinuxNamespace) -> Self {
        let mut oci_ns = oci::LinuxNamespace::default();

        oci_ns.set_typ(oci::LinuxNamespaceType::try_from(ns.Type()).unwrap_or_default());
        if !ns.Path().is_empty() {
            oci_ns.set_path(Some(PathBuf::from(ns.Path())));
        }

        oci_ns
    }
}

impl From<grpc::Linux> for oci::Linux {
    fn from(from: grpc::Linux) -> Self {
        let mut oci_linux = oci::Linux::default();

        if !from.UIDMappings().is_empty() {
            oci_linux.set_uid_mappings(Some(
                from.UIDMappings()
                    .iter()
                    .cloned()
                    .map(|uid| uid.into())
                    .collect(),
            ));
        }
        if !from.GIDMappings().is_empty() {
            oci_linux.set_gid_mappings(Some(
                from.GIDMappings()
                    .iter()
                    .cloned()
                    .map(|gid| gid.into())
                    .collect(),
            ));
        }
        if !from.Sysctl().is_empty() {
            oci_linux.set_sysctl(Some(from.Sysctl().clone()));
        }
        if from.has_Resources() {
            oci_linux.set_resources(Some(from.Resources().clone().into()));
        }
        if !from.CgroupsPath().is_empty() {
            oci_linux.set_cgroups_path(Some(PathBuf::from(&from.CgroupsPath())));
        }
        if !from.Namespaces().is_empty() {
            oci_linux.set_namespaces(Some(
                from.Namespaces()
                    .iter()
                    .cloned()
                    .map(|ns| ns.into())
                    .collect(),
            ));
        } else {
            // namespaces is MUST be set None as it's initialized with default namespaces.
            oci_linux.set_namespaces(None);
        }
        if !from.Devices().is_empty() {
            oci_linux.set_devices(Some(
                from.Devices()
                    .iter()
                    .cloned()
                    .map(|dev| dev.into())
                    .collect(),
            ));
        }
        if from.has_Seccomp() {
            oci_linux.set_seccomp(Some(from.Seccomp().clone().into()));
        }
        if !from.RootfsPropagation().is_empty() {
            oci_linux.set_rootfs_propagation(Some(from.RootfsPropagation().to_string()));
        }
        if !from.MaskedPaths().is_empty() {
            oci_linux.set_masked_paths(Some(from.MaskedPaths().to_vec()));
        } else {
            // masked_paths is MUST be set None as it's initialized with default paths which are useless for kata.
            oci_linux.set_masked_paths(None);
        }
        if !from.ReadonlyPaths().is_empty() {
            oci_linux.set_readonly_paths(Some(from.ReadonlyPaths().to_vec()));
        } else {
            // readonly_paths is MUST be set None as it's initialized by default paths which are useless for kata.
            oci_linux.set_readonly_paths(None);
        }
        if !from.MountLabel().is_empty() {
            oci_linux.set_mount_label(Some(from.MountLabel().to_string()));
        }
        if from.has_IntelRdt() {
            oci_linux.set_intel_rdt(Some(from.IntelRdt().clone().into()));
        }

        oci_linux
    }
}

#[cfg(target_os = "linux")]
impl From<grpc::POSIXRlimit> for oci::PosixRlimit {
    fn from(proto: grpc::POSIXRlimit) -> Self {
        oci::PosixRlimitBuilder::default()
            .typ(
                proto
                    .Type()
                    .parse::<oci::PosixRlimitType>()
                    .unwrap_or_else(|_| {
                        panic!(
                            "Failed to parse POSIXRlimit {:?} to Enum PosixRlimitType",
                            proto.Type()
                        )
                    }),
            )
            .hard(proto.Hard())
            .soft(proto.Soft())
            .build()
            .unwrap()
    }
}

impl From<grpc::LinuxCapabilities> for oci::LinuxCapabilities {
    fn from(from: grpc::LinuxCapabilities) -> Self {
        let cap_bounding = cap_vec2hashset(from.Bounding().to_vec());
        let cap_effective = cap_vec2hashset(from.Effective().to_vec());
        let cap_inheritable = cap_vec2hashset(from.Inheritable().to_vec());
        let cap_permitted = cap_vec2hashset(from.Permitted().to_vec());
        let cap_ambient = cap_vec2hashset(from.Ambient().to_vec());

        oci::LinuxCapabilitiesBuilder::default()
            .bounding(cap_bounding)
            .effective(cap_effective)
            .inheritable(cap_inheritable)
            .permitted(cap_permitted)
            .ambient(cap_ambient)
            .build()
            .unwrap()
    }
}

impl From<grpc::User> for oci::User {
    fn from(from: grpc::User) -> Self {
        let mut user = oci::User::default();
        user.set_uid(from.UID());
        user.set_gid(from.GID());
        user.set_additional_gids(Some(from.AdditionalGids().to_vec()));
        user.set_username(Some(from.Username().to_string()));

        user
    }
}

impl From<grpc::Box> for oci::Box {
    fn from(b: grpc::Box) -> Self {
        oci::BoxBuilder::default()
            .height(b.Height() as u64)
            .width(b.Width() as u64)
            .build()
            .unwrap()
    }
}

impl From<grpc::Process> for oci::Process {
    fn from(from: grpc::Process) -> Self {
        let mut process = oci::Process::default();

        process.set_terminal(Some(from.Terminal));
        if from.has_ConsoleSize() {
            process.set_console_size(Some(from.ConsoleSize().clone().into()));
        }
        process.set_user(from.User().clone().into());
        if !from.Args().is_empty() {
            process.set_args(Some(from.Args().to_vec()));
        }
        if !from.Env().is_empty() {
            process.set_env(Some(from.Env().to_vec()));
        } else {
            process.set_env(None);
        }
        process.set_cwd(PathBuf::from(&from.Cwd()));
        if from.has_Capabilities() {
            process.set_capabilities(Some(from.Capabilities().clone().into()));
        } else {
            process.set_capabilities(None);
        }

        #[cfg(target_os = "linux")]
        if !from.Rlimits().is_empty() {
            process.set_rlimits(Some(
                from.Rlimits().iter().cloned().map(|r| r.into()).collect(),
            ));
        } else {
            process.set_rlimits(None);
        }
        process.set_no_new_privileges(Some(from.NoNewPrivileges()));
        if !from.ApparmorProfile().is_empty() {
            process.set_apparmor_profile(Some(from.ApparmorProfile().to_string()));
        }
        if from.OOMScoreAdj() != 0 {
            process.set_oom_score_adj(Some(from.OOMScoreAdj() as i32));
        }
        if !from.SelinuxLabel().is_empty() {
            process.set_selinux_label(Some(from.SelinuxLabel().to_string()));
        }

        process
    }
}

impl From<grpc::Hook> for oci::Hook {
    fn from(hook: grpc::Hook) -> Self {
        let mut oci_hook = oci::Hook::default();

        oci_hook.set_path(PathBuf::from(&hook.Path()));
        oci_hook.set_args(Some(hook.Args().to_vec()));
        oci_hook.set_env(Some(hook.Env().to_vec()));
        if hook.Timeout > 0 {
            oci_hook.set_timeout(Some(hook.Timeout()));
        }

        oci_hook
    }
}

// grpc -> oci
impl From<grpc::Hooks> for oci::Hooks {
    fn from(hooks: grpc::Hooks) -> Self {
        let mut oci_hooks = oci::Hooks::default();

        if !hooks.Prestart().is_empty() {
            oci_hooks.set_prestart(Some(
                hooks
                    .Prestart()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            ));
        }
        if !hooks.CreateRuntime().is_empty() {
            oci_hooks.set_create_runtime(Some(
                hooks
                    .CreateRuntime()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            ));
        }
        if !hooks.CreateContainer().is_empty() {
            oci_hooks.set_create_container(Some(
                hooks
                    .CreateContainer()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            ));
        }
        if !hooks.StartContainer().is_empty() {
            oci_hooks.set_start_container(Some(
                hooks
                    .StartContainer()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            ));
        }
        if !hooks.Poststart().is_empty() {
            oci_hooks.set_poststart(Some(
                hooks
                    .Poststart()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            ));
        }
        if !hooks.Poststart().is_empty() {
            oci_hooks.set_poststop(Some(
                hooks
                    .Poststart()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            ));
        }

        oci_hooks
    }
}

impl From<grpc::Spec> for oci::Spec {
    fn from(from: grpc::Spec) -> Self {
        let mut oci_spec = oci::Spec::default();

        oci_spec.set_version(from.Version().to_owned());
        if from.has_Root() {
            oci_spec.set_root(Some(from.Root().clone().into()));
        }
        if !from.Mounts().is_empty() {
            oci_spec.set_mounts(Some(
                from.Mounts()
                    .iter()
                    .cloned()
                    .map(|m| m.into())
                    .collect::<Vec<_>>(),
            ));
        } else {
            // mount is MUST be set None as it's initialized with default mounts.
            oci_spec.set_mounts(None);
        }
        if from.has_Process() {
            oci_spec.set_process(Some(from.Process().clone().into()));
        }
        if from.has_Hooks() {
            oci_spec.set_hooks(Some(from.Hooks().clone().into()));
        }
        if !from.Annotations().is_empty() {
            oci_spec.set_annotations(Some(from.Annotations()).cloned());
        }
        if from.has_Linux() {
            oci_spec.set_linux(Some(from.Linux().clone().into()));
        }
        if !from.Hostname().is_empty() {
            oci_spec.set_hostname(Some(from.Hostname().to_owned()));
        } else {
            oci_spec.set_hostname(None);
        }

        oci_spec
    }
}

impl From<grpc::LinuxIntelRdt> for oci::LinuxIntelRdt {
    fn from(from: grpc::LinuxIntelRdt) -> Self {
        let mut intel_rdt = oci::LinuxIntelRdt::default();
        intel_rdt.set_l3_cache_schema(Some(from.L3CacheSchema));

        intel_rdt
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::cap_vec2hashset;
    use super::oci;

    fn from_vec<F: Sized, T: From<F>>(from: Vec<F>) -> Vec<T> {
        let mut to: Vec<T> = vec![];
        for data in from {
            to.push(data.into());
        }
        to
    }

    #[derive(Clone)]
    struct TestA {
        pub from: String,
    }

    #[derive(Clone)]
    struct TestB {
        pub to: String,
    }

    impl From<TestA> for TestB {
        fn from(from: TestA) -> Self {
            TestB { to: from.from }
        }
    }

    #[test]
    fn test_from() {
        let from = TestA {
            from: "a".to_string(),
        };
        let to: TestB = TestB::from(from.clone());

        assert_eq!(from.from, to.to);
    }

    #[test]
    fn test_from_vec_len_0() {
        let from: Vec<TestA> = vec![];
        let to: Vec<TestB> = from_vec(from.clone());
        assert_eq!(from.len(), to.len());
    }

    #[test]
    fn test_from_vec_len_1() {
        let from: Vec<TestA> = vec![TestA {
            from: "a".to_string(),
        }];
        let to: Vec<TestB> = from_vec(from.clone());

        assert_eq!(from.len(), to.len());
        assert_eq!(from[0].from, to[0].to);
    }

    #[test]
    fn test_cap_vec2hashset_good() {
        let expected: HashSet<oci::Capability> =
            vec![oci::Capability::NetAdmin, oci::Capability::Mknod]
                .into_iter()
                .collect();
        let actual = cap_vec2hashset(vec![
            "CAP_NET_ADMIN".to_string(),
            "\"CAP_MKNOD\"".to_string(),
        ]);

        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic]
    fn test_cap_vec2hashset_bad() {
        cap_vec2hashset(vec![
            "CAP_DOES_NOT_EXIST".to_string(),
        ]);
    }
}
