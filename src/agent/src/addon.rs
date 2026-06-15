// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Generic, data-driven contract for guest components shipped in addon images.
//!
//! An addon image is mounted read-only at `/run/kata-addons/<name>/` and ships a
//! manifest (`etc/kata-addons/components.toml`) describing the components it
//! provides. The agent consumes that manifest instead of carrying hard-coded
//! knowledge about any particular addon, so a new bundle can be introduced
//! without changing agent code as long as it only relies on the documented
//! substitution variables below.
//!
//! Manifest schema (TOML):
//!
//! ```toml
//! schema_version = 1
//!
//! # Path-only components, looked up by id and resolved relative to the addon root.
//! [paths]
//! "ocicrypt-config" = "etc/ocicrypt_config.json"
//! "pause-bundle"    = "pause_bundle"
//!
//! # Launchable processes, started in declaration order. A process is launched
//! # only when its `level` is <= the level requested by the agent configuration.
//! [[process]]
//! id            = "attestation-agent"
//! level         = 1
//! path          = "usr/local/bin/attestation-agent"
//! args          = ["--attestation_sock", "${aa_attestation_uri}"]
//! optional_args = [{ when = "initdata_toml_path", args = ["--initdata-toml", "${initdata_toml_path}"] }]
//! config        = "${aa_config_path}"
//! wait_socket   = "${aa_attestation_socket}"
//! timeout_secs  = "${launch_process_timeout}"
//! ```
//!
//! `${name}` tokens in `args`, `optional_args`, `config`, `env` values,
//! `wait_socket` and `timeout_secs` are substituted from a context map supplied
//! by the agent at runtime. Referencing an unknown variable is a hard error
//! (fail-closed). An `optional_args` group is included only when the variable
//! named by `when` is present and non-empty in the context.

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const COCO_COMPONENT_OCICRYPT_CONFIG: &str = "ocicrypt-config";
pub const COCO_COMPONENT_PAUSE_BUNDLE: &str = "pause-bundle";
pub const COCO_ADDON_NAME: &str = "coco";

const ADDONS_ROOT: &str = "/run/kata-addons";
const COMPONENTS_MANIFEST_REL_PATH: &str = "etc/kata-addons/components.toml";

#[derive(Debug, Deserialize)]
struct AddonManifest {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    #[serde(default)]
    paths: HashMap<String, String>,
    #[serde(default)]
    process: Vec<RawProcessSpec>,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Deserialize, Clone)]
struct RawProcessSpec {
    id: String,
    #[serde(default)]
    level: u32,
    // `path` is optional: a process either declares its `path` directly, or
    // provides `variants` and lets `select` choose which one is active.
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    select: Option<String>,
    #[serde(default)]
    variants: HashMap<String, Variant>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    optional_args: Vec<OptionalArgs>,
    #[serde(default)]
    config: Option<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    wait_socket: Option<String>,
    #[serde(default)]
    timeout_secs: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct OptionalArgs {
    when: String,
    args: Vec<String>,
}

/// A selectable flavour of a process. The active variant contributes its
/// `path` (overriding the process-level `path`, if any) and merges its extra
/// `args` (appended) and `env` (overriding on key conflicts) onto the base
/// process definition. This lets a single addon image ship several flavours of
/// the same component (e.g. a stock attestation-agent and an NVIDIA-attester
/// build) and have the consumer pick one without forking the image.
#[derive(Debug, Deserialize, Clone, Default)]
struct Variant {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

/// Default variant name used when the `select` value is empty or unset.
const DEFAULT_VARIANT: &str = "default";

/// A fully resolved description of a process the agent should launch. Paths are
/// absolute, all `${var}` tokens have been substituted, and the gating decision
/// has already been made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub id: String,
    pub path: PathBuf,
    pub args: Vec<String>,
    pub config: Option<String>,
    pub env: Vec<(String, String)>,
    pub wait_socket: Option<String>,
    pub timeout_secs: u64,
}

fn validate_addon_name(addon_name: &str) -> Result<()> {
    if addon_name.is_empty() {
        bail!("invalid empty addon name");
    }
    if addon_name.contains('/')
        || Path::new(addon_name).components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("invalid addon name '{}'", addon_name);
    }
    Ok(())
}

fn addon_root(addons_root: &Path, addon_name: &str) -> Result<PathBuf> {
    validate_addon_name(addon_name)?;
    Ok(addons_root.join(addon_name))
}

fn manifest_path(addons_root: &Path, addon_name: &str) -> Result<PathBuf> {
    Ok(addon_root(addons_root, addon_name)?.join(COMPONENTS_MANIFEST_REL_PATH))
}

/// Absolute mount root of an addon (e.g. `/run/kata-addons/coco`). Exposed so
/// callers can publish it as a manifest substitution variable.
pub fn addon_mount_root(addon_name: &str) -> Result<PathBuf> {
    addon_root(Path::new(ADDONS_ROOT), addon_name)
}

/// Resolve a relative path declared inside a manifest against the addon root,
/// rejecting absolute paths and `..` traversal.
fn resolve_rel_path(root: &Path, rel_path: &str, what: &str) -> Result<PathBuf> {
    let rel_path = rel_path.trim();
    if rel_path.is_empty() {
        bail!("empty path for {}", what);
    }
    if Path::new(rel_path)
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
    {
        bail!("invalid path '{}' for {}", rel_path, what);
    }
    Ok(root.join(rel_path.trim_start_matches('/')))
}

fn load_manifest(addons_root: &Path, addon_name: &str) -> Result<Option<AddonManifest>> {
    let root = addon_root(addons_root, addon_name)?;
    if !root.exists() {
        // The addon is not mounted at all; callers fall back to legacy paths or
        // their built-in defaults.
        return Ok(None);
    }

    let manifest_path = manifest_path(addons_root, addon_name)?;
    if !manifest_path.exists() {
        // A mounted addon without a manifest is a packaging bug: fail closed so
        // the misconfiguration is surfaced rather than silently ignored.
        bail!(
            "addon '{}' is mounted but manifest is missing at {}",
            addon_name,
            manifest_path.display()
        );
    }

    let data = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read addon manifest {}", manifest_path.display()))?;
    let manifest: AddonManifest = toml::from_str(&data)
        .with_context(|| format!("failed to parse addon manifest {}", manifest_path.display()))?;

    if manifest.schema_version != 1 {
        bail!(
            "unsupported addon manifest schema_version {} in {}",
            manifest.schema_version,
            manifest_path.display()
        );
    }

    Ok(Some(manifest))
}

fn resolve_component_path_in_root(
    addons_root: &Path,
    addon_name: &str,
    component_id: &str,
    legacy_path: &str,
) -> Result<PathBuf> {
    let manifest = match load_manifest(addons_root, addon_name)? {
        Some(manifest) => manifest,
        None => return Ok(PathBuf::from(legacy_path)),
    };

    let rel_path = manifest.paths.get(component_id).ok_or_else(|| {
        anyhow!(
            "component '{}' not found in addon '{}' manifest",
            component_id,
            addon_name
        )
    })?;

    let root = addon_root(addons_root, addon_name)?;
    resolve_rel_path(
        &root,
        rel_path,
        &format!("component '{}' in addon '{}'", component_id, addon_name),
    )
}

/// Resolve the absolute path of a path-only component declared in the addon's
/// `[paths]` table. When the addon is not mounted, `legacy_path` is returned so
/// non-addon (e.g. monolithic) images keep working unchanged.
pub fn resolve_component_path(
    addon_name: &str,
    component_id: &str,
    legacy_path: &str,
) -> Result<PathBuf> {
    resolve_component_path_in_root(
        Path::new(ADDONS_ROOT),
        addon_name,
        component_id,
        legacy_path,
    )
}

/// Replace every `${name}` token in `template` with the matching value from
/// `ctx`. An unknown variable is a hard error (fail-closed).
fn substitute(template: &str, ctx: &HashMap<String, String>) -> Result<String> {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            let start = i + 2;
            let end = template[start..]
                .find('}')
                .map(|p| start + p)
                .ok_or_else(|| anyhow!("unterminated '${{' in '{}'", template))?;
            let name = &template[start..end];
            let value = ctx
                .get(name)
                .ok_or_else(|| anyhow!("unknown substitution variable '{}'", name))?;
            out.push_str(value);
            i = end + 1;
        } else {
            // Push a full UTF-8 char to avoid splitting multi-byte sequences.
            let ch = template[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    Ok(out)
}

// Resolve the active variant (if any) for a process. Returns `None` when the
// process declares no variants.
fn select_variant<'a>(
    raw: &'a RawProcessSpec,
    ctx: &HashMap<String, String>,
) -> Result<Option<&'a Variant>> {
    if raw.variants.is_empty() {
        return Ok(None);
    }

    let mut selector = match &raw.select {
        Some(s) => substitute(s, ctx)?,
        None => String::new(),
    };
    if selector.is_empty() {
        selector = DEFAULT_VARIANT.to_string();
    }

    let variant = raw.variants.get(&selector).ok_or_else(|| {
        anyhow!(
            "process '{}' has no variant '{}' (available: {})",
            raw.id,
            selector,
            {
                let mut names: Vec<&str> = raw.variants.keys().map(String::as_str).collect();
                names.sort_unstable();
                names.join(", ")
            }
        )
    })?;

    Ok(Some(variant))
}

fn build_launch_spec(
    root: &Path,
    raw: &RawProcessSpec,
    ctx: &HashMap<String, String>,
) -> Result<LaunchSpec> {
    let variant = select_variant(raw, ctx)?;

    let path_template = variant
        .and_then(|v| v.path.as_deref())
        .or(raw.path.as_deref())
        .ok_or_else(|| {
            anyhow!(
                "process '{}' declares neither 'path' nor a variant path",
                raw.id
            )
        })?;
    let path_template = substitute(path_template, ctx)?;
    let path = resolve_rel_path(root, &path_template, &format!("process '{}'", raw.id))?;

    let mut args = Vec::with_capacity(raw.args.len());
    for a in &raw.args {
        args.push(substitute(a, ctx)?);
    }
    for group in &raw.optional_args {
        let enabled = ctx.get(&group.when).map(|v| !v.is_empty()).unwrap_or(false);
        if enabled {
            for a in &group.args {
                args.push(substitute(a, ctx)?);
            }
        }
    }
    if let Some(v) = variant {
        for a in &v.args {
            args.push(substitute(a, ctx)?);
        }
    }

    let config = match &raw.config {
        Some(c) => {
            let c = substitute(c, ctx)?;
            if c.is_empty() {
                None
            } else {
                Some(c)
            }
        }
        None => None,
    };

    // Base env first, then variant env (variant overrides on key conflict).
    let mut env_map: HashMap<String, String> = HashMap::new();
    for (k, v) in &raw.env {
        env_map.insert(k.clone(), substitute(v, ctx)?);
    }
    if let Some(var) = variant {
        for (k, v) in &var.env {
            env_map.insert(k.clone(), substitute(v, ctx)?);
        }
    }
    let mut env: Vec<(String, String)> = env_map.into_iter().collect();
    // Deterministic order keeps behaviour reproducible and testable.
    env.sort();

    let wait_socket = match &raw.wait_socket {
        Some(s) => {
            let s = substitute(s, ctx)?;
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        None => None,
    };

    let timeout_secs = match &raw.timeout_secs {
        Some(t) => {
            let t = substitute(t, ctx)?;
            if t.is_empty() {
                0
            } else {
                t.parse::<u64>().with_context(|| {
                    format!("invalid timeout_secs '{}' for process '{}'", t, raw.id)
                })?
            }
        }
        None => 0,
    };

    Ok(LaunchSpec {
        id: raw.id.clone(),
        path,
        args,
        config,
        env,
        wait_socket,
        timeout_secs,
    })
}

fn launch_plan_in_root(
    addons_root: &Path,
    addon_name: &str,
    max_level: u32,
    ctx: &HashMap<String, String>,
) -> Result<Option<Vec<LaunchSpec>>> {
    let manifest = match load_manifest(addons_root, addon_name)? {
        Some(manifest) => manifest,
        None => return Ok(None),
    };

    let root = addon_root(addons_root, addon_name)?;
    let mut specs = Vec::new();
    for raw in &manifest.process {
        if raw.level == 0 || raw.level > max_level {
            continue;
        }
        specs.push(build_launch_spec(&root, raw, ctx)?);
    }
    Ok(Some(specs))
}

/// Build the ordered list of processes the agent should launch for an addon,
/// gated by `max_level` and with all `${var}` tokens substituted from `ctx`.
///
/// Returns `Ok(None)` when the addon is not mounted, signalling the caller to
/// use its built-in default launch behaviour.
pub fn launch_plan(
    addon_name: &str,
    max_level: u32,
    ctx: &HashMap<String, String>,
) -> Result<Option<Vec<LaunchSpec>>> {
    launch_plan_in_root(Path::new(ADDONS_ROOT), addon_name, max_level, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_manifest(root: &Path, addon_name: &str, content: &str) {
        let manifest_dir = root.join(addon_name).join("etc/kata-addons");
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(manifest_dir.join("components.toml"), content).unwrap();
    }

    fn ctx() -> HashMap<String, String> {
        let mut c = HashMap::new();
        c.insert("aa_attestation_uri".into(), "unix:///run/aa.sock".into());
        c.insert("aa_attestation_socket".into(), "/run/aa.sock".into());
        c.insert("aa_config_path".into(), "/run/initdata/aa.toml".into());
        c.insert("cdh_config_path".into(), "/run/initdata/cdh.toml".into());
        c.insert("cdh_socket".into(), "/run/cdh.sock".into());
        c.insert(
            "ocicrypt_config_path".into(),
            "/run/kata-addons/coco/etc/ocicrypt_config.json".into(),
        );
        c.insert("rest_api_features".into(), "all".into());
        c.insert("launch_process_timeout".into(), "6".into());
        c.insert("initdata_toml_path".into(), "".into());
        c.insert("attester_variant".into(), "".into());
        c.insert("addon_root".into(), "/run/kata-addons/coco".into());
        c
    }

    const COCO_MANIFEST: &str = r#"
schema_version = 1

[paths]
"ocicrypt-config" = "etc/ocicrypt_config.json"
"pause-bundle"    = "pause_bundle"

[[process]]
id            = "attestation-agent"
level         = 1
args          = ["--attestation_sock", "${aa_attestation_uri}"]
optional_args = [{ when = "initdata_toml_path", args = ["--initdata-toml", "${initdata_toml_path}"] }]
config        = "${aa_config_path}"
wait_socket   = "${aa_attestation_socket}"
timeout_secs  = "${launch_process_timeout}"
select        = "${attester_variant}"

  [process.variants.default]
  path = "usr/local/bin/attestation-agent"

  [process.variants.nvidia]
  path = "usr/local/bin/attestation-agent-nv"
  env  = { LD_LIBRARY_PATH = "${addon_root}/usr/local/lib:/run/kata-addons/gpu/usr/lib" }

[[process]]
id           = "confidential-data-hub"
level        = 2
path         = "usr/local/bin/confidential-data-hub"
config       = "${cdh_config_path}"
env          = { OCICRYPT_KEYPROVIDER_CONFIG = "${ocicrypt_config_path}", PATH = "${addon_root}/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin" }
wait_socket  = "${cdh_socket}"
timeout_secs = "${launch_process_timeout}"

[[process]]
id           = "api-server-rest"
level        = 3
path         = "usr/local/bin/api-server-rest"
args         = ["--features", "${rest_api_features}"]
timeout_secs = "0"
"#;

    #[test]
    fn falls_back_to_legacy_when_addon_not_mounted() {
        let dir = tempdir().unwrap();
        let legacy = "/usr/local/bin/attestation-agent";
        let got = resolve_component_path_in_root(
            dir.path(),
            COCO_ADDON_NAME,
            COCO_COMPONENT_OCICRYPT_CONFIG,
            legacy,
        )
        .unwrap();
        assert_eq!(got, PathBuf::from(legacy));
    }

    #[test]
    fn launch_plan_is_none_when_addon_not_mounted() {
        let dir = tempdir().unwrap();
        let got = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 3, &ctx()).unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn fails_when_addon_root_exists_but_manifest_missing() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(COCO_ADDON_NAME)).unwrap();

        let err = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 3, &ctx()).unwrap_err();
        assert!(err.to_string().contains("manifest is missing"));
    }

    #[test]
    fn resolves_component_from_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_ADDON_NAME, COCO_MANIFEST);
        let got = resolve_component_path_in_root(
            dir.path(),
            COCO_ADDON_NAME,
            COCO_COMPONENT_PAUSE_BUNDLE,
            "/legacy/pause_bundle",
        )
        .unwrap();
        assert_eq!(got, dir.path().join(COCO_ADDON_NAME).join("pause_bundle"));
    }

    #[test]
    fn fails_when_component_is_missing_in_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            COCO_ADDON_NAME,
            r#"
schema_version = 1
[paths]
"pause-bundle" = "pause_bundle"
"#,
        );
        let err = resolve_component_path_in_root(
            dir.path(),
            COCO_ADDON_NAME,
            COCO_COMPONENT_OCICRYPT_CONFIG,
            "/legacy",
        )
        .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn rejects_path_traversal_in_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            COCO_ADDON_NAME,
            r#"
schema_version = 1
[paths]
"ocicrypt-config" = "../outside"
"#,
        );
        let err = resolve_component_path_in_root(
            dir.path(),
            COCO_ADDON_NAME,
            COCO_COMPONENT_OCICRYPT_CONFIG,
            "/legacy",
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid path"));
    }

    #[test]
    fn builds_full_coco_launch_plan() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_ADDON_NAME, COCO_MANIFEST);
        let specs = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 3, &ctx())
            .unwrap()
            .unwrap();

        assert_eq!(specs.len(), 3);

        let root = dir.path().join(COCO_ADDON_NAME);
        let aa = &specs[0];
        assert_eq!(aa.id, "attestation-agent");
        assert_eq!(aa.path, root.join("usr/local/bin/attestation-agent"));
        assert_eq!(aa.args, vec!["--attestation_sock", "unix:///run/aa.sock"]);
        assert_eq!(aa.config.as_deref(), Some("/run/initdata/aa.toml"));
        assert_eq!(aa.wait_socket.as_deref(), Some("/run/aa.sock"));
        assert_eq!(aa.timeout_secs, 6);

        let cdh = &specs[1];
        assert_eq!(cdh.id, "confidential-data-hub");
        // env is sorted by key, so OCICRYPT_KEYPROVIDER_CONFIG precedes PATH.
        // PATH puts the addon's cryptsetup (usr/sbin) ahead of the base dirs
        // that carry mke2fs/mkfs.ext4/dd, which CDH's secure_mount shells out to.
        assert_eq!(
            cdh.env,
            vec![
                (
                    "OCICRYPT_KEYPROVIDER_CONFIG".to_string(),
                    "/run/kata-addons/coco/etc/ocicrypt_config.json".to_string()
                ),
                (
                    "PATH".to_string(),
                    "/run/kata-addons/coco/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin".to_string()
                )
            ]
        );

        let api = &specs[2];
        assert_eq!(api.id, "api-server-rest");
        assert_eq!(api.args, vec!["--features", "all"]);
        assert_eq!(api.timeout_secs, 0);
    }

    #[test]
    fn gating_limits_launched_processes() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_ADDON_NAME, COCO_MANIFEST);

        let specs = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 1, &ctx())
            .unwrap()
            .unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].id, "attestation-agent");
    }

    #[test]
    fn optional_args_included_when_variable_set() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_ADDON_NAME, COCO_MANIFEST);

        let mut ctx = ctx();
        ctx.insert(
            "initdata_toml_path".into(),
            "/run/initdata/initdata.toml".into(),
        );
        let specs = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 1, &ctx)
            .unwrap()
            .unwrap();
        assert_eq!(
            specs[0].args,
            vec![
                "--attestation_sock",
                "unix:///run/aa.sock",
                "--initdata-toml",
                "/run/initdata/initdata.toml"
            ]
        );
    }

    #[test]
    fn unknown_substitution_variable_fails() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            COCO_ADDON_NAME,
            r#"
schema_version = 1
[[process]]
id    = "broken"
level = 1
path  = "usr/local/bin/broken"
args  = ["--flag", "${does_not_exist}"]
"#,
        );
        let err = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 1, &ctx()).unwrap_err();
        assert!(err.to_string().contains("unknown substitution variable"));
    }

    #[test]
    fn unsupported_schema_version_fails() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            COCO_ADDON_NAME,
            r#"
schema_version = 2
[paths]
"pause-bundle" = "pause_bundle"
"#,
        );
        let err = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 3, &ctx()).unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn default_variant_selected_when_selector_empty() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_ADDON_NAME, COCO_MANIFEST);

        // ctx() leaves attester_variant empty -> "default".
        let specs = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 1, &ctx())
            .unwrap()
            .unwrap();
        let aa = &specs[0];
        assert_eq!(
            aa.path,
            dir.path()
                .join(COCO_ADDON_NAME)
                .join("usr/local/bin/attestation-agent")
        );
        assert!(aa.env.is_empty());
    }

    #[test]
    fn nvidia_variant_selects_nv_binary_and_merges_env() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_ADDON_NAME, COCO_MANIFEST);

        let mut ctx = ctx();
        ctx.insert("attester_variant".into(), "nvidia".into());
        let specs = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 1, &ctx)
            .unwrap()
            .unwrap();
        let aa = &specs[0];
        assert_eq!(
            aa.path,
            dir.path()
                .join(COCO_ADDON_NAME)
                .join("usr/local/bin/attestation-agent-nv")
        );
        assert_eq!(
            aa.env,
            vec![(
                "LD_LIBRARY_PATH".to_string(),
                "/run/kata-addons/coco/usr/local/lib:/run/kata-addons/gpu/usr/lib".to_string()
            )]
        );
        // Base args are preserved for the selected variant.
        assert_eq!(aa.args, vec!["--attestation_sock", "unix:///run/aa.sock"]);
    }

    #[test]
    fn unknown_variant_fails() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_ADDON_NAME, COCO_MANIFEST);

        let mut ctx = ctx();
        ctx.insert("attester_variant".into(), "does-not-exist".into());
        let err = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 1, &ctx).unwrap_err();
        assert!(err.to_string().contains("has no variant 'does-not-exist'"));
    }

    #[test]
    fn process_without_path_or_variants_fails() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            COCO_ADDON_NAME,
            r#"
schema_version = 1
[[process]]
id    = "broken"
level = 1
args  = ["--flag"]
"#,
        );
        let err = launch_plan_in_root(dir.path(), COCO_ADDON_NAME, 1, &ctx()).unwrap_err();
        assert!(err
            .to_string()
            .contains("neither 'path' nor a variant path"));
    }
}
