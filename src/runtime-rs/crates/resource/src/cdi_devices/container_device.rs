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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::cdi_devices::DeviceInfo;
    use agent::types::Device;
    use oci_spec::runtime::SpecBuilder;

    use super::*;

    #[test]
    fn test_sort_devices_by_guest_pcipath() {
        let mut devices = vec![
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0xffff".to_string(),
                    class_id: "0x030x".to_string(),
                    host_path: PathBuf::from("/dev/device3"),
                }),
                device: Device {
                    options: vec!["pci_host_path03=BB:DD03.F03".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0xffff".to_string(),
                    class_id: "0x030x".to_string(),
                    host_path: PathBuf::from("/dev/device1"),
                }),
                device: Device {
                    options: vec!["pci_host_path01=BB:DD01.F01".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0xffff".to_string(),
                    class_id: "0x030x".to_string(),
                    host_path: PathBuf::from("/dev/device2"),
                }),
                device: Device {
                    options: vec!["pci_host_path02=BB:DD02.F02".to_string()],
                    ..Default::default()
                },
            },
        ];

        sort_devices_by_guest_pcipath(&mut devices);

        let expected_devices_order = vec![
            "/dev/device1".to_string(),
            "/dev/device2".to_string(),
            "/dev/device3".to_string(),
        ];
        let actual_devices_order: Vec<String> = devices
            .iter()
            .map(|cd| {
                cd.device_info
                    .as_ref()
                    .unwrap()
                    .host_path
                    .display()
                    .to_string()
            })
            .collect();

        assert_eq!(actual_devices_order, expected_devices_order);
    }

    #[test]
    fn test_sort_devices_with_empty_options() {
        let mut devices = vec![
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0xffff".to_string(),
                    class_id: "0x030x".to_string(),
                    host_path: PathBuf::from("/dev/device1"),
                }),
                device: Device {
                    options: vec![], // empty
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0xffff".to_string(),
                    class_id: "0x030x".to_string(),
                    host_path: PathBuf::from("/dev/device2"),
                }),
                device: Device {
                    options: vec!["pci_host_path02=BB:DD02.F02".to_string()],
                    ..Default::default()
                },
            },
        ];

        sort_devices_by_guest_pcipath(&mut devices);

        // As the first device has no options, ignore it.
        let expected_devices_order = vec!["BB:DD02.F02".to_string()];

        let actual_devices_order: Vec<String> = devices
            .iter()
            .filter_map(|d| d.device.options.first())
            .map(|option| option.split('=').nth(1).unwrap_or("").to_string())
            .collect();

        assert_eq!(actual_devices_order, expected_devices_order);
    }

    #[test]
    fn test_annotate_container_devices() {
        let devices = vec![
            ContainerDevice {
                device_info: None,
                device: Device {
                    id: "test0000x".to_string(),
                    container_path: "/dev/xvdx".to_string(),
                    field_type: "virtio-blk".to_string(),
                    vm_path: "/dev/vdx".to_string(),
                    options: vec![],
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0x1002".to_string(),
                    class_id: "0x0302".to_string(),
                    host_path: PathBuf::from("/dev/device2"),
                }),
                device: Device {
                    container_path: "/dev/device2".to_string(),
                    options: vec!["pci_host_path02=BB:DD02.F02".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0x1002".to_string(),
                    class_id: "0x0302".to_string(),
                    host_path: PathBuf::from("/dev/device3"),
                }),
                device: Device {
                    container_path: "/dev/device3".to_string(),
                    options: vec!["pci_host_path03=BB:DD03.F03".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0x1002".to_string(),
                    class_id: "0x0302".to_string(),
                    host_path: PathBuf::from("/dev/device1"),
                }),
                device: Device {
                    container_path: "/dev/device1".to_string(),
                    options: vec!["pci_host_path01=BB:DD01.F01".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: None,
                device: Device {
                    id: "test0000yx".to_string(),
                    container_path: "/dev/xvdyx".to_string(),
                    field_type: "virtio-blk".to_string(),
                    vm_path: "/dev/vdyx".to_string(),
                    options: vec![],
                },
            },
        ];

        let annotations = HashMap::new();
        let mut spec = SpecBuilder::default()
            .annotations(annotations)
            .build()
            .unwrap();

        // do annotate container devices
        let _devices = annotate_container_devices(&mut spec, devices);

        let expected_annotations: HashMap<String, String> = vec![
            (
                "cdi.k8s.io/vfiodevice3.0".to_owned(),
                "amd.com/gpu=2".to_owned(),
            ),
            (
                "cdi.k8s.io/vfiodevice1.0".to_owned(),
                "amd.com/gpu=0".to_owned(),
            ),
            (
                "cdi.k8s.io/vfiodevice2.0".to_owned(),
                "amd.com/gpu=1".to_owned(),
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(Some(expected_annotations), spec.annotations().clone());
    }

    #[test]
    fn test_annotate_container_multi_vendor_devices() {
        let devices = vec![
            ContainerDevice {
                device_info: None,
                device: Device {
                    id: "test0000x".to_string(),
                    container_path: "/dev/xvdx".to_string(),
                    field_type: "virtio-blk".to_string(),
                    vm_path: "/dev/vdx".to_string(),
                    options: vec![],
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0x10de".to_string(),
                    class_id: "0x0302".to_string(),
                    host_path: PathBuf::from("/dev/device2"),
                }),
                device: Device {
                    container_path: "/dev/device2".to_string(),
                    options: vec!["pci_host_path02=BB:DD02.F02".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0x10de".to_string(),
                    class_id: "0x0302".to_string(),
                    host_path: PathBuf::from("/dev/device3"),
                }),
                device: Device {
                    container_path: "/dev/device3".to_string(),
                    options: vec!["pci_host_path03=BB:DD03.F03".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0x8086".to_string(),
                    class_id: "0x0302".to_string(),
                    host_path: PathBuf::from("/dev/device1"),
                }),
                device: Device {
                    container_path: "/dev/device1".to_string(),
                    options: vec!["pci_host_path01=BB:DD01.F01".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: Some(DeviceInfo {
                    vendor_id: "0x8086".to_string(),
                    class_id: "0x0302".to_string(),
                    host_path: PathBuf::from("/dev/device4"),
                }),
                device: Device {
                    container_path: "/dev/device4".to_string(),
                    options: vec!["pci_host_path04=BB:DD01.F04".to_string()],
                    ..Default::default()
                },
            },
            ContainerDevice {
                device_info: None,
                device: Device {
                    id: "test0000yx".to_string(),
                    container_path: "/dev/xvdyx".to_string(),
                    field_type: "virtio-blk".to_string(),
                    vm_path: "/dev/vdyx".to_string(),
                    options: vec![],
                },
            },
        ];

        let annotations = HashMap::new();
        let mut spec = SpecBuilder::default()
            .annotations(annotations)
            .build()
            .unwrap();

        let _devices = annotate_container_devices(&mut spec, devices);

        let expected_annotations: HashMap<String, String> = vec![
            (
                "cdi.k8s.io/vfiodevice1.0".to_owned(),
                "intel.com/gpu=0".to_owned(),
            ),
            (
                "cdi.k8s.io/vfiodevice2.0".to_owned(),
                "nvidia.com/gpu=0".to_owned(),
            ),
            (
                "cdi.k8s.io/vfiodevice3.0".to_owned(),
                "nvidia.com/gpu=1".to_owned(),
            ),
            (
                "cdi.k8s.io/vfiodevice4.0".to_owned(),
                "intel.com/gpu=1".to_owned(),
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(Some(expected_annotations), spec.annotations().clone());
    }

    #[test]
    fn test_annotate_container_without_vfio_devices() {
        let devices = vec![
            ContainerDevice {
                device_info: None,
                device: Device {
                    id: "test0000x".to_string(),
                    container_path: "/dev/xvdx".to_string(),
                    field_type: "virtio-blk".to_string(),
                    vm_path: "/dev/vdx".to_string(),
                    options: vec![],
                },
            },
            ContainerDevice {
                device_info: None,
                device: Device {
                    id: "test0000y".to_string(),
                    container_path: "/dev/yvdy".to_string(),
                    field_type: "virtio-blk".to_string(),
                    vm_path: "/dev/vdy".to_string(),
                    options: vec![],
                },
            },
            ContainerDevice {
                device_info: None,
                device: Device {
                    id: "test0000z".to_string(),
                    container_path: "/dev/zvdz".to_string(),
                    field_type: "virtio-blk".to_string(),
                    vm_path: "/dev/zvdz".to_string(),
                    options: vec![],
                },
            },
        ];

        let annotations = HashMap::from([(
            "cdi.k8s.io/vfiodeviceX".to_owned(),
            "katacontainer.com/device=Y".to_owned(),
        )]);
        let mut spec = SpecBuilder::default()
            .annotations(annotations)
            .build()
            .unwrap();

        // do annotate container devices
        let annotated_devices = annotate_container_devices(&mut spec, devices.clone()).unwrap();

        let actual_devices = devices
            .iter()
            .map(|d| d.device.clone())
            .collect::<Vec<Device>>();
        let expected_annotations: HashMap<String, String> = HashMap::from([(
            "cdi.k8s.io/vfiodeviceX".to_owned(),
            "katacontainer.com/device=Y".to_owned(),
        )]);

        assert_eq!(Some(expected_annotations), spec.annotations().clone());
        assert_eq!(annotated_devices, actual_devices);
    }
}
