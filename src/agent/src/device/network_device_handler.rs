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
