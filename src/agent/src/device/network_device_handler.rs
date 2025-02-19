// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
use crate::device::pcipath_to_sysfs;
use crate::linux_abi::*;
use crate::pci;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use anyhow::{anyhow, Result};
use regex::Regex;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn wait_for_net_interface(
    sandbox: &Arc<Mutex<Sandbox>>,
    pcipath: &pci::Path,
) -> Result<()> {
    let root_bus_sysfs = format!("{}{}", SYSFS_DIR, create_pci_root_bus_path());
    let sysfs_rel_path = pcipath_to_sysfs(&root_bus_sysfs, pcipath)?;

    let matcher = NetPciMatcher::new(&sysfs_rel_path);

    // Check if the interface is already added in case network is cold-plugged
    // or the uevent loop is started before network is added.
    // We check for the pci deive in the sysfs directory for network devices.
    let pattern = format!(
        r"[./]+{}/[a-z0-9/]*net/[a-z0-9/]*",
        matcher.devpath.as_str()
    );
    let re = Regex::new(&pattern).expect("BUG: Failed to compile regex for NetPciMatcher");

    for entry in fs::read_dir(SYSFS_NET_PATH)? {
        let entry = entry?;
        let path = entry.path();
        let target_path = fs::read_link(path)?;
        let target_path_str = target_path
            .to_str()
            .ok_or_else(|| anyhow!("Expected symlink in dir {}", SYSFS_NET_PATH))?;

        if re.is_match(target_path_str) {
            return Ok(());
        }
    }
    let _uev = wait_for_uevent(sandbox, matcher).await?;

    Ok(())
}

#[derive(Debug)]
pub struct NetPciMatcher {
    devpath: String,
}

impl NetPciMatcher {
    pub fn new(relpath: &str) -> NetPciMatcher {
        let root_bus = create_pci_root_bus_path();

        NetPciMatcher {
            devpath: format!("{}{}", root_bus, relpath),
        }
    }
}

impl UeventMatcher for NetPciMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.devpath.starts_with(self.devpath.as_str())
            && uev.subsystem == "net"
            && !uev.interface.is_empty()
            && uev.action == "add"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
