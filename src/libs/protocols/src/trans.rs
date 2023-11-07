// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::convert::From;

use oci::{
    Hook, Hooks, Linux, LinuxBlockIO, LinuxCPU, LinuxCapabilities, LinuxDevice, LinuxHugepageLimit,
    LinuxIDMapping, LinuxIntelRdt, LinuxInterfacePriority, LinuxMemory, LinuxNamespace,
    LinuxNetwork, LinuxPids, LinuxResources, LinuxSeccomp, LinuxSeccompArg, LinuxSyscall,
    LinuxThrottleDevice, LinuxWeightDevice, Mount, POSIXRlimit, Process, Root, Spec, User,
};

// translate from interface to ttprc tools
fn from_option<F: Sized, T: From<F>>(from: Option<F>) -> protobuf::MessageField<T> {
    match from {
        Some(f) => protobuf::MessageField::from_option(Some(f.into())),
        None => protobuf::MessageField::none(),
    }
}

fn from_vec<F: Sized, T: From<F>>(from: Vec<F>) -> Vec<T> {
    let mut to: Vec<T> = vec![];
    for data in from {
        to.push(data.into());
    }
    to
}

impl From<oci::Box> for crate::oci::Box {
    fn from(from: oci::Box) -> Self {
        crate::oci::Box {
            Height: from.height,
            Width: from.width,
            ..Default::default()
        }
    }
}

impl From<oci::User> for crate::oci::User {
    fn from(from: User) -> Self {
        crate::oci::User {
            UID: from.uid,
            GID: from.gid,
            AdditionalGids: from.additional_gids,
            Username: from.username,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxCapabilities> for crate::oci::LinuxCapabilities {
    fn from(from: LinuxCapabilities) -> Self {
        crate::oci::LinuxCapabilities {
            Bounding: from.bounding,
            Effective: from.effective,
            Inheritable: from.inheritable,
            Permitted: from.permitted,
            Ambient: from.ambient,
            ..Default::default()
        }
    }
}

impl From<oci::POSIXRlimit> for crate::oci::POSIXRlimit {
    fn from(from: POSIXRlimit) -> Self {
        crate::oci::POSIXRlimit {
            Type: from.r#type,
            Hard: from.hard,
            Soft: from.soft,
            ..Default::default()
        }
    }
}

impl From<oci::Scheduler> for crate::oci::Scheduler {
    fn from(from: oci::Scheduler) -> Self {
        crate::oci::Scheduler {
            Policy: from.policy,
            Nice: from.nice,
            Priority: from.priority,
            Flags: from.flags,
            Runtime: from.runtime,
            Deadline: from.deadline,
            Period: from.period,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxPersonality> for crate::oci::LinuxPersonality {
    fn from(from: oci::LinuxPersonality) -> Self {
        crate::oci::LinuxPersonality {
            Domain: from.domain,
            Flags: from.flags,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxIOPriority> for crate::oci::LinuxIOPriority {
    fn from(from: oci::LinuxIOPriority) -> Self {
        crate::oci::LinuxIOPriority {
            Class: from.class,
            Priority: from.priority,
            ..Default::default()
        }
    }
}

impl From<oci::Process> for crate::oci::Process {
    fn from(from: Process) -> Self {
        crate::oci::Process {
            Terminal: from.terminal,
            ConsoleSize: from_option(from.console_size),
            User: from_option(Some(from.user)),
            Args: from.args,
            Env: from.env,
            Cwd: from.cwd,
            Capabilities: from_option(from.capabilities),
            Rlimits: from_vec(from.rlimits),
            NoNewPrivileges: from.no_new_privileges,
            ApparmorProfile: from.apparmor_profile,
            OOMScoreAdj: from.oom_score_adj.unwrap_or_default(),
            SelinuxLabel: from.selinux_label,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxDeviceCgroup> for crate::oci::LinuxDeviceCgroup {
    fn from(from: oci::LinuxDeviceCgroup) -> Self {
        crate::oci::LinuxDeviceCgroup {
            Allow: from.allow,
            Type: from.r#type,
            Major: from.major.unwrap_or_default(),
            Minor: from.minor.unwrap_or_default(),
            Access: from.access,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxMemory> for crate::oci::LinuxMemory {
    fn from(from: LinuxMemory) -> Self {
        crate::oci::LinuxMemory {
            Limit: from.limit.unwrap_or_default(),
            Reservation: from.reservation.unwrap_or_default(),
            Swap: from.swap.unwrap_or_default(),
            Kernel: from.kernel.unwrap_or_default(),
            KernelTCP: from.kernel_tcp.unwrap_or_default(),
            Swappiness: from.swappiness.unwrap_or_default(),
            DisableOOMKiller: from.disable_oom_killer.unwrap_or_default(),
            UseHierarchy: from.use_hierarchy.unwrap_or_default(),
            CheckBeforeUpdate: from.check_before_update.unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxCPU> for crate::oci::LinuxCPU {
    fn from(from: LinuxCPU) -> Self {
        crate::oci::LinuxCPU {
            Shares: from.shares.unwrap_or_default(),
            Quota: from.quota.unwrap_or_default(),
            Burst: from.burst.unwrap_or_default(),
            Period: from.period.unwrap_or_default(),
            RealtimeRuntime: from.realtime_runtime.unwrap_or_default(),
            RealtimePeriod: from.realtime_period.unwrap_or_default(),
            Cpus: from.cpus,
            Mems: from.mems,
            Idle: from.idle.unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxPids> for crate::oci::LinuxPids {
    fn from(from: LinuxPids) -> Self {
        crate::oci::LinuxPids {
            Limit: from.limit,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxWeightDevice> for crate::oci::LinuxWeightDevice {
    fn from(from: LinuxWeightDevice) -> Self {
        crate::oci::LinuxWeightDevice {
            // TODO : check
            Major: 0,
            Minor: 0,
            Weight: from.weight.map_or(0, |t| t as u32),
            LeafWeight: from.leaf_weight.map_or(0, |t| t as u32),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxThrottleDevice> for crate::oci::LinuxThrottleDevice {
    fn from(from: LinuxThrottleDevice) -> Self {
        crate::oci::LinuxThrottleDevice {
            // TODO : check
            Major: 0,
            Minor: 0,
            Rate: from.rate,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxBlockIO> for crate::oci::LinuxBlockIO {
    fn from(from: LinuxBlockIO) -> Self {
        crate::oci::LinuxBlockIO {
            Weight: from.weight.map_or(0, |t| t as u32),
            LeafWeight: from.leaf_weight.map_or(0, |t| t as u32),
            WeightDevice: from_vec(from.weight_device),
            ThrottleReadBpsDevice: from_vec(from.throttle_read_bps_device),
            ThrottleWriteBpsDevice: from_vec(from.throttle_write_bps_device),
            ThrottleReadIOPSDevice: from_vec(from.throttle_read_iops_device),
            ThrottleWriteIOPSDevice: from_vec(from.throttle_write_iops_device),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxHugepageLimit> for crate::oci::LinuxHugepageLimit {
    fn from(from: LinuxHugepageLimit) -> Self {
        crate::oci::LinuxHugepageLimit {
            Pagesize: from.page_size,
            Limit: from.limit,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxInterfacePriority> for crate::oci::LinuxInterfacePriority {
    fn from(from: LinuxInterfacePriority) -> Self {
        crate::oci::LinuxInterfacePriority {
            Name: from.name,
            Priority: from.priority,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxNetwork> for crate::oci::LinuxNetwork {
    fn from(from: LinuxNetwork) -> Self {
        crate::oci::LinuxNetwork {
            ClassID: from.class_id.map_or(0, |t| t),
            Priorities: from_vec(from.priorities),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxResources> for crate::oci::LinuxResources {
    fn from(from: LinuxResources) -> Self {
        crate::oci::LinuxResources {
            Devices: from_vec(from.devices),
            Memory: from_option(from.memory),
            CPU: from_option(from.cpu),
            Pids: from_option(from.pids),
            BlockIO: from_option(from.block_io),
            HugepageLimits: from_vec(from.hugepage_limits),
            Network: from_option(from.network),
            ..Default::default()
        }
    }
}

impl From<oci::Root> for crate::oci::Root {
    fn from(from: Root) -> Self {
        crate::oci::Root {
            Path: from.path,
            Readonly: from.readonly,
            ..Default::default()
        }
    }
}

impl From<oci::Mount> for crate::oci::Mount {
    fn from(from: Mount) -> Self {
        crate::oci::Mount {
            Destination: from.destination,
            Source: from.source,
            Type: from.r#type,
            Options: from.options,
            UIDMappings: from_vec(from.uid_mappings),
            GIDMappings: from_vec(from.gid_mappings),
            ..Default::default()
        }
    }
}

impl From<oci::Hook> for crate::oci::Hook {
    fn from(from: Hook) -> Self {
        crate::oci::Hook {
            Path: from.path,
            Args: from.args,
            Env: from.env,
            Timeout: from.timeout.unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::Hooks> for crate::oci::Hooks {
    fn from(from: Hooks) -> Self {
        crate::oci::Hooks {
            Prestart: from_vec(from.prestart),
            CreateRuntime: from_vec(from.create_runtime),
            CreateContainer: from_vec(from.create_container),
            StartContainer: from_vec(from.start_container),
            Poststart: from_vec(from.poststart),
            Poststop: from_vec(from.poststop),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxIDMapping> for crate::oci::LinuxIDMapping {
    fn from(from: LinuxIDMapping) -> Self {
        crate::oci::LinuxIDMapping {
            HostID: from.host_id,
            ContainerID: from.container_id,
            Size: from.size,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxNamespace> for crate::oci::LinuxNamespace {
    fn from(from: LinuxNamespace) -> Self {
        crate::oci::LinuxNamespace {
            Type: from.r#type,
            Path: from.path,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxTimeOffset> for crate::oci::LinuxTimeOffset {
    fn from(from: oci::LinuxTimeOffset) -> Self {
        crate::oci::LinuxTimeOffset {
            Secs: from.secs,
            Nanosecs: from.nanosecs,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxDevice> for crate::oci::LinuxDevice {
    fn from(from: LinuxDevice) -> Self {
        crate::oci::LinuxDevice {
            Path: from.path,
            Type: from.r#type,
            Major: from.major,
            Minor: from.minor,
            FileMode: from.file_mode.unwrap_or_default(),
            UID: from.uid.unwrap_or_default(),
            GID: from.gid.unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxSeccompArg> for crate::oci::LinuxSeccompArg {
    fn from(from: LinuxSeccompArg) -> Self {
        crate::oci::LinuxSeccompArg {
            Index: from.index,
            Value: from.value,
            ValueTwo: from.value_two,
            Op: from.op,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxSyscall> for crate::oci::LinuxSyscall {
    fn from(from: LinuxSyscall) -> Self {
        crate::oci::LinuxSyscall {
            Names: from.names,
            Action: from.action,
            Args: from_vec(from.args),
            ErrnoRet: Some(crate::oci::linux_syscall::ErrnoRet::Errnoret(
                from.errno_ret,
            )),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxSeccomp> for crate::oci::LinuxSeccomp {
    fn from(from: LinuxSeccomp) -> Self {
        crate::oci::LinuxSeccomp {
            DefaultAction: from.default_action,
            DefaultErrnoRet: from.default_errno_ret.unwrap_or_default(),
            Architectures: from.architectures,
            Flags: from.flags,
            ListenerPath: from.listener_path,
            ListenerMetadata: from.listener_metadata,
            Syscalls: from_vec(from.syscalls),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxIntelRdt> for crate::oci::LinuxIntelRdt {
    fn from(from: LinuxIntelRdt) -> Self {
        crate::oci::LinuxIntelRdt {
            ClosID: from.clos_id,
            L3CacheSchema: from.l3_cache_schema,
            MemBwSchema: from.mem_bw_schema,
            EnableCMT: from.enable_cmt,
            EnableMBM: from.enable_mbm,
            ..Default::default()
        }
    }
}

impl From<oci::Linux> for crate::oci::Linux {
    fn from(from: Linux) -> Self {
        crate::oci::Linux {
            UIDMappings: from_vec(from.uid_mappings),
            GIDMappings: from_vec(from.gid_mappings),
            Sysctl: from.sysctl,
            Resources: from_option(from.resources),
            CgroupsPath: from.cgroups_path,
            Namespaces: from_vec(from.namespaces),
            Devices: from_vec(from.devices),
            Seccomp: from_option(from.seccomp),
            RootfsPropagation: from.rootfs_propagation,
            MaskedPaths: from.masked_paths,
            ReadonlyPaths: from.readonly_paths,
            MountLabel: from.mount_label,
            IntelRdt: from_option(from.intel_rdt),
            Personality: from_option(from.personality),
            TimeOffsets: from
                .time_offsets
                .iter()
                .map(|(k, v)| (k.clone(), v.clone().into()))
                .collect::<HashMap<String, crate::oci::LinuxTimeOffset>>(),
            ..Default::default()
        }
    }
}

impl From<oci::Spec> for crate::oci::Spec {
    fn from(from: Spec) -> Self {
        crate::oci::Spec {
            Version: from.version,
            Process: from_option(from.process),
            Root: from_option(from.root),
            Hostname: from.hostname,
            Mounts: from_vec(from.mounts),
            Hooks: from_option(from.hooks),
            Annotations: from.annotations,
            Linux: from_option(from.linux),
            Solaris: Default::default(),
            Windows: Default::default(),
            ..Default::default()
        }
    }
}

impl From<crate::oci::Root> for oci::Root {
    fn from(mut from: crate::oci::Root) -> Self {
        Self {
            path: from.take_Path(),
            readonly: from.Readonly(),
        }
    }
}

impl From<crate::oci::Mount> for oci::Mount {
    fn from(mut from: crate::oci::Mount) -> Self {
        Self {
            r#type: from.take_Type(),
            destination: from.take_Destination(),
            source: from.take_Source(),
            options: from.take_Options(),
            uid_mappings: from_vec(from.take_UIDMappings()),
            gid_mappings: from_vec(from.take_GIDMappings()),
        }
    }
}

impl From<crate::oci::LinuxIDMapping> for oci::LinuxIDMapping {
    fn from(from: crate::oci::LinuxIDMapping) -> Self {
        LinuxIDMapping {
            container_id: from.ContainerID(),
            host_id: from.HostID(),
            size: from.Size(),
        }
    }
}

impl From<crate::oci::LinuxDeviceCgroup> for oci::LinuxDeviceCgroup {
    fn from(mut from: crate::oci::LinuxDeviceCgroup) -> Self {
        let major = if from.Major() > 0 {
            Some(from.Major())
        } else {
            None
        };

        let minor = if from.Minor() > 0 {
            Some(from.Minor())
        } else {
            None
        };

        oci::LinuxDeviceCgroup {
            allow: from.Allow(),
            r#type: from.take_Type(),
            major,
            minor,
            access: from.take_Access(),
        }
    }
}

impl From<crate::oci::LinuxMemory> for oci::LinuxMemory {
    fn from(from: crate::oci::LinuxMemory) -> Self {
        let limit = if from.Limit() > 0 {
            Some(from.Limit())
        } else {
            None
        };

        let reservation = if from.Reservation() > 0 {
            Some(from.Reservation())
        } else {
            None
        };

        let swap = if from.Swap() > 0 {
            Some(from.Swap())
        } else {
            None
        };

        let kernel = if from.Kernel() > 0 {
            Some(from.Kernel())
        } else {
            None
        };

        let kernel_tcp = if from.KernelTCP() > 0 {
            Some(from.KernelTCP())
        } else {
            None
        };

        let swappiness = if from.Swappiness() > 0 {
            Some(from.Swappiness())
        } else {
            None
        };

        oci::LinuxMemory {
            limit,
            reservation,
            swap,
            kernel,
            kernel_tcp,
            swappiness,
            disable_oom_killer: Some(from.DisableOOMKiller()),
            use_hierarchy: Some(from.UseHierarchy()),
            check_before_update: Some(from.CheckBeforeUpdate()),
        }
    }
}

impl From<crate::oci::LinuxCPU> for oci::LinuxCPU {
    fn from(mut from: crate::oci::LinuxCPU) -> Self {
        let shares = if from.Shares() > 0 {
            Some(from.Shares())
        } else {
            None
        };

        let quota = if from.Quota() > 0 {
            Some(from.Quota())
        } else {
            None
        };

        let burst = if from.Burst() > 0 {
            Some(from.Burst())
        } else {
            None
        };

        let period = if from.Period() > 0 {
            Some(from.Period())
        } else {
            None
        };

        let realtime_runtime = if from.RealtimeRuntime() > 0 {
            Some(from.RealtimeRuntime())
        } else {
            None
        };

        let realtime_period = if from.RealtimePeriod() > 0 {
            Some(from.RealtimePeriod())
        } else {
            None
        };

        let idle = if from.Idle() > 0 {
            Some(from.Idle())
        } else {
            None
        };

        oci::LinuxCPU {
            shares,
            quota,
            burst,
            period,
            realtime_runtime,
            realtime_period,
            cpus: from.take_Cpus(),
            mems: from.take_Mems(),
            idle,
        }
    }
}

impl From<crate::oci::LinuxPids> for oci::LinuxPids {
    fn from(from: crate::oci::LinuxPids) -> Self {
        oci::LinuxPids {
            limit: from.Limit(),
        }
    }
}

impl From<crate::oci::LinuxBlockIO> for oci::LinuxBlockIO {
    fn from(mut from: crate::oci::LinuxBlockIO) -> Self {
        let weight = if from.Weight() > 0 {
            Some(from.Weight() as u16)
        } else {
            None
        };

        let leaf_weight = if from.LeafWeight() > 0 {
            Some(from.LeafWeight() as u16)
        } else {
            None
        };

        oci::LinuxBlockIO {
            weight,
            leaf_weight,
            weight_device: from_vec(from.take_WeightDevice()),
            throttle_read_bps_device: from_vec(from.take_ThrottleReadBpsDevice()),
            throttle_write_bps_device: from_vec(from.take_ThrottleWriteBpsDevice()),
            throttle_read_iops_device: from_vec(from.take_ThrottleReadIOPSDevice()),
            throttle_write_iops_device: from_vec(from.take_ThrottleWriteIOPSDevice()),
        }
    }
}

impl From<crate::oci::LinuxThrottleDevice> for oci::LinuxThrottleDevice {
    fn from(from: crate::oci::LinuxThrottleDevice) -> Self {
        oci::LinuxThrottleDevice {
            blk: oci::LinuxBlockIODevice {
                major: from.Major(),
                minor: from.Minor(),
            },
            rate: from.Rate(),
        }
    }
}

impl From<crate::oci::LinuxWeightDevice> for oci::LinuxWeightDevice {
    fn from(from: crate::oci::LinuxWeightDevice) -> Self {
        oci::LinuxWeightDevice {
            blk: oci::LinuxBlockIODevice {
                major: from.Major(),
                minor: from.Minor(),
            },
            weight: Some(from.Weight() as u16),
            leaf_weight: Some(from.LeafWeight() as u16),
        }
    }
}

impl From<crate::oci::LinuxInterfacePriority> for oci::LinuxInterfacePriority {
    fn from(mut from: crate::oci::LinuxInterfacePriority) -> Self {
        oci::LinuxInterfacePriority {
            name: from.take_Name(),
            priority: from.Priority(),
        }
    }
}

impl From<crate::oci::LinuxNetwork> for oci::LinuxNetwork {
    fn from(mut from: crate::oci::LinuxNetwork) -> Self {
        let class_id = if from.ClassID() > 0 {
            Some(from.ClassID())
        } else {
            None
        };

        oci::LinuxNetwork {
            class_id,
            priorities: from_vec(from.take_Priorities()),
        }
    }
}

impl From<crate::oci::LinuxHugepageLimit> for oci::LinuxHugepageLimit {
    fn from(mut from: crate::oci::LinuxHugepageLimit) -> Self {
        oci::LinuxHugepageLimit {
            page_size: from.take_Pagesize(),
            limit: from.Limit(),
        }
    }
}

impl From<crate::oci::LinuxResources> for oci::LinuxResources {
    fn from(mut from: crate::oci::LinuxResources) -> Self {
        let memory = if from.has_Memory() {
            Some(from.take_Memory().into())
        } else {
            None
        };

        let cpu = if from.has_CPU() {
            Some(from.take_CPU().into())
        } else {
            None
        };

        let pids = if from.has_Pids() {
            Some(from.take_Pids().into())
        } else {
            None
        };

        let block_io = if from.has_BlockIO() {
            Some(from.take_BlockIO().into())
        } else {
            None
        };

        let network = if from.has_Network() {
            Some(from.take_Network().into())
        } else {
            None
        };

        LinuxResources {
            devices: from_vec(from.take_Devices()),
            memory,
            cpu,
            pids,
            block_io,
            hugepage_limits: from_vec(from.take_HugepageLimits()),
            network,
            rdma: HashMap::new(),
            unified: HashMap::new(),
        }
    }
}

impl From<crate::oci::LinuxDevice> for oci::LinuxDevice {
    fn from(mut from: crate::oci::LinuxDevice) -> Self {
        oci::LinuxDevice {
            path: from.take_Path(),
            r#type: from.take_Type(),
            major: from.Major(),
            minor: from.Minor(),
            file_mode: Some(from.FileMode()),
            uid: Some(from.UID()),
            gid: Some(from.GID()),
        }
    }
}

impl From<crate::oci::LinuxSeccompArg> for oci::LinuxSeccompArg {
    fn from(mut from: crate::oci::LinuxSeccompArg) -> Self {
        oci::LinuxSeccompArg {
            index: from.Index(),
            value: from.Value(),
            value_two: from.ValueTwo(),
            op: from.take_Op(),
        }
    }
}

impl From<crate::oci::LinuxSyscall> for oci::LinuxSyscall {
    fn from(mut from: crate::oci::LinuxSyscall) -> Self {
        oci::LinuxSyscall {
            names: from.take_Names(),
            action: from.take_Action(),
            args: from_vec(from.take_Args()),
            errno_ret: from.errnoret(),
        }
    }
}

impl From<crate::oci::LinuxSeccomp> for oci::LinuxSeccomp {
    fn from(mut from: crate::oci::LinuxSeccomp) -> Self {
        let default_errno_ret = if from.DefaultErrnoRet() > 0 {
            Some(from.DefaultErrnoRet())
        } else {
            None
        };

        oci::LinuxSeccomp {
            default_action: from.take_DefaultAction(),
            default_errno_ret,
            architectures: from.take_Architectures(),
            flags: from.take_Flags(),
            listener_path: from.take_ListenerPath(),
            listener_metadata: from.take_ListenerMetadata(),
            syscalls: from_vec(from.take_Syscalls()),
        }
    }
}

impl From<crate::oci::LinuxNamespace> for oci::LinuxNamespace {
    fn from(mut from: crate::oci::LinuxNamespace) -> Self {
        oci::LinuxNamespace {
            r#type: from.take_Type(),
            path: from.take_Path(),
        }
    }
}

impl From<crate::oci::LinuxTimeOffset> for oci::LinuxTimeOffset {
    fn from(from: crate::oci::LinuxTimeOffset) -> Self {
        oci::LinuxTimeOffset {
            secs: from.Secs(),
            nanosecs: from.Nanosecs(),
        }
    }
}

impl From<crate::oci::Linux> for oci::Linux {
    fn from(mut from: crate::oci::Linux) -> Self {
        let resources = if from.has_Resources() {
            Some(from.take_Resources().into())
        } else {
            None
        };

        let seccomp = if from.has_Seccomp() {
            Some(from.take_Seccomp().into())
        } else {
            None
        };

        let personality = if from.has_Personality() {
            Some(from.take_Personality().into())
        } else {
            None
        };

        let time_offsets = from
            .take_TimeOffsets()
            .into_iter()
            .map(|(k, v)| (k, v.into()))
            .collect();

        oci::Linux {
            uid_mappings: from_vec(from.take_UIDMappings()),
            gid_mappings: from_vec(from.take_GIDMappings()),
            sysctl: from.take_Sysctl(),
            resources,
            cgroups_path: from.take_CgroupsPath(),
            namespaces: from_vec(from.take_Namespaces()),
            devices: from_vec(from.take_Devices()),
            seccomp,
            rootfs_propagation: from.take_RootfsPropagation(),
            masked_paths: from.take_MaskedPaths(),
            readonly_paths: from.take_ReadonlyPaths(),
            mount_label: from.take_MountLabel(),
            intel_rdt: None,
            personality,
            time_offsets,
        }
    }
}

impl From<crate::oci::POSIXRlimit> for oci::POSIXRlimit {
    fn from(mut from: crate::oci::POSIXRlimit) -> Self {
        oci::POSIXRlimit {
            r#type: from.take_Type(),
            hard: from.Hard(),
            soft: from.Soft(),
        }
    }
}

impl From<crate::oci::LinuxCapabilities> for oci::LinuxCapabilities {
    fn from(mut from: crate::oci::LinuxCapabilities) -> Self {
        oci::LinuxCapabilities {
            bounding: from.take_Bounding(),
            effective: from.take_Effective(),
            inheritable: from.take_Inheritable(),
            permitted: from.take_Permitted(),
            ambient: from.take_Ambient(),
        }
    }
}

impl From<crate::oci::User> for oci::User {
    fn from(mut from: crate::oci::User) -> Self {
        let umask = if from.Umask() != 0 {
            Some(from.Umask())
        } else {
            None
        };

        oci::User {
            uid: from.UID(),
            gid: from.GID(),
            umask,
            additional_gids: from.take_AdditionalGids(),
            username: from.take_Username(),
        }
    }
}

impl From<crate::oci::Box> for oci::Box {
    fn from(from: crate::oci::Box) -> Self {
        oci::Box {
            height: from.Height(),
            width: from.Width(),
        }
    }
}

impl From<crate::oci::Scheduler> for oci::Scheduler {
    fn from(mut from: crate::oci::Scheduler) -> Self {
        oci::Scheduler {
            policy: from.take_Policy(),
            nice: from.Nice(),
            priority: from.Priority(),
            flags: from.take_Flags(),
            runtime: from.Runtime(),
            deadline: from.Deadline(),
            period: from.Period(),
        }
    }
}

impl From<crate::oci::LinuxPersonality> for oci::LinuxPersonality {
    fn from(mut from: crate::oci::LinuxPersonality) -> Self {
        oci::LinuxPersonality {
            domain: from.take_Domain(),
            flags: from.take_Flags(),
        }
    }
}

impl From<crate::oci::LinuxIOPriority> for oci::LinuxIOPriority {
    fn from(mut from: crate::oci::LinuxIOPriority) -> Self {
        oci::LinuxIOPriority {
            class: from.take_Class(),
            priority: from.Priority(),
        }
    }
}

impl From<crate::oci::Process> for oci::Process {
    fn from(mut from: crate::oci::Process) -> Self {
        let console_size = if from.has_ConsoleSize() {
            Some(from.take_ConsoleSize().into())
        } else {
            None
        };

        let capabilities = if from.has_Capabilities() {
            Some(from.take_Capabilities().into())
        } else {
            None
        };

        let rlimits = from_vec(from.take_Rlimits());

        let oom_score_adj = if from.OOMScoreAdj() != 0 {
            Some(from.OOMScoreAdj())
        } else {
            None
        };

        let scheduler = if from.has_Scheduler() {
            Some(from.take_Scheduler().into())
        } else {
            None
        };

        let io_priority = if from.has_IOPriority() {
            Some(from.take_IOPriority().into())
        } else {
            None
        };

        oci::Process {
            terminal: from.Terminal(),
            console_size,
            user: from.take_User().into(),
            args: from.take_Args(),
            command_line: from.take_CommandLine(),
            env: from.take_Env(),
            cwd: from.take_Cwd(),
            capabilities,
            rlimits,
            no_new_privileges: from.NoNewPrivileges(),
            apparmor_profile: from.take_ApparmorProfile(),
            oom_score_adj,
            scheduler,
            selinux_label: from.take_SelinuxLabel(),
            io_priority,
        }
    }
}

impl From<crate::oci::Hook> for oci::Hook {
    fn from(mut from: crate::oci::Hook) -> Self {
        let timeout = if from.Timeout() > 0 {
            Some(from.Timeout())
        } else {
            None
        };

        oci::Hook {
            path: from.take_Path(),
            args: from.take_Args(),
            env: from.take_Env(),
            timeout,
        }
    }
}

impl From<crate::oci::Hooks> for oci::Hooks {
    fn from(mut from: crate::oci::Hooks) -> Self {
        oci::Hooks {
            prestart: from_vec(from.take_Poststart()),
            create_runtime: from_vec(from.take_CreateRuntime()),
            create_container: from_vec(from.take_CreateContainer()),
            start_container: from_vec(from.take_StartContainer()),
            poststart: from_vec(from.take_Poststart()),
            poststop: from_vec(from.take_Poststop()),
        }
    }
}

impl From<crate::oci::Spec> for oci::Spec {
    fn from(mut from: crate::oci::Spec) -> Self {
        let process = if from.has_Process() {
            Some(from.take_Process().into())
        } else {
            None
        };

        let root = if from.has_Root() {
            Some(from.take_Root().into())
        } else {
            None
        };

        let hooks: Option<oci::Hooks> = if from.has_Hooks() {
            Some(from.take_Hooks().into())
        } else {
            None
        };

        let linux = if from.has_Linux() {
            Some(from.take_Linux().into())
        } else {
            None
        };

        oci::Spec {
            version: from.take_Version(),
            process,
            root,
            hostname: from.take_Hostname(),
            domainname: from.take_Domainname(),
            mounts: from_vec(from.take_Mounts()),
            hooks,
            annotations: from.take_Annotations(),
            linux,
            solaris: None,
            windows: None,
            vm: None,
            zos: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::trans::from_vec;

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
}
