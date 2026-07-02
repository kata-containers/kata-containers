// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Generic, data-driven contract for guest components shipped in extension images.
//!
//! An extension image is mounted read-only at `/run/kata-extensions/<name>/` and
//! ships a manifest (`etc/kata-extensions/components.toml`) that declares the
//! path-only components and launchable processes it provides. The agent consumes
//! that manifest and builds its launch plan from it, instead of carrying
//! hard-coded knowledge about any particular extension.
//!
//! This keeps the agent generic for anything that can be expressed purely in the
//! manifest: adding, removing or reordering processes, changing their args, env,
//! paths or `${var}` substitutions, or selecting a build variant all happen with
//! no agent change. Workflows that need the agent to actively do something new
//! (e.g. publish a new substitution variable into the context, or react to a
//! component in a bespoke way) still require agent changes -- the CoCo
//! guest-component launching built on top of this module is one such case.
//!
//! The manifest schema, the `${var}` substitution variables the agent publishes,
//! and the attester-variant / NVRC contract are documented in
//! `docs/design/proposals/composable-vm-images.md`, which is the source of
//! truth; this module must stay consistent with it.

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const COCO_COMPONENT_OCICRYPT_CONFIG: &str = "ocicrypt-config";
pub const COCO_COMPONENT_PAUSE_BUNDLE: &str = "pause-bundle";
pub const COCO_EXTENSION_NAME: &str = "coco";

const EXTENSIONS_ROOT: &str = "/run/kata-extensions";
const COMPONENTS_MANIFEST_REL_PATH: &str = "etc/kata-extensions/components.toml";

#[derive(Debug, Deserialize)]
struct ExtensionManifest {
    // Required: the manifest is a versioned contract, so a manifest without an
    // explicit schema_version is rejected rather than silently assumed to be v1.
    schema_version: u32,
    #[serde(default)]
    paths: HashMap<String, String>,
    #[serde(default)]
    process: Vec<RawProcessSpec>,
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
/// process definition. This lets a single extension image ship several flavours of
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

fn validate_extension_name(extension_name: &str) -> Result<()> {
    if extension_name.is_empty() {
        bail!("invalid empty extension name");
    }
    if extension_name.contains('/')
        || Path::new(extension_name).components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("invalid extension name '{}'", extension_name);
    }
    Ok(())
}

fn extension_root(extensions_root: &Path, extension_name: &str) -> Result<PathBuf> {
    validate_extension_name(extension_name)?;
    Ok(extensions_root.join(extension_name))
}

fn manifest_path(extensions_root: &Path, extension_name: &str) -> Result<PathBuf> {
    Ok(extension_root(extensions_root, extension_name)?.join(COMPONENTS_MANIFEST_REL_PATH))
}

/// Absolute mount root of an extension (e.g. `/run/kata-extensions/coco`). Exposed so
/// callers can publish it as a manifest substitution variable.
pub fn extension_mount_root(extension_name: &str) -> Result<PathBuf> {
    extension_root(Path::new(EXTENSIONS_ROOT), extension_name)
}

/// Resolve a relative path declared inside a manifest against the extension root,
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

fn load_manifest(
    extensions_root: &Path,
    extension_name: &str,
) -> Result<Option<ExtensionManifest>> {
    let root = extension_root(extensions_root, extension_name)?;
    if !root.exists() {
        // The extension is not mounted at all; callers fall back to legacy paths or
        // their built-in defaults.
        return Ok(None);
    }

    let manifest_path = manifest_path(extensions_root, extension_name)?;
    if !manifest_path.exists() {
        // A mounted extension without a manifest is a packaging bug: fail closed so
        // the misconfiguration is surfaced rather than silently ignored.
        bail!(
            "extension '{}' is mounted but manifest is missing at {}",
            extension_name,
            manifest_path.display()
        );
    }

    let data = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "failed to read extension manifest {}",
            manifest_path.display()
        )
    })?;
    let manifest: ExtensionManifest = toml::from_str(&data).with_context(|| {
        format!(
            "failed to parse extension manifest {}",
            manifest_path.display()
        )
    })?;

    if manifest.schema_version != 1 {
        bail!(
            "unsupported extension manifest schema_version {} in {}",
            manifest.schema_version,
            manifest_path.display()
        );
    }

    Ok(Some(manifest))
}

fn resolve_component_path_in_root(
    extensions_root: &Path,
    extension_name: &str,
    component_id: &str,
    legacy_path: &str,
) -> Result<PathBuf> {
    let manifest = match load_manifest(extensions_root, extension_name)? {
        Some(manifest) => manifest,
        None => return Ok(PathBuf::from(legacy_path)),
    };

    let rel_path = manifest.paths.get(component_id).ok_or_else(|| {
        anyhow!(
            "component '{}' not found in extension '{}' manifest",
            component_id,
            extension_name
        )
    })?;

    let root = extension_root(extensions_root, extension_name)?;
    resolve_rel_path(
        &root,
        rel_path,
        &format!(
            "component '{}' in extension '{}'",
            component_id, extension_name
        ),
    )
}

/// Resolve the absolute path of a path-only component declared in the extension's
/// `[paths]` table. When the extension is not mounted, `legacy_path` is returned so
/// non-extension (e.g. monolithic) images keep working unchanged.
pub fn resolve_component_path(
    extension_name: &str,
    component_id: &str,
    legacy_path: &str,
) -> Result<PathBuf> {
    resolve_component_path_in_root(
        Path::new(EXTENSIONS_ROOT),
        extension_name,
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
    extensions_root: &Path,
    extension_name: &str,
    max_level: u32,
    ctx: &HashMap<String, String>,
) -> Result<Option<Vec<LaunchSpec>>> {
    let manifest = match load_manifest(extensions_root, extension_name)? {
        Some(manifest) => manifest,
        None => return Ok(None),
    };

    let root = extension_root(extensions_root, extension_name)?;
    let mut specs = Vec::new();
    for raw in &manifest.process {
        if raw.level == 0 || raw.level > max_level {
            continue;
        }
        specs.push(build_launch_spec(&root, raw, ctx)?);
    }
    Ok(Some(specs))
}

/// Build the ordered list of processes the agent should launch for an extension,
/// gated by `max_level` and with all `${var}` tokens substituted from `ctx`.
///
/// Returns `Ok(None)` when the extension is not mounted, signalling the caller to
/// use its built-in default launch behaviour.
pub fn launch_plan(
    extension_name: &str,
    max_level: u32,
    ctx: &HashMap<String, String>,
) -> Result<Option<Vec<LaunchSpec>>> {
    launch_plan_in_root(Path::new(EXTENSIONS_ROOT), extension_name, max_level, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_manifest(root: &Path, extension_name: &str, content: &str) {
        let manifest_dir = root.join(extension_name).join("etc/kata-extensions");
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
            "/run/kata-extensions/coco/etc/ocicrypt_config.json".into(),
        );
        c.insert("rest_api_features".into(), "all".into());
        c.insert("launch_process_timeout".into(), "6".into());
        c.insert("initdata_toml_path".into(), "".into());
        c.insert("attester_variant".into(), "".into());
        c.insert("extension_root".into(), "/run/kata-extensions/coco".into());
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
  env  = { LD_LIBRARY_PATH = "${extension_root}/usr/local/lib:/run/kata-extensions/gpu/usr/lib" }

[[process]]
id           = "confidential-data-hub"
level        = 2
path         = "usr/local/bin/confidential-data-hub"
config       = "${cdh_config_path}"
env          = { OCICRYPT_KEYPROVIDER_CONFIG = "${ocicrypt_config_path}", PATH = "${extension_root}/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin" }
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
    fn falls_back_to_legacy_when_extension_not_mounted() {
        let dir = tempdir().unwrap();
        let legacy = "/usr/local/bin/attestation-agent";
        let got = resolve_component_path_in_root(
            dir.path(),
            COCO_EXTENSION_NAME,
            COCO_COMPONENT_OCICRYPT_CONFIG,
            legacy,
        )
        .unwrap();
        assert_eq!(got, PathBuf::from(legacy));
    }

    #[test]
    fn launch_plan_is_none_when_extension_not_mounted() {
        let dir = tempdir().unwrap();
        let got = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 3, &ctx()).unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn fails_when_extension_root_exists_but_manifest_missing() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(COCO_EXTENSION_NAME)).unwrap();

        let err = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 3, &ctx()).unwrap_err();
        assert!(err.to_string().contains("manifest is missing"));
    }

    #[test]
    fn resolves_component_from_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_EXTENSION_NAME, COCO_MANIFEST);
        let got = resolve_component_path_in_root(
            dir.path(),
            COCO_EXTENSION_NAME,
            COCO_COMPONENT_PAUSE_BUNDLE,
            "/legacy/pause_bundle",
        )
        .unwrap();
        assert_eq!(
            got,
            dir.path().join(COCO_EXTENSION_NAME).join("pause_bundle")
        );
    }

    #[test]
    fn fails_when_component_is_missing_in_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            COCO_EXTENSION_NAME,
            r#"
schema_version = 1
[paths]
"pause-bundle" = "pause_bundle"
"#,
        );
        let err = resolve_component_path_in_root(
            dir.path(),
            COCO_EXTENSION_NAME,
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
            COCO_EXTENSION_NAME,
            r#"
schema_version = 1
[paths]
"ocicrypt-config" = "../outside"
"#,
        );
        let err = resolve_component_path_in_root(
            dir.path(),
            COCO_EXTENSION_NAME,
            COCO_COMPONENT_OCICRYPT_CONFIG,
            "/legacy",
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid path"));
    }

    #[test]
    fn builds_full_coco_launch_plan() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_EXTENSION_NAME, COCO_MANIFEST);
        let specs = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 3, &ctx())
            .unwrap()
            .unwrap();

        assert_eq!(specs.len(), 3);

        let root = dir.path().join(COCO_EXTENSION_NAME);
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
        // PATH puts the extension's cryptsetup (usr/sbin) ahead of the base dirs
        // that carry mke2fs/mkfs.ext4/dd, which CDH's secure_mount shells out to.
        assert_eq!(
            cdh.env,
            vec![
                (
                    "OCICRYPT_KEYPROVIDER_CONFIG".to_string(),
                    "/run/kata-extensions/coco/etc/ocicrypt_config.json".to_string()
                ),
                (
                    "PATH".to_string(),
                    "/run/kata-extensions/coco/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin".to_string()
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
        write_manifest(dir.path(), COCO_EXTENSION_NAME, COCO_MANIFEST);

        let specs = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 1, &ctx())
            .unwrap()
            .unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].id, "attestation-agent");
    }

    #[test]
    fn optional_args_included_when_variable_set() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_EXTENSION_NAME, COCO_MANIFEST);

        let mut ctx = ctx();
        ctx.insert(
            "initdata_toml_path".into(),
            "/run/initdata/initdata.toml".into(),
        );
        let specs = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 1, &ctx)
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
            COCO_EXTENSION_NAME,
            r#"
schema_version = 1
[[process]]
id    = "broken"
level = 1
path  = "usr/local/bin/broken"
args  = ["--flag", "${does_not_exist}"]
"#,
        );
        let err = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 1, &ctx()).unwrap_err();
        assert!(err.to_string().contains("unknown substitution variable"));
    }

    #[test]
    fn unsupported_schema_version_fails() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            COCO_EXTENSION_NAME,
            r#"
schema_version = 2
[paths]
"pause-bundle" = "pause_bundle"
"#,
        );
        let err = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 3, &ctx()).unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn default_variant_selected_when_selector_empty() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_EXTENSION_NAME, COCO_MANIFEST);

        // ctx() leaves attester_variant empty -> "default".
        let specs = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 1, &ctx())
            .unwrap()
            .unwrap();
        let aa = &specs[0];
        assert_eq!(
            aa.path,
            dir.path()
                .join(COCO_EXTENSION_NAME)
                .join("usr/local/bin/attestation-agent")
        );
        assert!(aa.env.is_empty());
    }

    #[test]
    fn nvidia_variant_selects_nv_binary_and_merges_env() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_EXTENSION_NAME, COCO_MANIFEST);

        let mut ctx = ctx();
        ctx.insert("attester_variant".into(), "nvidia".into());
        let specs = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 1, &ctx)
            .unwrap()
            .unwrap();
        let aa = &specs[0];
        assert_eq!(
            aa.path,
            dir.path()
                .join(COCO_EXTENSION_NAME)
                .join("usr/local/bin/attestation-agent-nv")
        );
        assert_eq!(
            aa.env,
            vec![(
                "LD_LIBRARY_PATH".to_string(),
                "/run/kata-extensions/coco/usr/local/lib:/run/kata-extensions/gpu/usr/lib"
                    .to_string()
            )]
        );
        // Base args are preserved for the selected variant.
        assert_eq!(aa.args, vec!["--attestation_sock", "unix:///run/aa.sock"]);
    }

    #[test]
    fn unknown_variant_fails() {
        let dir = tempdir().unwrap();
        write_manifest(dir.path(), COCO_EXTENSION_NAME, COCO_MANIFEST);

        let mut ctx = ctx();
        ctx.insert("attester_variant".into(), "does-not-exist".into());
        let err = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 1, &ctx).unwrap_err();
        assert!(err.to_string().contains("has no variant 'does-not-exist'"));
    }

    #[test]
    fn process_without_path_or_variants_fails() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            COCO_EXTENSION_NAME,
            r#"
schema_version = 1
[[process]]
id    = "broken"
level = 1
args  = ["--flag"]
"#,
        );
        let err = launch_plan_in_root(dir.path(), COCO_EXTENSION_NAME, 1, &ctx()).unwrap_err();
        assert!(err
            .to_string()
            .contains("neither 'path' nor a variant path"));
    }
}
