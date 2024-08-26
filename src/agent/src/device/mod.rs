// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use self::block_device_handler::{VirtioBlkMmioDeviceHandler, VirtioBlkPciDeviceHandler};
use self::nvdimm_device_handler::VirtioNvdimmDeviceHandler;
use self::scsi_device_handler::ScsiDeviceHandler;
use self::vfio_device_handler::{VfioApDeviceHandler, VfioPciDeviceHandler};
use crate::pci;
use crate::sandbox::Sandbox;
use anyhow::{anyhow, Context, Result};
use kata_types::device::DeviceHandlerManager;
use nix::sys::stat;
use oci::{LinuxDeviceCgroup, Spec};
use oci_spec::runtime as oci;
use protocols::agent::Device;
use slog::Logger;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::prelude::FileTypeExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::instrument;

pub mod block_device_handler;
pub mod network_device_handler;
pub mod nvdimm_device_handler;
pub mod scsi_device_handler;
pub mod vfio_device_handler;

pub const BLOCK: &str = "block";

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    // Device type, "b" for block device and "c" for character device
    cgroup_type: String,
    // The major and minor numbers for the device within the guest
    guest_major: i64,
    guest_minor: i64,
}

impl DeviceInfo {
    /// Create a device info.
    ///
    /// # Arguments
    ///
    /// * `vm_path` - Device's vm path.
    /// * `is_rdev` - If the vm_path is a device, set to true. If the
    ///   vm_path is a file in a device, set to false.
    pub fn new(vm_path: &str, is_rdev: bool) -> Result<Self> {
        let cgroup_type;
        let devid;

        let vm_path = PathBuf::from(vm_path);
        if !vm_path.exists() {
            return Err(anyhow!("VM device path {:?} doesn't exist", vm_path));
        }

        let metadata = fs::metadata(&vm_path)?;

        if is_rdev {
            devid = metadata.rdev();
            let file_type = metadata.file_type();
            if file_type.is_block_device() {
                cgroup_type = String::from("b");
            } else if file_type.is_char_device() {
                cgroup_type = String::from("c");
            } else {
                return Err(anyhow!("Unknown device {:?}'s cgroup type", vm_path));
            }
        } else {
            devid = metadata.dev();
            cgroup_type = String::from("b");
        }

        let guest_major = stat::major(devid) as i64;
        let guest_minor = stat::minor(devid) as i64;

        Ok(DeviceInfo {
            cgroup_type,
            guest_major,
            guest_minor,
        })
    }
}

// Represents the device-node and resource related updates to the OCI
// spec needed for a particular device
#[derive(Debug, Clone)]
struct DevUpdate {
    info: DeviceInfo,
    // an optional new path to update the device to in the "inner" container
    // specification
    final_path: Option<String>,
}

impl DevUpdate {
    fn new(vm_path: &str, final_path: &str) -> Result<Self> {
        Ok(DevUpdate {
            final_path: Some(final_path.to_owned()),
            ..DeviceInfo::new(vm_path, true)?.into()
        })
    }
}

impl From<DeviceInfo> for DevUpdate {
    fn from(info: DeviceInfo) -> Self {
        DevUpdate {
            info,
            final_path: None,
        }
    }
}

// Represents the updates to the OCI spec needed for a particular device
#[derive(Debug, Clone, Default)]
pub struct SpecUpdate {
    dev: Option<DevUpdate>,
    // optional corrections for PCI addresses
    pci: Vec<(pci::Address, pci::Address)>,
}

impl<T: Into<DevUpdate>> From<T> for SpecUpdate {
    fn from(dev: T) -> Self {
        SpecUpdate {
            dev: Some(dev.into()),
            pci: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct DeviceContext<'a> {
    logger: &'a Logger,
    sandbox: &'a Arc<Mutex<Sandbox>>,
}

/// Trait object to handle device.
#[async_trait::async_trait]
pub trait DeviceHandler: Send + Sync {
    /// Handle the device
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate>;

    /// Return the driver types that the handler manages.
    fn driver_types(&self) -> &[&str];
}

#[rustfmt::skip]
lazy_static! {
    pub static ref DEVICE_HANDLERS: DeviceHandlerManager<Arc<dyn DeviceHandler>> = {
        let mut manager: DeviceHandlerManager<Arc<dyn DeviceHandler>> = DeviceHandlerManager::new();

        let handlers: Vec<Arc<dyn DeviceHandler>> = vec![
            Arc::new(VirtioBlkMmioDeviceHandler {}),
            Arc::new(VirtioBlkPciDeviceHandler {}),
            Arc::new(VirtioNvdimmDeviceHandler {}),
            Arc::new(ScsiDeviceHandler {}),
            Arc::new(VfioPciDeviceHandler {}),
            Arc::new(VfioApDeviceHandler {}),
            #[cfg(target_arch = "s390x")]
            Arc::new(self::block_device_handler::VirtioBlkCcwDeviceHandler {}),
        ];

        for handler in handlers {
            manager.add_handler(handler.driver_types(), handler.clone()).unwrap();
        }
        manager
    };
}

#[instrument]
pub async fn add_devices(
    logger: &Logger,
    devices: &[Device],
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    let mut dev_updates = HashMap::<&str, DevUpdate>::with_capacity(devices.len());

    for device in devices.iter() {
        validate_device(logger, device, sandbox).await?;
        if let Some(handler) = DEVICE_HANDLERS.handler(&device.type_) {
            let mut ctx = DeviceContext { logger, sandbox };

            match handler.device_handler(device, &mut ctx).await {
                Ok(update) => {
                    if let Some(dev_update) = update.dev {
                        if dev_updates
                            .insert(&device.container_path, dev_update.clone())
                            .is_some()
                        {
                            return Err(anyhow!(
                                "Conflicting device updates for {}",
                                &device.container_path
                            ));
                        }

                        // Update cgroup to allow all devices added to guest.
                        insert_devices_cgroup_rule(logger, spec, &dev_update.info, true, "rwm")
                            .context("Update device cgroup")?;
                    }

                    let mut sb = sandbox.lock().await;
                    for (host, guest) in update.pci {
                        if let Some(other_guest) = sb.pcimap.insert(host, guest) {
                            return Err(anyhow!(
                                "Conflicting guest address for host device {} ({} versus {})",
                                host,
                                guest,
                                other_guest
                            ));
                        }
                    }
                }
                Err(e) => {
                    error!(logger, "failed to add devices, error: {e:?}");
                    return Err(e);
                }
            }
        } else {
            return Err(anyhow!(
                "Failed to find the device handler {}",
                device.type_
            ));
        }
    }

    if let Some(process) = spec.process_mut() {
        let env_vec: &mut Vec<String> =
            &mut process.env_mut().get_or_insert_with(Vec::new).to_vec();
        update_env_pci(env_vec, &sandbox.lock().await.pcimap)?
    }
    update_spec_devices(logger, spec, dev_updates)
}

#[instrument]
async fn validate_device(
    logger: &Logger,
    device: &Device,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    // log before validation to help with debugging gRPC protocol version differences.
    info!(
        logger,
        "device-id: {}, device-type: {}, device-vm-path: {}, device-container-path: {}, device-options: {:?}",
        device.id, device.type_, device.vm_path, device.container_path, device.options
    );

    if device.type_.is_empty() {
        return Err(anyhow!("invalid type for device {:?}", device));
    }

    if device.id.is_empty() && device.vm_path.is_empty() {
        return Err(anyhow!("invalid ID and VM path for device {:?}", device));
    }

    if device.container_path.is_empty() {
        return Err(anyhow!("invalid container path for device {:?}", device));
    }
    return Ok(());
}

// Insert a devices cgroup rule to control access to device.
#[instrument]
pub fn insert_devices_cgroup_rule(
    logger: &Logger,
    spec: &mut Spec,
    dev_info: &DeviceInfo,
    allow: bool,
    access: &str,
) -> Result<()> {
    let linux = spec
        .linux_mut()
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't container linux field"))?;
    let devcgrp_type = dev_info
        .cgroup_type
        .parse::<oci::LinuxDeviceType>()
        .context(format!(
            "Failed to parse {:?} to Enum LinuxDeviceType",
            dev_info.cgroup_type
        ))?;
    let linux_resource = &mut oci::LinuxResources::default();
    let resource = linux.resources_mut().as_mut().unwrap_or(linux_resource);
    let mut device_cgrp = LinuxDeviceCgroup::default();
    device_cgrp.set_allow(allow);
    device_cgrp.set_major(Some(dev_info.guest_major));
    device_cgrp.set_minor(Some(dev_info.guest_minor));
    device_cgrp.set_typ(Some(devcgrp_type));
    device_cgrp.set_access(Some(access.to_owned()));

    debug!(
        logger,
        "Insert a devices cgroup rule";
        "linux_device_cgroup" => device_cgrp.allow(),
        "guest_major" => device_cgrp.major(),
        "guest_minor" => device_cgrp.minor(),
        "type" => device_cgrp.typ().unwrap().as_str(),
        "access" => device_cgrp.access().as_ref().unwrap().as_str(),
    );

    if let Some(devices) = resource.devices_mut() {
        devices.push(device_cgrp);
    } else {
        resource.set_devices(Some(vec![device_cgrp]));
    }

    Ok(())
}

// update_env_pci alters PCI addresses in a set of environment
// variables to be correct for the VM instead of the host.  It is
// given a map of (host address => guest address)
#[instrument]
pub fn update_env_pci(
    env: &mut [String],
    pcimap: &HashMap<pci::Address, pci::Address>,
) -> Result<()> {
    // SR-IOV device plugin may add two environment variables for one resource:
    // - PCIDEVICE_<prefix>_<resource-name>: a list of PCI device ids separated by comma
    // - PCIDEVICE_<prefix>_<resource-name>_INFO: detailed info in JSON for above PCI devices
    // Both environment variables hold information about the same set of PCI devices.
    // Below code updates both of them in two passes:
    // - 1st pass updates PCIDEVICE_<prefix>_<resource-name> and collects host to guest PCI address mapping
    let mut pci_dev_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    for envvar in env.iter_mut() {
        let eqpos = envvar
            .find('=')
            .ok_or_else(|| anyhow!("Malformed OCI env entry {:?}", envvar))?;

        let (name, eqval) = envvar.split_at(eqpos);
        let val = &eqval[1..];

        if !name.starts_with("PCIDEVICE_") || name.ends_with("_INFO") {
            continue;
        }

        let mut addr_map: HashMap<String, String> = HashMap::new();
        let mut guest_addrs = Vec::<String>::new();
        for host_addr_str in val.split(',') {
            let host_addr = pci::Address::from_str(host_addr_str)
                .with_context(|| format!("Can't parse {} environment variable", name))?;
            let guest_addr = pcimap
                .get(&host_addr)
                .ok_or_else(|| anyhow!("Unable to translate host PCI address {}", host_addr))?;

            guest_addrs.push(format!("{}", guest_addr));
            addr_map.insert(host_addr_str.to_string(), format!("{}", guest_addr));
        }

        pci_dev_map.insert(format!("{}_INFO", name), addr_map);

        envvar.replace_range(eqpos + 1.., guest_addrs.join(",").as_str());
    }

    // - 2nd pass update PCIDEVICE_<prefix>_<resource-name>_INFO if it exists
    for envvar in env.iter_mut() {
        let eqpos = envvar
            .find('=')
            .ok_or_else(|| anyhow!("Malformed OCI env entry {:?}", envvar))?;

        let (name, _) = envvar.split_at(eqpos);
        if !(name.starts_with("PCIDEVICE_") && name.ends_with("_INFO")) {
            continue;
        }

        if let Some(addr_map) = pci_dev_map.get(name) {
            for (host_addr, guest_addr) in addr_map {
                *envvar = envvar.replace(host_addr, guest_addr);
            }
        }
    }

    Ok(())
}

// update_spec_devices updates the device list in the OCI spec to make
// it include details appropriate for the VM, instead of the host.  It
// is given a map of (container_path => update) where:
//     container_path: the path to the device in the original OCI spec
//     update: information on changes to make to the device
#[instrument]
fn update_spec_devices(
    logger: &Logger,
    spec: &mut Spec,
    mut updates: HashMap<&str, DevUpdate>,
) -> Result<()> {
    let linux = spec
        .linux_mut()
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't contain linux field"))?;
    let mut res_updates = HashMap::<(String, i64, i64), DeviceInfo>::with_capacity(updates.len());

    let mut default_devices = Vec::new();
    let linux_devices = linux.devices_mut().as_mut().unwrap_or(&mut default_devices);
    for specdev in linux_devices.iter_mut() {
        let devtype = specdev.typ().as_str().to_string();
        if let Some(update) = updates.remove(specdev.path().clone().display().to_string().as_str())
        {
            let host_major = specdev.major();
            let host_minor = specdev.minor();

            info!(
                logger,
                "update_spec_devices() updating device";
                "container_path" => &specdev.path().display().to_string(),
                "type" => &devtype,
                "host_major" => host_major,
                "host_minor" => host_minor,
                "guest_major" => update.info.guest_major,
                "guest_minor" => update.info.guest_minor,
                "final_path" => update.final_path.as_ref(),
            );

            specdev.set_major(update.info.guest_major);
            specdev.set_minor(update.info.guest_minor);
            if let Some(final_path) = update.final_path {
                specdev.set_path(PathBuf::from(&final_path));
            }

            if res_updates
                .insert((devtype, host_major, host_minor), update.info)
                .is_some()
            {
                return Err(anyhow!(
                    "Conflicting resource updates for host_major={} host_minor={}",
                    host_major,
                    host_minor
                ));
            }
        }
    }

    // Make sure we applied all of our updates
    if !updates.is_empty() {
        return Err(anyhow!(
            "Missing devices in OCI spec: {:?}",
            updates
                .keys()
                .map(|d| format!("{:?}", d))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }

    if let Some(resources) = linux.resources_mut().as_mut() {
        if let Some(resources_devices) = resources.devices_mut().as_mut() {
            for d in resources_devices.iter_mut() {
                let dev_type = d.typ().unwrap_or_default().as_str().to_string();
                if let (Some(host_major), Some(host_minor)) = (d.major(), d.minor()) {
                    if let Some(update) =
                        res_updates.get(&(dev_type.clone(), host_major, host_minor))
                    {
                        info!(
                            logger,
                            "update_spec_devices() updating resource";
                            "type" => &dev_type,
                            "host_major" => host_major,
                            "host_minor" => host_minor,
                            "guest_major" => update.guest_major,
                            "guest_minor" => update.guest_minor,
                        );

                        d.set_major(Some(update.guest_major));
                        d.set_minor(Some(update.guest_minor));
                    }
                }
            }
        }
    }

    Ok(())
}

// pcipath_to_sysfs fetches the sysfs path for a PCI path, relative to
// the sysfs path for the PCI host bridge, based on the PCI path
// provided.
#[instrument]
pub fn pcipath_to_sysfs(root_bus_sysfs: &str, pcipath: &pci::Path) -> Result<String> {
    let mut bus = "0000:00".to_string();
    let mut relpath = String::new();

    for i in 0..pcipath.len() {
        let bdf = format!("{}:{}", bus, pcipath[i]);

        relpath = format!("{}/{}", relpath, bdf);

        if i == pcipath.len() - 1 {
            // Final device need not be a bridge
            break;
        }

        // Find out the bus exposed by bridge
        let bridgebuspath = format!("{}{}/pci_bus", root_bus_sysfs, relpath);
        let mut files: Vec<_> = fs::read_dir(&bridgebuspath)?.collect();

        match files.pop() {
            Some(busfile) if files.is_empty() => {
                bus = busfile?
                    .file_name()
                    .into_string()
                    .map_err(|e| anyhow!("Bad filename under {}: {:?}", &bridgebuspath, e))?;
            }
            _ => {
                return Err(anyhow!(
                    "Expected exactly one PCI bus in {}, got {} instead",
                    bridgebuspath,
                    // Adjust to original value as we've already popped
                    files.len() + 1
                ));
            }
        };
    }

    Ok(relpath)
}

#[instrument]
pub fn online_device(path: &str) -> Result<()> {
    fs::write(path, "1")?;
    Ok(())
}
