// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::{DeviceContext, DeviceHandler, DeviceInfo, SpecUpdate, BLOCK};
use crate::linux_abi::ACPI_DEV_PATH;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use anyhow::{anyhow, Context, Result};
use kata_types::device::DRIVER_NVDIMM_TYPE;
use protocols::agent::Device;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::instrument;

#[derive(Debug)]
pub struct VirtioNvdimmDeviceHandler {}

#[async_trait::async_trait]
impl DeviceHandler for VirtioNvdimmDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_NVDIMM_TYPE]
    }

    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        if device.vm_path.is_empty() {
            return Err(anyhow!("Invalid path for nvdimm device"));
        }
        Ok(DeviceInfo::new(device.vm_path(), true)
            .context("New device info")?
            .into())
    }
}

#[instrument]
pub async fn wait_for_pmem_device(sandbox: &Arc<Mutex<Sandbox>>, devpath: &str) -> Result<()> {
    let devname = match devpath.strip_prefix("/dev/") {
        Some(dev) => dev,
        None => {
            return Err(anyhow!(
                "Storage source '{}' must start with /dev/",
                devpath
            ))
        }
    };

    let matcher = PmemBlockMatcher::new(devname);
    let uev = wait_for_uevent(sandbox, matcher).await?;
    if uev.devname != devname {
        return Err(anyhow!(
            "Unexpected device name {} for pmem device (expected {})",
            uev.devname,
            devname
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct PmemBlockMatcher {
    suffix: String,
}

impl PmemBlockMatcher {
    pub fn new(devname: &str) -> PmemBlockMatcher {
        let suffix = format!(r"/block/{devname}");

        PmemBlockMatcher { suffix }
    }
}

impl UeventMatcher for PmemBlockMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.subsystem == BLOCK
            && uev.devpath.starts_with(ACPI_DEV_PATH)
            && uev.devpath.ends_with(&self.suffix)
            && !uev.devname.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::test_helpers;
    use rstest::rstest;

    // Helper to create a PMEM uevent
    fn create_pmem_uevent(devname: &str, region: u32) -> crate::uevent::Uevent {
        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = BLOCK.to_string();
        uev.devname = devname.to_string();
        uev.devpath = format!(
            "{}/LNXSYSTM:00/LNXSYBUS:00/ACPI0012:00/ndbus0/region{}/btt{}.0/block/{}",
            ACPI_DEV_PATH, region, region, devname
        );
        uev
    }

    #[rstest]
    #[case::pmem0_matches_pmem0("pmem0", "pmem0", 0, 0, true)]
    #[case::pmem1_matches_pmem1("pmem1", "pmem1", 1, 1, true)]
    #[case::pmem0_rejects_pmem1("pmem0", "pmem1", 0, 1, false)]
    #[case::pmem1_rejects_pmem0("pmem1", "pmem0", 1, 0, false)]
    #[tokio::test]
    async fn test_pmem_block_matcher_basic_matching(
        #[case] matcher_devname: &str,
        #[case] uevent_devname: &str,
        #[case] _matcher_region: u32,
        #[case] uevent_region: u32,
        #[case] should_match: bool,
    ) {
        let matcher = PmemBlockMatcher::new(matcher_devname);
        let uev = create_pmem_uevent(uevent_devname, uevent_region);

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "Matcher for '{}' should {} uevent for '{}'",
            matcher_devname,
            if should_match { "match" } else { "reject" },
            uevent_devname
        );
    }

    #[rstest]
    #[case::wrong_subsystem(test_helpers::SUBSYSTEM_NET, "Wrong subsystem should be rejected")]
    #[tokio::test]
    async fn test_pmem_block_matcher_wrong_subsystem(
        #[case] wrong_subsystem: &str,
        #[case] description: &str,
    ) {
        let devname = "pmem0";
        let matcher = PmemBlockMatcher::new(devname);
        let mut uev = create_pmem_uevent(devname, 0);
        uev.subsystem = wrong_subsystem.to_string();

        assert!(!matcher.is_match(&uev), "{}", description);
    }

    #[tokio::test]
    async fn test_pmem_block_matcher_empty_devname() {
        let devname = "pmem0";
        let matcher = PmemBlockMatcher::new(devname);
        let mut uev = create_pmem_uevent(devname, 0);
        uev.devname = String::new();

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent with empty devname"
        );
    }

    #[tokio::test]
    async fn test_pmem_block_matcher_wrong_prefix() {
        let devname = "pmem0";
        let matcher = PmemBlockMatcher::new(devname);
        let mut uev = create_pmem_uevent(devname, 0);
        uev.devpath = format!("/devices/pci0000:00/block/{}", devname);

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject devpath not starting with ACPI_DEV_PATH"
        );
    }

    #[tokio::test]
    async fn test_pmem_block_matcher_wrong_suffix() {
        let devname = "pmem0";
        let matcher = PmemBlockMatcher::new(devname);
        let mut uev = create_pmem_uevent(devname, 0);
        uev.devpath = format!(
            "{}/LNXSYSTM:00/LNXSYBUS:00/ACPI0012:00/ndbus0/region0/btt0.0/block/pmem2",
            ACPI_DEV_PATH
        );

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject devpath with wrong device suffix"
        );
    }
}
