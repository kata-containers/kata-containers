// Copyright (c) 2026 Kata Containers contributors
//
// SPDX-License-Identifier: Apache-2.0

//! Guest-side AppArmor profile preparation.
//!
//! Profile text is never received from the host at runtime, and no profile text
//! is read from the workload's container image. Policy comes from a guest
//! extension image that the runtime attaches to the VM and the guest mounts
//! read-only before the agent starts, which keeps the trust boundary explicit
//! and avoids turning the agent into a profile transport.
//!
//! The parser is not part of that extension. It ships in the base image, and
//! the extension carries policy text only.

use anyhow::{bail, Context, Result};
use nix::mount::MsFlags;
use std::fs::{self, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const APPARMOR_PROFILES_PATH: &str = "/sys/kernel/security/apparmor/profiles";
const SECURITYFS_PATH: &str = "/sys/kernel/security";
const APPARMOR_EXEC_PATH: &str = "/proc/self/attr/exec";
const DEFAULT_PARSER_PATHS: &[&str] = &["/sbin/apparmor_parser", "/usr/sbin/apparmor_parser"];
const APPARMOR_LOAD_LOCK_PATH: &str = "/run/kata-apparmor.lock";
const MAX_PROFILE_NAME_BYTES: usize = 127;
const PROFILE_FILE_PREFIX: &str = "localhost/";

/// Name of the guest extension that carries workload AppArmor policy.
const EXTENSION_NAME: &str = "apparmor";
/// Where the guest mounts guest extension images.
const EXTENSIONS_ROOT: &str = "/run/kata-extensions";
/// Subdirectory of the extension holding profile files.
const PROFILES_SUBDIR: &str = "profiles";

/// Root of the AppArmor extension inside the guest.
///
/// This is the mount point the runtime's extension support uses, and it is also
/// the only include root handed to `apparmor_parser`.
fn extension_root() -> PathBuf {
    Path::new(EXTENSIONS_ROOT).join(EXTENSION_NAME)
}

/// Path of the profile file for `name` inside the extension.
fn profile_path_for(name: &str) -> PathBuf {
    extension_root().join(PROFILES_SUBDIR).join(name)
}

/// How a profile is running in the kernel.
#[derive(Debug, PartialEq, Eq)]
enum ProfileMode {
    Enforce,
    Complain,
}

/// Prepare an OCI AppArmor profile in the guest before container setup.
///
/// This must run while the agent still has access to the guest extension mount
/// and the privileges required to load policy. It deliberately does not select
/// the profile for the workload process; use [`select_profile`] immediately
/// before `execve` for that step.
///
/// A requested profile that cannot be loaded fails the container instead of
/// letting it start unconfined, and a workload that requests nothing is
/// unaffected by the extension being absent.
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

    // A profile that is already loaded is reused. Anything other than an
    // enforced profile is refused: a profile in complain mode reports
    // violations without preventing them, so treating it as success would
    // silently leave the workload unconstrained.
    let loaded = loaded_profile_mode(&name)?;
    let mode = match loaded {
        Some(mode) => mode,
        None => {
            load_profile_from_extension(&name)?;
            loaded_profile_mode(&name)?.ok_or_else(|| {
                anyhow::anyhow!("AppArmor profile {name} is not loaded after preparation")
            })?
        }
    };

    match mode {
        ProfileMode::Enforce => Ok(Some(PreparedProfile { name })),
        ProfileMode::Complain => bail!(
            "AppArmor profile {name} is in complain mode; refusing to treat it \
             as an enforced profile"
        ),
    }
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

/// Turn the value received at the OCI boundary into a profile name.
///
/// `localhost/` is stripped because the agent resolves profiles inside the
/// guest extension rather than through the host's profile namespace.
///
/// The `runtime/default` mapping is a transitional compatibility measure whose
/// ownership is still open upstream: containerd may already have resolved
/// `RuntimeDefault` into a concrete name such as `cri-containerd.apparmor.d`
/// before the request ever reaches the guest. It is kept until that is settled.
fn resolve_profile_name(raw_name: &str) -> Result<String> {
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

/// Report how `name` is currently running, or `None` when it is not loaded.
fn loaded_profile_mode(name: &str) -> Result<Option<ProfileMode>> {
    let contents = fs::read_to_string(APPARMOR_PROFILES_PATH).with_context(|| {
        format!("failed to read loaded AppArmor profiles from {APPARMOR_PROFILES_PATH}")
    })?;

    for line in contents.lines() {
        let line = line.trim();
        let Some((loaded_name, rest)) = line.split_once(' ') else {
            // A bare name means the kernel did not report a mode. Treat that as
            // enforced: the kernel only adds an annotation when it differs from
            // the default, which is enforce.
            if line == name {
                return Ok(Some(ProfileMode::Enforce));
            }
            continue;
        };

        if loaded_name != name {
            continue;
        }

        let rest = rest.trim();
        if rest.contains("complain") {
            return Ok(Some(ProfileMode::Complain));
        }
        return Ok(Some(ProfileMode::Enforce));
    }

    Ok(None)
}

fn parser_path() -> Result<&'static Path> {
    DEFAULT_PARSER_PATHS
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "apparmor_parser is not installed in the guest base image; \
                 the guest image must provide it"
            )
        })
}

/// Explain why a profile could not be found, distinguishing a missing
/// extension from a missing profile inside a mounted one.
fn missing_profile_error(name: &str) -> anyhow::Error {
    let root = extension_root();
    let profile_path = profile_path_for(name);

    if !root.is_dir() {
        if extension_is_configured() {
            return anyhow::anyhow!(
                "AppArmor profile {name} was requested and the apparmor guest \
                 extension is configured, but {} is not mounted. With \
                 AGENT_INIT=yes guests do not mount guest extensions yet, so \
                 this configuration cannot work on such an image",
                root.display()
            );
        }
        return anyhow::anyhow!(
            "AppArmor profile {name} was requested but no apparmor guest \
             extension is configured for this runtime; add the extension to \
             the hypervisor guest_extension_images configuration"
        );
    }

    anyhow::anyhow!(
        "AppArmor profile {name} is not loaded and {} does not exist in the \
         apparmor guest extension",
        profile_path.display()
    )
}

/// Whether the runtime told the guest to attach an apparmor extension.
///
/// The runtime emits one `kata.extension.<name>.verity_params` entry per
/// configured extension, so its presence in the kernel command line is the
/// signal that an extension was configured even when it failed to mount.
fn extension_is_configured() -> bool {
    let Ok(cmdline) = fs::read_to_string("/proc/cmdline") else {
        return false;
    };
    let needle = format!("kata.extension.{EXTENSION_NAME}.verity_params");
    cmdline
        .split_whitespace()
        .any(|param| param == needle || param.starts_with(&format!("{needle}=")))
}

fn load_profile_from_extension(name: &str) -> Result<()> {
    let profile_path = profile_path_for(name);

    let metadata = fs::symlink_metadata(&profile_path)
        .map_err(|_| missing_profile_error(name))
        .with_context(|| format!("failed to inspect {}", profile_path.display()))?;
    if !metadata.file_type().is_file() {
        bail!(
            "guest AppArmor profile is not a regular file: {}",
            profile_path.display()
        )
    }

    let root = extension_root();
    let root_str = root
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("extension root is not valid UTF-8"))?;

    // The extension root is the only include root. Profiles may not reach the
    // distribution's /etc/apparmor.d, whose contents change independently of
    // this extension and would silently alter what a profile means.
    let output = Command::new(parser_path()?)
        .args([
            "-r",
            "-I",
            root_str,
            "-b",
            root_str,
            profile_path.to_str().unwrap_or_default(),
        ])
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

    if loaded_profile_mode(name)?.is_none() {
        bail!("apparmor_parser succeeded but profile {name} is not loaded")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_guest_profile_names() {
        validate_profile_name("kata-apparmor-test").unwrap();
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
        assert_eq!(
            profile_exec_request("kata-apparmor-test"),
            "exec kata-apparmor-test"
        );
    }

    #[test]
    fn prepared_profile_exposes_only_the_validated_name() {
        let profile = PreparedProfile {
            name: "kata-apparmor-test".to_owned(),
        };
        assert_eq!(profile.name(), "kata-apparmor-test");
    }

    #[test]
    fn resolves_profiles_inside_the_extension() {
        assert_eq!(
            profile_path_for("kata-apparmor-test"),
            Path::new("/run/kata-extensions/apparmor/profiles/kata-apparmor-test")
        );
        assert_eq!(
            extension_root(),
            Path::new("/run/kata-extensions/apparmor")
        );
    }

    #[test]
    fn never_resolves_profiles_from_the_distribution_directory() {
        // The distribution's own profiles are not a policy source: they change
        // independently of the extension and would make a profile's meaning
        // depend on the base image release.
        let resolved = profile_path_for("kata-apparmor-test");
        assert!(!resolved.starts_with("/etc/apparmor.d"));
    }
}
