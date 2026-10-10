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

/// Whether the kernel command line explicitly turns SELinux on. The kernel
/// honours the last `selinux=` occurrence, so this does too.
pub fn requested_on_cmdline(cmdline: &str) -> bool {
    cmdline
        .split_whitespace()
        .filter_map(|param| param.strip_prefix("selinux="))
        .next_back()
        == Some("1")
}

/// Whether a policy has been loaded. Until then the kernel reports the initial
/// SID's bare name (`kernel`) as the context of every task instead of a full
/// `user:role:type:level` context.
pub fn policy_loaded() -> bool {
    ["/proc/self/attr/selinux/current", "/proc/self/attr/current"]
        .iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .is_some_and(|context| is_full_context(&context))
}

fn is_full_context(context: &str) -> bool {
    context.trim_end_matches(['\0', '\n']).contains(':')
}

pub fn add_mount_label(data: &mut String, label: &str) {
    if data.is_empty() {
        let context = format!("context=\"{label}\"");
        data.push_str(&context);
    } else {
        let context = format!(",context=\"{label}\"");
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
    fn test_requested_on_cmdline() {
        assert!(requested_on_cmdline("console=hvc0 selinux=1 quiet"));
        assert!(!requested_on_cmdline("console=hvc0 selinux=0"));
        assert!(!requested_on_cmdline("console=hvc0"));
        assert!(!requested_on_cmdline("selinux=1 selinux=0"));
        assert!(requested_on_cmdline("selinux=0 selinux=1"));
        assert!(!requested_on_cmdline("kata.selinux=1"));
    }

    #[test]
    fn test_is_full_context() {
        assert!(!is_full_context("kernel\0"));
        assert!(!is_full_context(""));
        assert!(is_full_context("system_u:system_r:kata_agent_t:s0\0"));
        assert!(!is_full_context("unconfined\n"));
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
