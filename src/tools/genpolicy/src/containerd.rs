// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::policy;

// Default process field from containerd.
pub fn get_process(privileged_container: bool, common: &policy::CommonData) -> policy::KataProcess {
    let capabilities = if privileged_container {
        policy::KataLinuxCapabilities {
            Ambient: vec![],
            Bounding: common.privileged_caps.clone(),
            Effective: common.privileged_caps.clone(),
            Inheritable: vec![],
            Permitted: common.privileged_caps.clone(),
        }
    } else {
        policy::KataLinuxCapabilities {
            Ambient: vec![],
            Bounding: common.default_caps.clone(),
            Effective: common.default_caps.clone(),
            Inheritable: vec![],
            Permitted: common.default_caps.clone(),
        }
    };

    policy::KataProcess {
        Terminal: false,
        User: Default::default(),
        Args: Vec::new(),
        Env: Vec::new(),
        Cwd: "/".to_string(),
        Capabilities: capabilities,
        NoNewPrivileges: false,
    }
}

// Default mounts field from containerd.
pub fn get_mounts(is_pause_container: bool, privileged_container: bool) -> Vec<policy::KataMount> {
    let sysfs_read_write_option = if privileged_container { "rw" } else { "ro" };

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
            Devices: vec![],
        }
    } else {
        policy::KataLinux {
            Namespaces: vec![],
            MaskedPaths: vec![],
            ReadonlyPaths: vec![],
            Devices: vec![],
        }
    }
}

pub fn get_default_unix_env(env: &mut Vec<String>) {
    assert!(env.is_empty());

    // Return the value of defaultUnixEnv from containerd.
    env.push("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string());
}
