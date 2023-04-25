// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::policy;

use oci::{Linux, LinuxCapabilities, Mount};

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

// Default process field from containerd.
pub fn get_process() -> policy::OciProcess {
    let mut process: policy::OciProcess = Default::default();
    process.cwd = "/".to_string();
    process.no_new_privileges = true;

    let mut user = process.user;
    user.uid = 0;
    user.gid = 0;
    process.user = user;

    let mut capabilities: LinuxCapabilities = Default::default();
    capabilities.bounding = DEFAULT_UNIX_CAPS.into_iter().map(String::from).collect();
    capabilities.permitted = DEFAULT_UNIX_CAPS.into_iter().map(String::from).collect();
    capabilities.effective = DEFAULT_UNIX_CAPS.into_iter().map(String::from).collect();
    process.capabilities = Some(capabilities);

    /*
    process.rlimits.push(PosixRlimit {
        r#type: "RLIMIT_NOFILE".to_string(),
        hard: 1024,
        soft: 1024,
    });
    */

    process
}

// Default mounts field from containerd.
pub fn get_mounts(is_pause_container: bool) -> Vec<Mount> {
    let mut mounts = vec![
        Mount {
            destination: "/proc".to_string(),
            r#type: "proc".to_string(),
            source: "proc".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
            ],
        },
        Mount {
            destination: "/dev".to_string(),
            r#type: "tmpfs".to_string(),
            source: "tmpfs".to_string(),
            options: vec![
                "nosuid".to_string(),
                "strictatime".to_string(),
                "mode=755".to_string(),
                "size=65536k".to_string(),
            ],
        },
        Mount {
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
        },
        Mount {
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
        },
        Mount {
            destination: "/dev/mqueue".to_string(),
            r#type: "mqueue".to_string(),
            source: "mqueue".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
            ],
        },
        Mount {
            destination: "/sys".to_string(),
            r#type: "sysfs".to_string(),
            source: "sysfs".to_string(),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
                "ro".to_string(),
            ],
        },
    ];

    if !is_pause_container {
        mounts.push(Mount {
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
        });
    }

    mounts
}

// Default linux field from containerd.
pub fn get_linux() -> Linux {
    let mut linux: Linux = Default::default();

    linux.masked_paths = vec![
        "/proc/acpi".to_string(),
        "/proc/kcore".to_string(),
        "/proc/keys".to_string(),
        "/proc/latency_stats".to_string(),
        "/proc/timer_list".to_string(),
        "/proc/timer_stats".to_string(),
        "/proc/sched_debug".to_string(),
        "/proc/scsi".to_string(),
        "/sys/firmware".to_string(),
    ];

    linux.readonly_paths = vec![
        "/proc/asound".to_string(),
        "/proc/bus".to_string(),
        "/proc/fs".to_string(),
        "/proc/irq".to_string(),
        "/proc/sys".to_string(),
        "/proc/sysrq-trigger".to_string(),
    ];

    /*
    let mut device_cgroup: LinuxDeviceCgroup = Default::default();
    device_cgroup.allow = false;
    device_cgroup.access = "rwm".to_string();

    let mut resources: LinuxResources = Default::default();
    resources.devices.push(device_cgroup);
    linux.resources = Some(resources);
    */

    linux
}
