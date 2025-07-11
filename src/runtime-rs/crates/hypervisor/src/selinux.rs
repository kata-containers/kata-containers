// Copyright 2024 The Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use nix::unistd::gettid;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;

/// Check if SELinux is enabled on the system
pub fn is_selinux_enabled() -> Result<bool> {
    let buf = fs::read_to_string("/proc/mounts")
        .context("failed to read /proc/mounts")?;
    let enabled = buf.contains("selinuxfs");
    Ok(enabled)
}

/// Check if SELinux is in enforcing mode
pub fn is_selinux_enforcing() -> Result<bool> {
    if !is_selinux_enabled()? {
        return Ok(false);
    }
    
    let enforce_path = "/sys/fs/selinux/enforce";
    if !Path::new(enforce_path).exists() {
        return Ok(false);
    }
    
    let content = fs::read_to_string(enforce_path)
        .context("failed to read SELinux enforce mode")?;
    let mode = content.trim().parse::<i32>()
        .context("failed to parse SELinux enforce mode")?;
    
    Ok(mode == 1)
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


/// Add SELinux mount label to mount options
pub fn add_mount_label(data: &mut String, label: &str) {
    if label.is_empty() {
        return;
    }
    
    if data.is_empty() {
        let context = format!("context=\"{}\"", label);
        data.push_str(&context);
    } else {
        let context = format!(",context=\"{}\"", label);
        data.push_str(&context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    const TEST_LABEL: &str = "system_u:system_r:unconfined_t:s0";
    
    #[test]
    fn test_is_selinux_enabled() {
        let result = is_selinux_enabled();
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    }
    
    #[test]
    fn test_is_selinux_enforcing() {
        let result = is_selinux_enforcing();
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
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
    
} 