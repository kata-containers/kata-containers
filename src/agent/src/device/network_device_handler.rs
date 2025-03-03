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
    pcipath: &pci::Path,
) -> Result<()> {
    let root_bus_sysfs = format!("{}{}", SYSFS_DIR, create_pci_root_bus_path());
    let sysfs_rel_path = pcipath_to_sysfs(&root_bus_sysfs, pcipath)?;
    let matcher = NetPciMatcher::new(&sysfs_rel_path);
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
    pub fn new(relpath: &str) -> NetPciMatcher {
        let root_bus = create_pci_root_bus_path();

        NetPciMatcher {
            devpath: format!("{}{}", root_bus, relpath),
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
        let re = format!(
            r"{}/0\.[0-3]\.[0-9a-f]{{1,4}}/{}/virtio[0-9]+/net/",
            root_bus_path, device
        );
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
    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_net_pci_matcher() {
        let root_bus = create_pci_root_bus_path();
        let relpath_a = "/0000:00:02.0/0000:01:01.0";

        let mut uev_a = crate::uevent::Uevent::default();
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.devpath = format!("{}{}", root_bus, relpath_a);
        uev_a.subsystem = String::from("net");
        uev_a.interface = String::from("eth0");
        let matcher_a = NetPciMatcher::new(relpath_a);
        println!("Matcher a : {}", matcher_a.devpath);

        let relpath_b = "/0000:00:02.0/0000:01:02.0";
        let mut uev_b = uev_a.clone();
        uev_b.devpath = format!("{}{}", root_bus, relpath_b);
        let matcher_b = NetPciMatcher::new(relpath_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));

        let relpath_c = "/0000:00:02.0/0000:01:03.0";
        let net_substr = "/net/eth0";
        let mut uev_c = uev_a.clone();
        uev_c.devpath = format!("{}{}{}", root_bus, relpath_c, net_substr);
        let matcher_c = NetPciMatcher::new(relpath_c);

        assert!(matcher_c.is_match(&uev_c));
        assert!(!matcher_a.is_match(&uev_c));
        assert!(!matcher_b.is_match(&uev_c));
    }

    #[cfg(target_arch = "s390x")]
    #[tokio::test]
    async fn test_net_ccw_matcher() {
        let dev_a = ccw::Device::new(0, 1).unwrap();
        let dev_b = ccw::Device::new(1, 2).unwrap();

        let mut uev_a = crate::uevent::Uevent::default();
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.subsystem = String::from("net");
        uev_a.interface = String::from("eth0");
        uev_a.devpath = format!(
            "{}/0.0.0001/{}/virtio1/{}/{}",
            CCW_ROOT_BUS_PATH, dev_a, uev_a.subsystem, uev_a.interface
        );

        let mut uev_b = uev_a.clone();
        uev_b.devpath = format!(
            "{}/0.0.0001/{}/virtio1/{}/{}",
            CCW_ROOT_BUS_PATH, dev_b, uev_b.subsystem, uev_b.interface
        );

        let matcher_a = NetCcwMatcher::new(CCW_ROOT_BUS_PATH, &dev_a);
        let matcher_b = NetCcwMatcher::new(CCW_ROOT_BUS_PATH, &dev_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));
    }
}
