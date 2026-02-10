// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use log::info;
use std::env;

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
    pub pull_type_mapping_for_arch: Option<String>,
    pub installation_prefix: Option<String>,
    pub multi_install_suffix: Option<String>,
    pub helm_post_delete_hook: bool,
    pub experimental_setup_snapshotter: Option<Vec<String>>,
    pub experimental_force_guest_pull_for_arch: Vec<String>,
    pub dest_dir: String,
    pub host_install_dir: String,
    pub crio_drop_in_conf_dir: String,
    pub crio_drop_in_conf_file: String,
    pub crio_drop_in_conf_file_debug: String,
    pub containerd_conf_file: String,
    pub containerd_conf_file_backup: String,
    pub containerd_drop_in_conf_file: String,
    pub custom_runtimes_enabled: bool,
    pub custom_runtimes: Vec<CustomRuntime>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let arch = get_arch()?;
        let node_name =
            env::var("NODE_NAME").context("NODE_NAME environment variable is required")?;

        if node_name.trim().is_empty() {
            return Err(anyhow::anyhow!("NODE_NAME must not be empty"));
        }

        let debug = env::var("DEBUG").unwrap_or_else(|_| "false".to_string()) == "true";

        // Parse shims - only use arch-specific variable
        // Use architecture-specific default shims list (only shims supported for this arch)
        let default_shims = get_default_shims_for_arch(&arch);
        let shims_for_arch = get_arch_var("SHIMS", default_shims, &arch)
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let default_shim_for_arch = get_arch_var("DEFAULT_SHIM", "qemu", &arch);

        // Only use arch-specific variable for allowed hypervisor annotations
        let allowed_hypervisor_annotations_for_arch = get_arch_var(
            "ALLOWED_HYPERVISOR_ANNOTATIONS",
            "",
            &arch,
        )
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

        // Only use arch-specific variable for snapshotter handler mapping
        let snapshotter_handler_mapping_for_arch =
            get_arch_var_or_base("SNAPSHOTTER_HANDLER_MAPPING", &arch);

        // Normalize empty strings to None at the boundary
        let agent_https_proxy = env::var("AGENT_HTTPS_PROXY").ok().filter(|s| !s.is_empty());
        let agent_no_proxy = env::var("AGENT_NO_PROXY").ok().filter(|s| !s.is_empty());

        let pull_type_mapping_for_arch = get_arch_var_or_base("PULL_TYPE_MAPPING", &arch);

        let installation_prefix = env::var("INSTALLATION_PREFIX").ok().filter(|s| !s.is_empty());
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

        let host_install_dir = format!("/host{dest_dir}");

        let crio_drop_in_conf_dir = "/etc/crio/crio.conf.d/".to_string();
        let crio_drop_in_conf_file = if let Some(ref suffix) = multi_install_suffix {
            format!("{crio_drop_in_conf_dir}/99-kata-deploy-{suffix}")
        } else {
            format!("{crio_drop_in_conf_dir}/99-kata-deploy")
        };
        let crio_drop_in_conf_file_debug = format!("{crio_drop_in_conf_dir}/100-debug");

        let containerd_conf_file = "/etc/containerd/config.toml".to_string();
        let containerd_conf_file_backup = format!("{containerd_conf_file}.bak");
        let containerd_drop_in_conf_file =
            format!("{dest_dir}/containerd/config.d/kata-deploy.toml");

        let helm_post_delete_hook =
            env::var("HELM_POST_DELETE_HOOK").unwrap_or_else(|_| "false".to_string()) == "true";

        let experimental_setup_snapshotter = env::var("EXPERIMENTAL_SETUP_SNAPSHOTTER")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

        // Only use arch-specific variable for experimental force guest pull
        let experimental_force_guest_pull_for_arch = get_arch_var(
            "EXPERIMENTAL_FORCE_GUEST_PULL",
            "",
            &arch,
        )
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect();

        // Parse custom runtimes from ConfigMap
        let custom_runtimes_enabled =
            env::var("CUSTOM_RUNTIMES_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true";
        let custom_runtimes = if custom_runtimes_enabled {
            parse_custom_runtimes()?
        } else {
            Vec::new()
        };

        let config = Config {
            node_name,
            debug,
            shims_for_arch,
            default_shim_for_arch,
            allowed_hypervisor_annotations_for_arch,
            snapshotter_handler_mapping_for_arch,
            agent_https_proxy,
            agent_no_proxy,
            pull_type_mapping_for_arch,
            installation_prefix,
            multi_install_suffix,
            helm_post_delete_hook,
            experimental_setup_snapshotter,
            experimental_force_guest_pull_for_arch,
            dest_dir,
            host_install_dir,
            crio_drop_in_conf_dir,
            crio_drop_in_conf_file,
            crio_drop_in_conf_file_debug,
            containerd_conf_file,
            containerd_conf_file_backup,
            containerd_drop_in_conf_file,
            custom_runtimes_enabled,
            custom_runtimes,
        };

        // Validate the configuration
        config.validate()?;

        Ok(config)
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

        Ok(())
    }

    pub fn print_info(&self, action: &str) {
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
        info!("* PULL_TYPE_MAPPING: {:?}", self.pull_type_mapping_for_arch);
        info!("* INSTALLATION_PREFIX: {:?}", self.installation_prefix);
        info!("* MULTI_INSTALL_SUFFIX: {:?}", self.multi_install_suffix);
        info!("* HELM_POST_DELETE_HOOK: {}", self.helm_post_delete_hook);
        info!(
            "* EXPERIMENTAL_SETUP_SNAPSHOTTER: {:?}",
            self.experimental_setup_snapshotter
        );
        info!(
            "* EXPERIMENTAL_FORCE_GUEST_PULL: {}",
            self.experimental_force_guest_pull_for_arch.join(",")
        );
        info!(
            "* CUSTOM_RUNTIMES_ENABLED: {}",
            self.custom_runtimes_enabled
        );
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
    }

    /// Get containerd configuration file paths based on runtime type and containerd version
    pub async fn get_containerd_paths(&self, runtime: &str) -> Result<ContainerdPaths> {
        use crate::runtime::manager;

        // Check if drop-in files can be used based on containerd version
        let use_drop_in = manager::is_containerd_capable_of_using_drop_in_files(self, runtime).await?;

        let paths = match runtime {
            "k0s-worker" | "k0s-controller" => ContainerdPaths {
                config_file: "/etc/containerd/containerd.toml".to_string(),
                backup_file: "/etc/containerd/containerd.toml.bak".to_string(), // Never used, but needed for consistency
                imports_file: None, // k0s auto-loads from containerd.d/, imports not needed
                drop_in_file: "/etc/containerd/containerd.d/kata-deploy.toml".to_string(),
                use_drop_in,
            },
            "microk8s" => ContainerdPaths {
                // microk8s uses containerd-template.toml instead of config.toml
                config_file: "/etc/containerd/containerd-template.toml".to_string(),
                backup_file: "/etc/containerd/containerd-template.toml.bak".to_string(),
                imports_file: Some("/etc/containerd/containerd-template.toml".to_string()),
                drop_in_file: self.containerd_drop_in_conf_file.clone(),
                use_drop_in,
            },
            "k3s" | "k3s-agent" | "rke2-agent" | "rke2-server" => ContainerdPaths {
                // k3s/rke2 generates config.toml from config.toml.tmpl on each restart
                // We must modify the template file so our changes persist
                config_file: "/etc/containerd/config.toml.tmpl".to_string(),
                backup_file: "/etc/containerd/config.toml.tmpl.bak".to_string(),
                imports_file: Some("/etc/containerd/config.toml.tmpl".to_string()),
                drop_in_file: self.containerd_drop_in_conf_file.clone(),
                use_drop_in,
            },
            _ => ContainerdPaths {
                config_file: self.containerd_conf_file.clone(),
                backup_file: self.containerd_conf_file_backup.clone(),
                imports_file: Some(self.containerd_conf_file.clone()),
                drop_in_file: self.containerd_drop_in_conf_file.clone(),
                use_drop_in,
            },
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
        });
    }

    log::info!(
        "Parsed {} custom runtime(s) from {}",
        custom_runtimes.len(),
        list_file
    );
    Ok(custom_runtimes)
}

/// Get default shims list for a specific architecture
/// Returns only shims that are supported for that architecture
fn get_default_shims_for_arch(arch: &str) -> &'static str {
    match arch {
        "x86_64" => "clh cloud-hypervisor dragonball fc qemu qemu-coco-dev qemu-coco-dev-runtime-rs qemu-runtime-rs qemu-nvidia-gpu qemu-nvidia-gpu-snp qemu-nvidia-gpu-tdx qemu-snp qemu-snp-runtime-rs qemu-tdx qemu-tdx-runtime-rs",
        "aarch64" => "clh cloud-hypervisor dragonball fc qemu qemu-runtime-rs qemu-nvidia-gpu qemu-cca",
        "s390x" => "qemu qemu-runtime-rs qemu-se qemu-se-runtime-rs qemu-coco-dev qemu-coco-dev-runtime-rs",
        "ppc64le" => "qemu",
        _ => "qemu", // Fallback to qemu for unknown architectures
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
    //! IMPORTANT: All tests in this crate MUST be run serially (--test-threads=1)
    //! because they manipulate shared environment variables. Running tests in parallel
    //! will cause race conditions and test failures.
    //!
    //! Use: cargo test --bin kata-deploy -- --test-threads=1
    //! Or:  cargo test-serial (if the cargo alias is configured)

    use super::*;

    // NOTE: These tests modify environment variables which are process-global.
    // Run with: cargo test config::tests -- --test-threads=1
    // to ensure proper test isolation.

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

    #[test]
    fn test_get_arch() {
        let arch = get_arch().unwrap();
        assert!(!arch.is_empty());
        cleanup_env_vars();
    }

    #[test]
    fn test_get_arch_var() {
        std::env::set_var("SHIMS_X86_64", "test1 test2");
        let result = get_arch_var("SHIMS", "default", "x86_64");
        assert_eq!(result, "test1 test2");
        cleanup_env_vars();
    }

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

    #[test]
    fn test_validate_snapshotter_mapping_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("SNAPSHOTTER_HANDLER_MAPPING", "clh:nydus");

        assert_config_error_contains("SNAPSHOTTER_HANDLER_MAPPING");
        cleanup_env_vars();
    }

    #[test]
    fn test_validate_pull_type_mapping_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("PULL_TYPE_MAPPING", "clh:guest-pull");

        assert_config_error_contains("PULL_TYPE_MAPPING");
        cleanup_env_vars();
    }

    #[test]
    fn test_validate_force_guest_pull_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("EXPERIMENTAL_FORCE_GUEST_PULL", "clh,dragonball");

        assert_config_error_contains("EXPERIMENTAL_FORCE_GUEST_PULL");
        cleanup_env_vars();
    }

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

    #[test]
    fn test_missing_node_name_fails() {
        cleanup_env_vars();
        set_arch_var("SHIMS", "qemu");
        set_arch_var("DEFAULT_SHIM", "qemu");

        assert_config_error_contains("NODE_NAME");
        cleanup_env_vars();
    }

    #[test]
    fn test_empty_node_name_fails() {
        setup_minimal_env();
        std::env::set_var("NODE_NAME", "");

        assert_config_error_contains("NODE_NAME");
        cleanup_env_vars();
    }

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

    #[test]
    fn test_whitespace_only_default_shim_fails() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        set_arch_var("DEFAULT_SHIM", "   ");

        assert_config_error_contains("DEFAULT_SHIM");
        cleanup_env_vars();
    }

    #[test]
    fn test_whitespace_only_shims_fails() {
        setup_minimal_env();
        set_arch_var("SHIMS", "   ");

        assert_config_error_contains("SHIMS");
        cleanup_env_vars();
    }

    #[test]
    fn test_agent_no_proxy_invalid_shim() {
        setup_minimal_env();
        set_arch_var("SHIMS", "qemu fc");
        std::env::set_var("AGENT_NO_PROXY", "clh=localhost,127.0.0.1");

        assert_config_error_contains("AGENT_NO_PROXY");
        cleanup_env_vars();
    }

    #[test]
    fn test_multi_install_suffix_empty_treated_as_none() {
        setup_minimal_env();
        std::env::set_var("MULTI_INSTALL_SUFFIX", "");

        let config = Config::from_env().unwrap();
        assert!(config.multi_install_suffix.is_none());

        cleanup_env_vars();
    }

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
}
