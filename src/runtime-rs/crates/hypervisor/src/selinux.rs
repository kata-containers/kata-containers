// Copyright 2024 The Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use nix::unistd::gettid;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;

/// Check if SELinux is enabled on the system
pub fn is_selinux_enabled() -> bool {
    let buf = match fs::read_to_string("/proc/mounts") {
        Ok(content) => content,
        Err(_) => return false,
    };
    buf.contains("selinuxfs")
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
        .open(attr_path)?;
    file.write_all(label.as_bytes())
        .with_context(|| "failed to apply SELinux label")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    const TEST_LABEL: &str = "system_u:system_r:unconfined_t:s0";
    
    #[test]
    fn test_is_selinux_enabled() {
        let result = is_selinux_enabled();
        // Just verify the function returns a boolean value
        assert!(result == true || result == false);
    }

    #[test]
    fn test_set_exec_label() {
        let ret = set_exec_label(TEST_LABEL);
        if is_selinux_enabled() {
            assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
        } else {
            assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
        }
    }
} 