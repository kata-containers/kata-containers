// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use rustjail::errors::*;
use std::fs;
// use std::io::Write;
use libc::{c_uint, major, minor};
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::mount::{
    DRIVERBLKTYPE, DRIVERMMIOBLKTYPE, DRIVERNVDIMMTYPE, DRIVERSCSITYPE, TIMEOUT_HOTPLUG,
};
use crate::sandbox::Sandbox;
use crate::GLOBAL_DEVICE_WATCHER;
use protocols::agent::Device;
use protocols::oci::Spec;

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "device"))
    };
}

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "powerpc64le",
    target_arch = "s390x"
))]
pub const ROOT_BUS_PATH: &'static str = "/devices/pci0000:00";
#[cfg(target_arch = "arm")]
pub const ROOT_BUS_PATH: &'static str = "/devices/platform/4010000000.pcie/pci0000:00";

pub const SYSFS_DIR: &'static str = "/sys";

const SYS_BUS_PREFIX: &'static str = "/sys/bus/pci/devices";
const PCI_BUS_RESCAN_FILE: &'static str = "/sys/bus/pci/rescan";
const SYSTEM_DEV_PATH: &'static str = "/dev";

// SCSI const

// Here in "0:0", the first number is the SCSI host number because
// only one SCSI controller has been plugged, while the second number
// is always 0.
pub const SCSI_HOST_CHANNEL: &'static str = "0:0:";
const SYS_CLASS_PREFIX: &'static str = "/sys/class";
const SCSI_DISK_PREFIX: &'static str = "/sys/class/scsi_disk/0:0:";
pub const SCSI_BLOCK_SUFFIX: &'static str = "block";
const SCSI_DISK_SUFFIX: &'static str = "/device/block";
const SCSI_HOST_PATH: &'static str = "/sys/class/scsi_host";

// DeviceHandler is the type of callback to be defined to handle every
// type of device driver.
type DeviceHandler = fn(&Device, &mut Spec, Arc<Mutex<Sandbox>>) -> Result<()>;

// DeviceHandlerList lists the supported drivers.
#[cfg_attr(rustfmt, rustfmt_skip)]
lazy_static! {
    pub static ref DEVICEHANDLERLIST: HashMap<&'static str, DeviceHandler> = {
       let mut m = HashMap::new();
    let blk: DeviceHandler = virtio_blk_device_handler;
        m.insert(DRIVERBLKTYPE, blk);
    let virtiommio: DeviceHandler = virtiommio_blk_device_handler;
        m.insert(DRIVERMMIOBLKTYPE, virtiommio);
    let local: DeviceHandler = virtio_nvdimm_device_handler;
        m.insert(DRIVERNVDIMMTYPE, local);
    let scsi: DeviceHandler = virtio_scsi_device_handler;
        m.insert(DRIVERSCSITYPE, scsi);
        m
    };
}

pub fn rescan_pci_bus() -> Result<()> {
    online_device(PCI_BUS_RESCAN_FILE)
}

pub fn online_device(path: &str) -> Result<()> {
    fs::write(path, "1")?;
    Ok(())
}

// get_device_pci_address fetches the complete PCI address in sysfs, based on the PCI
// identifier provided. This should be in the format: "bridgeAddr/deviceAddr".
// Here, bridgeAddr is the address at which the brige is attached on the root bus,
// while deviceAddr is the address at which the device is attached on the bridge.
pub fn get_device_pci_address(pci_id: &str) -> Result<String> {
    let tokens: Vec<&str> = pci_id.split("/").collect();

    if tokens.len() != 2 {
        return Err(ErrorKind::ErrorCode(format!(
            "PCI Identifier for device should be of format [bridgeAddr/deviceAddr], got {}",
            pci_id
        ))
        .into());
    }

    let bridge_id = tokens[0];
    let device_id = tokens[1];

    // Deduce the complete bridge address based on the bridge address identifier passed
    // and the fact that bridges are attached on the main bus with function 0.
    let pci_bridge_addr = format!("0000:00:{}.0", bridge_id);

    // Find out the bus exposed by bridge
    let bridge_bus_path = format!("{}/{}/pci_bus/", SYS_BUS_PREFIX, pci_bridge_addr);

    let files_slice: Vec<_> = fs::read_dir(&bridge_bus_path)
        .unwrap()
        .map(|res| res.unwrap().path())
        .collect();
    let bus_num = files_slice.len();

    if bus_num != 1 {
        return Err(ErrorKind::ErrorCode(format!(
            "Expected an entry for bus in {}, got {} entries instead",
            bridge_bus_path, bus_num
        ))
        .into());
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

pub fn get_device_name(sandbox: Arc<Mutex<Sandbox>>, dev_addr: &str) -> Result<String> {
    let mut dev_name: String = String::default();
    let (tx, rx) = mpsc::channel::<String>();

    {
        let watcher = GLOBAL_DEVICE_WATCHER.clone();
        let mut w = watcher.lock().unwrap();

        let s = sandbox.clone();
        let sb = s.lock().unwrap();

        for (key, value) in &(sb.pci_device_map) {
            if key.contains(dev_addr) {
                dev_name = value.to_string();
                info!(sl!(), "Device {} found in pci device map", dev_addr);
                break;
            }
        }

        // If device is not found in the device map, hotplug event has not
        // been received yet, create and add channel to the watchers map.
        // The key of the watchers map is the device we are interested in.
        // Note this is done inside the lock, not to miss any events from the
        // global udev listener.
        if dev_name == "" {
            w.insert(dev_addr.to_string(), tx);
        }
    }

    if dev_name == "" {
        info!(sl!(), "Waiting on channel for device notification\n");

        match rx.recv_timeout(Duration::from_secs(TIMEOUT_HOTPLUG)) {
            Ok(name) => dev_name = name,
            Err(_) => {
                let watcher = GLOBAL_DEVICE_WATCHER.clone();
                let mut w = watcher.lock().unwrap();
                w.remove_entry(dev_addr);

                return Err(ErrorKind::ErrorCode(format!(
                    "Timeout reached after {} waiting for device {}",
                    TIMEOUT_HOTPLUG, dev_addr
                ))
                .into());
            }
        }
    }

    Ok(format!("{}/{}", SYSTEM_DEV_PATH, &dev_name))
}

pub fn get_scsi_device_name(sandbox: Arc<Mutex<Sandbox>>, scsi_addr: &str) -> Result<String> {
    scan_scsi_bus(scsi_addr)?;

    let dev_sub_path = format!("{}{}/{}", SCSI_HOST_CHANNEL, scsi_addr, SCSI_BLOCK_SUFFIX);

    get_device_name(sandbox, dev_sub_path.as_str())
}

pub fn get_pci_device_name(sandbox: Arc<Mutex<Sandbox>>, pci_id: &str) -> Result<String> {
    let pci_addr = get_device_pci_address(pci_id)?;

    rescan_pci_bus()?;

    get_device_name(sandbox, pci_addr.as_str())
}

// scan_scsi_bus scans SCSI bus for the given SCSI address(SCSI-Id and LUN)
pub fn scan_scsi_bus(scsi_addr: &str) -> Result<()> {
    let tokens: Vec<&str> = scsi_addr.split(":").collect();
    if tokens.len() != 2 {
        return Err(ErrorKind::Msg(format!(
            "Unexpected format for SCSI Address: {}, expect SCSIID:LUA",
            scsi_addr
        ))
        .into());
    }

    // Scan scsi host passing in the channel, SCSI id and LUN. Channel
    // is always 0 because we have only one SCSI controller.
    let scan_data = format!("0 {} {}", tokens[0], tokens[1]);

    for entry in fs::read_dir(SCSI_HOST_PATH)? {
        let entry = entry?;

        let host = entry.file_name();
        let scan_path = format!("{}/{}/{}", SCSI_HOST_PATH, host.to_str().unwrap(), "scan");

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
    // If no container_path is provided, we won't be able to match and
    // update the device in the OCI spec device list. This is an error.

    let major_id: c_uint;
    let minor_id: c_uint;

    // If no container_path is provided, we won't be able to match and
    // update the device in the OCI spec device list. This is an error.
    if device.container_path == "" {
        return Err(ErrorKind::Msg(format!(
            "container_path  cannot empty for device {:?}",
            device
        ))
        .into());
    }

    let linux = match spec.Linux.as_mut() {
        None => {
            return Err(
                ErrorKind::ErrorCode("Spec didn't container linux field".to_string()).into(),
            )
        }
        Some(l) => l,
    };

    if !Path::new(&device.vm_path).exists() {
        return Err(ErrorKind::Msg(format!("vm_path:{} doesn't exist", device.vm_path)).into());
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

    let devices = linux.Devices.as_mut_slice();
    for dev in devices.iter_mut() {
        if dev.Path == device.container_path {
            let host_major = dev.Major;
            let host_minor = dev.Minor;

            dev.Major = major_id as i64;
            dev.Minor = minor_id as i64;

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
            let resource = linux.Resources.as_mut();
            if resource.is_some() {
                let res = resource.unwrap();
                let ds = res.Devices.as_mut_slice();
                for d in ds.iter_mut() {
                    if d.Major == host_major && d.Minor == host_minor {
                        d.Major = major_id as i64;
                        d.Minor = minor_id as i64;

                        info!(
                            sl!(),
                            "set resources for device major: {} minor: {}\n", major_id, minor_id
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

// device.Id should be the predicted device name (vda, vdb, ...)
// device.VmPath already provides a way to send it in
fn virtiommio_blk_device_handler(
    device: &Device,
    spec: &mut Spec,
    _sandbox: Arc<Mutex<Sandbox>>,
) -> Result<()> {
    if device.vm_path == "" {
        return Err(ErrorKind::Msg("Invalid path for virtiommioblkdevice".to_string()).into());
    }

    update_spec_device_list(device, spec)
}

// device.Id should be the PCI address in the format  "bridgeAddr/deviceAddr".
// Here, bridgeAddr is the address at which the brige is attached on the root bus,
// while deviceAddr is the address at which the device is attached on the bridge.
fn virtio_blk_device_handler(
    device: &Device,
    spec: &mut Spec,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<()> {
    let dev_path = get_pci_device_name(sandbox, device.id.as_str())?;

    let mut dev = device.clone();
    dev.vm_path = dev_path;

    update_spec_device_list(&dev, spec)
}

// device.Id should be the SCSI address of the disk in the format "scsiID:lunID"
fn virtio_scsi_device_handler(
    device: &Device,
    spec: &mut Spec,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<()> {
    let dev_path = get_scsi_device_name(sandbox, device.id.as_str())?;

    let mut dev = device.clone();
    dev.vm_path = dev_path;

    update_spec_device_list(&dev, spec)
}

fn virtio_nvdimm_device_handler(
    device: &Device,
    spec: &mut Spec,
    _sandbox: Arc<Mutex<Sandbox>>,
) -> Result<()> {
    update_spec_device_list(device, spec)
}

pub fn add_devices(
    devices: Vec<Device>,
    spec: &mut Spec,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<()> {
    for device in devices.iter() {
        add_device(device, spec, sandbox.clone())?;
    }

    Ok(())
}

fn add_device(device: &Device, spec: &mut Spec, sandbox: Arc<Mutex<Sandbox>>) -> Result<()> {
    // log before validation to help with debugging gRPC protocol
    // version differences.
    info!(sl!(), "device-id: {}, device-type: {}, device-vm-path: {}, device-container-path: {}, device-options: {:?}",
          device.id, device.field_type, device.vm_path, device.container_path, device.options);

    if device.field_type == "" {
        return Err(ErrorKind::Msg(format!("invalid type for device {:?}", device)).into());
    }

    if device.id == "" && device.vm_path == "" {
        return Err(
            ErrorKind::Msg(format!("invalid ID and VM path for device {:?}", device)).into(),
        );
    }

    if device.container_path == "" {
        return Err(
            ErrorKind::Msg(format!("invalid container path for device {:?}", device)).into(),
        );
    }

    let dev_handler = match DEVICEHANDLERLIST.get(device.field_type.as_str()) {
        None => {
            return Err(ErrorKind::Msg(format!("Unknown device type {}", device.field_type)).into())
        }
        Some(t) => t,
    };

    dev_handler(device, spec, sandbox)
}
