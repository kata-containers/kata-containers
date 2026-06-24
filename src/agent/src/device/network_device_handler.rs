// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
#[cfg(target_arch = "s390x")]
use crate::ccw;
use crate::linux_abi::*;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
#[cfg(not(target_arch = "s390x"))]
use crate::{device::pcipath_to_sysfs, pci};
use anyhow::{anyhow, Result};
use regex::Regex;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

fn check_existing(re: Regex) -> Result<bool> {
    // Check if the interface is already added in case network is cold-plugged
    // or the uevent loop is started before network is added.
    // We check for the device in the sysfs directory for network devices.
    for entry in fs::read_dir(SYSFS_NET_PATH)? {
        let entry = entry?;
        let path = entry.path();
        let target_path = fs::read_link(path)?;
        let target_path_str = target_path
            .to_str()
            .ok_or_else(|| anyhow!("Expected symlink in dir {}", SYSFS_NET_PATH))?;

        if re.is_match(target_path_str) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(not(target_arch = "s390x"))]
pub async fn wait_for_pci_net_interface(
    sandbox: &Arc<Mutex<Sandbox>>,
    root_complex: &str,
    pcipath: &pci::Path,
) -> Result<()> {
    let root_bus_sysfs = format!("{}{}", SYSFS_DIR, create_pci_root_bus_path(root_complex));
    let sysfs_rel_path = pcipath_to_sysfs(&root_bus_sysfs, pcipath)?;
    let matcher = NetPciMatcher::new(&sysfs_rel_path, root_complex);
    let pattern = format!(
        r"[./]+{}/[a-z0-9/]*net/[a-z0-9/]*",
        matcher.devpath.as_str()
    );
    let re = Regex::new(&pattern).expect("BUG: Failed to compile regex for NetPciMatcher");
    if check_existing(re)? {
        return Ok(());
    }

    let _uev = wait_for_uevent(sandbox, matcher).await?;

    Ok(())
}

#[cfg(not(target_arch = "s390x"))]
#[derive(Debug)]
pub struct NetPciMatcher {
    devpath: String,
}

#[cfg(not(target_arch = "s390x"))]
impl NetPciMatcher {
    pub fn new(relpath: &str, root_complex: &str) -> NetPciMatcher {
        let root_bus = create_pci_root_bus_path(root_complex);

        NetPciMatcher {
            devpath: format!("{root_bus}{relpath}"),
        }
    }
}

#[cfg(not(target_arch = "s390x"))]
impl UeventMatcher for NetPciMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.devpath.starts_with(self.devpath.as_str())
            && uev.subsystem == "net"
            && !uev.interface.is_empty()
            && uev.action == "add"
    }
}

#[cfg(target_arch = "s390x")]
pub async fn wait_for_ccw_net_interface(
    sandbox: &Arc<Mutex<Sandbox>>,
    device: &ccw::Device,
) -> Result<()> {
    let matcher = NetCcwMatcher::new(CCW_ROOT_BUS_PATH, device);
    if check_existing(matcher.re.clone())? {
        return Ok(());
    }
    let _uev = wait_for_uevent(sandbox, matcher).await?;
    Ok(())
}

#[cfg(target_arch = "s390x")]
#[derive(Debug)]
struct NetCcwMatcher {
    re: Regex,
}

#[cfg(target_arch = "s390x")]
impl NetCcwMatcher {
    pub fn new(root_bus_path: &str, device: &ccw::Device) -> Self {
        let re = format!(r"{root_bus_path}/0\.[0-3]\.[0-9a-f]{{1,4}}/{device}/virtio[0-9]+/net/");
        NetCcwMatcher {
            re: Regex::new(&re).expect("BUG: failed to compile NetCCWMatcher regex"),
        }
    }
}

#[cfg(target_arch = "s390x")]
impl UeventMatcher for NetCcwMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        self.re.is_match(&uev.devpath)
            && uev.subsystem == "net"
            && !uev.interface.is_empty()
            && uev.action == "add"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_arch = "s390x"))]
    use crate::device::test_helpers;
    use rstest::rstest;

    #[cfg(not(target_arch = "s390x"))]
    // Helper to create a network PCI uevent
    fn create_net_pci_uevent(
        relpath: &str,
        root_complex: &str,
        interface: &str,
    ) -> crate::uevent::Uevent {
        let root_bus = create_pci_root_bus_path(root_complex);
        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.devpath = format!("{root_bus}{relpath}");
        uev.subsystem = String::from("net");
        uev.interface = String::from(interface);
        uev
    }

    #[cfg(not(target_arch = "s390x"))]
    #[rstest]
    #[case::matcher_a_matches_uev_a(
        "/0000:00:02.0/0000:01:01.0",
        "/0000:00:02.0/0000:01:01.0",
        true
    )]
    #[case::matcher_b_matches_uev_b(
        "/0000:00:02.0/0000:01:02.0",
        "/0000:00:02.0/0000:01:02.0",
        true
    )]
    #[case::matcher_a_rejects_uev_b(
        "/0000:00:02.0/0000:01:01.0",
        "/0000:00:02.0/0000:01:02.0",
        false
    )]
    #[case::matcher_b_rejects_uev_a(
        "/0000:00:02.0/0000:01:02.0",
        "/0000:00:02.0/0000:01:01.0",
        false
    )]
    #[tokio::test]
    async fn test_net_pci_matcher_basic_matching(
        #[case] matcher_relpath: &str,
        #[case] uevent_relpath: &str,
        #[case] should_match: bool,
    ) {
        let matcher = NetPciMatcher::new(matcher_relpath, "00");
        let uev = create_net_pci_uevent(uevent_relpath, "00", "eth0");

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "Matcher for '{}' should {} uevent for '{}'",
            matcher_relpath,
            if should_match { "match" } else { "reject" },
            uevent_relpath
        );
    }

    #[cfg(not(target_arch = "s390x"))]
    #[tokio::test]
    async fn test_net_pci_matcher_with_net_substring() {
        let relpath = "/0000:00:02.0/0000:01:03.0";
        let root_bus = create_pci_root_bus_path("00");
        let net_substr = "/net/eth0";

        let matcher = NetPciMatcher::new(relpath, "00");
        let mut uev = create_net_pci_uevent(relpath, "00", "eth0");
        uev.devpath = format!("{root_bus}{relpath}{net_substr}");

        assert!(
            matcher.is_match(&uev),
            "Matcher should match uevent with /net/ substring in devpath"
        );
    }

    #[cfg(not(target_arch = "s390x"))]
    #[rstest]
    #[case::wrong_subsystem(test_helpers::SUBSYSTEM_BLOCK, "Wrong subsystem should be rejected")]
    #[tokio::test]
    async fn test_net_pci_matcher_wrong_subsystem(
        #[case] wrong_subsystem: &str,
        #[case] description: &str,
    ) {
        let relpath = "/0000:00:02.0/0000:01:01.0";
        let matcher = NetPciMatcher::new(relpath, "00");
        let mut uev = create_net_pci_uevent(relpath, "00", "eth0");
        uev.subsystem = wrong_subsystem.to_string();

        assert!(!matcher.is_match(&uev), "{}", description);
    }

    #[cfg(not(target_arch = "s390x"))]
    #[rstest]
    #[case::wrong_action(test_helpers::ACTION_REMOVE, "Wrong action should be rejected")]
    #[tokio::test]
    async fn test_net_pci_matcher_wrong_action(
        #[case] wrong_action: &str,
        #[case] description: &str,
    ) {
        let relpath = "/0000:00:02.0/0000:01:01.0";
        let matcher = NetPciMatcher::new(relpath, "00");
        let mut uev = create_net_pci_uevent(relpath, "00", "eth0");
        uev.action = wrong_action.to_string();

        assert!(!matcher.is_match(&uev), "{}", description);
    }

    #[cfg(not(target_arch = "s390x"))]
    #[tokio::test]
    async fn test_net_pci_matcher_empty_interface() {
        let relpath = "/0000:00:02.0/0000:01:01.0";
        let matcher = NetPciMatcher::new(relpath, "00");
        let mut uev = create_net_pci_uevent(relpath, "00", "eth0");
        uev.interface = String::new();

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent with empty interface"
        );
    }

    #[cfg(not(target_arch = "s390x"))]
    #[tokio::test]
    async fn test_net_pci_matcher_wrong_devpath() {
        let relpath = "/0000:00:02.0/0000:01:01.0";
        let root_bus = create_pci_root_bus_path("00");
        let matcher = NetPciMatcher::new(relpath, "00");
        let mut uev = create_net_pci_uevent(relpath, "00", "eth0");
        uev.devpath = format!("{}/0000:00:03.0", root_bus);

        assert!(
            !matcher.is_match(&uev),
            "Matcher should reject uevent with wrong devpath"
        );
    }

    #[cfg(target_arch = "s390x")]
    // Helper to create a network CCW uevent
    fn create_net_ccw_uevent(device: &ccw::Device, interface: &str) -> crate::uevent::Uevent {
        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = String::from("net");
        uev.interface = String::from(interface);
        uev.devpath = format!(
            "{}/0.0.0001/{}/virtio1/{}/{}",
            CCW_ROOT_BUS_PATH, device, uev.subsystem, uev.interface
        );
        uev
    }

    #[cfg(target_arch = "s390x")]
    #[rstest]
    #[case::dev_a_matches_uev_a(0, 1, 0, 1, true)]
    #[case::dev_b_matches_uev_b(1, 2, 1, 2, true)]
    #[case::dev_a_rejects_uev_b(0, 1, 1, 2, false)]
    #[case::dev_b_rejects_uev_a(1, 2, 0, 1, false)]
    #[tokio::test]
    async fn test_net_ccw_matcher_basic_matching(
        #[case] matcher_ssid: u8,
        #[case] matcher_devno: u16,
        #[case] uevent_ssid: u8,
        #[case] uevent_devno: u16,
        #[case] should_match: bool,
    ) {
        let matcher_dev = ccw::Device::new(matcher_ssid, matcher_devno).unwrap();
        let uevent_dev = ccw::Device::new(uevent_ssid, uevent_devno).unwrap();

        let matcher = NetCcwMatcher::new(CCW_ROOT_BUS_PATH, &matcher_dev);
        let uev = create_net_ccw_uevent(&uevent_dev, "eth0");

        assert_eq!(
            matcher.is_match(&uev),
            should_match,
            "Matcher for device {} should {} uevent for device {}",
            matcher_dev,
            if should_match { "match" } else { "reject" },
            uevent_dev
        );
    }
}
