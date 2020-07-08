// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

/// Linux ABI related constants.

pub const SYSFS_DIR: &str = "/sys";

pub const SYSFS_PCI_BUS_PREFIX: &str = "/sys/bus/pci/devices";
pub const SYSFS_PCI_BUS_RESCAN_FILE: &str = "/sys/bus/pci/rescan";
#[cfg(any(
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "x86_64",
    target_arch = "x86"
))]
pub const PCI_ROOT_BUS_PATH: &str = "/devices/pci0000:00";
#[cfg(target_arch = "aarch64")]
pub const PCI_ROOT_BUS_PATH: &str = "/devices/platform/4010000000.pcie/pci0000:00";

pub const SYSFS_CPU_ONLINE_PATH: &str = "/sys/devices/system/cpu";

pub const SYSFS_MEMORY_BLOCK_SIZE_PATH: &str = "/sys/devices/system/memory/block_size_bytes";
pub const SYSFS_MEMORY_HOTPLUG_PROBE_PATH: &str = "/sys/devices/system/memory/probe";
pub const SYSFS_MEMORY_ONLINE_PATH: &str = "/sys/devices/system/memory";

// Here in "0:0", the first number is the SCSI host number because
// only one SCSI controller has been plugged, while the second number
// is always 0.
pub const SCSI_HOST_CHANNEL: &str = "0:0:";
pub const SCSI_BLOCK_SUFFIX: &str = "block";
pub const SYSFS_SCSI_HOST_PATH: &str = "/sys/class/scsi_host";

pub const SYSFS_CGROUPPATH: &str = "/sys/fs/cgroup";
pub const SYSFS_ONLINE_FILE: &str = "online";

pub const PROC_MOUNTSTATS: &str = "/proc/self/mountstats";
pub const PROC_CGROUPS: &str = "/proc/cgroups";

pub const SYSTEM_DEV_PATH: &str = "/dev";

// Linux UEvent related consts.
pub const U_EVENT_ACTION: &str = "ACTION";
pub const U_EVENT_ACTION_ADD: &str = "add";
pub const U_EVENT_DEV_PATH: &str = "DEVPATH";
pub const U_EVENT_SUB_SYSTEM: &str = "SUBSYSTEM";
pub const U_EVENT_SEQ_NUM: &str = "SEQNUM";
pub const U_EVENT_DEV_NAME: &str = "DEVNAME";
pub const U_EVENT_INTERFACE: &str = "INTERFACE";
