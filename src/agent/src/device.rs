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
use std::sync::{mpsc, Arc, Mutex};

use crate::linux_abi::*;
use crate::mount::{DRIVERBLKTYPE, DRIVERMMIOBLKTYPE, DRIVERNVDIMMTYPE, DRIVERSCSITYPE};
use crate::sandbox::Sandbox;
use crate::{AGENT_CONFIG, GLOBAL_DEVICE_WATCHER};
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

// DeviceHandler is the type of callback to be defined to handle every type of device driver.
type DeviceHandler = fn(&Device, &mut Spec, &Arc<Mutex<Sandbox>>) -> Result<()>;

// DeviceHandlerList lists the supported drivers.
#[cfg_attr(rustfmt, rustfmt_skip)]
lazy_static! {
    static ref DEVICEHANDLERLIST: HashMap<&'static str, DeviceHandler> = {
        let mut m: HashMap<&'static str, DeviceHandler> = HashMap::new();
        m.insert(DRIVERBLKTYPE, virtio_blk_device_handler);
        m.insert(DRIVERMMIOBLKTYPE, virtiommio_blk_device_handler);
        m.insert(DRIVERNVDIMMTYPE, virtio_nvdimm_device_handler);
        m.insert(DRIVERSCSITYPE, virtio_scsi_device_handler);
        m
    };
}

pub fn rescan_pci_bus() -> Result<()> {
    online_device(SYSFS_PCI_BUS_RESCAN_FILE)
}

pub fn online_device(path: &str) -> Result<()> {
    fs::write(path, "1")?;
    Ok(())
}

// get_pci_device_address fetches the complete PCI address in sysfs, based on the PCI
// identifier provided. This should be in the format: "bridgeAddr/deviceAddr".
// Here, bridgeAddr is the address at which the bridge is attached on the root bus,
// while deviceAddr is the address at which the device is attached on the bridge.
fn get_pci_device_address(pci_id: &str) -> Result<String> {
    let tokens: Vec<&str> = pci_id.split("/").collect();

    if tokens.len() != 2 {
        return Err(anyhow!(
            "PCI Identifier for device should be of format [bridgeAddr/deviceAddr], got {}",
            pci_id
        ));
    }

    let bridge_id = tokens[0];
    let device_id = tokens[1];

    // Deduce the complete bridge address based on the bridge address identifier passed
    // and the fact that bridges are attached on the main bus with function 0.
    let pci_bridge_addr = format!("0000:00:{}.0", bridge_id);

    // Find out the bus exposed by bridge
    let bridge_bus_path = format!("{}/{}/pci_bus/", SYSFS_PCI_BUS_PREFIX, pci_bridge_addr);

    let files_slice: Vec<_> = fs::read_dir(&bridge_bus_path)
        .unwrap()
        .map(|res| res.unwrap().path())
        .collect();
    let bus_num = files_slice.len();

    if bus_num != 1 {
        return Err(anyhow!(
            "Expected an entry for bus in {}, got {} entries instead",
            bridge_bus_path,
            bus_num
        ));
    }

    let bus = files_slice[0].file_name().unwrap().to_str().unwrap();

    // Device address is based on the bus of the bridge to which it is attached.
    // We do not pass devices as multifunction, hence the trailing 0 in the address.
    let pci_device_addr = format!("{}:{}.0", bus, device_id);

    let bridge_device_pci_addr = format!("{}/{}", pci_bridge_addr, pci_device_addr);

    info!(
        sl!(),
        "Fetched PCI address for device PCIAddr:{}\n", bridge_device_pci_addr
    );

    Ok(bridge_device_pci_addr)
}

fn get_device_name(sandbox: &Arc<Mutex<Sandbox>>, dev_addr: &str) -> Result<String> {
    // Keep the same lock order as uevent::handle_block_add_event(), otherwise it may cause deadlock.
    let mut w = GLOBAL_DEVICE_WATCHER.lock().unwrap();
    let sb = sandbox.lock().unwrap();
    for (key, value) in sb.pci_device_map.iter() {
        if key.contains(dev_addr) {
            info!(sl!(), "Device {} found in pci device map", dev_addr);
            return Ok(format!("{}/{}", SYSTEM_DEV_PATH, value));
        }
    }
    drop(sb);

    // If device is not found in the device map, hotplug event has not
    // been received yet, create and add channel to the watchers map.
    // The key of the watchers map is the device we are interested in.
    // Note this is done inside the lock, not to miss any events from the
    // global udev listener.
    let (tx, rx) = mpsc::channel::<String>();
    w.insert(dev_addr.to_string(), tx);
    drop(w);

    info!(sl!(), "Waiting on channel for device notification\n");
    let hotplug_timeout = AGENT_CONFIG.read().unwrap().hotplug_timeout;
    let dev_name = match rx.recv_timeout(hotplug_timeout) {
        Ok(name) => name,
        Err(_) => {
            GLOBAL_DEVICE_WATCHER.lock().unwrap().remove_entry(dev_addr);
            return Err(anyhow!(
                "Timeout reached after {:?} waiting for device {}",
                hotplug_timeout,
                dev_addr
            ));
        }
    };

    Ok(format!("{}/{}", SYSTEM_DEV_PATH, &dev_name))
}

pub fn get_scsi_device_name(sandbox: &Arc<Mutex<Sandbox>>, scsi_addr: &str) -> Result<String> {
    let dev_sub_path = format!("{}{}/{}", SCSI_HOST_CHANNEL, scsi_addr, SCSI_BLOCK_SUFFIX);

    scan_scsi_bus(scsi_addr)?;
    get_device_name(sandbox, &dev_sub_path)
}

pub fn get_pci_device_name(sandbox: &Arc<Mutex<Sandbox>>, pci_id: &str) -> Result<String> {
    let pci_addr = get_pci_device_address(pci_id)?;

    rescan_pci_bus()?;
    get_device_name(sandbox, &pci_addr)
}

/// Scan SCSI bus for the given SCSI address(SCSI-Id and LUN)
fn scan_scsi_bus(scsi_addr: &str) -> Result<()> {
    let tokens: Vec<&str> = scsi_addr.split(":").collect();
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
fn update_spec_device_list(device: &Device, spec: &mut Spec) -> Result<()> {
    let major_id: c_uint;
    let minor_id: c_uint;

    // If no container_path is provided, we won't be able to match and
    // update the device in the OCI spec device list. This is an error.
    if device.container_path == "" {
        return Err(anyhow!(
            "container_path cannot empty for device {:?}",
            device
        ));
    }

    let linux = match spec.linux.as_mut() {
        None => return Err(anyhow!("Spec didn't container linux field")),
        Some(l) => l,
    };

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

    let devices = linux.devices.as_mut_slice();
    for dev in devices.iter_mut() {
        if dev.path == device.container_path {
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

            // Resources must be updated since they are used to identify the
            // device in the devices cgroup.
            if let Some(res) = linux.resources.as_mut() {
                let ds = res.devices.as_mut_slice();
                for d in ds.iter_mut() {
                    if d.major == Some(host_major) && d.minor == Some(host_minor) {
                        d.major = Some(major_id as i64);
                        d.minor = Some(minor_id as i64);

                        info!(
                            sl!(),
                            "set resources for device major: {} minor: {}\n", major_id, minor_id
                        );
                    }
                }
            }
            return Ok(());
        }
    }

    Err(anyhow!(
        "Should have found a matching device {} in the spec",
        device.vm_path
    ))
}

// device.Id should be the predicted device name (vda, vdb, ...)
// device.VmPath already provides a way to send it in
fn virtiommio_blk_device_handler(
    device: &Device,
    spec: &mut Spec,
    _sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    if device.vm_path == "" {
        return Err(anyhow!("Invalid path for virtio mmio blk device"));
    }

    update_spec_device_list(device, spec)
}

// device.Id should be the PCI address in the format  "bridgeAddr/deviceAddr".
// Here, bridgeAddr is the address at which the brige is attached on the root bus,
// while deviceAddr is the address at which the device is attached on the bridge.
fn virtio_blk_device_handler(
    device: &Device,
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    let mut dev = device.clone();

    // When "Id (PCIAddr)" is not set, we allow to use the predicted "VmPath" passed from kata-runtime
    // Note this is a special code path for cloud-hypervisor when BDF information is not available
    if device.id != "" {
        dev.vm_path = get_pci_device_name(sandbox, &device.id)?;
    }

    update_spec_device_list(&dev, spec)
}

// device.Id should be the SCSI address of the disk in the format "scsiID:lunID"
fn virtio_scsi_device_handler(
    device: &Device,
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    let mut dev = device.clone();
    dev.vm_path = get_scsi_device_name(sandbox, &device.id)?;
    update_spec_device_list(&dev, spec)
}

fn virtio_nvdimm_device_handler(
    device: &Device,
    spec: &mut Spec,
    _sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    if device.vm_path == "" {
        return Err(anyhow!("Invalid path for nvdimm device"));
    }

    update_spec_device_list(device, spec)
}

pub fn add_devices(
    devices: &[Device],
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    for device in devices.iter() {
        add_device(device, spec, sandbox)?;
    }

    Ok(())
}

fn add_device(device: &Device, spec: &mut Spec, sandbox: &Arc<Mutex<Sandbox>>) -> Result<()> {
    // log before validation to help with debugging gRPC protocol version differences.
    info!(sl!(), "device-id: {}, device-type: {}, device-vm-path: {}, device-container-path: {}, device-options: {:?}",
          device.id, device.field_type, device.vm_path, device.container_path, device.options);

    if device.field_type == "" {
        return Err(anyhow!("invalid type for device {:?}", device));
    }

    if device.id == "" && device.vm_path == "" {
        return Err(anyhow!("invalid ID and VM path for device {:?}", device));
    }

    if device.container_path == "" {
        return Err(anyhow!("invalid container path for device {:?}", device));
    }

    match DEVICEHANDLERLIST.get(device.field_type.as_str()) {
        None => Err(anyhow!("Unknown device type {}", device.field_type)),
        Some(dev_handler) => dev_handler(device, spec, sandbox),
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

    let linux = match spec.linux.as_mut() {
        None => return Err(anyhow!("Spec didn't container linux field")),
        Some(l) => l,
    };

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

    #[test]
    fn test_update_device_cgroup() {
        let mut spec = Spec::default();

        spec.linux = Some(Linux::default());

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
}
