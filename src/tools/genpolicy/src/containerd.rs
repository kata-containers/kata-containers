// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::policy;

const DEFAULT_UNIX_CAPS: [&'static str; 14] = [
    "CAP_CHOWN",
    "CAP_DAC_OVERRIDE",
    "CAP_FSETID",
    "CAP_FOWNER",
    "CAP_MKNOD",
    "CAP_NET_RAW",
    "CAP_SETGID",
    "CAP_SETUID",
    "CAP_SETFCAP",
    "CAP_SETPCAP",
    "CAP_NET_BIND_SERVICE",
    "CAP_SYS_CHROOT",
    "CAP_KILL",
    "CAP_AUDIT_WRITE",
];

const DEFAULT_UNIX_CAPS_PRIVILEGED: [&'static str; 41] = [
    "CAP_CHOWN",
    "CAP_DAC_OVERRIDE",
    "CAP_DAC_READ_SEARCH",
    "CAP_FOWNER",
    "CAP_FSETID",
    "CAP_KILL",
    "CAP_SETGID",
    "CAP_SETUID",
    "CAP_SETPCAP",
    "CAP_LINUX_IMMUTABLE",
    "CAP_NET_BIND_SERVICE",
    "CAP_NET_BROADCAST",
    "CAP_NET_ADMIN",
    "CAP_NET_RAW",
    "CAP_IPC_LOCK",
    "CAP_IPC_OWNER",
    "CAP_SYS_MODULE",
    "CAP_SYS_RAWIO",
    "CAP_SYS_CHROOT",
    "CAP_SYS_PTRACE",
    "CAP_SYS_PACCT",
    "CAP_SYS_ADMIN",
    "CAP_SYS_BOOT",
    "CAP_SYS_NICE",
    "CAP_SYS_RESOURCE",
    "CAP_SYS_TIME",
    "CAP_SYS_TTY_CONFIG",
    "CAP_MKNOD",
    "CAP_LEASE",
    "CAP_AUDIT_WRITE",
    "CAP_AUDIT_CONTROL",
    "CAP_SETFCAP",
    "CAP_MAC_OVERRIDE",
    "CAP_MAC_ADMIN",
    "CAP_SYSLOG",
    "CAP_WAKE_ALARM",
    "CAP_BLOCK_SUSPEND",
    "CAP_AUDIT_READ",
    "CAP_PERFMON",
    "CAP_BPF",
    "CAP_CHECKPOINT_RESTORE",
];

// Default process field from containerd.
pub fn get_process(privileged_container: bool) -> policy::KataProcess {
    let capabilities = if privileged_container {
        policy::KataLinuxCapabilities {
            Ambient: vec![],
            Bounding: DEFAULT_UNIX_CAPS_PRIVILEGED.into_iter().map(String::from).collect(),
            Effective: DEFAULT_UNIX_CAPS_PRIVILEGED.into_iter().map(String::from).collect(),
            Inheritable: vec![],
            Permitted: DEFAULT_UNIX_CAPS_PRIVILEGED.into_iter().map(String::from).collect(),
        }
    } else {
        policy::KataLinuxCapabilities {
            Ambient: vec![],
            Bounding: DEFAULT_UNIX_CAPS.into_iter().map(String::from).collect(),
            Effective: DEFAULT_UNIX_CAPS.into_iter().map(String::from).collect(),
            Inheritable: vec![],
            Permitted: DEFAULT_UNIX_CAPS.into_iter().map(String::from).collect(),
        }
    };

    policy::KataProcess {
        Terminal: false,
        User: Default::default(),
        Args: Vec::new(),
        Env: Vec::new(),
        Cwd: "/".to_string(),
        Capabilities: Some(capabilities),
        NoNewPrivileges: true,
    }
}

// Default mounts field from containerd.
pub fn get_mounts(is_pause_container: bool, privileged_container: bool) -> Vec<policy::KataMount> {
    let sysfs_read_write_option = if privileged_container {
        "rw"
    } else {
        "ro"
    };

    let mut mounts = vec![
        policy::KataMount {
            destination: "/proc".to_string(),
            type_: "proc".to_string(),
            source: "proc".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
            ],
        },
        policy::KataMount {
            destination: "/dev".to_string(),
            type_: "tmpfs".to_string(),
            source: "tmpfs".to_string(),
            options: vec![
                "nosuid".to_string(),
                "strictatime".to_string(),
                "mode=755".to_string(),
                "size=65536k".to_string(),
            ],
        },
        policy::KataMount {
            destination: "/dev/pts".to_string(),
            type_: "devpts".to_string(),
            source: "devpts".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "newinstance".to_string(),
                "ptmxmode=0666".to_string(),
                "mode=0620".to_string(),
                "gid=5".to_string(),
            ],
        },
        policy::KataMount {
            destination: "/dev/shm".to_string(),
            type_: "tmpfs".to_string(),
            source: "shm".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
                "mode=1777".to_string(),
                "size=65536k".to_string(),
            ],
        },
        policy::KataMount {
            destination: "/dev/mqueue".to_string(),
            type_: "mqueue".to_string(),
            source: "mqueue".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
            ],
        },
        policy::KataMount {
            destination: "/sys".to_string(),
            type_: "sysfs".to_string(),
            source: "sysfs".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
                sysfs_read_write_option.to_string(),
            ],
        },
    ];

    if !is_pause_container {
        mounts.push(policy::KataMount {
            destination: "/sys/fs/cgroup".to_string(),
            type_: "cgroup".to_string(),
            source: "cgroup".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
                "relatime".to_string(),
                sysfs_read_write_option.to_string(),
            ],
        });
    }

    mounts
}

// Default policy::KataLinux field from containerd.
pub fn get_linux(privileged_container: bool) -> policy::KataLinux {
    if !privileged_container {
        policy::KataLinux {
            Namespaces: vec![],
            MaskedPaths: vec![
                "/proc/acpi".to_string(),
                "/proc/kcore".to_string(),
                "/proc/keys".to_string(),
                "/proc/latency_stats".to_string(),
                "/proc/timer_list".to_string(),
                "/proc/timer_stats".to_string(),
                "/proc/sched_debug".to_string(),
                "/proc/scsi".to_string(),
                "/sys/firmware".to_string(),
            ],
            ReadonlyPaths: vec![
                "/proc/asound".to_string(),
                "/proc/bus".to_string(),
                "/proc/fs".to_string(),
                "/proc/irq".to_string(),
                "/proc/sys".to_string(),
                "/proc/sysrq-trigger".to_string(),
            ],
        }
    } else {
        policy::KataLinux {
            Namespaces: vec![],
            MaskedPaths: vec![],
            ReadonlyPaths: vec![],
        }
    }
}
