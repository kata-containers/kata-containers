// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "s390x")]
use crate::ap;
use crate::device::{pcipath_to_sysfs, DevUpdate, DeviceContext, DeviceHandler, SpecUpdate};
use crate::linux_abi::*;
use crate::pci;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use anyhow::{anyhow, Context, Result};
use kata_types::device::{DRIVER_VFIO_AP_TYPE, DRIVER_VFIO_PCI_GK_TYPE, DRIVER_VFIO_PCI_TYPE};
use protocols::agent::Device;
use slog::Logger;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::instrument;

#[derive(Debug)]
pub struct VfioPciDeviceHandler {}

#[derive(Debug)]
pub struct VfioApDeviceHandler {}

#[async_trait::async_trait]
impl DeviceHandler for VfioPciDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_VFIO_PCI_GK_TYPE, DRIVER_VFIO_PCI_TYPE]
    }

    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        let vfio_in_guest = device.type_ != DRIVER_VFIO_PCI_GK_TYPE;
        let mut pci_fixups = Vec::<(pci::Address, pci::Address)>::new();
        let mut group = None;

        for opt in device.options.iter() {
            let (host, pcipath) = split_vfio_pci_option(opt)
                .ok_or_else(|| anyhow!("Malformed VFIO PCI option {:?}", opt))?;
            let host =
                pci::Address::from_str(host).context("Bad host PCI address in VFIO option {:?}")?;
            let pcipath = pci::Path::from_str(pcipath)?;

            let guestdev = wait_for_pci_device(ctx.sandbox, &pcipath).await?;
            if vfio_in_guest {
                pci_driver_override(ctx.logger, SYSFS_BUS_PCI_PATH, guestdev, "vfio-pci")?;

                // Devices must have an IOMMU group to be usable via VFIO
                let devgroup = pci_iommu_group(SYSFS_BUS_PCI_PATH, guestdev)?
                    .ok_or_else(|| anyhow!("{} has no IOMMU group", guestdev))?;

                if let Some(g) = group {
                    if g != devgroup {
                        return Err(anyhow!("{} is not in guest IOMMU group {}", guestdev, g));
                    }
                }

                group = Some(devgroup);
            }

            // collect PCI address mapping for both vfio-pci-gk and vfio-pci device
            pci_fixups.push((host, guestdev));
        }

        let dev_update = if vfio_in_guest {
            // If there are any devices at all, logic above ensures that group is not None
            let group = group.ok_or_else(|| anyhow!("failed to get VFIO group"))?;

            let vm_path = get_vfio_pci_device_name(group, ctx.sandbox).await?;

            Some(DevUpdate::new(&vm_path, &vm_path)?)
        } else {
            None
        };

        Ok(SpecUpdate {
            dev: dev_update,
            pci: pci_fixups,
        })
    }
}

#[async_trait::async_trait]
impl DeviceHandler for VfioApDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_VFIO_AP_TYPE]
    }

    #[cfg(target_arch = "s390x")]
    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        // Force AP bus rescan
        fs::write(AP_SCANS_PATH, "1")?;
        for apqn in device.options.iter() {
            wait_for_ap_device(ctx.sandbox, ap::Address::from_str(apqn)?).await?;
        }
        let dev_update = Some(DevUpdate::new(Z9_CRYPT_DEV_PATH, Z9_CRYPT_DEV_PATH)?);
        Ok(SpecUpdate {
            dev: dev_update,
            pci: Vec::new(),
        })
    }

    #[cfg(not(target_arch = "s390x"))]
    #[instrument]
    async fn device_handler(&self, _: &Device, _: &mut DeviceContext) -> Result<SpecUpdate> {
        Err(anyhow!("VFIO-AP is only supported on s390x"))
    }
}

async fn get_vfio_pci_device_name(
    grp: IommuGroup,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<String> {
    let matcher = VfioMatcher::new(grp);

    let uev = wait_for_uevent(sandbox, matcher).await?;
    Ok(format!("{}/{}", SYSTEM_DEV_PATH, &uev.devname))
}

#[derive(Debug)]
pub struct VfioMatcher {
    syspath: String,
}

impl VfioMatcher {
    pub fn new(grp: IommuGroup) -> VfioMatcher {
        VfioMatcher {
            syspath: format!("/devices/virtual/vfio/{}", grp),
        }
    }
}

impl UeventMatcher for VfioMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.devpath == self.syspath
    }
}

#[cfg(target_arch = "s390x")]
#[derive(Debug)]
pub struct ApMatcher {
    syspath: String,
}

#[cfg(target_arch = "s390x")]
impl ApMatcher {
    pub fn new(address: ap::Address) -> ApMatcher {
        ApMatcher {
            syspath: format!(
                "{}/card{:02x}/{}",
                AP_ROOT_BUS_PATH, address.adapter_id, address
            ),
        }
    }
}

#[cfg(target_arch = "s390x")]
impl UeventMatcher for ApMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.action == "add" && uev.devpath == self.syspath
    }
}

#[derive(Debug)]
pub struct PciMatcher {
    devpath: String,
}

impl PciMatcher {
    pub fn new(relpath: &str) -> Result<PciMatcher> {
        let root_bus = create_pci_root_bus_path();
        Ok(PciMatcher {
            devpath: format!("{}{}", root_bus, relpath),
        })
    }
}

impl UeventMatcher for PciMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.devpath == self.devpath
    }
}

#[cfg(target_arch = "s390x")]
#[instrument]
async fn wait_for_ap_device(sandbox: &Arc<Mutex<Sandbox>>, address: ap::Address) -> Result<()> {
    let matcher = ApMatcher::new(address);
    wait_for_uevent(sandbox, matcher).await?;
    Ok(())
}

pub async fn wait_for_pci_device(
    sandbox: &Arc<Mutex<Sandbox>>,
    pcipath: &pci::Path,
) -> Result<pci::Address> {
    let root_bus_sysfs = format!("{}{}", SYSFS_DIR, create_pci_root_bus_path());
    let sysfs_rel_path = pcipath_to_sysfs(&root_bus_sysfs, pcipath)?;
    let matcher = PciMatcher::new(&sysfs_rel_path)?;

    let uev = wait_for_uevent(sandbox, matcher).await?;

    let addr = uev
        .devpath
        .rsplit('/')
        .next()
        .ok_or_else(|| anyhow!("Bad device path {:?} in uevent", &uev.devpath))?;
    let addr = pci::Address::from_str(addr)?;
    Ok(addr)
}

// Represents an IOMMU group
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IommuGroup(u32);

impl fmt::Display for IommuGroup {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0)
    }
}

// Determine the IOMMU group of a PCI device
#[instrument]
fn pci_iommu_group<T>(syspci: T, dev: pci::Address) -> Result<Option<IommuGroup>>
where
    T: AsRef<OsStr> + std::fmt::Debug,
{
    let syspci = Path::new(&syspci);
    let grouppath = syspci
        .join("devices")
        .join(dev.to_string())
        .join("iommu_group");

    match fs::read_link(&grouppath) {
        // Device has no group
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!("Error reading link {:?}: {}", &grouppath, e)),
        Ok(group) => {
            if let Some(group) = group.file_name() {
                if let Some(group) = group.to_str() {
                    if let Ok(group) = group.parse::<u32>() {
                        return Ok(Some(IommuGroup(group)));
                    }
                }
            }
            Err(anyhow!(
                "Unexpected IOMMU group link {:?} => {:?}",
                grouppath,
                group
            ))
        }
    }
}

fn split_vfio_pci_option(opt: &str) -> Option<(&str, &str)> {
    let mut tokens = opt.split('=');
    let hostbdf = tokens.next()?;
    let path = tokens.next()?;
    if tokens.next().is_some() {
        None
    } else {
        Some((hostbdf, path))
    }
}

// Force a given PCI device to bind to the given driver, does
// basically the same thing as
//    driverctl set-override <PCI address> <driver>
#[instrument]
pub fn pci_driver_override<T, U>(
    logger: &Logger,
    syspci: T,
    dev: pci::Address,
    drv: U,
) -> Result<()>
where
    T: AsRef<OsStr> + std::fmt::Debug,
    U: AsRef<OsStr> + std::fmt::Debug,
{
    let syspci = Path::new(&syspci);
    let drv = drv.as_ref();
    info!(logger, "rebind_pci_driver: {} => {:?}", dev, drv);

    let devpath = syspci.join("devices").join(dev.to_string());
    let overridepath = &devpath.join("driver_override");

    fs::write(overridepath, drv.as_bytes())?;

    let drvpath = &devpath.join("driver");
    let need_unbind = match fs::read_link(drvpath) {
        Ok(d) if d.file_name() == Some(drv) => return Ok(()), // Nothing to do
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false, // No current driver
        Err(e) => return Err(anyhow!("Error checking driver on {}: {}", dev, e)),
        Ok(_) => true, // Current driver needs unbinding
    };
    if need_unbind {
        let unbindpath = &drvpath.join("unbind");
        fs::write(unbindpath, dev.to_string())?;
    }
    let probepath = syspci.join("drivers_probe");
    fs::write(probepath, dev.to_string())?;
    Ok(())
}
