// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

// looks like we can use caps to manipulate capabilities
// conveniently, use caps to do it directly.. maybe

use crate::log_child;
use crate::sync::write_count;
use anyhow::{anyhow, Result};
use caps::{self, runtime, CapSet, Capability, CapsHashSet};
use oci::{Capability as LinuxCapability, LinuxCapabilities};
use oci_spec::runtime as oci;
use std::collections::HashSet;
use std::os::unix::io::RawFd;
use std::str::FromStr;

fn to_capshashset(cfd_log: RawFd, capabilities: &Option<HashSet<LinuxCapability>>) -> CapsHashSet {
    let mut r = CapsHashSet::new();
    let binding: HashSet<LinuxCapability> = HashSet::new();
    let caps = capabilities.as_ref().unwrap_or(&binding);
    for cap in caps.iter() {
        match Capability::from_str(&format!("CAP_{}", cap)) {
            Err(_) => {
                log_child!(cfd_log, "{} is not a cap", &cap.to_string());
                continue;
            }
            Ok(c) => r.insert(c),
        };
    }

    r
}

pub fn get_all_caps() -> CapsHashSet {
    let mut caps_set =
        runtime::procfs_all_supported(None).unwrap_or_else(|_| runtime::thread_all_supported());
    if caps_set.is_empty() {
        caps_set = caps::all();
    }
    caps_set
}

pub fn reset_effective() -> Result<()> {
    let all = get_all_caps();
    caps::set(None, CapSet::Effective, &all).map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}

pub fn drop_privileges(cfd_log: RawFd, caps: &LinuxCapabilities) -> Result<()> {
    let all = get_all_caps();

    for c in all.difference(&to_capshashset(cfd_log, caps.bounding())) {
        caps::drop(None, CapSet::Bounding, *c).map_err(|e| anyhow!(e.to_string()))?;
    }

    caps::set(
        None,
        CapSet::Effective,
        &to_capshashset(cfd_log, caps.effective()),
    )
    .map_err(|e| anyhow!(e.to_string()))?;
    caps::set(
        None,
        CapSet::Permitted,
        &to_capshashset(cfd_log, caps.permitted()),
    )
    .map_err(|e| anyhow!(e.to_string()))?;
    caps::set(
        None,
        CapSet::Inheritable,
        &to_capshashset(cfd_log, caps.inheritable()),
    )
    .map_err(|e| anyhow!(e.to_string()))?;

    let _ = caps::set(
        None,
        CapSet::Ambient,
        &to_capshashset(cfd_log, caps.ambient()),
    )
    .map_err(|_| log_child!(cfd_log, "failed to set ambient capability"));

    Ok(())
}
