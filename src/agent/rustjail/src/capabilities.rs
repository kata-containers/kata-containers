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
        match Capability::from_str(&format!("CAP_{cap}")) {
            Err(_) => {
                log_child!(cfd_log, "{} is not a cap", &cap.to_string());
                continue;
            }
            Ok(c) => r.insert(c),
        };
    }

    r
}

pub fn get_all_caps() -> Result<CapsHashSet> {
    // This runs after joining the container mount namespace, so procfs may be
    // controlled by the workload. Query the kernel directly before installing
    // the workload seccomp filter, which may reject selected PR_CAPBSET_READ calls.
    let caps_set = runtime::thread_all_supported();
    if caps_set.is_empty() {
        return Err(anyhow!(
            "failed to enumerate supported capabilities with PR_CAPBSET_READ"
        ));
    }
    Ok(caps_set)
}

pub fn reset_effective() -> Result<()> {
    let all = get_all_caps()?;
    caps::set(None, CapSet::Effective, &all).map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}

pub fn restrict_bounding_set(cfd_log: RawFd, caps: &LinuxCapabilities) -> Result<()> {
    let all = get_all_caps()?;
    let allowed_bounding = to_capshashset(cfd_log, caps.bounding());

    for c in all.difference(&allowed_bounding) {
        caps::drop(None, CapSet::Bounding, *c).map_err(|e| anyhow!(e.to_string()))?;
    }

    // Verify the kernel's post-drop bounding set is no broader than the OCI allowlist.
    let remaining_bounding =
        caps::read(None, CapSet::Bounding).map_err(|e| anyhow!(e.to_string()))?;
    if !remaining_bounding.is_subset(&allowed_bounding) {
        return Err(anyhow!(
            "failed to remove disallowed capabilities from the bounding set"
        ));
    }

    Ok(())
}

pub fn drop_privileges(cfd_log: RawFd, caps: &LinuxCapabilities) -> Result<()> {
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
