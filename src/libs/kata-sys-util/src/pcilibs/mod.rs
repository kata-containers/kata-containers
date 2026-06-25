// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
mod pci_manager;

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::prelude::FileTypeExt;

use kata_types::device::{
    DRIVER_VFIO_AP_COLD_TYPE, DRIVER_VFIO_AP_TYPE, DRIVER_VFIO_PCI_GK_TYPE, DRIVER_VFIO_PCI_TYPE,
};
use nix::sys::stat;

pub use pci_manager::{is_pcie_device, PCIDevice, PCIDeviceManager};

pub fn is_vfio_device_type(device_type: &str) -> bool {
    matches!(
        device_type,
        DRIVER_VFIO_PCI_TYPE
            | DRIVER_VFIO_PCI_GK_TYPE
            | DRIVER_VFIO_AP_TYPE
            | DRIVER_VFIO_AP_COLD_TYPE
    )
}

/// One-line summary of every `/sys/class/infiniband*` device the
/// guest kernel currently exposes, plus every char device under
/// `/dev/infiniband/` and the PCI BDF backing each IB device.
///
/// Pure sysfs / devfs reads — no agent-specific dependencies.
/// Used as a diagnostic context string in log calls.
pub fn snapshot_infiniband() -> String {
    let mut ib_parts: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/infiniband") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let pci_bdf = fs::read_link(path.join("device"))
                .ok()
                .and_then(|t| t.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "<none>".to_string());
            let node_type = fs::read_to_string(path.join("node_type"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let fw = fs::read_to_string(path.join("fw_ver"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            ib_parts.push(format!(
                "{name}=[bdf={pci_bdf},node_type={node_type:?},fw={fw}]"
            ));
        }
    }

    let mut verbs_parts: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/infiniband_verbs") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("uverbs") {
                continue;
            }
            let path = entry.path();
            let ibdev = fs::read_to_string(path.join("ibdev"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let dev = fs::read_to_string(path.join("dev"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            verbs_parts.push(format!("{name}=[ibdev={ibdev},dev={dev}]"));
        }
    }

    let mut chardev_parts: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir("/dev/infiniband") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let kind = if metadata.file_type().is_char_device() {
                "char"
            } else if metadata.file_type().is_block_device() {
                "block"
            } else {
                "other"
            };
            let rdev = metadata.rdev();
            let major = stat::major(rdev);
            let minor = stat::minor(rdev);
            chardev_parts.push(format!("{name}=[{kind},{major}:{minor}]"));
        }
    }

    format!(
        "ib_devices=[{}] uverbs=[{}] chardevs=[{}]",
        ib_parts.join(", "),
        verbs_parts.join(", "),
        chardev_parts.join(", "),
    )
}
