// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::{DeviceContext, DeviceHandler, DeviceInfo, SpecUpdate, BLOCK};
use crate::linux_abi::*;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use anyhow::{anyhow, Context, Result};
use kata_types::device::DRIVER_SCSI_TYPE;
use protocols::agent::Device;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::instrument;

/// The path segment in the uevent devpath that separates the SCSI path and the block device name.
const BLOCK_SEGMENT: &str = "/block/";

#[derive(Debug)]
pub struct ScsiDeviceHandler {}

#[async_trait::async_trait]
impl DeviceHandler for ScsiDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_SCSI_TYPE]
    }

    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        let vm_path = get_scsi_device_name(ctx.sandbox, &device.id).await?;

        Ok(DeviceInfo::new(&vm_path, true)
            .context("New device info")?
            .into())
    }
}

#[instrument]
pub async fn get_scsi_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    scsi_addr: &str,
) -> Result<String> {
    let matcher = ScsiBlockMatcher::new(scsi_addr);

    scan_scsi_bus(scsi_addr)?;
    let uev = wait_for_uevent(sandbox, matcher).await?;
    Ok(format!("{}/{}", SYSTEM_DEV_PATH, &uev.devname))
}

// FIXME: This matcher is only correct if the guest has at most one
// SCSI host.
#[derive(Debug)]
pub struct ScsiBlockMatcher {
    /// Expected SCSI path suffix before `/block/`, e.g. `/0:0:2:0`
    scsi_path_suffix: String,
}

impl ScsiBlockMatcher {
    pub fn new(scsi_addr: &str) -> ScsiBlockMatcher {
        ScsiBlockMatcher {
            scsi_path_suffix: format!("/0:0:{scsi_addr}"),
        }
    }

    fn split_block_devpath<'a>(&self, devpath: &'a str) -> Option<(&'a str, &'a str)> {
        let idx = devpath.find(BLOCK_SEGMENT)?;
        let prefix = &devpath[..idx];
        let suffix = &devpath[idx + BLOCK_SEGMENT.len()..];
        Some((prefix, suffix))
    }
}

impl UeventMatcher for ScsiBlockMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        if uev.action != U_EVENT_ACTION_ADD {
            return false;
        }

        if uev.subsystem != BLOCK || uev.devname.is_empty() {
            return false;
        }

        let (prefix, suffix) = match self.split_block_devpath(&uev.devpath) {
            Some(parts) => parts,
            None => return false,
        };

        prefix.ends_with(&self.scsi_path_suffix) && !suffix.contains('/') && suffix == uev.devname
    }
}

/// Scan SCSI bus for the given SCSI address(SCSI-Id and LUN)
#[instrument]
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
    let scan_data = &format!("0 {} {}", tokens[0], tokens[1]);

    for entry in fs::read_dir(SYSFS_SCSI_HOST_PATH)? {
        let host = entry?.file_name();

        let host_str = host.to_str().ok_or_else(|| {
            anyhow!(
                "failed to convert directory entry to unicode for file {:?}",
                host
            )
        })?;

        let scan_path = PathBuf::from(&format!("{}/{}/{}", SYSFS_SCSI_HOST_PATH, host_str, "scan"));

        fs::write(scan_path, scan_data)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::test_helpers;
    use crate::linux_abi::U_EVENT_ACTION_ADD;
    use rstest::rstest;

    fn make_scsi_block_uevent(addr: &str, devname: &str, devpath_suffix: &str) -> Uevent {
        let root_bus = create_pci_root_bus_path("00");

        let mut uev = Uevent::default();
        uev.action = U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = BLOCK.to_string();
        uev.devname = devname.to_string();
        uev.devpath = format!(
            "{root_bus}/0000:00:00.0/virtio0/host0/target0:0:{target}/0:0:{addr}/block/{devpath_suffix}",
            target = addr.split(':').next().unwrap_or("0"),
            addr = addr,
            devpath_suffix = devpath_suffix,
        );
        uev
    }

    #[rstest]
    #[case::addr_a_matches_uev_a("0:0", "sda", "0:0", "sda", true)]
    #[case::addr_b_matches_uev_b("2:0", "sdb", "2:0", "sdb", true)]
    #[case::addr_a_rejects_uev_b("0:0", "sda", "2:0", "sdb", false)]
    #[case::addr_b_rejects_uev_a("2:0", "sdb", "0:0", "sda", false)]
    #[tokio::test]
    async fn test_scsi_block_matcher_basic_matching(
        #[case] matcher_addr: &str,
        #[case] _matcher_devname: &str,
        #[case] uevent_addr: &str,
        #[case] uevent_devname: &str,
        #[case] should_match: bool,
    ) {
        let matcher = ScsiBlockMatcher::new(matcher_addr);
        let uev = make_scsi_block_uevent(uevent_addr, uevent_devname, uevent_devname);

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "Matcher for SCSI addr '{}' should {} uevent for addr '{}'",
            matcher_addr,
            if should_match { "match" } else { "reject" },
            uevent_addr
        );
    }

    #[rstest]
    #[case::wrong_subsystem(test_helpers::SUBSYSTEM_NET, "Wrong subsystem should be rejected")]
    #[tokio::test]
    async fn test_scsi_block_matcher_wrong_subsystem(
        #[case] wrong_subsystem: &str,
        #[case] description: &str,
    ) {
        let addr = "0:0";
        let matcher = ScsiBlockMatcher::new(addr);
        let mut uev = make_scsi_block_uevent(addr, "sda", "sda");
        uev.subsystem = wrong_subsystem.to_string();

        assert!(!matcher.is_match(&uev), "{}", description);
    }

    #[tokio::test]
    async fn test_scsi_block_matcher_empty_devname() {
        let addr = "0:0";
        let matcher = ScsiBlockMatcher::new(addr);
        let mut uev = make_scsi_block_uevent(addr, "sda", "sda");
        uev.devname = String::new();

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent with empty devname"
        );
    }

    #[tokio::test]
    async fn test_scsi_block_matcher_wrong_path() {
        let root_bus = create_pci_root_bus_path("00");
        let addr = "0:0";
        let matcher = ScsiBlockMatcher::new(addr);
        let mut uev = make_scsi_block_uevent(addr, "sda", "sda");
        uev.devpath =
            format!("{root_bus}/0000:00:00.0/virtio0/host0/target0:0:1/0:0:1:0/block/sdc");

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject devpath not containing the SCSI address search string"
        );
    }

    #[rstest]
    #[case::addr_1_1_matches("1:1", "sdc", true)]
    #[case::addr_0_0_rejects("0:0", "sda", false)]
    #[tokio::test]
    async fn test_scsi_block_matcher_different_addresses(
        #[case] test_addr: &str,
        #[case] test_devname: &str,
        #[case] should_match_1_1: bool,
    ) {
        let root_bus = create_pci_root_bus_path("00");
        let matcher = ScsiBlockMatcher::new("1:1");
        let mut uev = make_scsi_block_uevent(test_addr, test_devname, test_devname);

        // Adjust devpath for addr 1:1
        if test_addr == "1:1" {
            uev.devpath = format!("{root_bus}/0000:00:00.0/virtio0/host0/target0:0:1/0:0:{test_addr}/block/{test_devname}");
        }

        assert_eq!(
            matcher.is_match(&uev),
            should_match_1_1,
            "Matcher for '1:1' should {} uevent for addr '{}'",
            if should_match_1_1 { "match" } else { "reject" },
            test_addr
        );
    }

    #[rstest]
    #[case::whole_disk("0:0", "sda", "sda", true)]
    #[case::partition("0:0", "sda1", "sda/sda1", false)]
    #[tokio::test]
    async fn test_scsi_block_matcher_rejects_partitions(
        #[case] addr: &str,
        #[case] devname: &str,
        #[case] devpath_suffix: &str,
        #[case] should_match: bool,
    ) {
        let matcher = ScsiBlockMatcher::new(addr);
        let uev = make_scsi_block_uevent(addr, devname, devpath_suffix);

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "{} uevent should {} match",
            if devpath_suffix.contains('/') {
                "partition"
            } else {
                "whole disk"
            },
            if should_match { "" } else { "not" }
        );
    }
}
