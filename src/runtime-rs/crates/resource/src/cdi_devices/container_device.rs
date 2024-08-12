//
// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use oci_spec::runtime::Spec;

use super::{resolve_cdi_device_kind, ContainerDevice};
use agent::types::Device;

const CDI_PREFIX: &str = "cdi.k8s.io";

// Sort the devices based on the first element's PCI_Guest_Path in the PCI bus according to options.
fn sort_devices_by_guest_pcipath(devices: &mut [ContainerDevice]) {
    // Extract first guest_pcipath from device_options
    let extract_first_guest_pcipath = |options: &[String]| -> Option<String> {
        options
            .first()
            .and_then(|option| option.split('=').nth(1))
            .map(|path| path.to_string())
    };

    devices.sort_by(|a, b| {
        let guest_path_a = extract_first_guest_pcipath(&a.device.options);
        let guest_path_b = extract_first_guest_pcipath(&b.device.options);

        guest_path_a.cmp(&guest_path_b)
    });
}

// Annotate container devices with CDI annotations in OCI Spec
pub fn annotate_container_devices(
    spec: &mut Spec,
    container_devices: Vec<ContainerDevice>,
) -> Result<Vec<Device>> {
    let mut devices_agent: Vec<Device> = Vec::new();
    // Make sure that annotations is Some().
    if spec.annotations().is_none() {
        spec.set_annotations(Some(HashMap::new()));
    }

    // Step 1: Extract all devices and filter out devices without device_info for vfio_devices
    let vfio_devices: Vec<ContainerDevice> = container_devices
        .into_iter()
        .map(|device| {
            // push every device's Device to agent_devices
            devices_agent.push(device.device.clone());
            device
        })
        .filter(|device| device.device_info.is_some())
        .collect();

    // Step 2: Group devices by vendor_id-class_id
    let mut grouped_devices: HashMap<String, Vec<ContainerDevice>> = HashMap::new();
    for device in vfio_devices {
        // Extract the vendor/class key and insert into the map if both are present
        if let Some(key) = device
            .device_info
            .as_ref()
            .and_then(|info| resolve_cdi_device_kind(&info.vendor_id, &info.class_id))
        {
            grouped_devices
                .entry(key.to_owned())
                .or_default()
                .push(device);
        }
    }

    // Step 3: Sort devices within each group by guest_pcipath
    grouped_devices
        .iter_mut()
        .for_each(|(vendor_class, container_devices)| {
            // The *offset* is a monotonically increasing counter that keeps track of the number of devices
            // within an IOMMU group. It increments by total_of whenever a new IOMMU group is processed.
            let offset: &mut usize = &mut 0;

            sort_devices_by_guest_pcipath(container_devices);
            container_devices
                .iter()
                .enumerate()
                .for_each(|(base, container_device)| {
                    let total_of = container_device.device.options.len();
                    // annotate device with cdi information in OCI Spec.
                    for index in 0..total_of {
                        if let Some(iommu_grpid) =
                            Path::new(&container_device.device.container_path)
                                .file_name()
                                .and_then(|name| name.to_str())
                        {
                            spec.annotations_mut().as_mut().unwrap().insert(
                                format!("{}/vfio{}.{}", CDI_PREFIX, iommu_grpid, index), // cdi.k8s.io/vfioX.y
                                format!("{}={}", vendor_class, base + *offset), // vendor/class=name
                            );
                        }
                    }

                    // update the offset with *total_of*.
                    *offset += total_of - 1;
                });
        });

    Ok(devices_agent)
}
