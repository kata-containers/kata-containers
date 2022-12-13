// Copyright 2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use nix::unistd::gettid;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;

pub fn is_enabled() -> Result<bool> {
    let buf = fs::read_to_string("/proc/mounts")?;
    let enabled = buf.contains("selinuxfs");

    Ok(enabled)
}

pub fn add_mount_label(data: &mut String, label: &str) {
    if data.is_empty() {
        let context = format!("context=\"{}\"", label);
        data.push_str(&context);
    } else {
        let context = format!(",context=\"{}\"", label);
        data.push_str(&context);
    }
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
    fn test_is_enabled() {
        let ret = is_enabled();
        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_add_mount_label() {
        let mut data = String::new();
        add_mount_label(&mut data, TEST_LABEL);
        assert_eq!(data, format!("context=\"{}\"", TEST_LABEL));

        let mut data = String::from("defaults");
        add_mount_label(&mut data, TEST_LABEL);
        assert_eq!(data, format!("defaults,context=\"{}\"", TEST_LABEL));
    }

    #[test]
    fn test_set_exec_label() {
        let ret = set_exec_label(TEST_LABEL);
        if is_enabled().unwrap() {
            assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
        } else {
            assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
        }
    }
}
