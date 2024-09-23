//
// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod container_device;

use agent::types::Device;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct DeviceInfo {
    pub class_id: String,
    pub vendor_id: String,
    pub host_path: PathBuf,
}

#[derive(Clone, Default)]
pub struct ContainerDevice {
    pub device_info: Option<DeviceInfo>,
    pub device: Device,
}

lazy_static! {
    // *CDI_DEVICE_KIND_TABLE* is static hash map to store a mapping between device vendor and class
    // identifiers and their corresponding CDI vendor and class strings. This mapping is essentially a
    // lookup table that allows the system to determine the appropriate CDI for a given device based on
    // its vendor and class information.
    // Note: Our device mapping is designed to be flexible and responsive to user needs. The current list
    // is not exhaustive and will be updated as required.
    pub static ref CDI_DEVICE_KIND_TABLE: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("0x10de-0x030", "nvidia.com/gpu");
        m.insert("0x8086-0x030", "intel.com/gpu");
        m.insert("0x1002-0x030", "amd.com/gpu");
        m.insert("0x15b3-0x020", "nvidia.com/nic");
        // TODO:  it will be updated as required.
        m
    };
}

// Sort devices by guest_pcipath
pub fn sort_options_by_pcipath(mut device_options: Vec<String>) -> Vec<String> {
    device_options.sort_by(|a, b| {
        let extract_path = |s: &str| s.split('=').nth(1).map(|path| path.to_string());
        let guest_path_a = extract_path(a);
        let guest_path_b = extract_path(b);

        guest_path_a.cmp(&guest_path_b)
    });
    device_options
}

// Resolve the CDI vendor ID/device Class by a lookup table based on the provided vendor and class.
pub fn resolve_cdi_device_kind<'a>(vendor_id: &'a str, class_id: &'a str) -> Option<&'a str> {
    let vendor_class = format!("{}-{}", vendor_id, class_id);
    // The first 12 characters of the string ("0x10de-0x030") provide a concise
    // and clear identification of both the manufacturer and the device category.
    // it returns "nvidia.com/gpu", "amd.com/gpu" or others.
    CDI_DEVICE_KIND_TABLE.get(&vendor_class[..12]).copied()
}
