// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::convert::From;

use oci::{
    Hook, Hooks, Linux, LinuxBlockIo, LinuxCapabilities, LinuxCpu, LinuxDevice, LinuxHugepageLimit,
    LinuxIdMapping, LinuxIntelRdt, LinuxInterfacePriority, LinuxMemory, LinuxNamespace,
    LinuxNetwork, LinuxPids, LinuxResources, LinuxSeccomp, LinuxSeccompArg, LinuxSyscall,
    LinuxThrottleDevice, LinuxWeightDevice, Mount, PosixRlimit, Process, Root, Spec, User,
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

impl From<oci::PosixRlimit> for crate::oci::POSIXRlimit {
    fn from(from: PosixRlimit) -> Self {
        crate::oci::POSIXRlimit {
            Type: from.r#type,
            Hard: from.hard,
            Soft: from.soft,
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
            OOMScoreAdj: from.oom_score_adj.map_or(0, |t| t as i64),
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
            Major: from.major.map_or(0, |t| t),
            Minor: from.minor.map_or(0, |t| t),
            Access: from.access,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxMemory> for crate::oci::LinuxMemory {
    fn from(from: LinuxMemory) -> Self {
        crate::oci::LinuxMemory {
            Limit: from.limit.map_or(0, |t| t),
            Reservation: from.reservation.map_or(0, |t| t),
            Swap: from.swap.map_or(0, |t| t),
            Kernel: from.kernel.map_or(0, |t| t),
            KernelTCP: from.kernel_tcp.map_or(0, |t| t),
            Swappiness: from.swappiness.map_or(0, |t| t),
            DisableOOMKiller: from.disable_oom_killer.map_or(false, |t| t),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxCpu> for crate::oci::LinuxCPU {
    fn from(from: LinuxCpu) -> Self {
        crate::oci::LinuxCPU {
            Shares: from.shares.map_or(0, |t| t),
            Quota: from.quota.map_or(0, |t| t),
            Period: from.period.map_or(0, |t| t),
            RealtimeRuntime: from.realtime_runtime.map_or(0, |t| t),
            RealtimePeriod: from.realtime_period.map_or(0, |t| t),
            Cpus: from.cpus,
            Mems: from.mems,
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

impl From<oci::LinuxBlockIo> for crate::oci::LinuxBlockIO {
    fn from(from: LinuxBlockIo) -> Self {
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
            destination: from.destination,
            source: from.source,
            type_: from.r#type,
            options: from.options,
            ..Default::default()
        }
    }
}

impl From<oci::Hook> for crate::oci::Hook {
    fn from(from: Hook) -> Self {
        let mut timeout: i64 = 0;
        if let Some(v) = from.timeout {
            timeout = v as i64;
        }
        crate::oci::Hook {
            Path: from.path,
            Args: from.args,
            Env: from.env,
            Timeout: timeout,
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

impl From<oci::LinuxIdMapping> for crate::oci::LinuxIDMapping {
    fn from(from: LinuxIdMapping) -> Self {
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

impl From<oci::LinuxDevice> for crate::oci::LinuxDevice {
    fn from(from: LinuxDevice) -> Self {
        crate::oci::LinuxDevice {
            Path: from.path,
            Type: from.r#type,
            Major: from.major,
            Minor: from.minor,
            FileMode: from.file_mode.map_or(0, |v| v),
            UID: from.uid.map_or(0, |v| v),
            GID: from.gid.map_or(0, |v| v),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxSeccompArg> for crate::oci::LinuxSeccompArg {
    fn from(from: LinuxSeccompArg) -> Self {
        crate::oci::LinuxSeccompArg {
            Index: from.index as u64,
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
            Architectures: from.architectures,
            Syscalls: from_vec(from.syscalls),
            Flags: from.flags,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxIntelRdt> for crate::oci::LinuxIntelRdt {
    fn from(from: LinuxIntelRdt) -> Self {
        crate::oci::LinuxIntelRdt {
            L3CacheSchema: from.l3_cache_schema,
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
    fn from(from: crate::oci::Root) -> Self {
        Self {
            path: from.Path,
            readonly: from.Readonly,
        }
    }
}

impl From<crate::oci::Mount> for oci::Mount {
    fn from(mut from: crate::oci::Mount) -> Self {
        let options = from.take_options().to_vec();
        Self {
            r#type: from.take_type_(),
            destination: from.take_destination(),
            source: from.take_source(),
            options,
        }
    }
}

impl From<crate::oci::LinuxIDMapping> for oci::LinuxIdMapping {
    fn from(from: crate::oci::LinuxIDMapping) -> Self {
        LinuxIdMapping {
            container_id: from.ContainerID(),
            host_id: from.HostID(),
            size: from.Size(),
        }
    }
}

impl From<crate::oci::LinuxDeviceCgroup> for oci::LinuxDeviceCgroup {
    fn from(mut from: crate::oci::LinuxDeviceCgroup) -> Self {
        let mut major = None;
        if from.Major() > 0 {
            major = Some(from.Major());
        }

        let mut minor = None;
        if from.Minor() > 0 {
            minor = Some(from.Minor())
        }

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
        let mut limit = None;
        if from.Limit() > 0 {
            limit = Some(from.Limit());
        }

        let mut reservation = None;
        if from.Reservation() > 0 {
            reservation = Some(from.Reservation());
        }

        let mut swap = None;
        if from.Swap() > 0 {
            swap = Some(from.Swap());
        }

        let mut kernel = None;
        if from.Kernel() > 0 {
            kernel = Some(from.Kernel());
        }

        let mut kernel_tcp = None;
        if from.KernelTCP() > 0 {
            kernel_tcp = Some(from.KernelTCP());
        }

        let mut swappiness = None;
        if from.Swappiness() > 0 {
            swappiness = Some(from.Swappiness());
        }

        let disable_oom_killer = Some(from.DisableOOMKiller());

        oci::LinuxMemory {
            limit,
            reservation,
            swap,
            kernel,
            kernel_tcp,
            swappiness,
            disable_oom_killer,
        }
    }
}

impl From<crate::oci::LinuxCPU> for oci::LinuxCpu {
    fn from(mut from: crate::oci::LinuxCPU) -> Self {
        let mut shares = None;
        if from.Shares() > 0 {
            shares = Some(from.Shares());
        }

        let mut quota = None;
        if from.Quota() > 0 {
            quota = Some(from.Quota());
        }

        let mut period = None;
        if from.Period() > 0 {
            period = Some(from.Period());
        }

        let mut realtime_runtime = None;
        if from.RealtimeRuntime() > 0 {
            realtime_runtime = Some(from.RealtimeRuntime());
        }

        let mut realtime_period = None;
        if from.RealtimePeriod() > 0 {
            realtime_period = Some(from.RealtimePeriod());
        }

        let cpus = from.take_Cpus();
        let mems = from.take_Mems();

        oci::LinuxCpu {
            shares,
            quota,
            period,
            realtime_runtime,
            realtime_period,
            cpus,
            mems,
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

impl From<crate::oci::LinuxBlockIO> for oci::LinuxBlockIo {
    fn from(from: crate::oci::LinuxBlockIO) -> Self {
        let mut weight = None;
        if from.Weight() > 0 {
            weight = Some(from.Weight() as u16);
        }
        let mut leaf_weight = None;
        if from.LeafWeight() > 0 {
            leaf_weight = Some(from.LeafWeight() as u16);
        }
        let mut weight_device = Vec::new();
        for wd in from.WeightDevice() {
            weight_device.push(wd.clone().into());
        }

        let mut throttle_read_bps_device = Vec::new();
        for td in from.ThrottleReadBpsDevice() {
            throttle_read_bps_device.push(td.clone().into());
        }

        let mut throttle_write_bps_device = Vec::new();
        for td in from.ThrottleWriteBpsDevice() {
            throttle_write_bps_device.push(td.clone().into());
        }

        let mut throttle_read_iops_device = Vec::new();
        for td in from.ThrottleReadIOPSDevice() {
            throttle_read_iops_device.push(td.clone().into());
        }

        let mut throttle_write_iops_device = Vec::new();
        for td in from.ThrottleWriteIOPSDevice() {
            throttle_write_iops_device.push(td.clone().into());
        }

        oci::LinuxBlockIo {
            weight,
            leaf_weight,
            weight_device,
            throttle_read_bps_device,
            throttle_write_bps_device,
            throttle_read_iops_device,
            throttle_write_iops_device,
        }
    }
}

impl From<crate::oci::LinuxThrottleDevice> for oci::LinuxThrottleDevice {
    fn from(from: crate::oci::LinuxThrottleDevice) -> Self {
        oci::LinuxThrottleDevice {
            blk: oci::LinuxBlockIoDevice {
                major: from.Major,
                minor: from.Minor,
            },
            rate: from.Rate,
        }
    }
}

impl From<crate::oci::LinuxWeightDevice> for oci::LinuxWeightDevice {
    fn from(from: crate::oci::LinuxWeightDevice) -> Self {
        oci::LinuxWeightDevice {
            blk: oci::LinuxBlockIoDevice {
                major: from.Major,
                minor: from.Minor,
            },
            weight: Some(from.Weight as u16),
            leaf_weight: Some(from.LeafWeight as u16),
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
        let mut class_id = None;
        if from.ClassID() > 0 {
            class_id = Some(from.ClassID());
        }
        let mut priorities = Vec::new();
        for p in from.take_Priorities() {
            priorities.push(p.into())
        }

        oci::LinuxNetwork {
            class_id,
            priorities,
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
        let mut devices = Vec::new();
        for d in from.take_Devices() {
            devices.push(d.into());
        }

        let mut memory = None;
        if from.has_Memory() {
            memory = Some(from.take_Memory().into());
        }

        let mut cpu = None;
        if from.has_CPU() {
            cpu = Some(from.take_CPU().into());
        }

        let mut pids = None;
        if from.has_Pids() {
            pids = Some(from.Pids().clone().into())
        }

        let mut block_io = None;
        if from.has_BlockIO() {
            block_io = Some(from.BlockIO().clone().into());
        }

        let mut hugepage_limits = Vec::new();
        for hl in from.HugepageLimits() {
            hugepage_limits.push(hl.clone().into());
        }

        let mut network = None;
        if from.has_Network() {
            network = Some(from.take_Network().into());
        }

        let rdma = HashMap::new();

        LinuxResources {
            devices,
            memory,
            cpu,
            pids,
            block_io,
            hugepage_limits,
            network,
            rdma,
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
            index: from.Index() as u32,
            value: from.Value(),
            value_two: from.ValueTwo(),
            op: from.take_Op(),
        }
    }
}

impl From<crate::oci::LinuxSyscall> for oci::LinuxSyscall {
    fn from(mut from: crate::oci::LinuxSyscall) -> Self {
        let mut args = Vec::new();
        for ag in from.take_Args() {
            args.push(ag.into());
        }
        oci::LinuxSyscall {
            names: from.take_Names().to_vec(),
            action: from.take_Action(),
            args,
            errno_ret: from.errnoret(),
        }
    }
}

impl From<crate::oci::LinuxSeccomp> for oci::LinuxSeccomp {
    fn from(mut from: crate::oci::LinuxSeccomp) -> Self {
        let mut syscalls = Vec::new();
        for s in from.take_Syscalls() {
            syscalls.push(s.into());
        }

        oci::LinuxSeccomp {
            default_action: from.take_DefaultAction(),
            architectures: from.take_Architectures().to_vec(),
            syscalls,
            flags: from.take_Flags().to_vec(),
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

impl From<crate::oci::Linux> for oci::Linux {
    fn from(mut from: crate::oci::Linux) -> Self {
        let mut uid_mappings = Vec::new();
        for id_map in from.take_UIDMappings() {
            uid_mappings.push(id_map.into())
        }

        let mut gid_mappings = Vec::new();
        for id_map in from.take_GIDMappings() {
            gid_mappings.push(id_map.into())
        }

        let sysctl = from.Sysctl().clone();
        let mut resources = None;
        if from.has_Resources() {
            resources = Some(from.take_Resources().into());
        }

        let cgroups_path = from.take_CgroupsPath();
        let mut namespaces = Vec::new();
        for ns in from.take_Namespaces() {
            namespaces.push(ns.into())
        }

        let mut devices = Vec::new();
        for d in from.take_Devices() {
            devices.push(d.into());
        }

        let mut seccomp = None;
        if from.has_Seccomp() {
            seccomp = Some(from.take_Seccomp().into());
        }

        let rootfs_propagation = from.take_RootfsPropagation();
        let masked_paths = from.take_MaskedPaths().to_vec();

        let readonly_paths = from.take_ReadonlyPaths().to_vec();

        let mount_label = from.take_MountLabel();
        let intel_rdt = None;

        oci::Linux {
            uid_mappings,
            gid_mappings,
            sysctl,
            resources,
            cgroups_path,
            namespaces,
            devices,
            seccomp,
            rootfs_propagation,
            masked_paths,
            readonly_paths,
            mount_label,
            intel_rdt,
        }
    }
}

impl From<crate::oci::POSIXRlimit> for oci::PosixRlimit {
    fn from(mut from: crate::oci::POSIXRlimit) -> Self {
        oci::PosixRlimit {
            r#type: from.take_Type(),
            hard: from.Hard(),
            soft: from.Soft(),
        }
    }
}

impl From<crate::oci::LinuxCapabilities> for oci::LinuxCapabilities {
    fn from(mut from: crate::oci::LinuxCapabilities) -> Self {
        oci::LinuxCapabilities {
            bounding: from.take_Bounding().to_vec(),
            effective: from.take_Effective().to_vec(),
            inheritable: from.take_Inheritable().to_vec(),
            permitted: from.take_Permitted().to_vec(),
            ambient: from.take_Ambient().to_vec(),
        }
    }
}

impl From<crate::oci::User> for oci::User {
    fn from(mut from: crate::oci::User) -> Self {
        oci::User {
            uid: from.UID(),
            gid: from.GID(),
            additional_gids: from.take_AdditionalGids().to_vec(),
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

impl From<crate::oci::Process> for oci::Process {
    fn from(mut from: crate::oci::Process) -> Self {
        let mut console_size = None;
        if from.has_ConsoleSize() {
            console_size = Some(from.take_ConsoleSize().into());
        }

        let user = from.take_User().into();
        let args = from.take_Args();
        let env = from.take_Env();
        let cwd = from.take_Cwd();
        let mut capabilities = None;
        if from.has_Capabilities() {
            capabilities = Some(from.take_Capabilities().into());
        }
        let mut rlimits = Vec::new();
        for rl in from.take_Rlimits() {
            rlimits.push(rl.into());
        }
        let no_new_privileges = from.NoNewPrivileges();
        let apparmor_profile = from.take_ApparmorProfile();
        let mut oom_score_adj = None;
        if from.OOMScoreAdj() != 0 {
            oom_score_adj = Some(from.OOMScoreAdj() as i32);
        }
        let selinux_label = from.take_SelinuxLabel();

        oci::Process {
            terminal: from.Terminal,
            console_size,
            user,
            args,
            env,
            cwd,
            capabilities,
            rlimits,
            no_new_privileges,
            apparmor_profile,
            oom_score_adj,
            selinux_label,
        }
    }
}

impl From<crate::oci::Hook> for oci::Hook {
    fn from(mut from: crate::oci::Hook) -> Self {
        let mut timeout = None;
        if from.Timeout() > 0 {
            timeout = Some(from.Timeout() as i32);
        }
        oci::Hook {
            path: from.take_Path(),
            args: from.take_Args().to_vec(),
            env: from.take_Env().to_vec(),
            timeout,
        }
    }
}

impl From<crate::oci::Hooks> for oci::Hooks {
    fn from(mut from: crate::oci::Hooks) -> Self {
        let prestart = from.take_Prestart().into_iter().map(|i| i.into()).collect();
        let create_runtime = from
            .take_CreateRuntime()
            .into_iter()
            .map(|i| i.into())
            .collect();
        let create_container = from
            .take_CreateContainer()
            .into_iter()
            .map(|i| i.into())
            .collect();
        let start_container = from
            .take_StartContainer()
            .into_iter()
            .map(|i| i.into())
            .collect();
        let poststart = from
            .take_Poststart()
            .into_iter()
            .map(|i| i.into())
            .collect();
        let poststop = from.take_Poststop().into_iter().map(|i| i.into()).collect();

        oci::Hooks {
            prestart,
            create_runtime,
            create_container,
            start_container,
            poststart,
            poststop,
        }
    }
}

impl From<crate::oci::Spec> for oci::Spec {
    fn from(mut from: crate::oci::Spec) -> Self {
        let mut process = None;
        if from.has_Process() {
            process = Some(from.take_Process().into());
        }

        let mut root = None;
        if from.has_Root() {
            root = Some(from.take_Root().into());
        }

        let mut mounts = Vec::new();
        for m in from.take_Mounts() {
            mounts.push(m.into())
        }

        let mut hooks: Option<oci::Hooks> = None;
        if from.has_Hooks() {
            hooks = Some(from.take_Hooks().into());
        }

        let annotations = from.take_Annotations();

        let mut linux = None;
        if from.has_Linux() {
            linux = Some(from.take_Linux().into());
        }

        oci::Spec {
            version: from.take_Version(),
            process,
            root,
            hostname: from.take_Hostname(),
            mounts,
            hooks,
            annotations,
            linux,
            solaris: None,
            windows: None,
            vm: None,
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
