// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashSet;
use std::convert::From;
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

fn hashset_to_vec(hash_set: &Option<HashSet<oci::Capability>>) -> Vec<String> {
    match hash_set {
        Some(set) => set
            .iter()
            .map(|cap: &oci::Capability| from_capability(cap).to_owned())
            .collect::<Vec<_>>(),
        None => Vec::new(),
    }
}

fn vec_to_hashset(caps: Vec<String>) -> HashSet<oci::Capability> {
    caps.iter()
        .map(|cap: &String| to_capability(cap).unwrap())
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

// LinuxDeviceType
pub fn to_device_type(raw: &str) -> oci::LinuxDeviceType {
    match raw {
        "b" => oci::LinuxDeviceType::B,
        "c" => oci::LinuxDeviceType::C,
        "u" => oci::LinuxDeviceType::U,
        "p" => oci::LinuxDeviceType::P,
        _ => oci::LinuxDeviceType::default(),
    }
}

// LinuxSeccompAction
pub fn from_seccomp_action(action: oci::LinuxSeccompAction) -> &'static str {
    match action {
        oci::LinuxSeccompAction::ScmpActKill => "SCMP_ACT_KILL",
        oci::LinuxSeccompAction::ScmpActKillProcess => "SCMP_ACT_KILL_PROCESS",
        oci::LinuxSeccompAction::ScmpActTrap => "SCMP_ACT_TRAP",
        oci::LinuxSeccompAction::ScmpActErrno => "SCMP_ACT_ERRNO",
        oci::LinuxSeccompAction::ScmpActNotify => "SCMP_ACT_NOTIFY",
        oci::LinuxSeccompAction::ScmpActTrace => "SCMP_ACT_TRACE",
        oci::LinuxSeccompAction::ScmpActLog => "SCMP_ACT_LOG",
        oci::LinuxSeccompAction::ScmpActAllow => "SCMP_ACT_ALLOW",
    }
}

pub fn to_seccomp_action(action: &str) -> oci::LinuxSeccompAction {
    match action {
        "SCMP_ACT_KILL" => oci::LinuxSeccompAction::ScmpActKill,
        "SCMP_ACT_KILL_PROCESS" => oci::LinuxSeccompAction::ScmpActKillProcess,
        "SCMP_ACT_TRAP" => oci::LinuxSeccompAction::ScmpActTrap,
        "SCMP_ACT_ERRNO" => oci::LinuxSeccompAction::ScmpActErrno,
        "SCMP_ACT_NOTIFY" => oci::LinuxSeccompAction::ScmpActNotify,
        "SCMP_ACT_TRACE" => oci::LinuxSeccompAction::ScmpActTrace,
        "SCMP_ACT_LOG" => oci::LinuxSeccompAction::ScmpActLog,
        // "SCMP_ACT_ALLOW" => oci::LinuxSeccompAction::ScmpActAllow,
        _ => oci::LinuxSeccompAction::default(),
    }
}

// LinuxSeccompOperator
pub fn from_seccomp_operator(op: oci::LinuxSeccompOperator) -> &'static str {
    match op {
        oci::LinuxSeccompOperator::ScmpCmpNe => "SCMP_CMP_NE",
        oci::LinuxSeccompOperator::ScmpCmpLt => "SCMP_CMP_LT",
        oci::LinuxSeccompOperator::ScmpCmpLe => "SCMP_CMP_LE",
        oci::LinuxSeccompOperator::ScmpCmpEq => "SCMP_CMP_EQ",
        oci::LinuxSeccompOperator::ScmpCmpGe => "SCMP_CMP_GE",
        oci::LinuxSeccompOperator::ScmpCmpGt => "SCMP_CMP_GT",
        oci::LinuxSeccompOperator::ScmpCmpMaskedEq => "SCMP_CMP_MASKED_EQ",
    }
}

pub fn to_seccomp_operator(op: &str) -> oci::LinuxSeccompOperator {
    match op {
        "SCMP_CMP_NE" => oci::LinuxSeccompOperator::ScmpCmpNe,
        "SCMP_CMP_LT" => oci::LinuxSeccompOperator::ScmpCmpLt,
        "SCMP_CMP_LE" => oci::LinuxSeccompOperator::ScmpCmpLe,
        "SCMP_CMP_GE" => oci::LinuxSeccompOperator::ScmpCmpGe,
        "SCMP_CMP_GT" => oci::LinuxSeccompOperator::ScmpCmpGt,
        "SCMP_CMP_MASKED_EQ" => oci::LinuxSeccompOperator::ScmpCmpMaskedEq,
        //  => oci::LinuxSeccompOperator::ScmpCmpEq,
        _ => oci::LinuxSeccompOperator::default(),
    }
}

// LinuxCapability
pub fn from_capability(cap: &oci::Capability) -> &'static str {
    match *cap {
        oci::Capability::AuditControl => "CAP_AUDIT_CONTROL",
        oci::Capability::AuditRead => "CAP_AUDIT_READ",
        oci::Capability::AuditWrite => "CAP_AUDIT_WRITE",
        oci::Capability::BlockSuspend => "CAP_BLOCK_SUSPEND",
        oci::Capability::Bpf => "CAP_BPF",
        oci::Capability::CheckpointRestore => "CAP_CHECKPOINT_RESTORE",
        oci::Capability::Chown => "CAP_CHOWN",
        oci::Capability::DacOverride => "CAP_DAC_OVERRIDE",
        oci::Capability::DacReadSearch => "CAP_DAC_READ_SEARCH",
        oci::Capability::Fowner => "CAP_FOWNER",
        oci::Capability::Fsetid => "CAP_FSETID",
        oci::Capability::IpcLock => "CAP_IPC_LOCK",
        oci::Capability::IpcOwner => "CAP_IPC_OWNER",
        oci::Capability::Kill => "CAP_KILL",
        oci::Capability::Lease => "CAP_LEASE",
        oci::Capability::LinuxImmutable => "CAP_LINUX_IMMUTABLE",
        oci::Capability::MacAdmin => "CAP_MAC_ADMIN",
        oci::Capability::MacOverride => "CAP_MAC_OVERRIDE",
        oci::Capability::Mknod => "CAP_MKNOD",
        oci::Capability::NetAdmin => "CAP_NET_ADMIN",
        oci::Capability::NetBindService => "CAP_NET_BIND_SERVICE",
        oci::Capability::NetBroadcast => "CAP_NET_BROADCAST",
        oci::Capability::NetRaw => "CAP_NET_RAW",
        oci::Capability::Perfmon => "CAP_PERFMON",
        oci::Capability::Setgid => "CAP_SETGID",
        oci::Capability::Setfcap => "CAP_SETFCAP",
        oci::Capability::Setpcap => "CAP_SETPCAP",
        oci::Capability::Setuid => "CAP_SETUID",
        oci::Capability::SysAdmin => "CAP_SYS_ADMIN",
        oci::Capability::SysBoot => "CAP_SYS_BOOT",
        oci::Capability::SysChroot => "CAP_SYS_CHROOT",
        oci::Capability::SysModule => "CAP_SYS_MODULE",
        oci::Capability::SysNice => "CAP_SYS_NICE",
        oci::Capability::SysPacct => "CAP_SYS_PACCT",
        oci::Capability::SysPtrace => "CAP_SYS_PTRACE",
        oci::Capability::SysRawio => "CAP_SYS_RAWIO",
        oci::Capability::SysResource => "CAP_SYS_RESOURCE",
        oci::Capability::SysTime => "CAP_SYS_TIME",
        oci::Capability::SysTtyConfig => "CAP_SYS_TTY_CONFIG",
        oci::Capability::Syslog => "CAP_SYSLOG",
        oci::Capability::WakeAlarm => "CAP_WAKE_ALARM",
    }
}

pub fn to_capability(cap: &str) -> Option<oci::Capability> {
    match cap {
        "CAP_AUDIT_CONTROL" => Some(oci::Capability::AuditControl),
        "CAP_AUDIT_READ" => Some(oci::Capability::AuditRead),
        "CAP_AUDIT_WRITE" => Some(oci::Capability::AuditWrite),
        "CAP_BLOCK_SUSPEND" => Some(oci::Capability::BlockSuspend),
        "CAP_BPF" => Some(oci::Capability::Bpf),
        "CAP_CHECKPOINT_RESTORE" => Some(oci::Capability::CheckpointRestore),
        "CAP_CHOWN" => Some(oci::Capability::Chown),
        "CAP_DAC_OVERRIDE" => Some(oci::Capability::DacOverride),
        "CAP_DAC_READ_SEARCH" => Some(oci::Capability::DacReadSearch),
        "CAP_FOWNER" => Some(oci::Capability::Fowner),
        "CAP_FSETID" => Some(oci::Capability::Fsetid),
        "CAP_IPC_LOCK" => Some(oci::Capability::IpcLock),
        "CAP_IPC_OWNER" => Some(oci::Capability::IpcOwner),
        "CAP_KILL" => Some(oci::Capability::Kill),
        "CAP_LEASE" => Some(oci::Capability::Lease),
        "CAP_LINUX_IMMUTABLE" => Some(oci::Capability::LinuxImmutable),
        "CAP_MAC_ADMIN" => Some(oci::Capability::MacAdmin),
        "CAP_MAC_OVERRIDE" => Some(oci::Capability::MacOverride),
        "CAP_MKNOD" => Some(oci::Capability::Mknod),
        "CAP_NET_ADMIN" => Some(oci::Capability::NetAdmin),
        "CAP_NET_BIND_SERVICE" => Some(oci::Capability::NetBindService),
        "CAP_NET_BROADCAST" => Some(oci::Capability::NetBroadcast),
        "CAP_NET_RAW" => Some(oci::Capability::NetRaw),
        "CAP_PERFMON" => Some(oci::Capability::Perfmon),
        "CAP_SETGID" => Some(oci::Capability::Setgid),
        "CAP_SETFCAP" => Some(oci::Capability::Setfcap),
        "CAP_SETPCAP" => Some(oci::Capability::Setpcap),
        "CAP_SETUID" => Some(oci::Capability::Setuid),
        "CAP_SYS_ADMIN" => Some(oci::Capability::SysAdmin),
        "CAP_SYS_BOOT" => Some(oci::Capability::SysBoot),
        "CAP_SYS_CHROOT" => Some(oci::Capability::SysChroot),
        "CAP_SYS_MODULE" => Some(oci::Capability::SysModule),
        "CAP_SYS_NICE" => Some(oci::Capability::SysNice),
        "CAP_SYS_PACCT" => Some(oci::Capability::SysPacct),
        "CAP_SYS_PTRACE" => Some(oci::Capability::SysPtrace),
        "CAP_SYS_RAWIO" => Some(oci::Capability::SysRawio),
        "CAP_SYS_RESOURCE" => Some(oci::Capability::SysResource),
        "CAP_SYS_TIME" => Some(oci::Capability::SysTime),
        "CAP_SYS_TTY_CONFIG" => Some(oci::Capability::SysTtyConfig),
        "CAP_SYSLOG" => Some(oci::Capability::Syslog),
        "CAP_WAKE_ALARM" => Some(oci::Capability::WakeAlarm),
        _ => None,
    }
}

// LinuxSeccompFilterFlag
pub fn from_seccomp_filter_flag(flag: oci::LinuxSeccompFilterFlag) -> &'static str {
    match flag {
        oci::LinuxSeccompFilterFlag::SeccompFilterFlagLog => "SECCOMP_FILTER_FLAG_LOG",
        oci::LinuxSeccompFilterFlag::SeccompFilterFlagTsync => "SECCOMP_FILTER_FLAG_TSYNC",
        oci::LinuxSeccompFilterFlag::SeccompFilterFlagSpecAllow => "SECCOMP_FILTER_FLAG_SPEC_ALLOW",
    }
}

pub fn to_seccomp_filter_flag(flag: &str) -> Option<oci::LinuxSeccompFilterFlag> {
    match flag {
        "SECCOMP_FILTER_FLAG_LOG" => Some(oci::LinuxSeccompFilterFlag::SeccompFilterFlagLog),
        "SECCOMP_FILTER_FLAG_TSYNC" => Some(oci::LinuxSeccompFilterFlag::SeccompFilterFlagTsync),
        "SECCOMP_FILTER_FLAG_SPEC_ALLOW" => {
            Some(oci::LinuxSeccompFilterFlag::SeccompFilterFlagSpecAllow)
        }
        _ => None,
    }
}

// Architecture
pub fn from_arch(a: oci::Arch) -> &'static str {
    match a {
        oci::Arch::ScmpArchNative => "SCMP_ARCH_NATIVE",
        oci::Arch::ScmpArchX86 => "SCMP_ARCH_X86",
        oci::Arch::ScmpArchX86_64 => "SCMP_ARCH_X86_64",
        oci::Arch::ScmpArchX32 => "SCMP_ARCH_X32",
        oci::Arch::ScmpArchArm => "SCMP_ARCH_ARM",
        oci::Arch::ScmpArchAarch64 => "SCMP_ARCH_AARCH64",
        oci::Arch::ScmpArchMips => "SCMP_ARCH_MIPS",
        oci::Arch::ScmpArchMips64 => "SCMP_ARCH_MIPS64",
        oci::Arch::ScmpArchMips64n32 => "SCMP_ARCH_MIPS64N32",
        oci::Arch::ScmpArchMipsel => "SCMP_ARCH_MIPSEL",
        oci::Arch::ScmpArchMipsel64 => "SCMP_ARCH_MIPSEL64",
        oci::Arch::ScmpArchMipsel64n32 => "SCMP_ARCH_MIPSEL64N32",
        oci::Arch::ScmpArchPpc => "SCMP_ARCH_PPC",
        oci::Arch::ScmpArchPpc64 => "SCMP_ARCH_PPC64",
        oci::Arch::ScmpArchPpc64le => "SCMP_ARCH_PPC64LE",
        oci::Arch::ScmpArchS390 => "SCMP_ARCH_S390",
        oci::Arch::ScmpArchS390x => "SCMP_ARCH_S390X",
    }
}

pub fn to_arch(a: &str) -> Option<oci::Arch> {
    match a {
        "SCMP_ARCH_NATIVE" => Some(oci::Arch::ScmpArchNative),
        "SCMP_ARCH_X86" => Some(oci::Arch::ScmpArchX86),
        "SCMP_ARCH_X86_64" => Some(oci::Arch::ScmpArchX86_64),
        "SCMP_ARCH_X32" => Some(oci::Arch::ScmpArchX32),
        "SCMP_ARCH_ARM" => Some(oci::Arch::ScmpArchArm),
        "SCMP_ARCH_AARCH64" => Some(oci::Arch::ScmpArchAarch64),
        "SCMP_ARCH_MIPS" => Some(oci::Arch::ScmpArchMips),
        "SCMP_ARCH_MIPS64" => Some(oci::Arch::ScmpArchMips64),
        "SCMP_ARCH_MIPS64N32" => Some(oci::Arch::ScmpArchMips64n32),
        "SCMP_ARCH_MIPSEL" => Some(oci::Arch::ScmpArchMipsel),
        "SCMP_ARCH_MIPSEL64" => Some(oci::Arch::ScmpArchMipsel64),
        "SCMP_ARCH_MIPSEL64N32" => Some(oci::Arch::ScmpArchMipsel64n32),
        "SCMP_ARCH_PPC" => Some(oci::Arch::ScmpArchPpc),
        "SCMP_ARCH_PPC64" => Some(oci::Arch::ScmpArchPpc64),
        "SCMP_ARCH_PPC64LE" => Some(oci::Arch::ScmpArchPpc64le),
        "SCMP_ARCH_S390" => Some(oci::Arch::ScmpArchS390),
        "SCMP_ARCH_S390X" => Some(oci::Arch::ScmpArchS390x),
        _ => None,
    }
}

// Namespace
pub fn to_namespace_type(namespace: &str) -> Option<oci::LinuxNamespaceType> {
    match namespace {
        "mnt" => Some(oci::LinuxNamespaceType::Mount),
        "cgroup" => Some(oci::LinuxNamespaceType::Cgroup),
        "uts" => Some(oci::LinuxNamespaceType::Uts),
        "ipc" => Some(oci::LinuxNamespaceType::Ipc),
        "user" => Some(oci::LinuxNamespaceType::User),
        "pid" => Some(oci::LinuxNamespaceType::Pid),
        "net" => Some(oci::LinuxNamespaceType::Network),
        "time" => Some(oci::LinuxNamespaceType::Time),
        _ => None,
    }
}

pub fn from_namespace_type(ns_type: &oci::LinuxNamespaceType) -> &'static str {
    match ns_type {
        oci::LinuxNamespaceType::Mount => "mount",
        oci::LinuxNamespaceType::Cgroup => "cgroup",
        oci::LinuxNamespaceType::Uts => "uts",
        oci::LinuxNamespaceType::Ipc => "ipc",
        oci::LinuxNamespaceType::User => "user",
        oci::LinuxNamespaceType::Pid => "pid",
        oci::LinuxNamespaceType::Network => "network",
        oci::LinuxNamespaceType::Time => "time",
    }
}

// PosixRlimitType
pub fn from_posix_rlimit_type(typ: &oci::PosixRlimitType) -> &'static str {
    match typ {
        oci::PosixRlimitType::RlimitCpu => "RLIMIT_CPU",
        oci::PosixRlimitType::RlimitFsize => "RLIMIT_FSIZE",
        oci::PosixRlimitType::RlimitData => "RLIMIT_DATA",
        oci::PosixRlimitType::RlimitStack => "RLIMIT_STACK",
        oci::PosixRlimitType::RlimitCore => "RLIMIT_CORE",
        oci::PosixRlimitType::RlimitRss => "RLIMIT_RSS",
        oci::PosixRlimitType::RlimitNproc => "RLIMIT_NPROC",
        oci::PosixRlimitType::RlimitNofile => "RLIMIT_NOFILE",
        oci::PosixRlimitType::RlimitMemlock => "RLIMIT_MEMLOCK",
        oci::PosixRlimitType::RlimitAs => "RLIMIT_AS",
        oci::PosixRlimitType::RlimitLocks => "RLIMIT_LOCKS",
        oci::PosixRlimitType::RlimitSigpending => "RLIMIT_SIGPENDING",
        oci::PosixRlimitType::RlimitMsgqueue => "RLIMIT_MSGQUEUE",
        oci::PosixRlimitType::RlimitNice => "RLIMIT_NICE",
        oci::PosixRlimitType::RlimitRtprio => "RLIMIT_RTPRIO",
        oci::PosixRlimitType::RlimitRttime => "RLIMIT_RTTIME",
    }
}

pub fn to_posix_rlimit_type(proto: &str) -> Option<oci::PosixRlimitType> {
    match proto {
        "RLIMIT_CPU" => Some(oci::PosixRlimitType::RlimitCpu),
        "RLIMIT_FSIZE" => Some(oci::PosixRlimitType::RlimitFsize),
        "RLIMIT_DATA" => Some(oci::PosixRlimitType::RlimitData),
        "RLIMIT_STACK" => Some(oci::PosixRlimitType::RlimitStack),
        "RLIMIT_CORE" => Some(oci::PosixRlimitType::RlimitCore),
        "RLIMIT_RSS" => Some(oci::PosixRlimitType::RlimitRss),
        "RLIMIT_NPROC" => Some(oci::PosixRlimitType::RlimitNproc),
        "RLIMIT_NOFILE" => Some(oci::PosixRlimitType::RlimitNofile),
        "RLIMIT_MEMLOCK" => Some(oci::PosixRlimitType::RlimitMemlock),
        "RLIMIT_AS" => Some(oci::PosixRlimitType::RlimitAs),
        "RLIMIT_LOCKS" => Some(oci::PosixRlimitType::RlimitLocks),
        "RLIMIT_SIGPENDING" => Some(oci::PosixRlimitType::RlimitSigpending),
        "RLIMIT_MSGQUEUE" => Some(oci::PosixRlimitType::RlimitMsgqueue),
        "RLIMIT_NICE" => Some(oci::PosixRlimitType::RlimitNice),
        "RLIMIT_RTPRIO" => Some(oci::PosixRlimitType::RlimitRtprio),
        "RLIMIT_RTTIME" => Some(oci::PosixRlimitType::RlimitRttime),
        _ => None,
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
            Username: if let Some(user_name) = from.username() {
                user_name.clone()
            } else {
                String::new()
            },
            ..Default::default()
        }
    }
}

impl From<oci::LinuxCapabilities> for grpc::LinuxCapabilities {
    fn from(from: oci::LinuxCapabilities) -> Self {
        grpc::LinuxCapabilities {
            Bounding: hashset_to_vec(from.bounding()),
            Effective: hashset_to_vec(from.effective()),
            Inheritable: hashset_to_vec(from.inheritable()),
            Permitted: hashset_to_vec(from.permitted()),
            Ambient: hashset_to_vec(from.ambient()),
            ..Default::default()
        }
    }
}

impl From<oci::PosixRlimit> for grpc::POSIXRlimit {
    fn from(from: oci::PosixRlimit) -> Self {
        grpc::POSIXRlimit {
            Type: from_posix_rlimit_type(&from.typ()).to_owned(),
            Hard: from.hard(),
            Soft: from.soft(),
            ..Default::default()
        }
    }
}

impl From<oci::Process> for grpc::Process {
    fn from(from: oci::Process) -> Self {
        let rlimits = from_option_vec(from.rlimits().clone());
        let cons_sz = from_option(from.console_size());
        let capabilities = from_option(from.capabilities().clone());

        grpc::Process {
            Terminal: if let Some(terminal) = from.terminal() {
                terminal
            } else {
                false
            },
            ConsoleSize: cons_sz,
            // User: from_some(grpc::User::from(from.user())),
            User: from_option(Some(from.user().clone())),
            Args: option_vec_to_vec(from.args()),
            Env: option_vec_to_vec(from.env()),
            Cwd: from.cwd().display().to_string(),
            Capabilities: capabilities,
            Rlimits: rlimits,
            NoNewPrivileges: from.no_new_privileges().unwrap_or_default(),
            ApparmorProfile: if let Some(app) = from.apparmor_profile() {
                app.clone()
            } else {
                String::new()
            },
            OOMScoreAdj: from.oom_score_adj().map_or(0, |t| t as i64),
            SelinuxLabel: if let Some(sel) = from.selinux_label() {
                sel.clone()
            } else {
                String::new()
            },
            ..Default::default()
        }
    }
}

impl From<oci::LinuxDeviceCgroup> for grpc::LinuxDeviceCgroup {
    fn from(from: oci::LinuxDeviceCgroup) -> Self {
        grpc::LinuxDeviceCgroup {
            Allow: from.allow(),
            Type: if let Some(t) = from.typ() {
                t.as_str().to_string()
            } else {
                String::new()
            },
            Major: from.major().map_or(0, |t| t),
            Minor: from.minor().map_or(0, |t| t),
            Access: if let Some(access) = from.access() {
                access.clone()
            } else {
                String::new()
            },
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
            Cpus: if let Some(cpus) = from.cpus() {
                cpus.clone()
            } else {
                String::new()
            },
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
            Weight: from.weight().map_or(0, |t| t as u32),
            LeafWeight: from.leaf_weight().map_or(0, |t| t as u32),
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
        let weight_device = from_option_vec(block_io.weight_device().clone());
        let throttle_read_bps_device = from_option_vec(block_io.throttle_read_bps_device().clone());
        let throttle_write_bps_device =
            from_option_vec(block_io.throttle_write_bps_device().clone());
        let throttle_read_iops_device =
            from_option_vec(block_io.throttle_read_iops_device().clone());
        let throttle_write_iops_device =
            from_option_vec(block_io.throttle_write_iops_device().clone());

        grpc::LinuxBlockIO {
            Weight: if let Some(weight) = block_io.weight().map(|w| w as u32) {
                weight
            } else {
                0
            },
            LeafWeight: if let Some(weight) = block_io.leaf_weight().map(|w| w as u32) {
                weight
            } else {
                0
            },
            WeightDevice: weight_device,
            ThrottleReadBpsDevice: throttle_read_bps_device,
            ThrottleWriteBpsDevice: throttle_write_bps_device,
            ThrottleReadIOPSDevice: throttle_read_iops_device,
            ThrottleWriteIOPSDevice: throttle_write_iops_device,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxHugepageLimit> for grpc::LinuxHugepageLimit {
    fn from(from: oci::LinuxHugepageLimit) -> Self {
        grpc::LinuxHugepageLimit {
            Pagesize: from.page_size().to_owned(),
            Limit: from.limit().to_owned() as u64,
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
        let priorities = from_option_vec(from.priorities().clone());

        grpc::LinuxNetwork {
            ClassID: from.class_id().map_or(0, |t| t),
            Priorities: priorities,
            ..Default::default()
        }
    }
}

impl From<oci::LinuxResources> for grpc::LinuxResources {
    fn from(from: oci::LinuxResources) -> Self {
        let devices = from_option_vec(from.devices().clone());
        let huge_limits = from_option_vec(from.hugepage_limits().clone());
        let block_io = from_option(from.block_io().clone());

        let memory = from_option(*from.memory());
        let network = from_option(from.network().clone());
        let cpu = from_option(from.cpu().clone());
        let pids = from_option(*from.pids());

        grpc::LinuxResources {
            Devices: devices,
            Memory: memory,
            CPU: cpu,
            Pids: pids,
            BlockIO: block_io,
            HugepageLimits: huge_limits,
            Network: network,
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
            source: if let Some(s) = from.source() {
                s.display().to_string()
            } else {
                String::new()
            },
            type_: if let Some(t) = from.typ() {
                t.clone()
            } else {
                String::new()
            },
            options: option_vec_to_vec(from.options()),
            ..Default::default()
        }
    }
}

impl From<oci::Hook> for grpc::Hook {
    fn from(from: oci::Hook) -> Self {
        let mut timeout: i64 = 0;
        if let Some(v) = from.timeout() {
            timeout = v;
        }
        grpc::Hook {
            Path: from.path().display().to_string(),
            Args: option_vec_to_vec(from.args()),
            Env: option_vec_to_vec(from.env()),
            Timeout: timeout,
            ..Default::default()
        }
    }
}

impl From<oci::Hooks> for grpc::Hooks {
    fn from(from: oci::Hooks) -> Self {
        let prestart = from_option_vec(from.prestart().clone());
        let create_runtime = from_option_vec(from.create_runtime().clone());
        let create_container = from_option_vec(from.create_container().clone());
        let start_container = from_option_vec(from.start_container().clone());
        let poststart = from_option_vec(from.poststart().clone());
        let poststop = from_option_vec(from.poststop().clone());

        grpc::Hooks {
            Prestart: prestart,
            CreateRuntime: create_runtime,
            CreateContainer: create_container,
            StartContainer: start_container,
            Poststart: poststart,
            Poststop: poststop,
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
            Type: from_namespace_type(&from.typ()).to_owned(),
            Path: if let Some(p) = from.path() {
                p.display().to_string()
            } else {
                String::new()
            },
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
            FileMode: from.file_mode().map_or(0, |v| v),
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
            ValueTwo: if let Some(vt) = from.value_two() {
                vt
            } else {
                0
            },
            Op: from_seccomp_operator(from.op()).to_owned(),
            ..Default::default()
        }
    }
}

impl From<oci::LinuxSyscall> for grpc::LinuxSyscall {
    fn from(from: oci::LinuxSyscall) -> Self {
        let args = from_option_vec(from.args().clone());

        grpc::LinuxSyscall {
            Names: from.names().to_vec(),
            Action: from_seccomp_action(from.action()).to_owned(),
            Args: args,
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
            DefaultAction: from_seccomp_action(from.default_action()).to_owned(),
            Architectures: from
                .architectures()
                .as_ref()
                .map(|arches| {
                    arches
                        .iter()
                        .map(|&arch| from_arch(arch).to_owned())
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
                .map(|flags| {
                    flags
                        .iter()
                        .map(|&flag| from_seccomp_filter_flag(flag).to_owned())
                        .collect()
                })
                .unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::Linux> for grpc::Linux {
    fn from(from: oci::Linux) -> Self {
        let uid_mappings = from_option_vec(from.uid_mappings().clone());
        let gid_mappings = from_option_vec(from.gid_mappings().clone());
        let devices = from_option_vec(from.devices().clone());

        let seccomp = from_option(from.seccomp().clone());

        grpc::Linux {
            UIDMappings: uid_mappings,
            GIDMappings: gid_mappings,
            Sysctl: from.sysctl().clone().unwrap_or_default(),
            Resources: from_option(from.resources().clone()),
            CgroupsPath: from
                .cgroups_path()
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            Namespaces: from_option_vec(from.namespaces().clone()),
            Devices: devices,
            Seccomp: seccomp,
            RootfsPropagation: from.rootfs_propagation().clone().unwrap_or_default(),
            MaskedPaths: from.masked_paths().clone().unwrap_or_default(),
            ReadonlyPaths: from.readonly_paths().clone().unwrap_or_default(),
            MountLabel: from.mount_label().clone().unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl From<oci::Spec> for grpc::Spec {
    fn from(from: oci::Spec) -> Self {
        let mounts = from_option_vec(from.mounts().clone());

        let hooks = from_option(from.hooks().clone());
        let process = from_option(from.process().clone());
        let root = from_option(from.root().clone());
        let linux = from_option(from.linux().clone());

        grpc::Spec {
            Version: from.version().to_string(),
            Process: process,
            Root: root,
            Hostname: from.hostname().clone().unwrap_or_default(),
            Mounts: mounts,
            Hooks: hooks,
            Annotations: from.annotations().clone().unwrap_or_default(),
            Linux: linux,
            Solaris: Default::default(),
            Windows: Default::default(),
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
        let options = mnt.options().to_vec();
        let mut oci_mount = oci::Mount::default();
        oci_mount.set_destination(PathBuf::from(&mnt.destination()));
        oci_mount.set_typ(if mnt.type_.is_empty() {
            None
        } else {
            Some(mnt.type_().to_string())
        });
        oci_mount.set_source(if mnt.source.is_empty() {
            None
        } else {
            Some(PathBuf::from(mnt.source()))
        });
        oci_mount.set_options(if options.is_empty() {
            None
        } else {
            Some(options)
        });

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
        oci_devcgrp.set_typ(Some(to_device_type(from.Type())));
        oci_devcgrp.set_major(if from.Major() > 0 {
            Some(from.Major())
        } else {
            None
        });
        oci_devcgrp.set_minor(if from.Minor() > 0 {
            Some(from.Minor())
        } else {
            None
        });
        oci_devcgrp.set_access(if from.Access().is_empty() {
            None
        } else {
            Some(from.Access().to_string())
        });

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

        let cpus = Some(from.Cpus().to_string());
        let mems = Some(from.Mems().to_string());

        let mut oci_lcpu = oci::LinuxCpu::default();
        oci_lcpu.set_shares(shares);
        oci_lcpu.set_quota(quota);
        oci_lcpu.set_period(period);
        oci_lcpu.set_realtime_runtime(realtime_runtime);
        oci_lcpu.set_realtime_period(realtime_period);
        oci_lcpu.set_cpus(cpus);
        oci_lcpu.set_mems(mems);

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
        let mut weight = None;
        if from.Weight() > 0 {
            weight = Some(from.Weight() as u16);
        }
        let mut leaf_weight = None;
        if from.LeafWeight() > 0 {
            leaf_weight = Some(from.LeafWeight() as u16);
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

        let mut oci_blkio = oci::LinuxBlockIo::default();
        oci_blkio.set_weight(weight);
        oci_blkio.set_leaf_weight(leaf_weight);
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

        oci_network.set_class_id(if network.ClassID() > 0 {
            Some(network.ClassID())
        } else {
            None
        });

        let priorities: Vec<oci::LinuxInterfacePriority> = network
            .Priorities()
            .iter()
            .cloned()
            .map(|pri| pri.into())
            .collect();
        oci_network.set_priorities(if priorities.is_empty() {
            None
        } else {
            Some(priorities)
        });

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
        oci_resources.set_devices(if resources.Devices().is_empty() {
            None
        } else {
            Some(
                resources
                    .Devices()
                    .iter()
                    .cloned()
                    .map(|dev| dev.into())
                    .collect(),
            )
        });
        oci_resources.set_memory(if !resources.has_Memory() {
            None
        } else {
            Some(resources.Memory().clone().into())
        });
        oci_resources.set_cpu(if !resources.has_CPU() {
            None
        } else {
            Some(resources.CPU().clone().into())
        });
        oci_resources.set_pids(if !resources.has_Pids() {
            None
        } else {
            Some(resources.Pids().clone().into())
        });
        oci_resources.set_block_io(if !resources.has_BlockIO() {
            None
        } else {
            Some(resources.BlockIO().clone().into())
        });
        oci_resources.set_hugepage_limits(if resources.HugepageLimits().is_empty() {
            None
        } else {
            Some(
                resources
                    .HugepageLimits()
                    .iter()
                    .cloned()
                    .map(|dev| dev.into())
                    .collect(),
            )
        });
        oci_resources.set_network(if !resources.has_Network() {
            None
        } else {
            Some(resources.Network().clone().into())
        });

        oci_resources
    }
}

// grpc -> oci
impl From<grpc::LinuxDevice> for oci::LinuxDevice {
    fn from(device: grpc::LinuxDevice) -> Self {
        let dev_type = to_device_type(device.Type());

        let mut oci_linuxdev = oci::LinuxDevice::default();
        oci_linuxdev.set_path(PathBuf::from(&device.Path()));
        oci_linuxdev.set_typ(dev_type);
        oci_linuxdev.set_major(device.Major());
        oci_linuxdev.set_minor(device.Minor());
        oci_linuxdev.set_file_mode(Some(device.FileMode()));
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
            .op(to_seccomp_operator(from.Op()))
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
        oci_syscall.set_action(to_seccomp_action(syscall.Action()));
        oci_syscall.set_errno_ret(Some(syscall.errnoret()));
        oci_syscall.set_args(if args.is_empty() { None } else { Some(args) });

        oci_syscall
    }
}

impl From<grpc::LinuxSeccomp> for oci::LinuxSeccomp {
    fn from(proto: grpc::LinuxSeccomp) -> Self {
        // TODO: safe to unwrap() ?
        let archs: Vec<oci::Arch> = proto
            .Architectures()
            .iter()
            .map(|arg0: &String| to_arch(arg0).unwrap())
            .collect();
        let flags: Vec<oci::LinuxSeccompFilterFlag> = proto
            .Flags()
            .iter()
            .map(|arg0: &String| to_seccomp_filter_flag(arg0).unwrap())
            .collect();
        let syscalls: Vec<oci::LinuxSyscall> = proto
            .Syscalls()
            .iter()
            .cloned()
            .map(|syscall| syscall.into())
            .collect();

        let mut oci_seccomp = oci::LinuxSeccomp::default();
        oci_seccomp.set_default_action(to_seccomp_action(proto.DefaultAction()));
        oci_seccomp.set_architectures(Some(archs));
        oci_seccomp.set_flags(Some(flags));
        oci_seccomp.set_syscalls(Some(syscalls));

        oci_seccomp
    }
}

impl From<grpc::LinuxNamespace> for oci::LinuxNamespace {
    fn from(ns: grpc::LinuxNamespace) -> Self {
        let mut oci_ns = oci::LinuxNamespace::default();
        // TODO: safe to unwrap() here ?
        oci_ns.set_typ(to_namespace_type(ns.Type()).unwrap());
        oci_ns.set_path(Some(PathBuf::from(ns.Path())));

        oci_ns
    }
}

impl From<grpc::Linux> for oci::Linux {
    fn from(from: grpc::Linux) -> Self {
        let uid_mappings = if from.UIDMappings().is_empty() {
            None
        } else {
            Some(
                from.UIDMappings()
                    .iter()
                    .cloned()
                    .map(|uid| uid.into())
                    .collect(),
            )
        };
        let gid_mappings = if from.GIDMappings().is_empty() {
            None
        } else {
            Some(
                from.GIDMappings()
                    .iter()
                    .cloned()
                    .map(|gid| gid.into())
                    .collect(),
            )
        };
        let sysctl = if from.Sysctl().is_empty() {
            None
        } else {
            Some(from.Sysctl().clone())
        };
        let resources = if from.has_Resources() {
            Some(from.Resources().clone().into())
        } else {
            None
        };
        let cgroups_path = if from.CgroupsPath().is_empty() {
            None
        } else {
            Some(PathBuf::from(&from.CgroupsPath()))
        };
        let namespaces = if from.Namespaces().is_empty() {
            None
        } else {
            Some(
                from.Namespaces()
                    .iter()
                    .cloned()
                    .map(|ns| ns.into())
                    .collect(),
            )
        };
        let devices = if from.Devices().is_empty() {
            None
        } else {
            Some(
                from.Devices()
                    .iter()
                    .cloned()
                    .map(|dev| dev.into())
                    .collect(),
            )
        };
        let seccomp = if from.has_Seccomp() {
            Some(from.Seccomp().clone().into())
        } else {
            None
        };
        let rootfs_propagation = if from.RootfsPropagation().is_empty() {
            None
        } else {
            Some(from.RootfsPropagation().to_string())
        };
        let masked_paths = if from.MaskedPaths().is_empty() {
            None
        } else {
            Some(from.MaskedPaths().to_vec())
        };
        let readonly_paths = if from.ReadonlyPaths().is_empty() {
            None
        } else {
            Some(from.ReadonlyPaths().to_vec())
        };
        let mount_label = if from.MountLabel().is_empty() {
            None
        } else {
            Some(from.MountLabel().to_string())
        };
        let intel_rdt = None;

        let mut oci_linux = oci::Linux::default();
        oci_linux.set_uid_mappings(uid_mappings);
        oci_linux.set_gid_mappings(gid_mappings);
        oci_linux.set_sysctl(sysctl);
        oci_linux.set_resources(resources);
        oci_linux.set_cgroups_path(cgroups_path);
        oci_linux.set_namespaces(namespaces);
        oci_linux.set_devices(devices);
        oci_linux.set_seccomp(seccomp);
        oci_linux.set_rootfs_propagation(rootfs_propagation);
        oci_linux.set_masked_paths(masked_paths);
        oci_linux.set_readonly_paths(readonly_paths);
        oci_linux.set_mount_label(mount_label);
        oci_linux.set_intel_rdt(intel_rdt);

        oci_linux
    }
}

impl From<grpc::POSIXRlimit> for oci::PosixRlimit {
    fn from(proto: grpc::POSIXRlimit) -> Self {
        // WARNING: safe to unwrap here  ?
        oci::PosixRlimitBuilder::default()
            .typ(to_posix_rlimit_type(proto.Type()).unwrap())
            .hard(proto.Hard())
            .soft(proto.Soft())
            .build()
            .unwrap()
    }
}

impl From<grpc::LinuxCapabilities> for oci::LinuxCapabilities {
    fn from(from: grpc::LinuxCapabilities) -> Self {
        let cap_bounding = vec_to_hashset(from.Bounding().to_vec());
        let cap_effective = vec_to_hashset(from.Effective().to_vec());
        let cap_inheritable = vec_to_hashset(from.Inheritable().to_vec());
        let cap_permitted = vec_to_hashset(from.Permitted().to_vec());
        let cap_ambient = vec_to_hashset(from.Ambient().to_vec());

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
        let console_size = if from.has_ConsoleSize() {
            Some(from.ConsoleSize().clone().into())
        } else {
            None
        };
        // let user = oci::User::from(from.User());

        let user = from.User().clone().into();
        let args = if from.Args().is_empty() {
            None
        } else {
            Some(from.Args().to_vec())
        };
        let env = if from.Env().is_empty() {
            None
        } else {
            Some(from.Env().to_vec())
        };
        let cwd = PathBuf::from(&from.Cwd());

        let mut capabilities = None;
        if from.has_Capabilities() {
            capabilities = Some(from.Capabilities().clone().into());
        }

        let rlimits = if from.Rlimits().is_empty() {
            None
        } else {
            Some(from.Rlimits().iter().cloned().map(|r| r.into()).collect())
        };

        let no_new_privileges = Some(from.NoNewPrivileges());
        let apparmor_profile = if from.ApparmorProfile().is_empty() {
            None
        } else {
            Some(from.ApparmorProfile().to_string())
        };
        let oom_score_adj = if from.OOMScoreAdj() != 0 {
            Some(from.OOMScoreAdj() as i32)
        } else {
            None
        };
        let selinux_label = if from.SelinuxLabel().is_empty() {
            None
        } else {
            Some(from.SelinuxLabel().to_string())
        };

        let mut process = oci::Process::default();

        process.set_terminal(Some(from.Terminal));
        process.set_console_size(console_size);
        process.set_user(user);
        process.set_args(args);
        process.set_env(env);
        process.set_cwd(cwd);
        process.set_capabilities(capabilities);
        process.set_rlimits(rlimits);
        process.set_no_new_privileges(no_new_privileges);
        process.set_apparmor_profile(apparmor_profile);
        process.set_oom_score_adj(oom_score_adj);
        process.set_selinux_label(selinux_label);

        process
    }
}

impl From<grpc::Hook> for oci::Hook {
    fn from(hook: grpc::Hook) -> Self {
        let mut oci_hook = oci::Hook::default();
        oci_hook.set_path(PathBuf::from(&hook.Path()));
        oci_hook.set_args(Some(hook.Args().to_vec()));
        oci_hook.set_env(Some(hook.Env().to_vec()));
        oci_hook.set_timeout(if hook.Timeout > 0 {
            Some(hook.Timeout())
        } else {
            None
        });

        oci_hook
    }
}

// grpc -> oci
impl From<grpc::Hooks> for oci::Hooks {
    fn from(hooks: grpc::Hooks) -> Self {
        let mut oci_hooks = oci::Hooks::default();
        oci_hooks.set_prestart(if hooks.Prestart().is_empty() {
            None
        } else {
            Some(
                hooks
                    .Prestart()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            )
        });
        oci_hooks.set_create_runtime(if hooks.CreateRuntime().is_empty() {
            None
        } else {
            Some(
                hooks
                    .CreateRuntime()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            )
        });
        oci_hooks.set_create_container(if hooks.CreateContainer().is_empty() {
            None
        } else {
            Some(
                hooks
                    .CreateContainer()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            )
        });
        oci_hooks.set_start_container(if hooks.StartContainer().is_empty() {
            None
        } else {
            Some(
                hooks
                    .StartContainer()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            )
        });
        oci_hooks.set_poststart(if hooks.Poststart().is_empty() {
            None
        } else {
            Some(
                hooks
                    .Poststart()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            )
        });
        oci_hooks.set_poststop(if hooks.Poststart().is_empty() {
            None
        } else {
            Some(
                hooks
                    .Poststart()
                    .iter()
                    .cloned()
                    .map(|hook| hook.into())
                    .collect(),
            )
        });

        oci_hooks
    }
}

impl From<grpc::Spec> for oci::Spec {
    fn from(from: grpc::Spec) -> Self {
        let root = if from.has_Root() {
            Some(from.Root().clone().into())
        } else {
            None
        };

        let mounts = if from.Mounts().is_empty() {
            None
        } else {
            Some(
                from.Mounts()
                    .iter()
                    .cloned()
                    .map(|m| m.into())
                    .collect::<Vec<_>>(),
            )
        };
        let process = if from.has_Process() {
            Some(from.Process().clone().into())
        } else {
            None
        };
        let hooks = if from.has_Hooks() {
            Some(from.Hooks().clone().into())
        } else {
            None
        };
        let annotations = if from.Annotations().is_empty() {
            None
        } else {
            Some(from.Annotations())
        };
        let linux = if from.has_Linux() {
            Some(from.Linux().clone().into())
        } else {
            None
        };

        let mut oci_spec = oci::Spec::default();
        oci_spec.set_version(from.Version().to_owned());
        oci_spec.set_process(process);
        oci_spec.set_root(root);
        oci_spec.set_hostname(Some(from.Hostname().to_owned()));
        oci_spec.set_mounts(mounts);
        oci_spec.set_hooks(hooks);
        oci_spec.set_annotations(annotations.cloned());
        oci_spec.set_linux(linux);

        oci_spec
    }
}

#[cfg(test)]
mod tests {
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
}
