// Copyright (c) 2026 Kata Containers contributors
//
// SPDX-License-Identifier: Apache-2.0

//! Guest-side AppArmor profile preparation.
//!
//! Profile text is never received from the host at runtime. It must already
//! be present in the guest rootfs or loaded during guest boot. This keeps the
//! trust boundary explicit and avoids turning the agent into a profile
//! transport.

use anyhow::{bail, Context, Result};
use nix::mount::MsFlags;
use std::fs::{self, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::{Command, Stdio};

const APPARMOR_PROFILES_PATH: &str = "/sys/kernel/security/apparmor/profiles";
const SECURITYFS_PATH: &str = "/sys/kernel/security";
const APPARMOR_EXEC_PATH: &str = "/proc/self/attr/exec";
const DEFAULT_PROFILE_DIR: &str = "/etc/apparmor.d";
const DEFAULT_PARSER_PATHS: &[&str] = &["/sbin/apparmor_parser", "/usr/sbin/apparmor_parser"];
const APPARMOR_LOAD_LOCK_PATH: &str = "/run/kata-apparmor.lock";
const MAX_PROFILE_NAME_BYTES: usize = 127;
const PROFILE_FILE_PREFIX: &str = "localhost/";

/// Prepare an OCI AppArmor profile in the guest before container setup.
///
/// This function must run while the agent still has access to the Guest root
/// filesystem and the privileges required to load policy. It deliberately
/// does not select the profile for the workload process; use
/// [`select_profile`] immediately before `execve` for that step.
pub fn prepare_profile(profile: Option<&str>) -> Result<Option<PreparedProfile>> {
    let Some(raw_name) = profile else {
        return Ok(None);
    };

    let raw_name = raw_name.trim();
    if raw_name.is_empty() || raw_name == "unconfined" {
        return Ok(None);
    }

    let name = resolve_profile_name(raw_name)?;

    // Container init processes can prepare the same profile concurrently.
    // Serialize the check/load/verify sequence so only the first process runs
    // apparmor_parser and later processes reuse the loaded profile.
    let _load_lock = acquire_load_lock()?;
    ensure_securityfs_mounted()?;
    if !profile_is_loaded(&name)? {
        load_profile_from_guest_rootfs(&name)?;
    }

    if !profile_is_loaded(&name)? {
        bail!("AppArmor profile {name} is not loaded after preparation")
    }

    Ok(Some(PreparedProfile { name }))
}

fn acquire_load_lock() -> Result<std::fs::File> {
    fs::create_dir_all("/run").context("failed to create /run for AppArmor lock")?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(APPARMOR_LOAD_LOCK_PATH)
        .with_context(|| format!("failed to open {APPARMOR_LOAD_LOCK_PATH}"))?;

    let result = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) };
    if result != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("failed to lock {APPARMOR_LOAD_LOCK_PATH}"));
    }

    Ok(lock)
}

/// Select a previously prepared profile for the next workload `execve`.
///
/// Keeping this separate from [`prepare_profile`] prevents profile loading
/// from accidentally using the workload rootfs after `pivot_root`.
pub fn select_profile(profile: &PreparedProfile) -> Result<()> {
    let exec_request = profile_exec_request(&profile.name);
    fs::write(APPARMOR_EXEC_PATH, exec_request.as_bytes()).with_context(|| {
        format!(
            "failed to select guest AppArmor profile {} via {APPARMOR_EXEC_PATH}",
            profile.name
        )
    })?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub struct PreparedProfile {
    name: String,
}

impl PreparedProfile {
    pub fn name(&self) -> &str {
        &self.name
    }
}

fn resolve_profile_name(raw_name: &str) -> Result<String> {
    // These are compatibility normalizations for values observed at the OCI
    // boundary. Their ownership is intentionally left open for upstream
    // design discussion in issue #7586.
    let name = if raw_name == "runtime/default" {
        "kata-default"
    } else {
        raw_name
            .strip_prefix(PROFILE_FILE_PREFIX)
            .unwrap_or(raw_name)
    };
    validate_profile_name(name)?;
    Ok(name.to_owned())
}

fn profile_exec_request(name: &str) -> String {
    format!("exec {name}")
}

fn ensure_securityfs_mounted() -> Result<()> {
    if Path::new(APPARMOR_PROFILES_PATH).is_file() {
        return Ok(());
    }

    let mountpoint = Path::new(SECURITYFS_PATH);
    if !mountpoint.exists() {
        fs::create_dir_all(mountpoint)
            .with_context(|| format!("failed to create {SECURITYFS_PATH}"))?;
    }

    crate::mount::mount(
        Some("securityfs"),
        mountpoint,
        Some("securityfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        None::<&str>,
    )
    .with_context(|| format!("failed to mount securityfs at {SECURITYFS_PATH}"))?;

    if !Path::new(APPARMOR_PROFILES_PATH).is_file() {
        bail!("securityfs is mounted but guest AppArmor is not active")
    }
    Ok(())
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > MAX_PROFILE_NAME_BYTES {
        bail!("invalid AppArmor profile name length")
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        bail!("invalid AppArmor profile name: {name}")
    }
    Ok(())
}

fn profile_is_loaded(name: &str) -> Result<bool> {
    let contents = fs::read_to_string(APPARMOR_PROFILES_PATH).with_context(|| {
        format!("failed to read loaded AppArmor profiles from {APPARMOR_PROFILES_PATH}")
    })?;
    Ok(contents.lines().any(|line| {
        let loaded_name = line
            .trim_start()
            .split_once(' ')
            .map_or(line.trim(), |(loaded_name, _)| loaded_name);
        loaded_name == name
    }))
}

fn load_profile_from_guest_rootfs(name: &str) -> Result<()> {
    let profile_path = Path::new(DEFAULT_PROFILE_DIR).join(name);
    let metadata = fs::symlink_metadata(&profile_path).with_context(|| {
        format!(
            "guest AppArmor profile {name} is not loaded and rootfs profile is missing: {}",
            profile_path.display()
        )
    })?;
    if !metadata.file_type().is_file() {
        bail!(
            "guest AppArmor profile is not a regular file: {}",
            profile_path.display()
        )
    }

    let parser = DEFAULT_PARSER_PATHS
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .ok_or_else(|| anyhow::anyhow!("apparmor_parser is not installed in the guest rootfs"))?;

    let output = Command::new(parser)
        .args(["-r", profile_path.to_str().unwrap_or_default()])
        .stdin(Stdio::null())
        .output()
        .context("failed to start apparmor_parser in guest")?;
    if !output.status.success() {
        bail!(
            "apparmor_parser failed for {}: {}",
            profile_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }

    if !profile_is_loaded(name)? {
        bail!("apparmor_parser succeeded but profile {name} is not loaded")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_guest_profile_names() {
        validate_profile_name("kata-default").unwrap();
        validate_profile_name("my.profile_v1").unwrap();
    }

    #[test]
    fn resolves_oci_profile_compatibility_values() {
        assert_eq!(
            resolve_profile_name("runtime/default").unwrap(),
            "kata-default"
        );
        assert_eq!(resolve_profile_name("localhost/custom").unwrap(), "custom");
        assert_eq!(resolve_profile_name("custom").unwrap(), "custom");
    }

    #[test]
    fn treats_unconfined_and_empty_profiles_as_noop() {
        assert!(prepare_profile(None).unwrap().is_none());
        assert!(prepare_profile(Some("")).unwrap().is_none());
        assert!(prepare_profile(Some("unconfined")).unwrap().is_none());
    }

    #[test]
    fn rejects_path_like_profile_names() {
        assert!(validate_profile_name("../escape").is_err());
        assert!(validate_profile_name("name/child").is_err());
        assert!(validate_profile_name("name\nchild").is_err());
    }

    #[test]
    fn uses_apparmor_exec_attribute_command() {
        assert_eq!(profile_exec_request("kata-default"), "exec kata-default");
    }

    #[test]
    fn prepared_profile_exposes_only_the_validated_name() {
        let profile = PreparedProfile {
            name: "kata-default".to_owned(),
        };
        assert_eq!(profile.name(), "kata-default");
    }
}
