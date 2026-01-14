// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;

use anyhow::{Context, Result};
use nix::unistd::gettid;

/// Check if SELinux is enabled on the system
pub fn is_selinux_enabled() -> bool {
    fs::read_to_string("/proc/mounts")
        .map(|buf| buf.contains("selinuxfs"))
        .unwrap_or_default()
}

pub fn set_exec_label(label: &str) -> Result<()> {
    let mut attr_path = Path::new("/proc/thread-self/attr/exec").to_path_buf();
    if !attr_path.exists() {
        // Fall back to the old convention
        attr_path = Path::new("/proc/self/task")
            .join(gettid().to_string())
            .join("attr/exec")
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(attr_path)
        .context("open attr path")?;

    file.write_all(label.as_bytes())
        .with_context(|| "failed to apply SELinux label")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LABEL: &str = "system_u:system_r:unconfined_t:s0";

    #[test]
    fn test_set_exec_label() {
        let ret = set_exec_label(TEST_LABEL);
        if is_selinux_enabled() {
            assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
            // Check that the label was set correctly
            let mut attr_path = std::path::Path::new("/proc/thread-self/attr/exec").to_path_buf();
            if !attr_path.exists() {
                attr_path = std::path::Path::new("/proc/self/task")
                    .join(nix::unistd::gettid().to_string())
                    .join("attr/exec");
            }
            let label = std::fs::read_to_string(attr_path).unwrap();
            assert_eq!(label.trim_end_matches('\0'), TEST_LABEL);
        } else {
            assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
        }
    }
}
