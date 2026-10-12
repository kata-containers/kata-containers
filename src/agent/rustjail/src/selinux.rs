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

fn read_proc_attr(name: &str) -> String {
    let tid = gettid().to_string();
    let paths = [
        format!("/proc/thread-self/attr/{name}"),
        format!("/proc/self/attr/selinux/{name}"),
        format!("/proc/self/task/{tid}/attr/{name}"),
        format!("/proc/self/attr/{name}"),
    ];
    for path in paths {
        if let Ok(buf) = fs::read_to_string(&path) {
            let context = buf.trim_end_matches(['\0', '\n']).to_string();
            if !context.is_empty() {
                return context;
            }
        }
    }
    "<unread>".to_string()
}

pub fn current_context() -> String {
    read_proc_attr("current")
}

pub fn exec_context() -> String {
    read_proc_attr("exec")
}

pub fn self_exe() -> String {
    fs::read_link("/proc/self/exe")
        .map(|path| path.display().to_string())
        .unwrap_or_else(|err| format!("<error {err}>"))
}

pub fn file_context(path: &Path) -> String {
    match xattr::get(path, "security.selinux") {
        Ok(Some(bytes)) => String::from_utf8_lossy(&bytes)
            .trim_end_matches(['\0', '\n'])
            .to_string(),
        Ok(None) => "<no xattr>".to_string(),
        Err(err) => format!("<error {err}>"),
    }
}

/// Mount point and flags for `path`, from this process's mount namespace.
pub fn mount_flags(path: &Path) -> String {
    let Ok(info) = fs::read_to_string("/proc/self/mountinfo") else {
        return "<unread>".to_string();
    };
    let target = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let target = target.to_string_lossy();
    let mut best = String::new();
    let mut best_len = 0usize;
    for line in info.lines() {
        let mut fields = line.split_whitespace();
        let Some(mount_point) = fields.nth(4) else {
            continue;
        };
        let Some(options) = fields.next() else {
            continue;
        };
        if target.starts_with(mount_point) && mount_point.len() >= best_len {
            best_len = mount_point.len();
            best = format!("{mount_point} {options}");
        }
    }
    if best.is_empty() {
        "<none>".to_string()
    } else {
        best
    }
}

pub fn debug_kmsg(message: &str) {
    if let Ok(mut file) = OpenOptions::new().write(true).open("/dev/kmsg") {
        let _ = writeln!(file, "kata-agent: DEBUG selinux: {message}");
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
