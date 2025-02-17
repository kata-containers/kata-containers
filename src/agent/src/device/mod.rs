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
use cdi::annotations::parse_annotations;
use cdi::cache::{new_cache, with_auto_refresh, CdiOption};
use cdi::spec_dirs::with_spec_dirs;
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
use tokio::time;
use tokio::time::Duration;
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
pub async fn handle_cdi_devices(
    logger: &Logger,
    spec: &mut Spec,
    spec_dir: &str,
    cdi_timeout: time::Duration,
) -> Result<()> {
    if let Some(container_type) = spec
        .annotations()
        .as_ref()
        .and_then(|a| a.get("io.katacontainers.pkg.oci.container_type"))
    {
        if container_type == "pod_sandbox" {
            return Ok(());
        }
    }

    let (_, devices) = parse_annotations(spec.annotations().as_ref().unwrap())?;

    if devices.is_empty() {
        info!(logger, "no CDI annotations, no devices to inject");
        return Ok(());
    }
    // Explicitly set the cache options to disable auto-refresh and
    // to use the single spec dir "/var/run/cdi" for tests it can be overridden
    let options: Vec<CdiOption> = vec![with_auto_refresh(false), with_spec_dirs(&[spec_dir])];
    let cache: Arc<std::sync::Mutex<cdi::cache::Cache>> = new_cache(options);

    for i in 0..=cdi_timeout.as_secs() {
        let inject_result = {
            // Lock cache within this scope, std::sync::Mutex has no Send
            // and await will not work with time::sleep
            let mut cache = cache.lock().unwrap();
            match cache.refresh() {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!("error refreshing cache: {:?}", e));
                }
            }
            cache.inject_devices(Some(spec), devices.clone())
        };

        match inject_result {
            Ok(_) => {
                info!(
                    logger,
                    "all devices injected successfully, modified CDI container spec: {:?}", &spec
                );
                return Ok(());
            }
            Err(e) => {
                info!(
                    logger,
                    "waiting for CDI spec(s) to be generated ({} of {} max tries) {:?}",
                    i,
                    cdi_timeout.as_secs(),
                    e
                );
            }
        }
        time::sleep(Duration::from_secs(1)).await;
    }
    Err(anyhow!(
        "failed to inject devices after CDI timeout of {} seconds",
        cdi_timeout.as_secs()
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux_abi::create_pci_root_bus_path;
    use crate::uevent::{spawn_test_watcher, wait_for_uevent};
    use oci::{
        Linux, LinuxBuilder, LinuxDeviceBuilder, LinuxDeviceCgroupBuilder, LinuxDeviceType,
        LinuxResources, LinuxResourcesBuilder, SpecBuilder,
    };
    use oci_spec::runtime as oci;
    use std::iter::FromIterator;
    use tempfile::tempdir;

    const VM_ROOTFS: &str = "/";

    #[test]
    fn test_update_device_cgroup() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut linux = Linux::default();
        linux.set_resources(Some(LinuxResources::default()));
        let mut spec = SpecBuilder::default().linux(linux).build().unwrap();

        let dev_info = DeviceInfo::new(VM_ROOTFS, false).unwrap();
        insert_devices_cgroup_rule(&logger, &mut spec, &dev_info, false, "rw").unwrap();

        let devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(devices.len(), 1);

        let meta = fs::metadata(VM_ROOTFS).unwrap();
        let rdev = meta.dev();
        let major = stat::major(rdev) as i64;
        let minor = stat::minor(rdev) as i64;

        assert_eq!(devices[0].major(), Some(major));
        assert_eq!(devices[0].minor(), Some(minor));
    }

    #[test]
    fn test_update_spec_devices() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let (major, minor) = (7, 2);
        let mut spec = Spec::default();

        // vm_path empty
        let update = DeviceInfo::new("", true);
        assert!(update.is_err());

        // linux is empty
        let container_path = "/dev/null";
        let vm_path = "/dev/null";
        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_err());

        spec.set_linux(Some(Linux::default()));

        // linux.devices doesn't contain the updated device
        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_err());

        spec.linux_mut()
            .as_mut()
            .unwrap()
            .set_devices(Some(vec![LinuxDeviceBuilder::default()
                .path(PathBuf::from("/dev/null2"))
                .major(major)
                .minor(minor)
                .build()
                .unwrap()]));

        // guest and host path are not the same
        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(
            res.is_err(),
            "container_path={:?} vm_path={:?} spec={:?}",
            container_path,
            vm_path,
            spec
        );

        spec.linux_mut()
            .as_mut()
            .unwrap()
            .devices_mut()
            .as_mut()
            .unwrap()[0]
            .set_path(PathBuf::from(container_path));

        // spec.linux.resources is empty
        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_ok());

        // update both devices and cgroup lists
        spec.linux_mut()
            .as_mut()
            .unwrap()
            .set_devices(Some(vec![LinuxDeviceBuilder::default()
                .path(PathBuf::from(container_path))
                .major(major)
                .minor(minor)
                .build()
                .unwrap()]));

        spec.linux_mut().as_mut().unwrap().set_resources(Some(
            oci::LinuxResourcesBuilder::default()
                .devices(vec![LinuxDeviceCgroupBuilder::default()
                    .major(major)
                    .minor(minor)
                    .build()
                    .unwrap()])
                .build()
                .unwrap(),
        ));

        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_update_spec_devices_guest_host_conflict() {
        let logger = slog::Logger::root(slog::Discard, o!());

        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();
        let zero_rdev = fs::metadata("/dev/zero").unwrap().rdev();
        let full_rdev = fs::metadata("/dev/full").unwrap().rdev();

        let host_major_a = stat::major(null_rdev) as i64;
        let host_minor_a = stat::minor(null_rdev) as i64;
        let host_major_b = stat::major(zero_rdev) as i64;
        let host_minor_b = stat::minor(zero_rdev) as i64;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/a"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major_a)
                            .minor(host_minor_a)
                            .build()
                            .unwrap(),
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/b"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major_b)
                            .minor(host_minor_b)
                            .build()
                            .unwrap(),
                    ])
                    .resources(
                        LinuxResourcesBuilder::default()
                            .devices(vec![
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major_a)
                                    .minor(host_minor_a)
                                    .build()
                                    .unwrap(),
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major_b)
                                    .minor(host_minor_b)
                                    .build()
                                    .unwrap(),
                            ])
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let container_path_a = "/dev/a";
        let vm_path_a = "/dev/zero";

        let guest_major_a = stat::major(zero_rdev) as i64;
        let guest_minor_a = stat::minor(zero_rdev) as i64;

        let container_path_b = "/dev/b";
        let vm_path_b = "/dev/full";

        let guest_major_b = stat::major(full_rdev) as i64;
        let guest_minor_b = stat::minor(full_rdev) as i64;

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(host_major_a, specdevices[0].major());
        assert_eq!(host_minor_a, specdevices[0].minor());
        assert_eq!(host_major_b, specdevices[1].major());
        assert_eq!(host_minor_b, specdevices[1].minor());

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(host_major_a), specresources_devices[0].major());
        assert_eq!(Some(host_minor_a), specresources_devices[0].minor());
        assert_eq!(Some(host_major_b), specresources_devices[1].major());
        assert_eq!(Some(host_minor_b), specresources_devices[1].minor());

        let updates = HashMap::from_iter(vec![
            (
                container_path_a,
                DeviceInfo::new(vm_path_a, true).unwrap().into(),
            ),
            (
                container_path_b,
                DeviceInfo::new(vm_path_b, true).unwrap().into(),
            ),
        ]);
        let res = update_spec_devices(&logger, &mut spec, updates);
        assert!(res.is_ok());

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(guest_major_a, specdevices[0].major());
        assert_eq!(guest_minor_a, specdevices[0].minor());
        assert_eq!(guest_major_b, specdevices[1].major());
        assert_eq!(guest_minor_b, specdevices[1].minor());

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(guest_major_a), specresources_devices[0].major());
        assert_eq!(Some(guest_minor_a), specresources_devices[0].minor());
        assert_eq!(Some(guest_major_b), specresources_devices[1].major());
        assert_eq!(Some(guest_minor_b), specresources_devices[1].minor());
    }

    #[test]
    fn test_update_spec_devices_char_block_conflict() {
        let logger = slog::Logger::root(slog::Discard, o!());

        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();

        let guest_major = stat::major(null_rdev) as i64;
        let guest_minor = stat::minor(null_rdev) as i64;
        let host_major: i64 = 99;
        let host_minor: i64 = 99;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/char"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major)
                            .minor(host_minor)
                            .build()
                            .unwrap(),
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/block"))
                            .typ(LinuxDeviceType::B)
                            .major(host_major)
                            .minor(host_minor)
                            .build()
                            .unwrap(),
                    ])
                    .resources(
                        LinuxResourcesBuilder::default()
                            .devices(vec![
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major)
                                    .minor(host_minor)
                                    .build()
                                    .unwrap(),
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::B)
                                    .major(host_major)
                                    .minor(host_minor)
                                    .build()
                                    .unwrap(),
                            ])
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let container_path = "/dev/char";
        let vm_path = "/dev/null";

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(host_major), specresources_devices[0].major());
        assert_eq!(Some(host_minor), specresources_devices[0].minor());
        assert_eq!(Some(host_major), specresources_devices[1].major());
        assert_eq!(Some(host_minor), specresources_devices[1].minor());

        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_ok());

        // Only the char device, not the block device should be updated
        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(guest_major), specresources_devices[0].major());
        assert_eq!(Some(guest_minor), specresources_devices[0].minor());
        assert_eq!(Some(host_major), specresources_devices[1].major());
        assert_eq!(Some(host_minor), specresources_devices[1].minor());
    }

    #[test]
    fn test_update_spec_devices_final_path() {
        let logger = slog::Logger::root(slog::Discard, o!());

        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();
        let guest_major = stat::major(null_rdev) as i64;
        let guest_minor = stat::minor(null_rdev) as i64;

        let container_path = "/dev/original";
        let host_major: i64 = 99;
        let host_minor: i64 = 99;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![LinuxDeviceBuilder::default()
                        .path(PathBuf::from(container_path))
                        .typ(LinuxDeviceType::C)
                        .major(host_major)
                        .minor(host_minor)
                        .build()
                        .unwrap()])
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let vm_path = "/dev/null";
        let final_path = "/dev/new";

        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DevUpdate::new(vm_path, final_path).unwrap(),
            )]),
        );
        assert!(res.is_ok());

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(guest_major, specdevices[0].major());
        assert_eq!(guest_minor, specdevices[0].minor());
        assert_eq!(&PathBuf::from(final_path), specdevices[0].path());
    }

    #[test]
    fn test_update_env_pci() {
        let example_map = [
            // Each is a host,guest pair of pci addresses
            ("0000:1a:01.0", "0000:01:01.0"),
            ("0000:1b:02.0", "0000:01:02.0"),
            // This one has the same host address as guest address
            // above, to test that we're not double-translating
            ("0000:01:01.0", "ffff:02:1f.7"),
        ];

        let pci_dev_info_original = r#"PCIDEVICE_x_INFO={"0000:1a:01.0":{"generic":{"deviceID":"0000:1a:01.0"}},"0000:1b:02.0":{"generic":{"deviceID":"0000:1b:02.0"}}}"#;
        let pci_dev_info_expected = r#"PCIDEVICE_x_INFO={"0000:01:01.0":{"generic":{"deviceID":"0000:01:01.0"}},"0000:01:02.0":{"generic":{"deviceID":"0000:01:02.0"}}}"#;
        let mut env = vec![
            "PCIDEVICE_x=0000:1a:01.0,0000:1b:02.0".to_string(),
            pci_dev_info_original.to_string(),
            "PCIDEVICE_y=0000:01:01.0".to_string(),
            "NOTAPCIDEVICE_blah=abcd:ef:01.0".to_string(),
        ];

        let pci_fixups = example_map
            .iter()
            .map(|(h, g)| {
                (
                    pci::Address::from_str(h).unwrap(),
                    pci::Address::from_str(g).unwrap(),
                )
            })
            .collect();

        let res = update_env_pci(&mut env, &pci_fixups);
        assert!(res.is_ok(), "error: {}", res.err().unwrap());

        assert_eq!(env[0], "PCIDEVICE_x=0000:01:01.0,0000:01:02.0");
        assert_eq!(env[1], pci_dev_info_expected);
        assert_eq!(env[2], "PCIDEVICE_y=ffff:02:1f.7");
        assert_eq!(env[3], "NOTAPCIDEVICE_blah=abcd:ef:01.0");
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

    // We use device specific variants of this for real cases, but
    // they have some complications that make them troublesome to unit
    // test
    async fn example_get_device_name(
        sandbox: &Arc<Mutex<Sandbox>>,
        relpath: &str,
    ) -> Result<String> {
        let matcher = crate::device::block_device_handler::VirtioBlkPciMatcher::new(relpath);

        let uev = wait_for_uevent(sandbox, matcher).await?;

        Ok(uev.devname)
    }

    #[tokio::test]
    async fn test_get_device_name() {
        let devname = "vda";
        let root_bus = create_pci_root_bus_path();
        let relpath = "/0000:00:0a.0/0000:03:0b.0";
        let devpath = format!("{}{}/virtio4/block/{}", root_bus, relpath, devname);

        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = BLOCK.to_string();
        uev.devpath = devpath.clone();
        uev.devname = devname.to_string();

        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Arc::new(Mutex::new(Sandbox::new(&logger).unwrap()));

        let mut sb = sandbox.lock().await;
        sb.uevent_map.insert(devpath.clone(), uev);
        drop(sb); // unlock

        let name = example_get_device_name(&sandbox, relpath).await;
        assert!(name.is_ok(), "{}", name.unwrap_err());
        assert_eq!(name.unwrap(), devname);

        let mut sb = sandbox.lock().await;
        let uev = sb.uevent_map.remove(&devpath).unwrap();
        drop(sb); // unlock

        spawn_test_watcher(sandbox.clone(), uev);

        let name = example_get_device_name(&sandbox, relpath).await;
        assert!(name.is_ok(), "{}", name.unwrap_err());
        assert_eq!(name.unwrap(), devname);
    }

    #[tokio::test]
    async fn test_handle_cdi_devices() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut spec = Spec::default();

        let mut annotations = HashMap::new();
        // cdi.k8s.io/vendor1_devices: vendor1.com/device=foo
        annotations.insert(
            "cdi.k8s.io/vfio17".to_string(),
            "kata.com/gpu=0".to_string(),
        );
        spec.set_annotations(Some(annotations));

        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let cdi_file = temp_dir.path().join("kata.json");

        let cdi_version = "0.6.0";
        let kind = "kata.com/gpu";
        let device_name = "0";
        let annotation_whatever = "false";
        let annotation_whenever = "true";
        let inner_env = "TEST_INNER_ENV=TEST_INNER_ENV_VALUE";
        let outer_env = "TEST_OUTER_ENV=TEST_OUTER_ENV_VALUE";
        let inner_device = "/dev/zero";
        let outer_device = "/dev/null";

        let cdi_content = format!(
            r#"{{
            "cdiVersion": "{cdi_version}",
            "kind": "{kind}",
            "devices": [
                {{
                    "name": "{device_name}",
                    "annotations": {{
                        "whatever": "{annotation_whatever}",
                        "whenever": "{annotation_whenever}"
                    }},
                    "containerEdits": {{
                        "env": [
                            "{inner_env}"
                        ],
                        "deviceNodes": [
                            {{
                                "path": "{inner_device}"
                            }}
                        ]
                    }}
                }}
            ],
            "containerEdits": {{
                "env": [
                    "{outer_env}"
                ],
                "deviceNodes": [
                    {{
                        "path": "{outer_device}"
                    }}
                ]
            }}
        }}"#
        );

        fs::write(&cdi_file, cdi_content).expect("Failed to write CDI file");

        let cdi_timeout = Duration::from_secs(0);

        let res = handle_cdi_devices(
            &logger,
            &mut spec,
            temp_dir.path().to_str().unwrap(),
            cdi_timeout,
        )
        .await;
        println!("modfied spec {:?}", spec);
        assert!(res.is_ok(), "{}", res.err().unwrap());

        let linux = spec.linux().as_ref().unwrap();
        let devices = linux
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .as_ref()
            .unwrap();
        assert_eq!(devices.len(), 2);

        let env = spec.process().as_ref().unwrap().env().as_ref().unwrap();

        // find string TEST_OUTER_ENV in env
        let outer_env = env.iter().find(|e| e.starts_with("TEST_OUTER_ENV"));
        assert!(outer_env.is_some(), "TEST_OUTER_ENV not found in env");

        // find TEST_INNER_ENV in env
        let inner_env = env.iter().find(|e| e.starts_with("TEST_INNER_ENV"));
        assert!(inner_env.is_some(), "TEST_INNER_ENV not found in env");
    }
}
