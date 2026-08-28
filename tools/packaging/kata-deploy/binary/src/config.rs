// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use log::info;
use std::env;
use std::fs;
use std::path::Path;

use crate::k8s;

/// K3s/RKE2 containerd config template filenames (under the mounted containerd dir).
/// `config-v3.toml.tmpl` is used when the rendered config uses split-CRI schema (containerd config version >= 3, including 4+).
/// `config.toml.tmpl` is for legacy CRI (version 2).
pub const K3S_RKE2_CONTAINERD_V3_TMPL: &str = "/etc/containerd/config-v3.toml.tmpl";
pub const K3S_RKE2_CONTAINERD_V2_TMPL: &str = "/etc/containerd/config.toml.tmpl";

/// Name of the nydus-snapshotter instance deployed and managed by kata-deploy for TEE workloads.
/// Used as the systemd service name, the containerd proxy plugin key, the runtime class
/// snapshotter field, and the base name for the data directory and socket path on the host.
pub const NYDUS_FOR_KATA_TEE: &str = "nydus-for-kata-tee";

/// Check if containerd config has an imports directive that would auto-load conf.d files.
/// Returns true if the config file has "imports = [...]" directive that includes /etc/containerd/conf.d.
fn config_has_containerd_confd_import(config_file: &str) -> bool {
    use crate::utils::toml as toml_utils;

    let has_conf_d_import = toml_utils::get_toml_array(Path::new(config_file), ".imports")
        .map(|imports| {
            imports
                .iter()
                .any(|path| path.contains("/etc/containerd/conf.d"))
        })
        .unwrap_or(false);

    if has_conf_d_import {
        info!(
            "Found imports directive with /etc/containerd/conf.d in {}, will use conf.d auto-loading",
            config_file
        );
    } else {
        info!(
            "No imports directive with /etc/containerd/conf.d in {}, will add it explicitly",
            config_file
        );
    }

    has_conf_d_import
}

/// Resolves whether to use the containerd 2.x split-CRI layout (true) or the v1 CRI gRPC layout (false) for K3s/RKE2.
/// 1. Tries config.toml: if it has `version = 2` use legacy CRI table; if `version >= 3` (including 4+) use split CRI.
/// 2. Else falls back to the node's containerRuntimeVersion (e.g. "containerd://2.1.5-k3s1").
/// 3. If neither is available, returns an error.
pub fn k3s_rke2_resolve_use_v3(
    config_file_path: &str,
    container_runtime_version: Option<&str>,
) -> Result<bool> {
    use crate::runtime::manager;
    use crate::utils::major_version_from_config_toml;

    // 1. Try config.toml (generated config that may already exist on the node)
    if let Ok(content) = fs::read_to_string(config_file_path) {
        if let Some(v) = major_version_from_config_toml(&content) {
            if v == 2 {
                return Ok(false);
            }
            if v >= 3 {
                return Ok(true);
            }
        }
    }

    // 2. Fall back to node's container runtime version
    if let Some(version) = container_runtime_version {
        return Ok(manager::containerd_version_is_2_or_newer(version));
    }

    // 3. Neither source available
    Err(anyhow::anyhow!(
        "K3s/RKE2: cannot determine containerd config version (v2 vs split-CRI). \
         Need version from {config_file_path} (version = 2 or >= 3) or node containerRuntimeVersion."
    ))
}

/// Returns the K3s/RKE2 containerd template path. Use v3 for containerd 2.x, v2 for 1.x.
pub fn k3s_rke2_containerd_template_path(use_v3: bool) -> &'static str {
    if use_v3 {
        K3S_RKE2_CONTAINERD_V3_TMPL
    } else {
        K3S_RKE2_CONTAINERD_V2_TMPL
    }
}

/// Returns the containerd CRI plugin ID for K3s/RKE2 (section key we write under).
/// Config v3 uses "io.containerd.cri.v1.runtime", v2 uses "io.containerd.grpc.v1.cri".
pub fn k3s_rke2_containerd_plugin_id(use_v3: bool) -> &'static str {
    if use_v3 {
        "\"io.containerd.cri.v1.runtime\""
    } else {
        "\"io.containerd.grpc.v1.cri\""
    }
}

/// K3s/RKE2: drop-in directory name in the rendered config (config.toml.d or config-v3.toml.d).
pub fn k3s_rke2_drop_in_dir_name(use_v3: bool) -> &'static str {
    if use_v3 {
        "config-v3.toml.d"
    } else {
        "config.toml.d"
    }
}

/// Path to the rendered containerd config.
/// K3s/RKE2 always render to config.toml regardless of which template
/// (config.toml.tmpl or config-v3.toml.tmpl) they use.
pub fn k3s_rke2_rendered_config_path() -> &'static str {
    "/etc/containerd/config.toml"
}

/// Returns true if the rendered config content imports the correct drop-in dir.
/// We only use k3s/rke2 drop-in when the distro has already configured this import.
pub fn k3s_rke2_rendered_has_import(content: &str, use_v3: bool) -> bool {
    content.contains(k3s_rke2_drop_in_dir_name(use_v3))
}

/// Default Kata Containers installation directory.
/// This is where Kata artifacts are installed by default.
pub const DEFAULT_KATA_INSTALL_DIR: &str = "/opt/kata";

/// Containerd configuration paths and capabilities for a specific runtime
#[derive(Debug, Clone)]
pub struct ContainerdPaths {
    /// File to read containerd version from and write to (non-drop-in mode)
    pub config_file: String,
    /// Backup file path before modification
    pub backup_file: String,
    /// File to add/remove drop-in imports from (drop-in mode)
    /// None if imports are not needed (e.g., k0s auto-loads from containerd.d/)
    pub imports_file: Option<String>,
    /// Path to the drop-in configuration file
    pub drop_in_file: String,
    /// Whether drop-in files can be used (based on containerd version)
    pub use_drop_in: bool,
    /// For K3s/RKE2: CRI plugin ID to use (derived from containerd version). Others: None (read from file).
    pub plugin_id: Option<String>,
}

/// Custom runtime configuration parsed from ConfigMap
#[derive(Debug, Clone)]
pub struct CustomRuntime {
    /// Handler name (e.g., "kata-my-custom-runtime")
    pub handler: String,
    /// Base configuration to copy (e.g., "qemu", "qemu-nvidia-gpu")
    pub base_config: String,
    /// Path to the drop-in file (if provided)
    pub drop_in_file: Option<String>,
    /// Containerd snapshotter to use (e.g., "nydus", "erofs")
    pub containerd_snapshotter: Option<String>,
    /// CRI-O pull type (e.g., "guest-pull")
    pub crio_pull_type: Option<String>,
    /// True for kata-deploy-synthesized debug variant runtimes (kata-<shim>-debug
    /// and kata-<shim>-devkit). Guest debug settings are applied only on these
    /// handlers.
    pub debug_variant: bool,
    /// True for the devkit runtime (kata-<shim>-devkit), which is a debug variant
    /// with an extra drop-in wiring the extension image and debug console shell.
    pub devkit: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub node_name: String,
    pub debug: bool,
    pub shims_for_arch: Vec<String>,
    pub default_shim_for_arch: String,
    pub allowed_hypervisor_annotations_for_arch: Vec<String>,
    pub snapshotter_handler_mapping_for_arch: Option<String>,
    pub agent_https_proxy: Option<String>,
    pub agent_no_proxy: Option<String>,
    /// Shims whose guests run the NVIDIA DCGM stack (nv-hostengine and
    /// dcgm-exporter). A set rather than a per-shim mapping: the setting is a
    /// boolean, so naming a shim here means it is on.
    pub nvrc_enable_dcgm: Vec<String>,
    pub pull_type_mapping_for_arch: Option<String>,
    pub installation_prefix: Option<String>,
    pub multi_install_suffix: Option<String>,
    pub helm_post_delete_hook: bool,
    pub experimental_setup_snapshotter: Option<Vec<String>>,
    /// EROFS snapshotter merge mode: "merged" (default) or "unmerged".
    ///
    /// In "unmerged" mode kata-deploy does not force containerd's erofs
    /// snapshotter to merge layers (it leaves `max_unmerged_layers` at the
    /// containerd default), so each image layer is exposed as its own
    /// per-layer `layer.erofs`. This is the only layout the Go runtime can
    /// consume; the merged (`fsmeta.erofs`) layout is runtime-rs only.
    pub erofs_merge_mode: Option<String>,
    pub experimental_force_guest_pull_for_arch: Vec<String>,
    pub dest_dir: String,
    pub host_install_dir: String,
    pub crio_drop_in_conf_dir: String,
    pub crio_drop_in_conf_file: String,
    pub crio_drop_in_conf_file_debug: String,
    pub containerd_conf_file: String,
    pub containerd_conf_file_backup: String,
    pub containerd_drop_in_conf_file: String,
    pub containerd_user_drop_in_source_file: Option<String>,
    pub daemonset_name: String,
    pub custom_runtimes_enabled: bool,
    pub custom_runtimes: Vec<CustomRuntime>,
    /// Install the devkit extension and a kata-<shim>-devkit custom runtime per
    /// enabled shim. From the DEVKIT env var, honored only when debug is also on:
    /// the devkit drop-in enables the agent debug console, which must never come
    /// up without debug.
    pub devkit_enabled: bool,
    /// EROFS snapshotter rw-layer backing mode ("disk" or "memory").
    pub erofs_snapshotter_mode: Option<String>,
    /// Enable dm-verity integrity for EROFS lower layers.
    /// Independent of rw-layer backing; works with both disk and memory modes.
    pub erofs_dmverity: bool,
    /// Startup taints to remove from the node once Kata is installed and the
    /// node has been labeled `katacontainers.io/kata-runtime=true`. Each entry
    /// is either a bare taint key (matches any effect) or `key:effect` (matches
    /// only that effect). Empty means "remove nothing" and is the default, so
    /// the behavior is opt-in and a no-op for users who don't configure it.
    ///
    /// This lets a node be provisioned with a startup taint that keeps Kata
    /// workloads from being scheduled before the runtime binaries exist; kata-deploy
    /// removes the taint as its final install step, closing the window in which a
    /// pod could land on a not-yet-ready node.
    pub startup_taints: Vec<String>,
    /// This node's `status.nodeInfo.containerRuntimeVersion`, supplied by
    /// whoever launched this process (`CONTAINER_RUNTIME_VERSION`).
    ///
    /// The job-mode dispatcher already holds every Node object it selected, so it
    /// passes this down to the per-node Job. That is what lets those Jobs run
    /// without a ServiceAccount token: they are the privileged, host-mutating part
    /// of the install, and the less they can reach the better. Absent (the
    /// DaemonSet), the value is read from the Node.
    pub container_runtime_version: Option<String>,
    /// The Kubernetes flavour the chart was configured for (`K8S_DISTRIBUTION`),
    /// which is what chose the host directory mounted at /etc/containerd.
    ///
    /// Absent when the operator pinned that directory themselves, or when this
    /// process was not started by the chart.
    pub k8s_distribution: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let arch = get_arch()?;
        let node_name =
            env::var("NODE_NAME").context("NODE_NAME environment variable is required")?;

        if node_name.trim().is_empty() {
            return Err(anyhow::anyhow!("NODE_NAME must not be empty"));
        }

        let daemonset_name = env::var("DAEMONSET_NAME")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "kata-deploy".to_string());

        let debug = env::var("DEBUG").unwrap_or_else(|_| "false".to_string()) == "true";

        // Parse shims - only use arch-specific variable
        // Use architecture-specific default shims list (only shims supported for this arch)
        let default_shims = get_default_shims_for_arch(&arch);
        let shims_for_arch: Vec<String> = get_arch_var("SHIMS", default_shims, &arch)
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let default_shim_for_arch =
            get_arch_var("DEFAULT_SHIM", get_default_shim_for_arch(&arch), &arch);

        // Only use arch-specific variable for allowed hypervisor annotations
        let allowed_hypervisor_annotations_for_arch =
            get_arch_var("ALLOWED_HYPERVISOR_ANNOTATIONS", "", &arch)
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

        // Only use arch-specific variable for snapshotter handler mapping
        let snapshotter_handler_mapping_for_arch =
            get_arch_var_or_base("SNAPSHOTTER_HANDLER_MAPPING", &arch);

        // Normalize empty strings to None at the boundary
        let agent_https_proxy = env::var("AGENT_HTTPS_PROXY").ok().filter(|s| !s.is_empty());
        let agent_no_proxy = env::var("AGENT_NO_PROXY").ok().filter(|s| !s.is_empty());

        let nvrc_enable_dcgm = env::var("NVRC_ENABLE_DCGM")
            .unwrap_or_default()
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let pull_type_mapping_for_arch = get_arch_var_or_base("PULL_TYPE_MAPPING", &arch);

        let installation_prefix = env::var("INSTALLATION_PREFIX")
            .ok()
            .filter(|s| !s.is_empty());
        let dest_dir = match installation_prefix {
            Some(ref prefix) => {
                if !prefix.starts_with('/') {
                    return Err(anyhow::anyhow!(
                        r#"INSTALLATION_PREFIX must begin with a "/" (ex. /hoge/fuga)"#
                    ));
                }
                format!("{prefix}{DEFAULT_KATA_INSTALL_DIR}")
            }
            None => DEFAULT_KATA_INSTALL_DIR.to_string(),
        };

        let multi_install_suffix = env::var("MULTI_INSTALL_SUFFIX").ok().and_then(|s| {
            if s.trim().is_empty() {
                None
            } else {
                Some(s)
            }
        });
        let dest_dir = if let Some(ref suffix) = multi_install_suffix {
            format!("{dest_dir}-{suffix}")
        } else {
            dest_dir
        };

        // Install dir is mounted into the container at the same absolute path as
        // on the host (see kata-deploy.installDir in the Helm chart).
        let host_install_dir = dest_dir.clone();

        let crio_drop_in_conf_dir = "/etc/crio/crio.conf.d/".to_string();
        let crio_drop_in_conf_file = if let Some(ref suffix) = multi_install_suffix {
            format!("{crio_drop_in_conf_dir}/99-kata-deploy-{suffix}")
        } else {
            format!("{crio_drop_in_conf_dir}/99-kata-deploy")
        };
        let crio_drop_in_conf_file_debug = format!("{crio_drop_in_conf_dir}/100-debug");

        let containerd_config_file_name = env::var("CONTAINERD_CONFIG_FILE_NAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "config.toml".to_string());
        let containerd_conf_path = Path::new("/etc/containerd").join(&containerd_config_file_name);
        if containerd_conf_path.parent() != Some(Path::new("/etc/containerd"))
            || containerd_conf_path.file_name() != Some(containerd_config_file_name.as_ref())
        {
            return Err(anyhow::anyhow!(
                "CONTAINERD_CONFIG_FILE_NAME must be a simple file name without path separators, \
                 got: '{containerd_config_file_name}'"
            ));
        }
        let containerd_conf_file = containerd_conf_path.to_string_lossy().to_string();
        let containerd_conf_file_backup = format!("{containerd_conf_file}.bak");
        let containerd_drop_in_conf_file =
            format!("{dest_dir}/containerd/config.d/kata-deploy.toml");
        let containerd_user_drop_in_source_file = env::var("CONTAINERD_USER_DROP_IN_SOURCE_FILE")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let helm_post_delete_hook =
            env::var("HELM_POST_DELETE_HOOK").unwrap_or_else(|_| "false".to_string()) == "true";

        let experimental_setup_snapshotter = env::var("EXPERIMENTAL_SETUP_SNAPSHOTTER")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

        let erofs_merge_mode = env::var("EROFS_MERGE_MODE")
            .ok()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());

        // Only use arch-specific variable for experimental force guest pull
        let experimental_force_guest_pull_for_arch =
            get_arch_var("EXPERIMENTAL_FORCE_GUEST_PULL", "", &arch)
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect();

        // Parse custom runtimes from ConfigMap
        let custom_runtimes_from_configmap =
            env::var("CUSTOM_RUNTIMES_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true";
        let mut custom_runtimes = if custom_runtimes_from_configmap {
            parse_custom_runtimes()?
        } else {
            Vec::new()
        };

        if debug {
            synthesize_variant_runtimes(
                Variant::Debug,
                &shims_for_arch,
                multi_install_suffix.as_deref(),
                snapshotter_handler_mapping_for_arch.as_deref(),
                pull_type_mapping_for_arch.as_deref(),
                &mut custom_runtimes,
            );
        }

        // Gate devkit on debug: the devkit drop-in turns on the agent debug
        // console, so honoring DEVKIT without DEBUG would silently enable it,
        // breaking the "only effective with debug" contract.
        let devkit_requested = env::var("DEVKIT").unwrap_or_else(|_| "false".to_string()) == "true";
        if devkit_requested && !debug {
            log::warn!("DEVKIT=true ignored: it requires DEBUG=true to take effect");
        }
        let devkit_enabled = devkit_requested && debug;
        if devkit_enabled {
            synthesize_variant_runtimes(
                Variant::Devkit,
                &shims_for_arch,
                multi_install_suffix.as_deref(),
                snapshotter_handler_mapping_for_arch.as_deref(),
                pull_type_mapping_for_arch.as_deref(),
                &mut custom_runtimes,
            );
        }

        // Enable the custom-runtime code paths if either source produced entries.
        let custom_runtimes_enabled = !custom_runtimes.is_empty();

        let erofs_snapshotter_mode = env::var("EROFS_SNAPSHOTTER_MODE")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let erofs_dmverity = env::var("EROFS_DMVERITY")
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case("dmverity");

        // Startup taints to remove after install+label. Comma- or whitespace-separated
        // list of `key` or `key:effect` entries. Empty/unset means "remove nothing".
        let startup_taints = env::var("STARTUP_TAINTS")
            .unwrap_or_default()
            .split([',', ' ', '\t', '\n'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Empty is treated as absent, so a template that renders the env var
        // unconditionally still means "look it up".
        let container_runtime_version = env::var("CONTAINER_RUNTIME_VERSION")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let k8s_distribution = env::var("K8S_DISTRIBUTION")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let config = Config {
            node_name,
            debug,
            shims_for_arch,
            default_shim_for_arch,
            allowed_hypervisor_annotations_for_arch,
            snapshotter_handler_mapping_for_arch,
            agent_https_proxy,
            agent_no_proxy,
            nvrc_enable_dcgm,
            pull_type_mapping_for_arch,
            installation_prefix,
            multi_install_suffix,
            helm_post_delete_hook,
            experimental_setup_snapshotter,
            erofs_merge_mode,
            experimental_force_guest_pull_for_arch,
            dest_dir,
            host_install_dir,
            crio_drop_in_conf_dir,
            crio_drop_in_conf_file,
            crio_drop_in_conf_file_debug,
            containerd_conf_file,
            containerd_conf_file_backup,
            containerd_drop_in_conf_file,
            containerd_user_drop_in_source_file,
            daemonset_name,
            custom_runtimes_enabled,
            custom_runtimes,
            devkit_enabled,
            erofs_snapshotter_mode,
            erofs_dmverity,
            startup_taints,
            container_runtime_version,
            k8s_distribution,
        };

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    /// The CRI runtime handlers this install writes for its shims.
    ///
    /// Custom runtimes and variants are left out: they are configured
    /// conditionally, so their absence from a runtime would say nothing.
    pub fn shim_handlers(&self) -> Vec<String> {
        self.shims_for_arch
            .iter()
            .map(|shim| shim_handler(shim, self.multi_install_suffix.as_deref()))
            .collect()
    }

    /// Validate configuration parameters
    ///
    /// All validations are performed on the `_for_arch` values, which are the final
    /// values after architecture-specific processing.
    fn validate(&self) -> Result<()> {
        // Must have either standard shims OR custom runtimes enabled
        let has_standard_shims = !self.shims_for_arch.is_empty();
        let has_custom_runtimes = self.custom_runtimes_enabled && !self.custom_runtimes.is_empty();

        if !has_standard_shims && !has_custom_runtimes {
            return Err(anyhow::anyhow!(
                "No runtimes configured. Please provide at least one shim via SHIMS \
                 or enable custom runtimes with CUSTOM_RUNTIMES_ENABLED=true"
            ));
        }

        // Check for empty shim names (only if we have standard shims)
        for shim in &self.shims_for_arch {
            if shim.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "SHIMS contains empty shim name. All shim names must be non-empty"
                ));
            }
        }

        // Validate DEFAULT_SHIM only if we have standard shims
        if has_standard_shims {
            if self.default_shim_for_arch.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "DEFAULT_SHIM for the current architecture must not be empty"
                ));
            }

            if !self.shims_for_arch.contains(&self.default_shim_for_arch) {
                return Err(anyhow::anyhow!(
                    "DEFAULT_SHIM '{}' must be one of the configured SHIMS for this architecture: [{}]",
                    self.default_shim_for_arch,
                    self.shims_for_arch.join(", ")
                ));
            }
        }

        // Validate ALLOWED_HYPERVISOR_ANNOTATIONS_FOR_ARCH shim-specific entries
        // These use the format "shim:annotation1,annotation2" or just "annotation"
        for annotation in &self.allowed_hypervisor_annotations_for_arch {
            if annotation.contains(':') {
                let parts: Vec<&str> = annotation.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let shim = parts[0].trim();
                    if !shim.is_empty() && !self.shims_for_arch.contains(&shim.to_string()) {
                        return Err(anyhow::anyhow!(
                            "ALLOWED_HYPERVISOR_ANNOTATIONS for current architecture references unknown shim '{}'. \
                             Valid shims: [{}]",
                            shim,
                            self.shims_for_arch.join(", ")
                        ));
                    }
                }
            }
        }

        // Validate AGENT_HTTPS_PROXY shim-specific mappings
        // Format: "shim1=proxy1;shim2=proxy2" or just "proxy_url"
        match self.agent_https_proxy.as_ref() {
            Some(proxy) if !proxy.is_empty() && proxy.contains('=') => {
                for mapping in proxy.split(';') {
                    let parts: Vec<&str> = mapping.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        let shim = parts[0].trim();
                        if !shim.is_empty() && !self.shims_for_arch.contains(&shim.to_string()) {
                            return Err(anyhow::anyhow!(
                                "AGENT_HTTPS_PROXY references unknown shim '{}'. \
                                 Valid shims for this architecture: [{}]",
                                shim,
                                self.shims_for_arch.join(", ")
                            ));
                        }
                    }
                }
            }
            _ => {}
        }

        // Validate AGENT_NO_PROXY shim-specific mappings
        // Format: "shim1=noproxy1;shim2=noproxy2" or just "noproxy_list"
        match self.agent_no_proxy.as_ref() {
            Some(no_proxy) if !no_proxy.is_empty() && no_proxy.contains('=') => {
                for mapping in no_proxy.split(';') {
                    let parts: Vec<&str> = mapping.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        let shim = parts[0].trim();
                        if !shim.is_empty() && !self.shims_for_arch.contains(&shim.to_string()) {
                            return Err(anyhow::anyhow!(
                                "AGENT_NO_PROXY references unknown shim '{}'. \
                                 Valid shims for this architecture: [{}]",
                                shim,
                                self.shims_for_arch.join(", ")
                            ));
                        }
                    }
                }
            }
            _ => {}
        }

        // Validate NVRC_ENABLE_DCGM
        // Format: "shim1;shim2", naming the shims whose guests run DCGM.
        //
        // Only the NVIDIA GPU images carry DCGM and an NVRC to start it, so
        // nvrc.dcgm=on anywhere else is a setting that would silently do
        // nothing. Membership in shims_for_arch is deliberately not required:
        // the chart builds this list from every enabled shim, and the same
        // value reaches nodes of every architecture, so a GPU shim missing here
        // means "not on this node" rather than a mistake.
        for shim in &self.nvrc_enable_dcgm {
            if !shim.contains("nvidia-gpu") {
                return Err(anyhow::anyhow!(
                    "NVRC_ENABLE_DCGM references '{}', which is not an NVIDIA GPU shim. \
                     DCGM only runs in the NVIDIA GPU guest images.",
                    shim
                ));
            }
        }

        // Validate SNAPSHOTTER_HANDLER_MAPPING_FOR_ARCH
        // Format: "shim1:snapshotter1,shim2:snapshotter2"
        match self.snapshotter_handler_mapping_for_arch.as_ref() {
            Some(mapping) if !mapping.is_empty() => {
                for m in mapping.split(',') {
                    let parts: Vec<&str> = m.split(':').collect();
                    if parts.len() == 2 {
                        let shim = parts[0].trim();
                        if !shim.is_empty() && !self.shims_for_arch.contains(&shim.to_string()) {
                            return Err(anyhow::anyhow!(
                                "SNAPSHOTTER_HANDLER_MAPPING for current architecture references unknown shim '{}'. \
                                 Valid shims: [{}]",
                                shim,
                                self.shims_for_arch.join(", ")
                            ));
                        }
                    }
                }
            }
            _ => {}
        }

        // Validate PULL_TYPE_MAPPING_FOR_ARCH
        // Format: "shim1:pull_type1,shim2:pull_type2"
        match self.pull_type_mapping_for_arch.as_ref() {
            Some(mapping) if !mapping.is_empty() => {
                for m in mapping.split(',') {
                    let parts: Vec<&str> = m.split(':').collect();
                    if parts.len() == 2 {
                        let shim = parts[0].trim();
                        if !shim.is_empty() && !self.shims_for_arch.contains(&shim.to_string()) {
                            return Err(anyhow::anyhow!(
                                "PULL_TYPE_MAPPING for current architecture references unknown shim '{}'. \
                                 Valid shims: [{}]",
                                shim,
                                self.shims_for_arch.join(", ")
                            ));
                        }
                    }
                }
            }
            _ => {}
        }

        // Validate EROFS_MERGE_MODE
        // Only "merged" (default) and "unmerged" are accepted.
        if let Some(mode) = self.erofs_merge_mode.as_ref() {
            if mode != "merged" && mode != "unmerged" {
                return Err(anyhow::anyhow!(
                    "EROFS_MERGE_MODE must be either 'merged' or 'unmerged', got '{}'",
                    mode
                ));
            }
        }

        // Validate EXPERIMENTAL_FORCE_GUEST_PULL_FOR_ARCH
        // This is a list of shim names
        for shim in &self.experimental_force_guest_pull_for_arch {
            if !shim.trim().is_empty() && !self.shims_for_arch.contains(shim) {
                return Err(anyhow::anyhow!(
                    "EXPERIMENTAL_FORCE_GUEST_PULL for current architecture references unknown shim '{}'. \
                     Valid shims: [{}]",
                    shim,
                    self.shims_for_arch.join(", ")
                ));
            }
        }

        // Validate EROFS_SNAPSHOTTER_MODE.
        if let Some(mode) = self.erofs_snapshotter_mode.as_ref() {
            match mode.as_str() {
                "disk" | "memory" => {}
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported EROFS_SNAPSHOTTER_MODE: '{}'. Supported values: disk, memory",
                        mode
                    ));
                }
            }
        }

        Ok(())
    }

    /// `full` prints the resolved configuration too. See its caller for why the
    /// later stages of a staged run leave it out.
    pub fn print_info(&self, action: &str, full: bool) {
        if !full {
            info!("Action: {action}");
            return;
        }

        info!("Action:");
        info!("* {action}");
        info!("");
        info!("Environment variables passed to this script");
        info!("* NODE_NAME: {}", self.node_name);
        info!("* DEBUG: {}", self.debug);
        info!("* SHIMS: {}", self.shims_for_arch.join(" "));
        info!("* DEFAULT_SHIM: {}", self.default_shim_for_arch);
        info!(
            "* ALLOWED_HYPERVISOR_ANNOTATIONS: {}",
            self.allowed_hypervisor_annotations_for_arch.join(" ")
        );
        info!(
            "* SNAPSHOTTER_HANDLER_MAPPING: {:?}",
            self.snapshotter_handler_mapping_for_arch
        );
        info!("* AGENT_HTTPS_PROXY: {:?}", self.agent_https_proxy);
        info!("* AGENT_NO_PROXY: {:?}", self.agent_no_proxy);
        info!("* NVRC_ENABLE_DCGM: {:?}", self.nvrc_enable_dcgm);
        info!("* PULL_TYPE_MAPPING: {:?}", self.pull_type_mapping_for_arch);
        info!("* INSTALLATION_PREFIX: {:?}", self.installation_prefix);
        info!("* MULTI_INSTALL_SUFFIX: {:?}", self.multi_install_suffix);
        info!("* HELM_POST_DELETE_HOOK: {}", self.helm_post_delete_hook);
        info!(
            "* EXPERIMENTAL_SETUP_SNAPSHOTTER: {:?}",
            self.experimental_setup_snapshotter
        );
        info!("* EROFS_MERGE_MODE: {:?}", self.erofs_merge_mode);
        info!(
            "* EROFS_SNAPSHOTTER_MODE: {:?}",
            self.erofs_snapshotter_mode
        );
        info!("* EROFS_DMVERITY: {}", self.erofs_dmverity);
        info!(
            "* EXPERIMENTAL_FORCE_GUEST_PULL: {}",
            self.experimental_force_guest_pull_for_arch.join(",")
        );
        info!("* CONTAINERD_CONF_FILE: {}", self.containerd_conf_file);
        info!(
            "* CONTAINERD_USER_DROP_IN_SOURCE_FILE: {:?}",
            self.containerd_user_drop_in_source_file
        );
        info!(
            "* CUSTOM_RUNTIMES_ENABLED: {}",
            self.custom_runtimes_enabled
        );
        info!("* STARTUP_TAINTS: {}", self.startup_taints.join(" "));
        if !self.custom_runtimes.is_empty() {
            info!("* CUSTOM_RUNTIMES:");
            for runtime in &self.custom_runtimes {
                info!(
                    "  - {}: base_config={}, drop_in={}, containerd_snapshotter={:?}, crio_pull_type={:?}",
                    runtime.handler,
                    runtime.base_config,
                    runtime.drop_in_file.is_some(),
                    runtime.containerd_snapshotter,
                    runtime.crio_pull_type
                );
            }
        }

        log::debug!("Resolved kata-deploy configuration:\n{:#?}", self);
    }

    /// This node's container runtime version, e.g. `containerd://2.1.5-k3s1`.
    ///
    /// Every caller goes through here so there is a single place where the value
    /// can come from the environment (job mode, where the dispatcher passes it in
    /// and the pod holds no credentials) instead of from the Node object.
    pub async fn resolve_container_runtime_version(&self) -> Result<String> {
        match self.container_runtime_version.as_deref() {
            Some(version) => Ok(version.to_string()),
            None => k8s::get_container_runtime_version(self).await.context(
                "could not read this node's container runtime version from the apiserver. In job \
                 mode the dispatcher passes it in CONTAINER_RUNTIME_VERSION precisely because \
                 these pods hold no credentials, so reaching this means it did not",
            ),
        }
    }

    /// Get containerd configuration file paths based on runtime type and containerd version
    pub async fn get_containerd_paths(&self, runtime: &str) -> Result<ContainerdPaths> {
        use crate::runtime::manager;

        // Get containerd version once for drop-in and conf.d capability checks.
        // Not required for k0s (drop-ins are always supported there).
        let container_runtime_version = if matches!(runtime, "k0s-worker" | "k0s-controller") {
            None
        } else {
            Some(self.resolve_container_runtime_version().await?)
        };
        let use_drop_in = manager::is_containerd_capable_of_drop_in(
            runtime,
            container_runtime_version.as_deref(),
        );

        let paths = match runtime {
            "k0s-worker" | "k0s-controller" => ContainerdPaths {
                config_file: "/etc/containerd/containerd.toml".to_string(),
                backup_file: "/etc/containerd/containerd.toml.bak".to_string(), // Never used, but needed for consistency
                imports_file: None, // k0s auto-loads from containerd.d/, imports not needed
                drop_in_file: "/etc/containerd/containerd.d/kata-deploy.toml".to_string(),
                use_drop_in,
                plugin_id: None,
            },
            "microk8s" => ContainerdPaths {
                // microk8s uses containerd-template.toml instead of config.toml
                config_file: "/etc/containerd/containerd-template.toml".to_string(),
                backup_file: "/etc/containerd/containerd-template.toml.bak".to_string(),
                imports_file: Some("/etc/containerd/containerd-template.toml".to_string()),
                drop_in_file: self.containerd_drop_in_conf_file.clone(),
                use_drop_in,
                plugin_id: None,
            },
            "k3s" | "k3s-agent" | "rke2-agent" | "rke2-server" => {
                // K3s/RKE2: we only use drop-in when the rendered config already imports the
                // versioned drop-in dir (config.toml.d or config-v3.toml.d). If the import is
                // missing we bail; the cluster must configure the template with the import
                // (e.g. in tests or via a custom k3s/RKE2 setup). Refs: docs.k3s.io/advanced#configuring-containerd
                let use_v3 = k3s_rke2_resolve_use_v3(
                    k3s_rke2_rendered_config_path(),
                    container_runtime_version.as_deref(),
                )?;
                let config_file = k3s_rke2_containerd_template_path(use_v3).to_string();
                let rendered_path = k3s_rke2_rendered_config_path().to_string();
                let content = fs::read_to_string(&rendered_path).with_context(|| {
                    format!(
                        "K3s/RKE2: cannot read rendered config at {rendered_path}. \
                         Ensure the containerd config dir is mounted and k3s/RKE2 has rendered the config."
                    )
                })?;
                if !k3s_rke2_rendered_has_import(&content, use_v3) {
                    anyhow::bail!(
                        "K3s/RKE2: rendered config at {} does not import the drop-in dir '{}'. \
                         kata-deploy requires the containerd template to include that import. \
                         Add e.g. imports = [\".../{}/*.toml\"] to the template and restart k3s/RKE2.",
                        rendered_path,
                        k3s_rke2_drop_in_dir_name(use_v3),
                        k3s_rke2_drop_in_dir_name(use_v3),
                    );
                }
                let template_dir = Path::new(&config_file)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/etc/containerd".to_string());
                let drop_in_file = format!(
                    "{}/{}/kata-deploy.toml",
                    template_dir,
                    k3s_rke2_drop_in_dir_name(use_v3),
                );
                let backup_file = format!("{config_file}.bak");
                ContainerdPaths {
                    config_file: config_file.clone(),
                    backup_file,
                    imports_file: None, // we do not modify the template; import is already there
                    drop_in_file,
                    use_drop_in: true,
                    plugin_id: Some(k3s_rke2_containerd_plugin_id(use_v3).to_string()),
                }
            }
            _ => {
                // For containerd >= 2.2.0, use /etc/containerd/conf.d/ which is auto-imported
                // by containerd, avoiding the need to modify the main config entirely.
                // Check if the config actually has imports before skipping adding it explicitly.
                let supports_conf_d = container_runtime_version
                    .as_deref()
                    .map(|v| {
                        manager::containerd_version_is_2_2_or_newer(v)
                            && config_has_containerd_confd_import(&self.containerd_conf_file)
                    })
                    .unwrap_or(false);

                let (imports_file, drop_in_file) = if supports_conf_d {
                    let drop_in = if let Some(ref suffix) = self.multi_install_suffix {
                        format!("/etc/containerd/conf.d/kata-deploy-{suffix}.toml")
                    } else {
                        "/etc/containerd/conf.d/kata-deploy.toml".to_string()
                    };
                    (None, drop_in)
                } else {
                    (
                        Some(self.containerd_conf_file.clone()),
                        self.containerd_drop_in_conf_file.clone(),
                    )
                };

                ContainerdPaths {
                    config_file: self.containerd_conf_file.clone(),
                    backup_file: self.containerd_conf_file_backup.clone(),
                    imports_file,
                    drop_in_file,
                    use_drop_in,
                    plugin_id: None,
                }
            }
        };

        Ok(paths)
    }
}

fn get_arch() -> Result<String> {
    let arch = std::env::consts::ARCH;
    Ok(match arch {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        "s390x" => "s390x",
        // Rust's std::env::consts::ARCH returns "powerpc64" for both big and little endian.
        // Kata Containers only supports ppc64le (little-endian).
        "powerpc64" => "ppc64le",
        _ => arch,
    }
    .to_string())
}

/// Parse custom runtimes from the mounted ConfigMap at /custom-configs/
/// Reads the custom-runtimes.list file which contains entries in the format:
/// handler:baseConfig:containerd_snapshotter:crio_pulltype
/// Optionally reads drop-in files named dropin-{handler}.toml
fn parse_custom_runtimes() -> Result<Vec<CustomRuntime>> {
    let custom_configs_dir = "/custom-configs";
    let list_file = format!("{}/custom-runtimes.list", custom_configs_dir);

    let list_content = match std::fs::read_to_string(&list_file) {
        Ok(content) => content,
        Err(e) => {
            log::warn!(
                "Could not read custom runtimes list at {}: {}",
                list_file,
                e
            );
            return Ok(Vec::new());
        }
    };

    let mut custom_runtimes = Vec::new();
    for line in list_content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse format: handler:baseConfig:containerd_snapshotter:crio_pulltype
        let parts: Vec<&str> = line.split(':').collect();
        let handler = parts.first().map(|s| s.trim()).unwrap_or("");
        if handler.is_empty() {
            continue;
        }

        let base_config = parts.get(1).map(|s| s.trim()).unwrap_or("");
        if base_config.is_empty() {
            anyhow::bail!(
                "Custom runtime '{}' missing required baseConfig field",
                handler
            );
        }

        let containerd_snapshotter = parts
            .get(2)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let crio_pull_type = parts
            .get(3)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // Check for optional drop-in file
        let drop_in_file_path = format!("{}/dropin-{}.toml", custom_configs_dir, handler);
        let drop_in_file = if std::path::Path::new(&drop_in_file_path).exists() {
            Some(drop_in_file_path)
        } else {
            None
        };

        log::info!(
            "Found custom runtime: handler={}, base_config={}, drop_in={:?}, containerd_snapshotter={:?}, crio_pull_type={:?}",
            handler,
            base_config,
            drop_in_file.is_some(),
            containerd_snapshotter,
            crio_pull_type
        );

        custom_runtimes.push(CustomRuntime {
            handler: handler.to_string(),
            base_config: base_config.to_string(),
            drop_in_file,
            containerd_snapshotter,
            crio_pull_type,
            debug_variant: false,
            devkit: false,
        });
    }

    log::info!(
        "Parsed {} custom runtime(s) from {}",
        custom_runtimes.len(),
        list_file
    );
    Ok(custom_runtimes)
}

/// Look up `shim`'s value in a comma-separated "shim1:value1,shim2:value2"
/// mapping (SNAPSHOTTER_HANDLER_MAPPING, PULL_TYPE_MAPPING). None if absent or
/// empty.
fn lookup_mapping_value(mapping: &str, shim: &str) -> Option<String> {
    mapping.split(',').find_map(|entry| {
        let parts: Vec<&str> = entry.split(':').collect();
        if parts.len() == 2 && parts[0].trim() == shim {
            let value = parts[1].trim();
            (!value.is_empty()).then(|| value.to_string())
        } else {
            None
        }
    })
}

/// A RuntimeClass kata-deploy synthesizes for a shim, in addition to the plain
/// one. Both carry guest debug configuration, which is what keeps it off the
/// plain RuntimeClass and its kernel cmdline stable for measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// kata-<shim>-debug: guest debug configuration.
    Debug,
    /// kata-<shim>-devkit: the same, plus the devkit extension image and the
    /// debug console shell it carries.
    Devkit,
}

impl Variant {
    /// Suffix distinguishing the variant's handler from the plain one.
    fn suffix(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Devkit => "devkit",
        }
    }
}

/// The handler name, and hence the RuntimeClass name, of a shim.
///
/// In one place because it is both written into the CRI configuration and read
/// back out of a running CRI to confirm that configuration was loaded.
pub fn shim_handler(shim: &str, multi_install_suffix: Option<&str>) -> String {
    match multi_install_suffix {
        Some(install_suffix) if !install_suffix.is_empty() => {
            format!("kata-{shim}-{install_suffix}")
        }
        _ => format!("kata-{shim}"),
    }
}

/// The handler name, and hence the RuntimeClass name, of a shim's variant.
///
/// In one place because the cleanup of stale variants has to look for exactly
/// the names the synthesis below produced, multi-install suffix and all.
pub fn variant_handler(shim: &str, multi_install_suffix: Option<&str>, variant: Variant) -> String {
    let suffix = variant.suffix();
    match multi_install_suffix {
        Some(install_suffix) if !install_suffix.is_empty() => {
            format!("kata-{shim}-{install_suffix}-{suffix}")
        }
        _ => format!("kata-{shim}-{suffix}"),
    }
}

/// Append a variant custom runtime per enabled shim, modelling them as custom
/// runtimes so they reuse the whole install/register/cleanup machinery: all
/// that distinguishes them is which drop-ins install.rs writes.
///
/// The handler is derived from the standard runtime name (kata-<shim>, or
/// kata-<shim>-<suffix> under a multi-install) so concurrent kata-deploy
/// instances on the same node do not fight over one variant handler.
fn synthesize_variant_runtimes(
    variant: Variant,
    shims_for_arch: &[String],
    multi_install_suffix: Option<&str>,
    snapshotter_handler_mapping_for_arch: Option<&str>,
    pull_type_mapping_for_arch: Option<&str>,
    custom_runtimes: &mut Vec<CustomRuntime>,
) {
    let suffix = variant.suffix();

    for shim in shims_for_arch {
        let handler = variant_handler(shim, multi_install_suffix, variant);
        if custom_runtimes.iter().any(|r| r.handler == handler) {
            continue;
        }

        // Inherit the base shim's image-pulling config: the variant RuntimeClass
        // shares the base config (same shared_fs), so it must pull the same way.
        // Otherwise containerd/CRI-O default to overlayfs/node-pull, which breaks
        // shims running shared_fs = none (e.g. nvidia runtime-rs) and makes every
        // pod on the variant terminate.
        let containerd_snapshotter = snapshotter_handler_mapping_for_arch
            .and_then(|mapping| lookup_mapping_value(mapping, shim));
        let crio_pull_type =
            pull_type_mapping_for_arch.and_then(|mapping| lookup_mapping_value(mapping, shim));

        log::info!(
            "Synthesizing {} variant runtime: handler={}, base_config={}",
            suffix,
            handler,
            shim
        );

        custom_runtimes.push(CustomRuntime {
            handler,
            base_config: shim.clone(),
            drop_in_file: None,
            containerd_snapshotter,
            crio_pull_type,
            debug_variant: true,
            devkit: variant == Variant::Devkit,
        });
    }
}

/// Get default shims list for a specific architecture
/// Returns only shims that are supported for that architecture
fn get_default_shims_for_arch(arch: &str) -> &'static str {
    match arch {
        "x86_64" => "clh clh-runtime-rs dragonball fc qemu qemu-coco-dev qemu-coco-dev-runtime-rs qemu-runtime-rs qemu-nvidia-cpu qemu-nvidia-cpu-runtime-rs qemu-nvidia-gpu qemu-nvidia-gpu-runtime-rs qemu-nvidia-gpu-snp qemu-nvidia-gpu-snp-runtime-rs qemu-nvidia-gpu-tdx qemu-nvidia-gpu-tdx-runtime-rs qemu-snp qemu-snp-runtime-rs qemu-tdx qemu-tdx-runtime-rs",
        "aarch64" => "clh clh-runtime-rs dragonball fc qemu qemu-coco-dev-runtime-rs qemu-runtime-rs qemu-nvidia-cpu qemu-nvidia-cpu-runtime-rs qemu-nvidia-gpu",
        "s390x" => "qemu qemu-runtime-rs qemu-se qemu-se-runtime-rs qemu-coco-dev qemu-coco-dev-runtime-rs",
        "ppc64le" => "qemu qemu-runtime-rs",
        _ => "qemu", // Fallback to qemu for unknown architectures
    }
}

/// Get the default shim for a specific architecture.
///
/// Since the Kata Containers 4.0 release, the Rust runtime (runtime-rs,
/// "qemu-runtime-rs") is the default wherever a runtime-rs build exists.
/// This only acts as a fallback: the Helm chart normally provides DEFAULT_SHIM
/// explicitly via values.yaml (`defaultShim`).
fn get_default_shim_for_arch(arch: &str) -> &'static str {
    match arch {
        "x86_64" | "aarch64" | "s390x" | "ppc64le" => "qemu-runtime-rs",
        _ => "qemu", // Fallback to the Go runtime for unknown architectures
    }
}

/// Get architecture-specific variable (e.g., SHIMS_X86_64)
/// Falls back to provided default if arch-specific variable is not found or empty
fn get_arch_var(base_name: &str, default: &str, arch: &str) -> String {
    get_arch_var_or_base(base_name, arch).unwrap_or_else(|| default.to_string())
}

/// Get architecture-specific variable (e.g., SHIMS_X86_64)
/// Returns None if the arch-specific variable does not exist or is empty
/// Empty strings are normalized to None for consistent Option semantics
fn get_arch_var_or_base(base_name: &str, arch: &str) -> Option<String> {
    let arch_suffix = match arch {
        "x86_64" => "_X86_64",
        "aarch64" => "_AARCH64",
        "s390x" => "_S390X",
        "ppc64le" => "_PPC64LE",
        _ => return None,
    };

    let arch_var = format!("{base_name}{arch_suffix}");
    env::var(&arch_var).ok().filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    //! Tests for configuration parsing and validation.
    //!
    //! Tests that touch environment variables use `serial_test::serial` so they do not run
    //! in parallel within this process. For extra isolation you can still use
    //! `cargo test -p kata-deploy config::tests -- --test-threads=1`.

    use super::*;
    use rstest::rstest;
    use serial_test::serial;

    // NOTE: Env-var tests use #[serial] (see above) for safe parallel execution with other modules.

    /// Helper to clean up common environment variables used in tests
    fn cleanup_env_vars() {
        let vars = [
            "MULTI_INSTALL_SUFFIX",
            "INSTALLATION_PREFIX",
            "NODE_NAME",
            "DEBUG",
            "SHIMS",
            "SHIMS_X86_64",
            "SHIMS_AARCH64",
            "SHIMS_S390X",
            "SHIMS_PPC64LE",
            "DEFAULT_SHIM",
            "DEFAULT_SHIM_X86_64",
            "DEFAULT_SHIM_AARCH64",
            "DEFAULT_SHIM_S390X",
            "DEFAULT_SHIM_PPC64LE",
            "ALLOWED_HYPERVISOR_ANNOTATIONS",
            "ALLOWED_HYPERVISOR_ANNOTATIONS_X86_64",
            "ALLOWED_HYPERVISOR_ANNOTATIONS_AARCH64",
            "ALLOWED_HYPERVISOR_ANNOTATIONS_S390X",
            "ALLOWED_HYPERVISOR_ANNOTATIONS_PPC64LE",
            "AGENT_HTTPS_PROXY",
            "AGENT_NO_PROXY",
            "NVRC_ENABLE_DCGM",
            "SNAPSHOTTER_HANDLER_MAPPING",
            "SNAPSHOTTER_HANDLER_MAPPING_X86_64",
            "SNAPSHOTTER_HANDLER_MAPPING_AARCH64",
            "SNAPSHOTTER_HANDLER_MAPPING_S390X",
            "SNAPSHOTTER_HANDLER_MAPPING_PPC64LE",
            "PULL_TYPE_MAPPING",
            "PULL_TYPE_MAPPING_X86_64",
            "PULL_TYPE_MAPPING_AARCH64",
            "PULL_TYPE_MAPPING_S390X",
            "PULL_TYPE_MAPPING_PPC64LE",
            "EXPERIMENTAL_FORCE_GUEST_PULL",
            "EXPERIMENTAL_FORCE_GUEST_PULL_X86_64",
            "EXPERIMENTAL_FORCE_GUEST_PULL_AARCH64",
            "EXPERIMENTAL_FORCE_GUEST_PULL_S390X",
            "EXPERIMENTAL_FORCE_GUEST_PULL_PPC64LE",
            "CONTAINERD_CONFIG_FILE_NAME",
            "STARTUP_TAINTS",
            "CUSTOM_RUNTIMES_ENABLED",
            "DEVKIT",
        ];
        for var in &vars {
            std::env::remove_var(var);
        }
    }

    /// Helper to set up minimal valid config environment
    /// Always cleans up first to ensure test isolation
    fn setup_minimal_env() {
        cleanup_env_vars();
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("DEBUG", "false");

        // Set arch-specific variables based on current architecture
        let arch = get_arch().unwrap();
        let arch_suffix = match arch.as_str() {
            "x86_64" => "_X86_64",
            "aarch64" => "_AARCH64",
            "s390x" => "_S390X",
            "ppc64le" => "_PPC64LE",
            _ => "",
        };

        if !arch_suffix.is_empty() {
            std::env::set_var(format!("SHIMS{}", arch_suffix), "qemu");
            std::env::set_var(format!("DEFAULT_SHIM{}", arch_suffix), "qemu");
        }
    }

    /// Helper to set an arch-specific environment variable for testing
    fn set_arch_var(base_name: &str, value: &str) {
        let arch = get_arch().unwrap();
        let arch_suffix = match arch.as_str() {
            "x86_64" => "_X86_64",
            "aarch64" => "_AARCH64",
            "s390x" => "_S390X",
            "ppc64le" => "_PPC64LE",
            _ => "",
        };

        if !arch_suffix.is_empty() {
            std::env::set_var(format!("{}{}", base_name, arch_suffix), value);
        }
    }

    /// Helper to test that Config::from_env() fails with expected error message
    fn assert_config_error_contains(expected_msg: &str) {
        let result = Config::from_env();
        assert!(result.is_err(), "Expected error but got Ok");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains(expected_msg),
            "Error message '{}' does not contain '{}'",
            err_msg,
            expected_msg
        );
    }

    #[serial]
    #[test]
    fn test_get_arch() {
        let arch = get_arch().unwrap();
        assert!(!arch.is_empty());
        cleanup_env_vars();
    }

    #[rstest]
    #[case("x86_64", "qemu-runtime-rs")]
    #[case("aarch64", "qemu-runtime-rs")]
    #[case("s390x", "qemu-runtime-rs")]
    #[case("ppc64le", "qemu-runtime-rs")]
    #[case("riscv64", "qemu")]
    fn test_get_default_shim_for_arch(#[case] arch: &str, #[case] expected: &str) {
        assert_eq!(get_default_shim_for_arch(arch), expected);
    }

    #[serial]
    #[test]
    fn test_get_arch_var() {
        std::env::set_var("SHIMS_X86_64", "test1 test2");
        let result = get_arch_var("SHIMS", "default", "x86_64");
        assert_eq!(result, "test1 test2");
        cleanup_env_vars();
    }

    // --- k3s/rke2 helpers (no env vars) ---

    #[rstest]
    #[case(false, "config.toml.d")]
    #[case(true, "config-v3.toml.d")]
    #[serial]
    fn test_k3s_rke2_drop_in_dir_name(#[case] use_v3: bool, #[case] expected: &str) {
        assert_eq!(k3s_rke2_drop_in_dir_name(use_v3), expected);
    }

    #[serial]
    #[test]
    fn test_k3s_rke2_rendered_config_path() {
        assert_eq!(
            k3s_rke2_rendered_config_path(),
            "/etc/containerd/config.toml"
        );
    }

    #[serial]
    #[test]
    fn test_k3s_rke2_resolve_use_v3_from_config_version_4_without_node_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "version = 4\n").unwrap();
        assert!(k3s_rke2_resolve_use_v3(path.to_str().unwrap(), None).unwrap());
    }

    #[rstest]
    #[case(
        "imports = [\"/var/lib/rancher/k3s/agent/etc/containerd/config.toml.d/*.toml\"]\n",
        false,
        true
    )]
    #[case("version = 2\n", false, false)]
    #[case("imports = [\"/path/config-v3.toml.d/*.toml\"]", true, true)]
    #[case("imports = [\"/path/config.toml.d/*.toml\"]", true, false)]
    #[serial]
    fn test_k3s_rke2_rendered_has_import(
        #[case] content: &str,
        #[case] use_v3: bool,
        #[case] expected: bool,
    ) {
        assert_eq!(k3s_rke2_rendered_has_import(content, use_v3), expected);
    }

    #[serial]
    #[test]
    fn test_multi_install_suffix_not_set() {
        setup_minimal_env();

        let config = Config::from_env().unwrap();

        assert_eq!(config.multi_install_suffix, None);
        assert!(config.dest_dir.ends_with("/opt/kata"));
        assert_eq!(
            config.crio_drop_in_conf_file,
            "/etc/crio/crio.conf.d//99-kata-deploy"
        );

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_devkit_disabled_by_default() {
        setup_minimal_env();

        let config = Config::from_env().unwrap();

        assert!(!config.devkit_enabled);
        assert!(config.custom_runtimes.iter().all(|r| !r.devkit));
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_devkit_requires_debug() {
        setup_minimal_env();
        // DEBUG defaults to false in setup_minimal_env.
        std::env::set_var("DEVKIT", "true");

        let config = Config::from_env().unwrap();

        assert!(!config.devkit_enabled);
        assert!(config.custom_runtimes.iter().all(|r| !r.devkit));
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_devkit_synthesizes_per_shim_runtime() {
        setup_minimal_env();
        std::env::set_var("DEBUG", "true");
        std::env::set_var("DEVKIT", "true");

        let config = Config::from_env().unwrap();

        assert!(config.devkit_enabled);
        assert!(config.custom_runtimes_enabled);

        // setup_minimal_env configures a single "qemu" shim for this arch.
        let devkit: Vec<_> = config.custom_runtimes.iter().filter(|r| r.devkit).collect();
        assert_eq!(devkit.len(), 1);
        assert_eq!(devkit[0].handler, "kata-qemu-devkit");
        assert_eq!(devkit[0].base_config, "qemu");
        assert!(devkit[0].drop_in_file.is_none());
        // With no snapshotter/pull-type mapping, the devkit runtime inherits
        // nothing and containerd/CRI-O use their defaults for the base shim.
        assert!(devkit[0].containerd_snapshotter.is_none());
        assert!(devkit[0].crio_pull_type.is_none());

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_devkit_inherits_base_shim_snapshotter_and_pull_type() {
        setup_minimal_env();
        std::env::set_var("DEBUG", "true");
        std::env::set_var("DEVKIT", "true");
        // The base "qemu" shim is mapped to the erofs snapshotter / guest-pull;
        // its devkit variant must inherit both so pods on the devkit
        // RuntimeClass pull images the same way as the base shim (crucial for
        // shims running with shared_fs = none).
        set_arch_var("SNAPSHOTTER_HANDLER_MAPPING", "qemu:erofs");
        set_arch_var("PULL_TYPE_MAPPING", "qemu:guest-pull");

        let config = Config::from_env().unwrap();

        let devkit: Vec<_> = config.custom_runtimes.iter().filter(|r| r.devkit).collect();
        assert_eq!(devkit.len(), 1);
        assert_eq!(devkit[0].handler, "kata-qemu-devkit");
        assert_eq!(devkit[0].containerd_snapshotter.as_deref(), Some("erofs"));
        assert_eq!(devkit[0].crio_pull_type.as_deref(), Some("guest-pull"));

        cleanup_env_vars();
    }

    /// Guest debug lives on the variant RuntimeClasses rather than the plain
    /// one, so the devkit RuntimeClass has to be a debug variant as well:
    /// otherwise the class whose whole purpose is debugging would boot a guest
    /// with no agent debug console to reach.
    #[serial]
    #[test]
    fn test_devkit_runtime_is_also_a_debug_variant() {
        setup_minimal_env();
        std::env::set_var("DEBUG", "true");
        std::env::set_var("DEVKIT", "true");

        let config = Config::from_env().unwrap();

        let devkit = config
            .custom_runtimes
            .iter()
            .find(|r| r.devkit)
            .expect("expected synthesized devkit runtime");
        assert!(devkit.debug_variant);

        // And the plain debug variant is still there, on its own handler.
        let handlers: Vec<_> = config
            .custom_runtimes
            .iter()
            .map(|r| r.handler.as_str())
            .collect();
        assert_eq!(handlers, vec!["kata-qemu-debug", "kata-qemu-devkit"]);

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_devkit_handler_includes_multi_install_suffix() {
        setup_minimal_env();
        std::env::set_var("DEBUG", "true");
        std::env::set_var("DEVKIT", "true");
        std::env::set_var("MULTI_INSTALL_SUFFIX", "dev");

        let config = Config::from_env().unwrap();

        let devkit = config
            .custom_runtimes
            .iter()
            .find(|r| r.devkit)
            .expect("expected synthesized devkit runtime");
        // Same reason as the debug variant: two kata-deploy instances on one
        // node must not share a handler.
        assert_eq!(devkit.handler, "kata-qemu-dev-devkit");

        cleanup_env_vars();
    }

    #[test]
    fn test_lookup_mapping_value() {
        let mapping = "qemu:erofs, fc:nydus,clh:";
        assert_eq!(
            lookup_mapping_value(mapping, "qemu").as_deref(),
            Some("erofs")
        );
        // Entries may carry surrounding whitespace.
        assert_eq!(
            lookup_mapping_value(mapping, "fc").as_deref(),
            Some("nydus")
        );
        // Empty value is treated as absent.
        assert!(lookup_mapping_value(mapping, "clh").is_none());
        // Unknown shim.
        assert!(lookup_mapping_value(mapping, "stratovirt").is_none());
        // Nothing mapped at all.
        assert!(lookup_mapping_value("", "qemu").is_none());
    }

    #[serial]
    #[test]
    fn test_multi_install_suffix_with_value() {
        setup_minimal_env();
        std::env::set_var("MULTI_INSTALL_SUFFIX", "dev");

        let config = Config::from_env().unwrap();

        assert_eq!(config.multi_install_suffix, Some("dev".to_string()));
        assert!(
            config.dest_dir.ends_with("/opt/kata-dev"),
            "dest_dir should have suffix: {}",
            config.dest_dir
        );
        assert_eq!(
            config.crio_drop_in_conf_file,
            "/etc/crio/crio.conf.d//99-kata-deploy-dev"
        );

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_multi_install_suffix_different_values() {
        let suffixes = ["staging", "prod", "v2", "test123"];

        for suffix in &suffixes {
            setup_minimal_env();
            std::env::set_var("MULTI_INSTALL_SUFFIX", suffix);

            let config = Config::from_env().unwrap();

            assert_eq!(config.multi_install_suffix, Some(suffix.to_string()));
            assert!(config.dest_dir.contains(&format!("-{}", suffix)));
            assert!(config
                .crio_drop_in_conf_file
                .contains(&format!("-{}", suffix)));

            cleanup_env_vars();
        }
    }

    #[serial]
    #[test]
    fn test_multi_install_prefix_and_suffix() {
        setup_minimal_env();
        std::env::set_var("INSTALLATION_PREFIX", "/custom");
        std::env::set_var("MULTI_INSTALL_SUFFIX", "dev");

        let config = Config::from_env().unwrap();

        assert_eq!(config.installation_prefix, Some("/custom".to_string()));
        assert_eq!(config.multi_install_suffix, Some("dev".to_string()));
        assert!(
            config.dest_dir.starts_with("/custom/opt/kata-dev")
                || config.dest_dir == "/custom/opt/kata-dev"
        );

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_empty_shims_no_custom_runtimes() {
        setup_minimal_env();
        // Empty strings are filtered out, so we need to unset the variable
        // and ensure no default is provided. Since we always have a default,
        // this test verifies that if somehow we get empty shims AND no custom runtimes,
        // validation fails.
        let arch = get_arch().unwrap();
        let arch_suffix = match arch.as_str() {
            "x86_64" => "_X86_64",
            "aarch64" => "_AARCH64",
            "s390x" => "_S390X",
            "ppc64le" => "_PPC64LE",
            _ => return, // Skip test on unsupported arch
        };
        std::env::remove_var(format!("SHIMS{}", arch_suffix));
        // Set a variable that will result in empty shims after split
        std::env::set_var(format!("SHIMS{}", arch_suffix), "   ");
        // Ensure custom runtimes are disabled
        std::env::set_var("CUSTOM_RUNTIMES_ENABLED", "false");

        assert_config_error_contains("No runtimes configured");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_default_shim_not_in_shims() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("DEFAULT_SHIM", "clh");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("DEFAULT_SHIM"));
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_hypervisor_annotation_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("ALLOWED_HYPERVISOR_ANNOTATIONS", "clh:some.annotation");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("references unknown shim 'clh'"));

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_agent_https_proxy_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        std::env::set_var("AGENT_HTTPS_PROXY", "clh=http://proxy:8080");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("AGENT_HTTPS_PROXY references unknown shim"));

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_nvrc_enable_dcgm_parses_shim_set() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu qemu-nvidia-gpu");
        std::env::set_var(
            "NVRC_ENABLE_DCGM",
            "qemu-nvidia-gpu;qemu-nvidia-gpu-snp-runtime-rs",
        );

        let config = Config::from_env().unwrap();
        // The chart lists every enabled shim regardless of architecture, so a
        // name absent from SHIMS is carried rather than rejected.
        assert_eq!(
            config.nvrc_enable_dcgm,
            vec!["qemu-nvidia-gpu", "qemu-nvidia-gpu-snp-runtime-rs"]
        );

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_nvrc_enable_dcgm_rejects_non_gpu_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu qemu-nvidia-gpu");
        std::env::set_var("NVRC_ENABLE_DCGM", "qemu");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not an NVIDIA GPU shim"));

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_snapshotter_mapping_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("SNAPSHOTTER_HANDLER_MAPPING", "clh:nydus");

        assert_config_error_contains("SNAPSHOTTER_HANDLER_MAPPING");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_pull_type_mapping_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("PULL_TYPE_MAPPING", "clh:guest-pull");

        assert_config_error_contains("PULL_TYPE_MAPPING");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_force_guest_pull_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("EXPERIMENTAL_FORCE_GUEST_PULL", "clh,dragonball");

        assert_config_error_contains("EXPERIMENTAL_FORCE_GUEST_PULL");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_validate_success() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc clh");
        set_arch_var(
            "ALLOWED_HYPERVISOR_ANNOTATIONS",
            "qemu:ann1,ann2 global-ann",
        );
        std::env::set_var("AGENT_HTTPS_PROXY", "qemu=http://proxy:8080");
        set_arch_var("SNAPSHOTTER_HANDLER_MAPPING", "qemu:nydus,fc:default");
        set_arch_var("PULL_TYPE_MAPPING", "qemu:guest-pull");
        set_arch_var("EXPERIMENTAL_FORCE_GUEST_PULL", "qemu,fc");

        let result = Config::from_env();
        assert!(result.unwrap().validate().is_ok());

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_missing_node_name_fails() {
        cleanup_env_vars();
        set_arch_var("SHIMS", "qemu");
        set_arch_var("DEFAULT_SHIM", "qemu");

        assert_config_error_contains("NODE_NAME");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_empty_node_name_fails() {
        setup_minimal_env();
        std::env::set_var("NODE_NAME", "");

        assert_config_error_contains("NODE_NAME");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_empty_default_shim_fails() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        // Empty strings are filtered out, so we need to set whitespace-only value
        // that will pass the empty check but fail validation
        set_arch_var("DEFAULT_SHIM", "   ");

        assert_config_error_contains("DEFAULT_SHIM");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_whitespace_only_default_shim_fails() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("DEFAULT_SHIM", "   ");

        assert_config_error_contains("DEFAULT_SHIM");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_whitespace_only_shims_fails() {
        setup_minimal_env();
        set_arch_var("SHIMS", "   ");

        assert_config_error_contains("SHIMS");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_agent_no_proxy_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        std::env::set_var("AGENT_NO_PROXY", "clh=localhost,127.0.0.1");

        assert_config_error_contains("AGENT_NO_PROXY");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_multi_install_suffix_empty_treated_as_none() {
        setup_minimal_env();
        std::env::set_var("MULTI_INSTALL_SUFFIX", "");

        let config = Config::from_env().unwrap();
        assert!(config.multi_install_suffix.is_none());

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_default() {
        setup_minimal_env();

        let config = Config::from_env().unwrap();
        assert_eq!(config.containerd_conf_file, "/etc/containerd/config.toml");
        assert_eq!(
            config.containerd_conf_file_backup,
            "/etc/containerd/config.toml.bak"
        );

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_custom() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", "my-config.toml");

        let config = Config::from_env().unwrap();
        assert_eq!(
            config.containerd_conf_file,
            "/etc/containerd/my-config.toml"
        );
        assert_eq!(
            config.containerd_conf_file_backup,
            "/etc/containerd/my-config.toml.bak"
        );

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_empty_uses_default() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", "");

        let config = Config::from_env().unwrap();
        assert_eq!(config.containerd_conf_file, "/etc/containerd/config.toml");

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_whitespace_only_uses_default() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", "   ");

        let config = Config::from_env().unwrap();
        assert_eq!(config.containerd_conf_file, "/etc/containerd/config.toml");

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_trimmed() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", "  my-config.toml  ");

        let config = Config::from_env().unwrap();
        assert_eq!(
            config.containerd_conf_file,
            "/etc/containerd/my-config.toml"
        );

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_rejects_path_separator() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", "../etc/shadow");

        assert_config_error_contains("simple file name");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_rejects_slash() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", "subdir/config.toml");

        assert_config_error_contains("simple file name");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_rejects_dotdot() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", "..");

        assert_config_error_contains("simple file name");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_rejects_dot() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", ".");

        assert_config_error_contains("simple file name");
        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_containerd_config_file_name_allows_dots_in_name() {
        setup_minimal_env();
        std::env::set_var("CONTAINERD_CONFIG_FILE_NAME", "config.v2.toml");

        let config = Config::from_env().unwrap();
        assert_eq!(
            config.containerd_conf_file,
            "/etc/containerd/config.v2.toml"
        );

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_arch_specific_all_variables() {
        // Test ALL architecture-specific variables work without base variables
        // This is the real-world use case where users set only arch-specific vars in Helm charts

        cleanup_env_vars();

        // Test 1 & 2: Only run on x86_64 since they test x86_64-specific env vars
        if cfg!(target_arch = "x86_64") {
            // Test 1: Only arch-specific vars set (no base vars) - like user's Helm values
            std::env::set_var("NODE_NAME", "test-node");
            std::env::set_var("SHIMS_X86_64", "qemu-coco-dev");
            std::env::set_var("DEFAULT_SHIM_X86_64", "qemu-coco-dev");
            std::env::set_var(
                "ALLOWED_HYPERVISOR_ANNOTATIONS_X86_64",
                "qemu-coco-dev:default_vcpus",
            );
            std::env::set_var("SNAPSHOTTER_HANDLER_MAPPING_X86_64", "qemu-coco-dev:nydus");
            std::env::set_var("PULL_TYPE_MAPPING_X86_64", "qemu-coco-dev:guest-pull");
            std::env::set_var("EXPERIMENTAL_FORCE_GUEST_PULL_X86_64", "qemu-coco-dev");

            let config = Config::from_env().unwrap();

            // On x86_64, should pick up ALL arch-specific values
            assert_eq!(config.shims_for_arch, vec!["qemu-coco-dev"]);
            assert_eq!(config.default_shim_for_arch, "qemu-coco-dev");
            assert_eq!(
                config.allowed_hypervisor_annotations_for_arch,
                vec!["qemu-coco-dev:default_vcpus"]
            );
            assert_eq!(
                config.snapshotter_handler_mapping_for_arch,
                Some("qemu-coco-dev:nydus".to_string())
            );
            assert_eq!(
                config.pull_type_mapping_for_arch,
                Some("qemu-coco-dev:guest-pull".to_string())
            );
            assert_eq!(
                config.experimental_force_guest_pull_for_arch,
                vec!["qemu-coco-dev"]
            );

            cleanup_env_vars();

            // Test 2: Only arch-specific vars set (same as Test 1, verifying consistency)
            std::env::set_var("NODE_NAME", "test-node");
            std::env::set_var("SHIMS_X86_64", "qemu-coco-dev");
            std::env::set_var("DEFAULT_SHIM_X86_64", "qemu-coco-dev");
            std::env::set_var(
                "ALLOWED_HYPERVISOR_ANNOTATIONS_X86_64",
                "qemu-coco-dev:default_vcpus",
            );
            std::env::set_var("SNAPSHOTTER_HANDLER_MAPPING_X86_64", "qemu-coco-dev:nydus");
            std::env::set_var("PULL_TYPE_MAPPING_X86_64", "qemu-coco-dev:guest-pull");
            std::env::set_var("EXPERIMENTAL_FORCE_GUEST_PULL_X86_64", "qemu-coco-dev");

            let config2 = Config::from_env().unwrap();

            // On x86_64, should use arch-specific values
            assert_eq!(config2.shims_for_arch, vec!["qemu-coco-dev"]);
            assert_eq!(config2.default_shim_for_arch, "qemu-coco-dev");
            assert_eq!(
                config2.allowed_hypervisor_annotations_for_arch,
                vec!["qemu-coco-dev:default_vcpus"]
            );
            assert_eq!(
                config2.snapshotter_handler_mapping_for_arch,
                Some("qemu-coco-dev:nydus".to_string())
            );
            assert_eq!(
                config2.pull_type_mapping_for_arch,
                Some("qemu-coco-dev:guest-pull".to_string())
            );
            assert_eq!(
                config2.experimental_force_guest_pull_for_arch,
                vec!["qemu-coco-dev"]
            );

            cleanup_env_vars();
        }
    }

    #[serial]
    #[test]
    fn test_debug_variant_not_synthesized_when_debug_disabled() {
        setup_minimal_env();
        std::env::set_var("DEBUG", "false");

        let config = Config::from_env().unwrap();

        assert!(!config.custom_runtimes_enabled);
        assert!(config.custom_runtimes.is_empty());

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_debug_variant_synthesizes_per_shim_runtime() {
        setup_minimal_env();
        std::env::set_var("DEBUG", "true");

        let config = Config::from_env().unwrap();

        assert!(config.custom_runtimes_enabled);

        let debug_variants: Vec<_> = config
            .custom_runtimes
            .iter()
            .filter(|r| r.debug_variant)
            .collect();
        assert_eq!(debug_variants.len(), 1);
        assert_eq!(debug_variants[0].handler, "kata-qemu-debug");
        assert_eq!(debug_variants[0].base_config, "qemu");
        assert!(debug_variants[0].drop_in_file.is_none());
        assert!(debug_variants[0].containerd_snapshotter.is_none());
        assert!(debug_variants[0].crio_pull_type.is_none());

        cleanup_env_vars();
    }

    #[test]
    fn shim_handlers_name_what_the_cri_config_declares() {
        assert_eq!(shim_handler("qemu", None), "kata-qemu");
        assert_eq!(shim_handler("qemu", Some("")), "kata-qemu");
        assert_eq!(shim_handler("qemu", Some("dev")), "kata-qemu-dev");
    }

    #[serial]
    #[test]
    fn test_debug_variant_handler_includes_multi_install_suffix() {
        setup_minimal_env();
        std::env::set_var("DEBUG", "true");
        std::env::set_var("MULTI_INSTALL_SUFFIX", "dev");

        let config = Config::from_env().unwrap();

        let debug_variant = config
            .custom_runtimes
            .iter()
            .find(|r| r.debug_variant)
            .expect("expected synthesized debug variant runtime");
        // Mirrors the standard handler (kata-<shim>-<suffix>) so two kata-deploy
        // instances on one node do not share a debug handler.
        assert_eq!(debug_variant.handler, "kata-qemu-dev-debug");
        assert_eq!(debug_variant.base_config, "qemu");

        cleanup_env_vars();
    }

    #[serial]
    #[test]
    fn test_debug_variant_inherits_base_shim_snapshotter_and_pull_type() {
        setup_minimal_env();
        std::env::set_var("DEBUG", "true");
        set_arch_var("SNAPSHOTTER_HANDLER_MAPPING", "qemu:nydus");
        set_arch_var("PULL_TYPE_MAPPING", "qemu:guest-pull");

        let config = Config::from_env().unwrap();

        let debug_variant = config
            .custom_runtimes
            .iter()
            .find(|r| r.debug_variant)
            .expect("expected synthesized debug variant runtime");
        assert_eq!(debug_variant.handler, "kata-qemu-debug");
        assert_eq!(
            debug_variant.containerd_snapshotter.as_deref(),
            Some("nydus")
        );
        assert_eq!(debug_variant.crio_pull_type.as_deref(), Some("guest-pull"));

        cleanup_env_vars();
    }
}
