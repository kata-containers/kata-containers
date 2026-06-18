// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::{pcipath_to_sysfs, DevUpdate, DeviceContext, DeviceHandler, SpecUpdate};
use crate::linux_abi::*;
use crate::pci;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use anyhow::{anyhow, Context, Result};
use cfg_if::cfg_if;
use kata_types::device::{
    DRIVER_VFIO_AP_COLD_TYPE, DRIVER_VFIO_AP_TYPE, DRIVER_VFIO_PCI_GK_TYPE, DRIVER_VFIO_PCI_TYPE,
};
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

cfg_if! {
    if #[cfg(target_arch = "s390x")] {
        use crate::ap;
        use crate::confidential_data_hub::get_cdh_resource;
        use std::convert::TryFrom;
        use pv_core::ap::{
            Apqn,
            apqn_info::Ep11,
            assoc_state::AssocState,
            bind_state::BindState,
        };
        use pv_core::misc::{encode_hex, pv_guest_bit_set};
        use pv_core::uv;
    }
}

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

            // An empty pcipath means the runtime could not resolve the guest PCI
            // path (e.g. GPU behind a pxb-pcie bridge in a NUMA topology where
            // the QOM walk fails).  Skip guest-device lookup and PCI fixup for
            // this device — the device is already present in the guest and does
            // not need driver override or address remapping.
            if pcipath.is_empty() {
                warn!(
                    ctx.logger,
                    "vfio device {:?} has empty guest PCI path, skipping guest-side setup", opt
                );
                continue;
            }

            let (root_complex, pcipath) = pcipath_from_dev_tree_path(pcipath)?;

            let guestdev =
                wait_for_pci_device(ctx.logger, ctx.sandbox, root_complex, &pcipath).await?;
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
        &[DRIVER_VFIO_AP_TYPE, DRIVER_VFIO_AP_COLD_TYPE]
    }

    #[cfg(target_arch = "s390x")]
    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        // Force AP bus rescan
        let mut ap_context = String::from("Failed to rescan AP bus");
        if pv_guest_bit_set() {
            ap_context.push_str(
                ". Verify your host kernel supports AP pass-through with Secure Execution",
            );
        }
        fs::write(AP_SCANS_PATH, "1").context(ap_context)?;

        for apqn in device.options.iter() {
            let ap_address = ap::Address::from_str(apqn).context("Failed to parse AP address")?;
            match device.type_.as_str() {
                DRIVER_VFIO_AP_TYPE => {
                    wait_for_ap_device(ctx.sandbox, ap_address).await?;
                }
                DRIVER_VFIO_AP_COLD_TYPE => {
                    check_ap_device(ap_address).await?;
                }
                _ => return Err(anyhow!("Unsupported AP device type: {}", device.type_)),
            }
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
            syspath: format!("/devices/virtual/vfio/{grp}"),
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
    pub fn new(relpath: &str, root_complex: &str) -> Result<PciMatcher> {
        let root_bus = create_pci_root_bus_path(root_complex);
        Ok(PciMatcher {
            devpath: format!("{root_bus}{relpath}"),
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

#[cfg(target_arch = "s390x")]
#[instrument]
async fn check_ap_device(address: ap::Address) -> Result<()> {
    let apqn = Apqn::try_from(&address.to_string() as &str)
        .context("Failed to establish AP at {address}")?;
    if apqn.info.is_none() {
        return Err(anyhow!("Failed to read info for AP {address}"));
    }
    if !pv_guest_bit_set() {
        return Ok(());
    }
    apqn.set_bind_state(BindState::Bound)
        .context(anyhow!("Failed to bind AP {address}"))?;
    if let Some(Ep11(ep11_info)) = &apqn.info {
        if ep11_info.mkvp.is_empty() {
            return Err(anyhow!(
                "Master key verification pattern for AP {address} is unset"
            ));
        }
        associate_ap_device(&apqn, &ep11_info.mkvp)
            .await
            .context(anyhow!("Failed to associate AP {address}"))?;
    }
    Ok(())
}

#[cfg(target_arch = "s390x")]
async fn associate_ap_device(apqn: &Apqn, mkvp: &str) -> Result<()> {
    let resource_path = format!("/vfio_ap/{mkvp}");
    let secret_resource_path = format!("{resource_path}/secret");
    let secret_id_resource_path = format!("{resource_path}/secret_id");

    let uv_secret = get_cdh_resource(&secret_resource_path)
        .await
        .context(anyhow!(
            "Failed to read Confidential Data Hub secret {secret_resource_path}. \
             Provide the desired Ultravisor secret for this MKVP with an appropriate key broker service."
        ))?;
    let secret_id_bytes = get_cdh_resource(&secret_id_resource_path)
        .await
        .context(anyhow!(
            "Failed to read Confidential Data Hub secret {secret_id_resource_path}. \
             Provide the desired Ultravisor secret ID for this MKVP with an appropriate key broker service."
        ))?;
    let secret_id = std::str::from_utf8(&secret_id_bytes)?
        .trim_start_matches("0x")
        .trim_end();

    // TODO Once initdata is stable, enable and mandate this request be signed
    // (`pvsecret create --user-sign-key`, `pvsecret verify --user-cert`)
    let uv = uv::UvDevice::open()?;
    let mut add_cmd = uv::AddCmd::new(&mut uv_secret.as_slice())
        .context("Failed to create add secret request")?;
    uv.send_cmd(&mut add_cmd).context("Failed to add secret")?;
    let mut list_cmd = uv::ListCmd::new();
    uv.send_cmd(&mut list_cmd)?;

    let secret_idx = uv::SecretList::try_from(list_cmd)?
        .iter()
        .find(|&s| encode_hex(s.id()) == secret_id)
        .ok_or_else(|| anyhow!("Could not find secret with the ID {secret_id}. \
                                Perhaps there is a mismatch between the provided secret and secret ID."))?
        .index();
    Ok(apqn.set_associate_state(AssocState::Associated(secret_idx))?)
}

fn pci_addr_from_sysfs_path(sysfs_abs: &Path) -> Result<pci::Address> {
    // sysfs_abs like: /sys/devices/pci0000:00/0000:00:06.0/0000:02:00.0
    let name = sysfs_abs
        .file_name()
        .ok_or_else(|| anyhow!("bad sysfs path (no file_name): {:?}", sysfs_abs))?
        .to_str()
        .ok_or_else(|| anyhow!("bad sysfs path (non-utf8): {:?}", sysfs_abs))?;

    pci::Address::from_str(name)
        .map_err(|e| anyhow!("failed to parse pci bdf from sysfs '{}': {e}", name))
}

pub async fn wait_for_pci_device(
    logger: &Logger,
    sandbox: &Arc<Mutex<Sandbox>>,
    root_complex: &str,
    pcipath: &pci::Path,
) -> Result<pci::Address> {
    info!(logger, "wait_for_pci_device at {}", pcipath);
    let root_bus_rel = create_pci_root_bus_path(root_complex); // "/devices/pci0000:00"
    let root_bus_sysfs = format!("{}{}", SYSFS_DIR, &root_bus_rel); // "/sys/devices/pci0000:00"
    info!(
        logger,
        "wait_for_pci_device: root_bus_sysfs {} pcipath {}", &root_bus_sysfs, pcipath
    );
    let sysfs_rel_path = pcipath_to_sysfs(&root_bus_sysfs, pcipath)?; // "/0000:00:06.0/0000:02:00.0"

    // "/sys/devices/pci0000:00/0000:00:06.0/0000:02:00.0"
    let sysfs_abs = format!("{root_bus_sysfs}{sysfs_rel_path}");
    let sysfs_abs_path = std::path::PathBuf::from(&sysfs_abs);

    if tokio::fs::metadata(&sysfs_abs_path).await.is_ok() {
        info!(
            logger,
            "wait_for_pci_device: PCI device {} already exists at {}", pcipath, sysfs_abs
        );
        return pci_addr_from_sysfs_path(&sysfs_abs_path);
    } else {
        info!(
            logger,
            "wait_for_pci_device: Waiting uevent for PCI device {} at {}", pcipath, sysfs_abs
        );
    }

    let matcher = PciMatcher::new(&sysfs_rel_path, root_complex)?;

    let uev = wait_for_uevent(sandbox, matcher).await?;

    // uev.devpath like "/devices/pci0000:00/0000:00:06.0/0000:02:00.0"
    let addr = uev
        .devpath
        .rsplit('/')
        .next()
        .ok_or_else(|| anyhow!("Bad device path {:?} in uevent", &uev.devpath))?;

    pci::Address::from_str(addr)
}

// Represents an IOMMU group
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::tempdir;

    // Helper to create a VFIO uevent for testing
    fn create_vfio_uevent(group: IommuGroup) -> Uevent {
        let mut uev = Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.devname = format!("vfio/{group}");
        uev.devpath = format!("/devices/virtual/vfio/{group}");
        uev
    }

    #[rstest]
    #[case::group_1_matches(IommuGroup(1), IommuGroup(1), true)]
    #[case::group_22_matches(IommuGroup(22), IommuGroup(22), true)]
    #[case::group_1_rejects_22(IommuGroup(1), IommuGroup(22), false)]
    #[case::group_22_rejects_1(IommuGroup(22), IommuGroup(1), false)]
    #[tokio::test]
    async fn test_vfio_matcher_basic_matching(
        #[case] matcher_group: IommuGroup,
        #[case] uevent_group: IommuGroup,
        #[case] should_match: bool,
    ) {
        let matcher = VfioMatcher::new(matcher_group);
        let uev = create_vfio_uevent(uevent_group);

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "Matcher for group {} should {} uevent for group {}",
            matcher_group,
            if should_match { "match" } else { "reject" },
            uevent_group
        );
    }

    #[tokio::test]
    async fn test_vfio_matcher_wrong_devpath() {
        let group = IommuGroup(1);
        let matcher = VfioMatcher::new(group);
        let mut uev = create_vfio_uevent(group);
        uev.devpath = "/devices/virtual/vfio/wrong".to_string();

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject devpath with wrong IOMMU group"
        );
    }

    #[tokio::test]
    async fn test_vfio_matcher_partial_match() {
        let group = IommuGroup(1);
        let matcher = VfioMatcher::new(group);
        let mut uev = create_vfio_uevent(group);
        uev.devpath = "/devices/virtual/vfio/1extra".to_string();

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject devpath with extra characters after group number"
        );
    }

    #[rstest]
    #[case::valid_option("0000:01:00.0=02/01", Some(("0000:01:00.0", "02/01")))]
    #[case::too_many_equals("0000:01:00.0=02/01=rubbish", None)]
    #[case::missing_equals("0000:01:00.0", None)]
    #[test]
    fn test_split_vfio_pci_option(#[case] input: &str, #[case] expected: Option<(&str, &str)>) {
        assert_eq!(split_vfio_pci_option(input), expected);
    }

    #[test]
    fn test_pci_driver_override() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let testdir = tempdir().expect("failed to create tmpdir");
        let syspci = testdir.path(); // Path to mock /sys/bus/pci

        let dev0 = pci::Address::new(0, 0, pci::SlotFn::new(0, 0).unwrap());
        let dev0path = syspci.join("devices").join(dev0.to_string());
        let dev0drv = dev0path.join("driver");
        let dev0override = dev0path.join("driver_override");

        let drvapath = syspci.join("drivers").join("drv_a");
        let drvaunbind = drvapath.join("unbind");

        let probepath = syspci.join("drivers_probe");

        // Start mocking dev0 as being unbound
        fs::create_dir_all(&dev0path).unwrap();

        pci_driver_override(&logger, syspci, dev0, "drv_a").unwrap();
        assert_eq!(fs::read_to_string(&dev0override).unwrap(), "drv_a");
        assert_eq!(fs::read_to_string(&probepath).unwrap(), dev0.to_string());

        // Now mock dev0 already being attached to drv_a
        fs::create_dir_all(&drvapath).unwrap();
        std::os::unix::fs::symlink(&drvapath, dev0drv).unwrap();
        std::fs::remove_file(&probepath).unwrap();

        pci_driver_override(&logger, syspci, dev0, "drv_a").unwrap(); // no-op
        assert_eq!(fs::read_to_string(&dev0override).unwrap(), "drv_a");
        assert!(!probepath.exists());

        // Now try binding to a different driver
        pci_driver_override(&logger, syspci, dev0, "drv_b").unwrap();
        assert_eq!(fs::read_to_string(&dev0override).unwrap(), "drv_b");
        assert_eq!(fs::read_to_string(&probepath).unwrap(), dev0.to_string());
        assert_eq!(fs::read_to_string(drvaunbind).unwrap(), dev0.to_string());
    }

    #[test]
    fn test_pci_iommu_group() {
        let testdir = tempdir().expect("failed to create tmpdir"); // mock /sys
        let syspci = testdir.path().join("bus").join("pci");

        // Mock dev0, which has no group
        let dev0 = pci::Address::new(0, 0, pci::SlotFn::new(0, 0).unwrap());
        let dev0path = syspci.join("devices").join(dev0.to_string());

        fs::create_dir_all(dev0path).unwrap();

        // Test dev0
        assert!(pci_iommu_group(&syspci, dev0).unwrap().is_none());

        // Mock dev1, which is in group 12
        let dev1 = pci::Address::new(0, 1, pci::SlotFn::new(0, 0).unwrap());
        let dev1path = syspci.join("devices").join(dev1.to_string());
        let dev1group = dev1path.join("iommu_group");

        fs::create_dir_all(&dev1path).unwrap();
        std::os::unix::fs::symlink("../../../kernel/iommu_groups/12", dev1group).unwrap();

        // Test dev1
        assert_eq!(
            pci_iommu_group(&syspci, dev1).unwrap(),
            Some(IommuGroup(12))
        );

        // Mock dev2, which has a bogus group (dir instead of symlink)
        let dev2 = pci::Address::new(0, 2, pci::SlotFn::new(0, 0).unwrap());
        let dev2path = syspci.join("devices").join(dev2.to_string());
        let dev2group = dev2path.join("iommu_group");

        fs::create_dir_all(dev2group).unwrap();

        // Test dev2
        assert!(pci_iommu_group(&syspci, dev2).is_err());
    }

    // Helper to create a PCI uevent for testing
    fn create_pci_uevent(relpath: &str, root_complex: &str) -> Uevent {
        let root_bus = create_pci_root_bus_path(root_complex);
        let mut uev = Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.devpath = format!("{root_bus}{relpath}");
        uev
    }

    #[rstest]
    #[case::relpath_a_matches("/0000:00:06.0", "/0000:00:06.0", "00", true)]
    #[case::relpath_b_matches(
        "/0000:00:06.0/0000:02:00.0",
        "/0000:00:06.0/0000:02:00.0",
        "00",
        true
    )]
    #[case::relpath_a_rejects_b("/0000:00:06.0", "/0000:00:06.0/0000:02:00.0", "00", false)]
    #[case::relpath_b_rejects_a("/0000:00:06.0/0000:02:00.0", "/0000:00:06.0", "00", false)]
    #[test]
    fn test_pci_matcher_basic_matching(
        #[case] matcher_relpath: &str,
        #[case] uevent_relpath: &str,
        #[case] root_complex: &str,
        #[case] should_match: bool,
    ) {
        let matcher = PciMatcher::new(matcher_relpath, root_complex).unwrap();
        let uev = create_pci_uevent(uevent_relpath, root_complex);

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "Matcher for '{}' should {} uevent for '{}'",
            matcher_relpath,
            if should_match { "match" } else { "reject" },
            uevent_relpath
        );
    }

    #[test]
    fn test_pci_matcher_different_root_complex() {
        let relpath = "/0000:00:06.0";
        let matcher = PciMatcher::new(relpath, "00").unwrap();
        let uev = create_pci_uevent(relpath, "01");

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent from different root complex"
        );
    }

    #[test]
    fn test_pci_matcher_partial_path() {
        let root_bus = create_pci_root_bus_path("00");
        let relpath = "/0000:00:06.0";
        let matcher = PciMatcher::new(relpath, "00").unwrap();
        let mut uev = create_pci_uevent(relpath, "00");
        uev.devpath = format!("{root_bus}/0000:00:06");

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject partial PCI path match"
        );
    }

    #[cfg(target_arch = "s390x")]
    // Helper to create an AP uevent for testing
    fn create_ap_uevent(card: &str, relpath: &str, action: &str) -> Uevent {
        let mut uev = Uevent::default();
        uev.action = action.to_string();
        uev.subsystem = "ap".to_string();
        uev.devpath = format!("{AP_ROOT_BUS_PATH}/card{card}/{relpath}");
        uev
    }

    #[cfg(target_arch = "s390x")]
    #[tokio::test]
    async fn test_vfio_ap_matcher_add_action() {
        let card = "0a";
        let relpath = format!("{card}.0001");
        let ap_address = ap::Address::from_str(&relpath).unwrap();
        let matcher = ApMatcher::new(ap_address);
        let uev = create_ap_uevent(card, &relpath, U_EVENT_ACTION_ADD);

        assert!(
            matcher.is_match(&uev),
            "Matcher should match uevent with add action"
        );
    }

    #[cfg(target_arch = "s390x")]
    #[tokio::test]
    async fn test_vfio_ap_matcher_remove_action() {
        let card = "0a";
        let relpath = format!("{card}.0001");
        let ap_address = ap::Address::from_str(&relpath).unwrap();
        let matcher = ApMatcher::new(ap_address);
        let uev = create_ap_uevent(card, &relpath, U_EVENT_ACTION_REMOVE);

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent with remove action"
        );
    }

    #[cfg(target_arch = "s390x")]
    #[tokio::test]
    async fn test_vfio_ap_matcher_different_device() {
        let card = "0a";
        let relpath = format!("{card}.0001");
        let ap_address = ap::Address::from_str(&relpath).unwrap();
        let matcher = ApMatcher::new(ap_address);
        let other_relpath = format!("{card}.0002");
        let uev = create_ap_uevent(card, &other_relpath, U_EVENT_ACTION_ADD);

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent for different AP device"
        );
    }
}
