// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use libc::{c_uint, major, minor};
use nix::sys::stat;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::linux_abi::*;
use crate::mount::{DRIVER_BLK_TYPE, DRIVER_MMIO_BLK_TYPE, DRIVER_NVDIMM_TYPE, DRIVER_SCSI_TYPE};
use crate::pci;
use crate::sandbox::Sandbox;
use crate::uevent::Uevent;
use crate::AGENT_CONFIG;
use anyhow::{anyhow, Result};
use oci::{LinuxDeviceCgroup, LinuxResources, Spec};
use protocols::agent::Device;

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "device"))
    };
}

const VM_ROOTFS: &str = "/";

struct DevIndexEntry {
    idx: usize,
    residx: Vec<usize>,
}

struct DevIndex(HashMap<String, DevIndexEntry>);

pub fn rescan_pci_bus() -> Result<()> {
    online_device(SYSFS_PCI_BUS_RESCAN_FILE)
}

pub fn online_device(path: &str) -> Result<()> {
    fs::write(path, "1")?;
    Ok(())
}

// pcipath_to_sysfs fetches the sysfs path for a PCI path, relative to
// the sysfs path for the PCI host bridge, based on the PCI path
// provided.
fn pcipath_to_sysfs(root_bus_sysfs: &str, pcipath: &pci::Path) -> Result<String> {
    let mut bus = "0000:00".to_string();
    let mut relpath = String::new();

    for i in 0..pcipath.len() {
        let bdf = format!("{}:{}.0", bus, pcipath[i]);

        relpath = format!("{}/{}", relpath, bdf);

        if i == pcipath.len() - 1 {
            // Final device need not be a bridge
            break;
        }

        // Find out the bus exposed by bridge
        let bridgebuspath = format!("{}{}/pci_bus", root_bus_sysfs, relpath);
        let mut files: Vec<_> = fs::read_dir(&bridgebuspath)?.collect();

        if files.len() != 1 {
            return Err(anyhow!(
                "Expected exactly one PCI bus in {}, got {} instead",
                bridgebuspath,
                files.len()
            ));
        }

        // unwrap is safe, because of the length test above
        let busfile = files.pop().unwrap()?;
        bus = busfile
            .file_name()
            .into_string()
            .map_err(|e| anyhow!("Bad filename under {}: {:?}", &bridgebuspath, e))?;
    }

    Ok(relpath)
}

async fn get_device_name(sandbox: &Arc<Mutex<Sandbox>>, dev_addr: &str) -> Result<String> {
    let mut sb = sandbox.lock().await;
    for (key, uev) in sb.uevent_map.iter() {
        if key.contains(dev_addr) {
            info!(sl!(), "Device {} found in pci device map", dev_addr);
            return Ok(format!("{}/{}", SYSTEM_DEV_PATH, uev.devname));
        }
    }

    // If device is not found in the device map, hotplug event has not
    // been received yet, create and add channel to the watchers map.
    // The key of the watchers map is the device we are interested in.
    // Note this is done inside the lock, not to miss any events from the
    // global udev listener.
    let (tx, rx) = tokio::sync::oneshot::channel::<Uevent>();
    sb.dev_watcher.insert(dev_addr.to_string(), tx);
    drop(sb); // unlock

    info!(sl!(), "Waiting on channel for device notification\n");
    let hotplug_timeout = AGENT_CONFIG.read().await.hotplug_timeout;

    let uev = match tokio::time::timeout(hotplug_timeout, rx).await {
        Ok(v) => v?,
        Err(_) => {
            let mut sb = sandbox.lock().await;
            sb.dev_watcher.remove_entry(dev_addr);

            return Err(anyhow!(
                "Timeout reached after {:?} waiting for device {}",
                hotplug_timeout,
                dev_addr
            ));
        }
    };

    Ok(format!("{}/{}", SYSTEM_DEV_PATH, &uev.devname))
}

pub async fn get_scsi_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    scsi_addr: &str,
) -> Result<String> {
    let dev_sub_path = format!("{}{}/{}", SCSI_HOST_CHANNEL, scsi_addr, SCSI_BLOCK_SUFFIX);

    scan_scsi_bus(scsi_addr)?;
    get_device_name(sandbox, &dev_sub_path).await
}

pub async fn get_pci_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    pcipath: &pci::Path,
) -> Result<String> {
    let root_bus_sysfs = format!("{}{}", SYSFS_DIR, create_pci_root_bus_path());
    let sysfs_rel_path = pcipath_to_sysfs(&root_bus_sysfs, pcipath)?;

    rescan_pci_bus()?;
    get_device_name(sandbox, &sysfs_rel_path).await
}

pub async fn get_pmem_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    pmem_devname: &str,
) -> Result<String> {
    let dev_sub_path = format!("/{}/{}", SCSI_BLOCK_SUFFIX, pmem_devname);
    get_device_name(sandbox, &dev_sub_path).await
}

/// Scan SCSI bus for the given SCSI address(SCSI-Id and LUN)
fn scan_scsi_bus(scsi_addr: &str) -> Result<()> {
    let tokens: Vec<&str> = scsi_addr.split(':').collect();
    if tokens.len() != 2 {
        return Err(anyhow!(
            "Unexpected format for SCSI Address: {}, expect SCSIID:LUA",
            scsi_addr
        ));
    }

    // Scan scsi host passing in the channel, SCSI id and LUN.
    // Channel is always 0 because we have only one SCSI controller.
    let scan_data = format!("0 {} {}", tokens[0], tokens[1]);

    for entry in fs::read_dir(SYSFS_SCSI_HOST_PATH)? {
        let host = entry?.file_name();
        let scan_path = format!(
            "{}/{}/{}",
            SYSFS_SCSI_HOST_PATH,
            host.to_str().unwrap(),
            "scan"
        );

        fs::write(scan_path, &scan_data)?;
    }

    Ok(())
}

// update_spec_device_list takes a device description provided by the caller,
// trying to find it on the guest. Once this device has been identified, the
// "real" information that can be read from inside the VM is used to update
// the same device in the list of devices provided through the OCI spec.
// This is needed to update information about minor/major numbers that cannot
// be predicted from the caller.
fn update_spec_device_list(device: &Device, spec: &mut Spec, devidx: &DevIndex) -> Result<()> {
    let major_id: c_uint;
    let minor_id: c_uint;

    // If no container_path is provided, we won't be able to match and
    // update the device in the OCI spec device list. This is an error.
    if device.container_path.is_empty() {
        return Err(anyhow!(
            "container_path cannot empty for device {:?}",
            device
        ));
    }

    let linux = spec
        .linux
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't container linux field"))?;

    if !Path::new(&device.vm_path).exists() {
        return Err(anyhow!("vm_path:{} doesn't exist", device.vm_path));
    }

    let meta = fs::metadata(&device.vm_path)?;
    let dev_id = meta.rdev();
    unsafe {
        major_id = major(dev_id);
        minor_id = minor(dev_id);
    }

    info!(
        sl!(),
        "got the device: dev_path: {}, major: {}, minor: {}\n", &device.vm_path, major_id, minor_id
    );

    if let Some(idxdata) = devidx.0.get(device.container_path.as_str()) {
        let dev = &mut linux.devices[idxdata.idx];
        let host_major = dev.major;
        let host_minor = dev.minor;

        dev.major = major_id as i64;
        dev.minor = minor_id as i64;

        info!(
            sl!(),
            "change the device from major: {} minor: {} to vm device major: {} minor: {}",
            host_major,
            host_minor,
            major_id,
            minor_id
        );

        // Resources must be updated since they are used to identify
        // the device in the devices cgroup.
        for ridx in &idxdata.residx {
            // unwrap is safe, because residx would be empty if there
            // were no resources
            let res = &mut linux.resources.as_mut().unwrap().devices[*ridx];
            res.major = Some(major_id as i64);
            res.minor = Some(minor_id as i64);

            info!(
                sl!(),
                "set resources for device major: {} minor: {}\n", major_id, minor_id
            );
        }
        Ok(())
    } else {
        Err(anyhow!(
            "Should have found a matching device {} in the spec",
            device.vm_path
        ))
    }
}

// device.Id should be the predicted device name (vda, vdb, ...)
// device.VmPath already provides a way to send it in
async fn virtiommio_blk_device_handler(
    device: &Device,
    spec: &mut Spec,
    _sandbox: &Arc<Mutex<Sandbox>>,
    devidx: &DevIndex,
) -> Result<()> {
    if device.vm_path.is_empty() {
        return Err(anyhow!("Invalid path for virtio mmio blk device"));
    }

    update_spec_device_list(device, spec, devidx)
}

// device.Id should be a PCI path string
async fn virtio_blk_device_handler(
    device: &Device,
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
    devidx: &DevIndex,
) -> Result<()> {
    let mut dev = device.clone();

    // When "Id (PCI path)" is not set, we allow to use the predicted
    // "VmPath" passed from kata-runtime Note this is a special code
    // path for cloud-hypervisor when BDF information is not available
    if !device.id.is_empty() {
        let pcipath = pci::Path::from_str(&device.id)?;
        dev.vm_path = get_pci_device_name(sandbox, &pcipath).await?;
    }

    update_spec_device_list(&dev, spec, devidx)
}

// device.Id should be the SCSI address of the disk in the format "scsiID:lunID"
async fn virtio_scsi_device_handler(
    device: &Device,
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
    devidx: &DevIndex,
) -> Result<()> {
    let mut dev = device.clone();
    dev.vm_path = get_scsi_device_name(sandbox, &device.id).await?;
    update_spec_device_list(&dev, spec, devidx)
}

async fn virtio_nvdimm_device_handler(
    device: &Device,
    spec: &mut Spec,
    _sandbox: &Arc<Mutex<Sandbox>>,
    devidx: &DevIndex,
) -> Result<()> {
    if device.vm_path.is_empty() {
        return Err(anyhow!("Invalid path for nvdimm device"));
    }

    update_spec_device_list(device, spec, devidx)
}

impl DevIndex {
    fn new(spec: &Spec) -> DevIndex {
        let mut map = HashMap::new();

        if let Some(linux) = spec.linux.as_ref() {
            for (i, d) in linux.devices.iter().enumerate() {
                let mut residx = Vec::new();

                if let Some(linuxres) = linux.resources.as_ref() {
                    for (j, r) in linuxres.devices.iter().enumerate() {
                        if r.r#type == d.r#type
                            && r.major == Some(d.major)
                            && r.minor == Some(d.minor)
                        {
                            residx.push(j);
                        }
                    }
                }
                map.insert(d.path.clone(), DevIndexEntry { idx: i, residx });
            }
        }
        DevIndex(map)
    }
}

pub async fn add_devices(
    devices: &[Device],
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    let devidx = DevIndex::new(spec);

    for device in devices.iter() {
        add_device(device, spec, sandbox, &devidx).await?;
    }

    Ok(())
}

async fn add_device(
    device: &Device,
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
    devidx: &DevIndex,
) -> Result<()> {
    // log before validation to help with debugging gRPC protocol version differences.
    info!(sl!(), "device-id: {}, device-type: {}, device-vm-path: {}, device-container-path: {}, device-options: {:?}",
          device.id, device.field_type, device.vm_path, device.container_path, device.options);

    if device.field_type.is_empty() {
        return Err(anyhow!("invalid type for device {:?}", device));
    }

    if device.id.is_empty() && device.vm_path.is_empty() {
        return Err(anyhow!("invalid ID and VM path for device {:?}", device));
    }

    if device.container_path.is_empty() {
        return Err(anyhow!("invalid container path for device {:?}", device));
    }

    match device.field_type.as_str() {
        DRIVER_BLK_TYPE => virtio_blk_device_handler(device, spec, sandbox, devidx).await,
        DRIVER_MMIO_BLK_TYPE => virtiommio_blk_device_handler(device, spec, sandbox, devidx).await,
        DRIVER_NVDIMM_TYPE => virtio_nvdimm_device_handler(device, spec, sandbox, devidx).await,
        DRIVER_SCSI_TYPE => virtio_scsi_device_handler(device, spec, sandbox, devidx).await,
        _ => Err(anyhow!("Unknown device type {}", device.field_type)),
    }
}

// update_device_cgroup update the device cgroup for container
// to not allow access to the guest root partition. This prevents
// the container from being able to access the VM rootfs.
pub fn update_device_cgroup(spec: &mut Spec) -> Result<()> {
    let meta = fs::metadata(VM_ROOTFS)?;
    let rdev = meta.dev();
    let major = stat::major(rdev) as i64;
    let minor = stat::minor(rdev) as i64;

    let linux = spec
        .linux
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't container linux field"))?;

    if linux.resources.is_none() {
        linux.resources = Some(LinuxResources::default());
    }

    let resources = linux.resources.as_mut().unwrap();
    resources.devices.push(LinuxDeviceCgroup {
        allow: false,
        major: Some(major),
        minor: Some(minor),
        r#type: String::from("b"),
        access: String::from("rw"),
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci::Linux;
    use tempfile::tempdir;

    #[test]
    fn test_update_device_cgroup() {
        let mut spec = Spec {
            linux: Some(Linux::default()),
            ..Default::default()
        };

        update_device_cgroup(&mut spec).unwrap();

        let devices = spec.linux.unwrap().resources.unwrap().devices;
        assert_eq!(devices.len(), 1);

        let meta = fs::metadata(VM_ROOTFS).unwrap();
        let rdev = meta.dev();
        let major = stat::major(rdev) as i64;
        let minor = stat::minor(rdev) as i64;

        assert_eq!(devices[0].major, Some(major));
        assert_eq!(devices[0].minor, Some(minor));
    }

    #[test]
    fn test_update_spec_device_list() {
        let (major, minor) = (7, 2);
        let mut device = Device::default();
        let mut spec = Spec::default();

        // container_path empty
        let devidx = DevIndex::new(&spec);
        let res = update_spec_device_list(&device, &mut spec, &devidx);
        assert!(res.is_err());

        device.container_path = "/dev/null".to_string();

        // linux is empty
        let devidx = DevIndex::new(&spec);
        let res = update_spec_device_list(&device, &mut spec, &devidx);
        assert!(res.is_err());

        spec.linux = Some(Linux::default());

        // linux.devices is empty
        let devidx = DevIndex::new(&spec);
        let res = update_spec_device_list(&device, &mut spec, &devidx);
        assert!(res.is_err());

        spec.linux.as_mut().unwrap().devices = vec![oci::LinuxDevice {
            path: "/dev/null2".to_string(),
            major,
            minor,
            ..oci::LinuxDevice::default()
        }];

        // vm_path empty
        let devidx = DevIndex::new(&spec);
        let res = update_spec_device_list(&device, &mut spec, &devidx);
        assert!(res.is_err());

        device.vm_path = "/dev/null".to_string();

        // guest and host path are not the same
        let devidx = DevIndex::new(&spec);
        let res = update_spec_device_list(&device, &mut spec, &devidx);
        assert!(res.is_err(), "device={:?} spec={:?}", device, spec);

        spec.linux.as_mut().unwrap().devices[0].path = device.container_path.clone();

        // spec.linux.resources is empty
        let devidx = DevIndex::new(&spec);
        let res = update_spec_device_list(&device, &mut spec, &devidx);
        assert!(res.is_ok());

        // update both devices and cgroup lists
        spec.linux.as_mut().unwrap().devices = vec![oci::LinuxDevice {
            path: device.container_path.clone(),
            major,
            minor,
            ..oci::LinuxDevice::default()
        }];

        spec.linux.as_mut().unwrap().resources = Some(oci::LinuxResources {
            devices: vec![oci::LinuxDeviceCgroup {
                major: Some(major),
                minor: Some(minor),
                ..oci::LinuxDeviceCgroup::default()
            }],
            ..oci::LinuxResources::default()
        });

        let devidx = DevIndex::new(&spec);
        let res = update_spec_device_list(&device, &mut spec, &devidx);
        assert!(res.is_ok());
    }

    #[test]
    fn test_update_spec_device_list_guest_host_conflict() {
        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();
        let zero_rdev = fs::metadata("/dev/zero").unwrap().rdev();
        let full_rdev = fs::metadata("/dev/full").unwrap().rdev();

        let host_major_a = stat::major(null_rdev) as i64;
        let host_minor_a = stat::minor(null_rdev) as i64;
        let host_major_b = stat::major(zero_rdev) as i64;
        let host_minor_b = stat::minor(zero_rdev) as i64;

        let mut spec = Spec {
            linux: Some(Linux {
                devices: vec![
                    oci::LinuxDevice {
                        path: "/dev/a".to_string(),
                        r#type: "c".to_string(),
                        major: host_major_a,
                        minor: host_minor_a,
                        ..oci::LinuxDevice::default()
                    },
                    oci::LinuxDevice {
                        path: "/dev/b".to_string(),
                        r#type: "c".to_string(),
                        major: host_major_b,
                        minor: host_minor_b,
                        ..oci::LinuxDevice::default()
                    },
                ],
                resources: Some(LinuxResources {
                    devices: vec![
                        oci::LinuxDeviceCgroup {
                            r#type: "c".to_string(),
                            major: Some(host_major_a),
                            minor: Some(host_minor_a),
                            ..oci::LinuxDeviceCgroup::default()
                        },
                        oci::LinuxDeviceCgroup {
                            r#type: "c".to_string(),
                            major: Some(host_major_b),
                            minor: Some(host_minor_b),
                            ..oci::LinuxDeviceCgroup::default()
                        },
                    ],
                    ..LinuxResources::default()
                }),
                ..Linux::default()
            }),
            ..Spec::default()
        };
        let devidx = DevIndex::new(&spec);

        let dev_a = Device {
            container_path: "/dev/a".to_string(),
            vm_path: "/dev/zero".to_string(),
            ..Device::default()
        };

        let guest_major_a = stat::major(zero_rdev) as i64;
        let guest_minor_a = stat::minor(zero_rdev) as i64;

        let dev_b = Device {
            container_path: "/dev/b".to_string(),
            vm_path: "/dev/full".to_string(),
            ..Device::default()
        };

        let guest_major_b = stat::major(full_rdev) as i64;
        let guest_minor_b = stat::minor(full_rdev) as i64;

        let specdevices = &spec.linux.as_ref().unwrap().devices;
        assert_eq!(host_major_a, specdevices[0].major);
        assert_eq!(host_minor_a, specdevices[0].minor);
        assert_eq!(host_major_b, specdevices[1].major);
        assert_eq!(host_minor_b, specdevices[1].minor);

        let specresources = spec.linux.as_ref().unwrap().resources.as_ref().unwrap();
        assert_eq!(Some(host_major_a), specresources.devices[0].major);
        assert_eq!(Some(host_minor_a), specresources.devices[0].minor);
        assert_eq!(Some(host_major_b), specresources.devices[1].major);
        assert_eq!(Some(host_minor_b), specresources.devices[1].minor);

        let res = update_spec_device_list(&dev_a, &mut spec, &devidx);
        assert!(res.is_ok());

        let specdevices = &spec.linux.as_ref().unwrap().devices;
        assert_eq!(guest_major_a, specdevices[0].major);
        assert_eq!(guest_minor_a, specdevices[0].minor);
        assert_eq!(host_major_b, specdevices[1].major);
        assert_eq!(host_minor_b, specdevices[1].minor);

        let specresources = spec.linux.as_ref().unwrap().resources.as_ref().unwrap();
        assert_eq!(Some(guest_major_a), specresources.devices[0].major);
        assert_eq!(Some(guest_minor_a), specresources.devices[0].minor);
        assert_eq!(Some(host_major_b), specresources.devices[1].major);
        assert_eq!(Some(host_minor_b), specresources.devices[1].minor);

        let res = update_spec_device_list(&dev_b, &mut spec, &devidx);
        assert!(res.is_ok());

        let specdevices = &spec.linux.as_ref().unwrap().devices;
        assert_eq!(guest_major_a, specdevices[0].major);
        assert_eq!(guest_minor_a, specdevices[0].minor);
        assert_eq!(guest_major_b, specdevices[1].major);
        assert_eq!(guest_minor_b, specdevices[1].minor);

        let specresources = spec.linux.as_ref().unwrap().resources.as_ref().unwrap();
        assert_eq!(Some(guest_major_a), specresources.devices[0].major);
        assert_eq!(Some(guest_minor_a), specresources.devices[0].minor);
        assert_eq!(Some(guest_major_b), specresources.devices[1].major);
        assert_eq!(Some(guest_minor_b), specresources.devices[1].minor);
    }

    #[test]
    fn test_update_spec_device_list_char_block_conflict() {
        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();

        let guest_major = stat::major(null_rdev) as i64;
        let guest_minor = stat::minor(null_rdev) as i64;
        let host_major: i64 = 99;
        let host_minor: i64 = 99;

        let mut spec = Spec {
            linux: Some(Linux {
                devices: vec![
                    oci::LinuxDevice {
                        path: "/dev/char".to_string(),
                        r#type: "c".to_string(),
                        major: host_major,
                        minor: host_minor,
                        ..oci::LinuxDevice::default()
                    },
                    oci::LinuxDevice {
                        path: "/dev/block".to_string(),
                        r#type: "b".to_string(),
                        major: host_major,
                        minor: host_minor,
                        ..oci::LinuxDevice::default()
                    },
                ],
                resources: Some(LinuxResources {
                    devices: vec![
                        LinuxDeviceCgroup {
                            r#type: "c".to_string(),
                            major: Some(host_major),
                            minor: Some(host_minor),
                            ..LinuxDeviceCgroup::default()
                        },
                        LinuxDeviceCgroup {
                            r#type: "b".to_string(),
                            major: Some(host_major),
                            minor: Some(host_minor),
                            ..LinuxDeviceCgroup::default()
                        },
                    ],
                    ..LinuxResources::default()
                }),
                ..Linux::default()
            }),
            ..Spec::default()
        };
        let devidx = DevIndex::new(&spec);

        let dev = Device {
            container_path: "/dev/char".to_string(),
            vm_path: "/dev/null".to_string(),
            ..Device::default()
        };

        let specresources = spec.linux.as_ref().unwrap().resources.as_ref().unwrap();
        assert_eq!(Some(host_major), specresources.devices[0].major);
        assert_eq!(Some(host_minor), specresources.devices[0].minor);
        assert_eq!(Some(host_major), specresources.devices[1].major);
        assert_eq!(Some(host_minor), specresources.devices[1].minor);

        let res = update_spec_device_list(&dev, &mut spec, &devidx);
        assert!(res.is_ok());

        // Only the char device, not the block device should be updated
        let specresources = spec.linux.as_ref().unwrap().resources.as_ref().unwrap();
        assert_eq!(Some(guest_major), specresources.devices[0].major);
        assert_eq!(Some(guest_minor), specresources.devices[0].minor);
        assert_eq!(Some(host_major), specresources.devices[1].major);
        assert_eq!(Some(host_minor), specresources.devices[1].minor);
    }

    #[test]
    fn test_pcipath_to_sysfs() {
        let testdir = tempdir().expect("failed to create tmpdir");
        let rootbuspath = testdir.path().to_str().unwrap();

        let path2 = pci::Path::from_str("02").unwrap();
        let path23 = pci::Path::from_str("02/03").unwrap();
        let path234 = pci::Path::from_str("02/03/04").unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert!(relpath.is_err());

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files for the device at 0000:00:02.0
        let bridge2path = format!("{}{}", rootbuspath, "/0000:00:02.0");

        fs::create_dir_all(&bridge2path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert!(relpath.is_err());

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files to indicate that 0000:00:02.0 is a bridge to bus 01
        let bridge2bus = "0000:01";
        let bus2path = format!("{}/pci_bus/{}", bridge2path, bridge2bus);

        fs::create_dir_all(bus2path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files for a bridge at 0000:01:03.0 to bus 02
        let bridge3path = format!("{}/0000:01:03.0", bridge2path);
        let bridge3bus = "0000:02";
        let bus3path = format!("{}/pci_bus/{}", bridge3path, bridge3bus);

        fs::create_dir_all(bus3path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0/0000:02:04.0");
    }

    #[tokio::test]
    async fn test_get_device_name() {
        let devname = "vda";
        let root_bus = create_pci_root_bus_path();
        let relpath = "/0000:00:0a.0/0000:03:0b.0";
        let devpath = format!("{}{}/virtio4/block/{}", root_bus, relpath, devname);

        let mut uev = crate::uevent::Uevent::default();
        uev.devpath = devpath.clone();
        uev.devname = devname.to_string();

        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Arc::new(Mutex::new(Sandbox::new(&logger).unwrap()));

        let mut sb = sandbox.lock().await;
        sb.uevent_map.insert(devpath.clone(), uev);
        drop(sb); // unlock

        let name = get_device_name(&sandbox, relpath).await;
        assert!(name.is_ok(), "{}", name.unwrap_err());
        assert_eq!(name.unwrap(), format!("{}/{}", SYSTEM_DEV_PATH, devname));

        let mut sb = sandbox.lock().await;
        let uev = sb.uevent_map.remove(&devpath).unwrap();
        drop(sb); // unlock

        let watcher_sandbox = Arc::clone(&sandbox);
        tokio::spawn(async move {
            loop {
                let mut sb = watcher_sandbox.lock().await;
                let matched_key = sb
                    .dev_watcher
                    .keys()
                    .filter(|dev_addr| devpath.contains(*dev_addr))
                    .cloned()
                    .next();

                if let Some(k) = matched_key {
                    let sender = sb.dev_watcher.remove(&k).unwrap();
                    let _ = sender.send(uev);
                    return;
                }
                drop(sb); // unlock
            }
        });

        let name = get_device_name(&sandbox, relpath).await;
        assert!(name.is_ok(), "{}", name.unwrap_err());
        assert_eq!(name.unwrap(), format!("{}/{}", SYSTEM_DEV_PATH, devname));
    }
}
