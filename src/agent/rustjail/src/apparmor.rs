// Copyright (c) 2026 Kata Containers contributors
//
// SPDX-License-Identifier: Apache-2.0

//! Agent-owned, sandbox-lifetime AppArmor policy preparation.
//! Only the agent parent loads policy. Init helpers receive a prepared identity
//! over the private setup pipe and select it immediately before workload exec.
//! A new registry never adopts profiles previously loaded by an unknown owner.

use anyhow::{bail, Context, Result};
use nix::mount::MsFlags;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};

const PROFILES_PATH: &str = "/sys/kernel/security/apparmor/profiles";
const POLICY_PATH: &str = "/sys/kernel/security/apparmor/policy/profiles";
const SECURITYFS_PATH: &str = "/sys/kernel/security";
const EXEC_PATH: &str = "/proc/self/attr/exec";
const EXTENSION_ROOT: &str = "/run/kata-extensions/apparmor";
const EXTENSION_PARAMETER: &str = "kata.extension.apparmor.verity_params";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PolicySource {
    mount: String,
    // Hash the whole policy tree, including all possible include files.
    sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct KernelProfile {
    algorithm: String,
    digest: String,
}

/// Internal setup data, never a field accepted from an OCI/RPC caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedProfile {
    name: String,
    source: PolicySource,
    kernel: KernelProfile,
}

impl PreparedProfile {
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Owned by Sandbox and accessed under its mutex. Entries outlive containers;
/// deleting one container must not unload policy shared with another container.
#[derive(Debug, Default)]
pub struct ProfileRegistry {
    profiles: HashMap<String, PreparedProfile>,
}

trait PolicyBackend {
    fn source(&self, name: &str) -> Result<PolicySource>;
    fn kernel(&self, name: &str) -> Result<Option<KernelProfile>>;
    fn add(&mut self, name: &str) -> Result<()>;
}

impl ProfileRegistry {
    pub fn prepare(&mut self, profile: Option<&str>) -> Result<Option<PreparedProfile>> {
        self.prepare_with(profile, &mut GuestPolicy)
    }

    fn prepare_with(
        &mut self,
        profile: Option<&str>,
        backend: &mut impl PolicyBackend,
    ) -> Result<Option<PreparedProfile>> {
        let Some(name) = requested_name(profile)? else {
            return Ok(None);
        };
        let source = backend.source(&name)?;
        let kernel = backend.kernel(&name)?;
        if let Some(known) = self.profiles.get(&name) {
            verify_identity(known, &source, kernel.as_ref())?;
            return Ok(Some(known.clone()));
        }
        if kernel.is_some() {
            bail!("AppArmor profile {name} already exists with unknown provenance")
        }
        // Add, never replace: a concurrent external load must fail rather than
        // change the confinement of processes already using the same name.
        backend.add(&name)?;
        let kernel = backend
            .kernel(&name)?
            .ok_or_else(|| anyhow::anyhow!("AppArmor profile {name} is missing after loading"))?;
        if backend.source(&name)? != source {
            bail!("AppArmor extension changed while loading profile {name}")
        }
        let prepared = PreparedProfile {
            name,
            source,
            kernel,
        };
        self.profiles
            .insert(prepared.name.clone(), prepared.clone());
        Ok(Some(prepared))
    }

    pub fn verify(&self, prepared: &PreparedProfile) -> Result<()> {
        let known = self.profiles.get(&prepared.name).ok_or_else(|| {
            anyhow::anyhow!(
                "AppArmor profile {} is not owned by this agent",
                prepared.name
            )
        })?;
        if known != prepared {
            bail!("AppArmor container policy identity does not match the sandbox registry");
        }
        let backend = GuestPolicy;
        verify_identity(
            known,
            &backend.source(&known.name)?,
            backend.kernel(&known.name)?.as_ref(),
        )
    }
}

fn verify_identity(
    known: &PreparedProfile,
    source: &PolicySource,
    kernel: Option<&KernelProfile>,
) -> Result<()> {
    if &known.source != source || kernel != Some(&known.kernel) {
        bail!(
            "AppArmor profile {} identity changed; refusing to reload or replace it",
            known.name
        )
    }
    Ok(())
}

/// Empty fields inherit; explicit unconfined cannot remove confinement.
pub fn exec_profile(
    container: Option<&PreparedProfile>,
    requested: Option<&str>,
) -> Result<Option<PreparedProfile>> {
    if requested.is_none_or(|value| value.trim().is_empty()) {
        return Ok(container.cloned());
    }
    if requested_name(requested)?.as_deref() != container.map(PreparedProfile::name) {
        bail!("ExecProcess cannot change the container AppArmor profile")
    }
    Ok(container.cloned())
}

/// Reject any OCI profile for which the parent prepared no matching identity.
pub fn validate_prepared(
    requested: Option<&str>,
    prepared: Option<&PreparedProfile>,
) -> Result<()> {
    if requested_name(requested)?.as_deref() != prepared.map(PreparedProfile::name) {
        bail!("AppArmor profile was not prepared by the agent parent")
    }
    Ok(())
}

/// Opened by the final init helper before joining workload mount namespaces.
/// Keeping the actual procfs inode open prevents maskedPaths or a workload
/// rootfs from redirecting the later write to /dev/null or a regular file.
pub struct ProfileExec {
    name: String,
    attribute: File,
}

impl ProfileExec {
    pub fn open(profile: &PreparedProfile) -> Result<Self> {
        validate_profile_name(&profile.name)?;
        let attribute = OpenOptions::new()
            .write(true)
            .open(EXEC_PATH)
            .context("open trusted guest AppArmor exec attribute")?;
        verify_exec_attribute(&attribute)?;
        Ok(Self {
            name: profile.name.clone(),
            attribute,
        })
    }

    pub fn select(mut self) -> Result<()> {
        verify_exec_attribute(&self.attribute)?;
        self.attribute
            .write_all(format!("exec {}", self.name).as_bytes())
            .with_context(|| format!("failed to select guest AppArmor profile {}", self.name))
    }
}

fn verify_exec_attribute(attribute: &File) -> Result<()> {
    use nix::sys::statfs::{fstatfs, PROC_SUPER_MAGIC};
    if fstatfs(attribute)?.filesystem_type() != PROC_SUPER_MAGIC {
        bail!("AppArmor exec attribute is not on trusted procfs")
    }
    Ok(())
}

fn requested_name(profile: Option<&str>) -> Result<Option<String>> {
    match profile.map(str::trim) {
        None | Some("" | "unconfined") => Ok(None),
        Some(value) => {
            // Transitional mapping, not an agreed Kubernetes API.
            let name = if value == "runtime/default" {
                "kata-default"
            } else {
                value.strip_prefix("localhost/").unwrap_or(value)
            };
            validate_profile_name(name)?;
            Ok(Some(name.to_owned()))
        }
    }
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 127
        || matches!(name, "." | ".." | "unconfined")
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        bail!("invalid AppArmor profile name: {name}");
    }
    Ok(())
}

struct GuestPolicy;

impl PolicyBackend for GuestPolicy {
    fn source(&self, name: &str) -> Result<PolicySource> {
        let cmdline = fs::read_to_string("/proc/cmdline")?;
        if !cmdline.split_whitespace().any(|p| {
            p == EXTENSION_PARAMETER
                || p.strip_prefix(EXTENSION_PARAMETER)
                    .is_some_and(|s| s.starts_with('='))
        }) {
            bail!(
                "AppArmor profile {name} requested but no apparmor guest extension is configured"
            );
        }
        let root = Path::new(EXTENSION_ROOT);
        let mount = extension_mount(&fs::read_to_string("/proc/self/mountinfo")?, root)?;
        if fs::canonicalize(root)? != root {
            bail!("AppArmor extension root must not contain symbolic links");
        }
        let metadata = fs::symlink_metadata(root.join("profiles").join(name)).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("AppArmor profile {name} is missing from the extension: {error}")
            } else {
                anyhow::anyhow!("failed to inspect AppArmor profile {name}: {error}")
            }
        })?;
        if !metadata.file_type().is_file() {
            bail!("AppArmor profile {name} is not a regular file in the extension")
        }
        Ok(PolicySource {
            mount,
            sha256: policy_tree_digest(root)?,
        })
    }

    fn kernel(&self, name: &str) -> Result<Option<KernelProfile>> {
        ensure_securityfs()?;
        if !profile_is_enforced(&fs::read_to_string(PROFILES_PATH)?, name)? {
            return Ok(None);
        }
        // AppArmor mangles directory names. Read the exported name instead.
        for entry in fs::read_dir(POLICY_PATH)? {
            let path = entry?.path();
            if fs::read_to_string(path.join("name"))?.trim_end() != name {
                continue;
            }
            if fs::read_to_string(path.join("mode"))?.trim_end() != "enforce" {
                bail!("AppArmor profile {name} is not in enforce mode")
            }
            // Older kernels export SHA-1, newer kernels SHA-256. Neither is
            // the source-tree SHA-256 recorded in PolicySource.
            for (algorithm, length) in [("sha256", 64), ("sha1", 40)] {
                match fs::read_to_string(path.join(algorithm)) {
                    Ok(value) => {
                        let digest = value.trim_end();
                        if digest.len() != length || !digest.bytes().all(|b| b.is_ascii_hexdigit())
                        {
                            bail!("invalid AppArmor kernel policy digest for {name}")
                        }
                        return Ok(Some(KernelProfile {
                            algorithm: algorithm.to_owned(),
                            digest: digest.to_owned(),
                        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e).context("read AppArmor kernel policy digest"),
                }
            }
            bail!("AppArmor policy digest unavailable for {name}; enable guest policy hashing")
        }
        bail!("AppArmor profile {name} has no verifiable kernel policy identity")
    }

    fn add(&mut self, name: &str) -> Result<()> {
        let root = Path::new(EXTENSION_ROOT);
        let profile = root.join("profiles").join(name);
        let parser = ["/sbin/apparmor_parser", "/usr/sbin/apparmor_parser"]
            .iter()
            .map(Path::new)
            .find(|p| p.is_file())
            .ok_or_else(|| {
                anyhow::anyhow!("apparmor_parser is missing from the guest base image")
            })?;
        let parsed = run_parser(parser, root, &profile, &["-Q", "-N"])?;
        if std::str::from_utf8(&parsed.stdout)?.trim() != name {
            bail!(
                "AppArmor file {} must declare exactly profile {name}",
                profile.display()
            )
        }
        run_parser(parser, root, &profile, &["-a"])?;
        Ok(())
    }
}

fn run_parser(parser: &Path, root: &Path, profile: &Path, action: &[&str]) -> Result<Output> {
    let output = Command::new(parser)
        .arg("--config-file=/dev/null")
        .args(action)
        // Never consult a cache built from another source or feature set.
        .arg("-K")
        .arg("-I")
        .arg(root)
        .arg("-b")
        .arg(root)
        .arg(profile)
        .stdin(Stdio::null())
        .output()
        .context("run guest apparmor_parser")?;
    if !output.status.success() {
        bail!(
            "apparmor_parser failed for {}: {}",
            profile.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
    Ok(output)
}

fn ensure_securityfs() -> Result<()> {
    if Path::new(PROFILES_PATH).is_file() {
        return Ok(());
    }
    fs::create_dir_all(SECURITYFS_PATH)?;
    crate::mount::mount(
        Some("securityfs"),
        Path::new(SECURITYFS_PATH),
        Some("securityfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        None::<&str>,
    )?;
    if !Path::new(PROFILES_PATH).is_file() {
        bail!("securityfs is mounted but guest AppArmor is not active");
    }
    Ok(())
}

fn profile_is_enforced(contents: &str, name: &str) -> Result<bool> {
    let mut found = false;
    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() != Some(name) {
            continue;
        }
        if found || fields.next() != Some("(enforce)") || fields.next().is_some() {
            bail!("AppArmor profile {name} is not unambiguously in enforce mode")
        }
        found = true;
    }
    Ok(found)
}

fn extension_mount(mountinfo: &str, root: &Path) -> Result<String> {
    let root = root.to_str().context("non-UTF8 extension root")?;
    let mut mount = None;
    for line in mountinfo.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let Some(point) = fields.get(4) else {
            continue;
        };
        if point
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
        {
            bail!("nested mounts are not allowed inside the AppArmor policy root")
        }
        if *point == root {
            let separator = fields
                .iter()
                .position(|f| *f == "-")
                .context("invalid mountinfo")?;
            if mount.is_some()
                || !fields
                    .get(5)
                    .is_some_and(|v| v.split(',').any(|v| v == "ro"))
                || !matches!(fields.get(separator + 1).copied(), Some("erofs" | "ext4"))
            {
                bail!("AppArmor extension must be a single read-only erofs/ext4 mount");
            }
            mount = Some(line.to_owned());
        }
    }
    mount.ok_or_else(|| {
        anyhow::anyhow!(
            "configured AppArmor extension is not mounted at {root}; systemd guest required"
        )
    })
}

fn policy_tree_digest(root: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_policy_files(root, root, &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    for path in files {
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .context("non-UTF8 policy path")?;
        let content = fs::read(&path)?;
        validate_includes(root, &path, &content)?;
        digest.update((relative.len() as u64).to_le_bytes());
        digest.update(relative.as_bytes());
        digest.update((content.len() as u64).to_le_bytes());
        digest.update(&content);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn collect_policy_files(root: &Path, path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || metadata.dev() != fs::metadata(root)?.dev() {
        bail!(
            "AppArmor policy path escapes the extension: {}",
            path.display()
        )
    }
    if metadata.is_file() {
        files.push(path.to_owned());
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            collect_policy_files(root, &entry?.path(), files)?;
        }
    } else {
        bail!(
            "AppArmor policy root contains a non-regular file: {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_includes(root: &Path, path: &Path, content: &[u8]) -> Result<()> {
    // Closed include grammar: no absolute/variable paths or continued directives
    // that could consult the distribution policy outside this extension.
    let text =
        std::str::from_utf8(content).context("AppArmor extension must contain policy text only")?;
    let inline = regex::Regex::new(r#"(^|[\s{}])(include|abi)([\s<\"]|$)"#)?;
    for line in text.lines() {
        let trimmed = line.trim_start();
        // Only contiguous #include is a parser directive; '# include' is a
        // comment, even if its text looks like an out-of-root include.
        let directive = if let Some(rest) = trimmed.strip_prefix('#') {
            if !rest.starts_with("include") {
                continue;
            }
            rest
        } else {
            trimmed
        };
        let (abi, rest) = if let Some(rest) = directive.strip_prefix("include") {
            (false, rest)
        } else if let Some(rest) = directive.strip_prefix("abi") {
            (true, rest)
        } else {
            let code = policy_rule_code(line)?;
            if code.trim_end().ends_with('\\') {
                bail!(
                    "continued AppArmor policy lines are not supported in the closed policy root"
                );
            }
            if inline.is_match(&code) {
                bail!("AppArmor include/ABI must start on its own line");
            }
            continue;
        };
        if !rest.is_empty()
            && !rest.starts_with(|c: char| c.is_whitespace() || c == '<' || c == '"')
        {
            continue;
        }
        let rest = rest.trim_start();
        let optional_rest = rest.strip_prefix("if").and_then(|rest| {
            rest.starts_with(char::is_whitespace)
                .then(|| rest.trim_start())
                .and_then(|rest| rest.strip_prefix("exists"))
                .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
        });
        let (optional, rest) = if !abi && optional_rest.is_some() {
            (true, optional_rest.unwrap().trim_start())
        } else {
            (false, rest)
        };
        let (closing, relative) = match rest.as_bytes().first() {
            Some(b'<') => ('>', &rest[1..]),
            Some(b'"') => ('"', &rest[1..]),
            _ => bail!("unsupported AppArmor include in {}", path.display()),
        };
        let (relative, tail) = relative
            .split_once(closing)
            .context("unterminated AppArmor include")?;
        let tail = if abi {
            tail.trim_start()
                .strip_prefix(',')
                .context("AppArmor abi directive requires a comma")?
        } else {
            tail
        };
        if !tail.trim().is_empty() && !tail.trim_start().starts_with('#') {
            bail!("unsupported AppArmor include suffix");
        }
        if relative.is_empty()
            || relative.contains(['@', '\\', '$', '*', '?', '[', ']'])
            || Path::new(relative)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            bail!("AppArmor include must remain inside the extension: {relative}");
        }
        if !optional && !root.join(relative).exists() {
            bail!("AppArmor include missing from extension: {relative}");
        }
    }
    Ok(())
}

// Omit quoted rule paths and ordinary comments before checking for unsupported
// inline directives. This must not interpret comment prose as policy syntax.
fn policy_rule_code(line: &str) -> Result<String> {
    let mut code = String::new();
    let mut quoted = false;
    let mut chars = line.char_indices();
    while let Some((offset, ch)) = chars.next() {
        if quoted {
            match ch {
                '\\' => {
                    chars.next();
                }
                '"' => quoted = false,
                _ => (),
            }
        } else {
            match ch {
                '"' => {
                    quoted = true;
                    code.push(' ');
                }
                '#' => {
                    if let Some(rest) = line[offset..].strip_prefix("#include") {
                        if rest.is_empty()
                            || rest.starts_with(|c: char| c.is_whitespace() || c == '<' || c == '"')
                        {
                            bail!("inline AppArmor #include is not supported");
                        }
                    }
                    break;
                }
                _ => code.push(ch),
            }
        }
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Backend {
        source: Option<PolicySource>,
        kernel: Option<KernelProfile>,
        loads: usize,
        fail_source: bool,
        fail_add: bool,
    }
    impl PolicyBackend for Backend {
        fn source(&self, _: &str) -> Result<PolicySource> {
            if self.fail_source {
                bail!("test extension unavailable");
            }
            Ok(self.source.clone().unwrap_or(PolicySource {
                mount: "mount-1".into(),
                sha256: "source-1".into(),
            }))
        }
        fn kernel(&self, _: &str) -> Result<Option<KernelProfile>> {
            Ok(self.kernel.clone())
        }
        fn add(&mut self, _: &str) -> Result<()> {
            self.loads += 1;
            if self.fail_add {
                bail!("test parser add failed");
            }
            self.kernel = Some(KernelProfile {
                algorithm: "sha256".into(),
                digest: "kernel-1".into(),
            });
            Ok(())
        }
    }
    fn prepared() -> PreparedProfile {
        ProfileRegistry::default()
            .prepare_with(Some("test"), &mut Backend::default())
            .unwrap()
            .unwrap()
    }
    #[test]
    fn load_once_and_reuse_only_the_registered_identity() {
        let mut registry = ProfileRegistry::default();
        let mut backend = Backend::default();
        let first = registry.prepare_with(Some("test"), &mut backend).unwrap();
        assert_eq!(
            registry
                .prepare_with(Some("localhost/test"), &mut backend)
                .unwrap(),
            first
        );
        assert_eq!(backend.loads, 1);
        backend.source = Some(PolicySource {
            mount: "other".into(),
            sha256: "changed".into(),
        });
        assert!(registry.prepare_with(Some("test"), &mut backend).is_err());
        assert_eq!(backend.loads, 1);
    }
    #[test]
    fn reject_unknown_preloaded_profile_even_with_matching_contents() {
        let mut backend = Backend {
            kernel: Some(prepared().kernel),
            ..Default::default()
        };
        assert!(ProfileRegistry::default()
            .prepare_with(Some("test"), &mut backend)
            .is_err());
        assert_eq!(backend.loads, 0);
    }
    #[test]
    fn reject_missing_or_changed_kernel_identity_without_reloading() {
        let mut registry = ProfileRegistry::default();
        let mut backend = Backend::default();
        registry.prepare_with(Some("test"), &mut backend).unwrap();
        backend.kernel = None;
        assert!(registry.prepare_with(Some("test"), &mut backend).is_err());
        backend.kernel = Some(KernelProfile {
            algorithm: "sha256".into(),
            digest: "replaced".into(),
        });
        assert!(registry.prepare_with(Some("test"), &mut backend).is_err());
        assert_eq!(backend.loads, 1);
    }
    #[test]
    fn exec_inherits_and_cannot_change_confinement() {
        let p = prepared();
        for request in [
            None,
            Some(""),
            Some("  "),
            Some("test"),
            Some("localhost/test"),
        ] {
            assert_eq!(exec_profile(Some(&p), request).unwrap(), Some(p.clone()));
        }
        for request in [Some("unconfined"), Some("other")] {
            assert!(exec_profile(Some(&p), request).is_err());
        }
        assert!(exec_profile(None, Some("test")).is_err());
        assert_eq!(exec_profile(None, Some("unconfined")).unwrap(), None);
        assert!(validate_prepared(Some("test"), None).is_err());
        assert!(validate_prepared(None, Some(&p)).is_err());
        validate_prepared(Some("localhost/test"), Some(&p)).unwrap();
    }
    #[test]
    fn no_profile_requires_no_backend_access() {
        for p in [None, Some(""), Some("unconfined")] {
            assert!(ProfileRegistry::default()
                .prepare_with(
                    p,
                    &mut Backend {
                        fail_source: true,
                        ..Default::default()
                    }
                )
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn failed_source_and_parser_never_register_a_policy() {
        let mut registry = ProfileRegistry::default();
        let mut backend = Backend {
            fail_source: true,
            ..Default::default()
        };
        assert!(registry.prepare_with(Some("test"), &mut backend).is_err());
        assert_eq!(backend.loads, 0);
        backend.fail_source = false;
        backend.fail_add = true;
        assert!(registry.prepare_with(Some("test"), &mut backend).is_err());
        assert!(registry.profiles.is_empty());
    }

    #[test]
    fn policy_state_cannot_be_adopted_by_a_new_agent_registry() {
        let mut registry = ProfileRegistry::default();
        let mut backend = Backend::default();
        registry.prepare_with(Some("test"), &mut backend).unwrap();
        assert!(ProfileRegistry::default()
            .prepare_with(Some("test"), &mut backend)
            .is_err());
        assert_eq!(backend.loads, 1);
    }
    #[test]
    fn modes_are_explicit_and_exact() {
        assert!(profile_is_enforced("test (enforce)\nother (complain)\n", "test").unwrap());
        for line in [
            "test",
            "test (complain)",
            "test (unconfined)",
            "test (unknown)",
            "test (enforce) extra",
            "test (enforce)\ntest (enforce)",
        ] {
            assert!(profile_is_enforced(line, "test").is_err());
        }
        assert!(!profile_is_enforced("other (enforce)", "test").unwrap());
    }
    #[test]
    fn names_are_not_paths_or_reserved_labels() {
        assert_eq!(
            requested_name(Some("runtime/default")).unwrap(),
            Some("kata-default".into())
        );
        for name in [
            "..",
            ".",
            "../escape",
            "name/child",
            "name\nchild",
            "localhost/unconfined",
        ] {
            assert!(requested_name(Some(name)).is_err());
        }
    }
    #[test]
    fn mount_must_be_exact_read_only_and_without_overmounts() {
        let line = "20 1 8:1 / /run/kata-extensions/apparmor ro,nosuid - erofs /dev/vdb ro";
        assert!(extension_mount(line, Path::new(EXTENSION_ROOT)).is_ok());
        assert!(
            extension_mount(&line.replace("ro,nosuid", "rw"), Path::new(EXTENSION_ROOT)).is_err()
        );
        assert!(extension_mount("", Path::new(EXTENSION_ROOT)).is_err());
        assert!(extension_mount(
            &format!(
                "{line}\n21 20 8:2 / /run/kata-extensions/apparmor/profiles ro - ext4 /dev/vdc ro"
            ),
            Path::new(EXTENSION_ROOT)
        )
        .is_err());
    }
    #[test]
    fn tree_digest_covers_includes_and_rejects_escapes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join("profiles")).unwrap();
        fs::create_dir(root.join("abstractions")).unwrap();
        fs::write(
            root.join("profiles/test"),
            "#include <abstractions/common>\nprofile test {}\n",
        )
        .unwrap();
        fs::write(root.join("abstractions/common"), "# first\n").unwrap();
        let first = policy_tree_digest(root).unwrap();
        fs::write(root.join("abstractions/common"), "# changed\n").unwrap();
        assert_ne!(first, policy_tree_digest(root).unwrap());
        for include in [
            "/etc/apparmor.d/global",
            "../outside",
            "missing",
            "@{outside}/file",
        ] {
            fs::write(
                root.join("profiles/test"),
                format!("#include <{include}>\n"),
            )
            .unwrap();
            assert!(policy_tree_digest(root).is_err());
        }
        fs::write(
            root.join("profiles/test"),
            "abi </etc/apparmor.d/abi/3.0>,\n",
        )
        .unwrap();
        assert!(policy_tree_digest(root).is_err());
        for text in [
            "profile test { include </etc/passwd>\n}\n",
            "profile test { abi </etc/passwd>,\n}\n",
            "profile test { #include </etc/passwd>\n}\n",
            "inclu\\\nde </etc/passwd>\n",
        ] {
            fs::write(root.join("profiles/test"), text).unwrap();
            assert!(policy_tree_digest(root).is_err());
        }
        fs::write(
            root.join("profiles/test"),
            "#include if\t exists <optional>\nprofile test {}\n",
        )
        .unwrap();
        policy_tree_digest(root).unwrap();
        fs::write(root.join("profiles/test"), "profile test {}\n").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", root.join("abstractions/link")).unwrap();
        assert!(policy_tree_digest(root).is_err());
    }

    #[test]
    fn ordinary_comments_and_quoted_rule_paths_are_not_directives() {
        let tmp = tempfile::tempdir().unwrap();
        let text = "# include </not-a-directive>\n\
            # This comment mentions include </outside> and abi <$HOST>.\n\
            profile test {\n\
              /tmp/** rw, # include </comment>\n\
              \"/tmp/include text\" r, # ordinary prose with #include here\n\
            }\n";
        validate_includes(tmp.path(), &tmp.path().join("test"), text.as_bytes()).unwrap();
    }

    #[test]
    fn exec_attribute_rejects_regular_files_and_devices() {
        let temporary = tempfile::tempfile().unwrap();
        assert!(verify_exec_attribute(&temporary).is_err());
        let null = OpenOptions::new().write(true).open("/dev/null").unwrap();
        assert!(verify_exec_attribute(&null).is_err());
    }

    #[test]
    #[serial_test::serial]
    fn exec_attribute_remains_procfs_after_masking() {
        test_utils::skip_if_not_root!();
        use nix::mount::{mount, umount2, MntFlags};
        use nix::sched::{unshare, CloneFlags};
        unshare(CloneFlags::CLONE_NEWNS).unwrap();
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .unwrap();
        let mut trusted = OpenOptions::new().write(true).open(EXEC_PATH).unwrap();
        verify_exec_attribute(&trusted).unwrap();
        mount(
            Some("/dev/null"),
            EXEC_PATH,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .unwrap();
        scopeguard::defer! { let _ = umount2(EXEC_PATH, MntFlags::MNT_DETACH); }
        let mut masked = OpenOptions::new().write(true).open(EXEC_PATH).unwrap();
        assert!(verify_exec_attribute(&masked).is_err());
        assert!(masked
            .write_all(b"invalid AppArmor attribute command")
            .is_ok());
        verify_exec_attribute(&trusted).unwrap();
        // An invalid command cannot change any process policy. The real procfs
        // descriptor rejects it even though the masked pathname accepts it.
        assert!(trusted
            .write_all(b"invalid AppArmor attribute command")
            .is_err());
    }

    #[test]
    fn concurrent_requests_are_serialized_by_the_sandbox_owner() {
        let registry = std::sync::Arc::new(std::sync::Mutex::new((
            ProfileRegistry::default(),
            Backend::default(),
        )));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let r = registry.clone();
                std::thread::spawn(move || {
                    let mut r = r.lock().unwrap();
                    let (registry, backend) = &mut *r;
                    registry.prepare_with(Some("test"), backend).unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(registry.lock().unwrap().1.loads, 1);
    }
}
