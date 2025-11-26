// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use log::info;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub node_name: String,
    pub debug: bool,
    pub shims_for_arch: Vec<String>,
    pub default_shim_for_arch: String,
    pub create_runtimeclasses: bool,
    pub create_default_runtimeclass: bool,
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

        // Parse shims
        let shims = env::var("SHIMS").unwrap_or_else(|_| {
            "clh cloud-hypervisor dragonball fc qemu qemu-coco-dev qemu-coco-dev-runtime-rs qemu-runtime-rs qemu-se-runtime-rs qemu-snp qemu-tdx stratovirt qemu-nvidia-gpu qemu-nvidia-gpu-snp qemu-nvidia-gpu-tdx qemu-cca".to_string()
        });
        let shims_for_arch = get_arch_var("SHIMS", &shims, &arch)
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let default_shim = env::var("DEFAULT_SHIM").unwrap_or_else(|_| "qemu".to_string());
        let default_shim_for_arch = get_arch_var("DEFAULT_SHIM", &default_shim, &arch);

        let create_runtimeclasses =
            env::var("CREATE_RUNTIMECLASSES").unwrap_or_else(|_| "false".to_string()) == "true";
        let create_default_runtimeclass = env::var("CREATE_DEFAULT_RUNTIMECLASS")
            .unwrap_or_else(|_| "false".to_string())
            == "true";

        let allowed_hypervisor_annotations =
            env::var("ALLOWED_HYPERVISOR_ANNOTATIONS").unwrap_or_else(|_| String::new());
        let allowed_hypervisor_annotations_for_arch = get_arch_var(
            "ALLOWED_HYPERVISOR_ANNOTATIONS",
            &allowed_hypervisor_annotations,
            &arch,
        )
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

        // For these variables, try arch-specific first, then fall back to base variable
        let snapshotter_handler_mapping_for_arch =
            get_arch_var_or_base("SNAPSHOTTER_HANDLER_MAPPING", &arch);

        let agent_https_proxy = env::var("AGENT_HTTPS_PROXY").ok();
        let agent_no_proxy = env::var("AGENT_NO_PROXY").ok();

        let pull_type_mapping_for_arch = get_arch_var_or_base("PULL_TYPE_MAPPING", &arch);

        let installation_prefix = env::var("INSTALLATION_PREFIX").ok();
        let default_dest_dir = "/opt/kata";
        let dest_dir = match installation_prefix {
            Some(ref prefix) if !prefix.is_empty() => {
                if !prefix.starts_with('/') {
                    return Err(anyhow::anyhow!(
                        "INSTALLATION_PREFIX must begin with a \"/\" (ex. /hoge/fuga)"
                    ));
                }
                format!("{prefix}{default_dest_dir}")
            }
            _ => default_dest_dir.to_string(),
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
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

        let experimental_force_guest_pull =
            env::var("EXPERIMENTAL_FORCE_GUEST_PULL").unwrap_or_else(|_| String::new());
        let experimental_force_guest_pull_for_arch = get_arch_var(
            "EXPERIMENTAL_FORCE_GUEST_PULL",
            &experimental_force_guest_pull,
            &arch,
        )
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect();

        let config = Config {
            node_name,
            debug,
            shims_for_arch,
            default_shim_for_arch,
            create_runtimeclasses,
            create_default_runtimeclass,
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
        // Validate SHIMS_FOR_ARCH is not empty and not just whitespace
        if self.shims_for_arch.is_empty() {
            return Err(anyhow::anyhow!(
                "SHIMS for the current architecture must not be empty. \
                 Please provide at least one shim via SHIMS or SHIMS_<ARCH>"
            ));
        }

        // Check for empty shim names
        for shim in &self.shims_for_arch {
            if shim.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "SHIMS contains empty shim name. All shim names must be non-empty"
                ));
            }
        }

        // Validate DEFAULT_SHIM_FOR_ARCH exists in SHIMS_FOR_ARCH
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
        info!("* CREATE_RUNTIMECLASSES: {}", self.create_runtimeclasses);
        info!(
            "* CREATE_DEFAULT_RUNTIMECLASS: {}",
            self.create_default_runtimeclass
        );
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
    }
}

fn get_arch() -> Result<String> {
    let arch = std::env::consts::ARCH;
    Ok(match arch {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        "s390x" => "s390x",
        "powerpc64le" => "ppc64le",
        _ => arch,
    }
    .to_string())
}

/// Get architecture-specific variable with fallback chain:
/// 1. Try arch-specific variable (e.g., SHIMS_X86_64)
/// 2. Fall back to base variable (e.g., SHIMS)
/// 3. Fall back to provided default
fn get_arch_var(base_name: &str, default: &str, arch: &str) -> String {
    let arch_suffix = match arch {
        "x86_64" => "_X86_64",
        "aarch64" => "_AARCH64",
        "s390x" => "_S390X",
        "ppc64le" => "_PPC64LE",
        _ => "",
    };

    // Try arch-specific first
    if !arch_suffix.is_empty() {
        let arch_var = format!("{base_name}{arch_suffix}");
        if let Ok(val) = env::var(&arch_var) {
            return val;
        }
    }

    // Fall back to base variable, then default
    env::var(base_name).unwrap_or_else(|_| default.to_string())
}

/// Get architecture-specific variable, falling back to base variable if arch-specific not found
/// Returns None only if neither exists
fn get_arch_var_or_base(base_name: &str, arch: &str) -> Option<String> {
    let arch_suffix = match arch {
        "x86_64" => "_X86_64",
        "aarch64" => "_AARCH64",
        "s390x" => "_S390X",
        "ppc64le" => "_PPC64LE",
        _ => "",
    };

    // Try arch-specific first
    if !arch_suffix.is_empty() {
        let arch_var = format!("{base_name}{arch_suffix}");
        if let Ok(val) = env::var(&arch_var) {
            return Some(val);
        }
    }

    // Fall back to base variable
    env::var(base_name).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_arch() {
        let arch = get_arch().unwrap();
        assert!(!arch.is_empty());
    }

    #[test]
    fn test_get_arch_var() {
        std::env::set_var("SHIMS_X86_64", "test1 test2");
        let result = get_arch_var("SHIMS", "default", "x86_64");
        assert_eq!(result, "test1 test2");
        std::env::remove_var("SHIMS_X86_64");
    }

    #[test]
    fn test_multi_install_suffix_not_set() {
        // Clean up ALL relevant env vars first (test isolation)
        std::env::remove_var("MULTI_INSTALL_SUFFIX");
        std::env::remove_var("INSTALLATION_PREFIX");
        std::env::remove_var("NODE_NAME");
        std::env::remove_var("DEBUG");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");

        // Test without MULTI_INSTALL_SUFFIX
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("DEBUG", "false");
        std::env::set_var("SHIMS", "qemu");
        std::env::set_var("DEFAULT_SHIM", "qemu");

        let config = Config::from_env().unwrap();

        assert_eq!(config.multi_install_suffix, None);
        assert!(config.dest_dir.ends_with("/opt/kata"));
        assert_eq!(
            config.crio_drop_in_conf_file,
            "/etc/crio/crio.conf.d//99-kata-deploy"
        );

        // Cleanup
        std::env::remove_var("NODE_NAME");
        std::env::remove_var("DEBUG");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_multi_install_suffix_with_value() {
        // Clean up ALL relevant env vars first (test isolation)
        std::env::remove_var("INSTALLATION_PREFIX");
        std::env::remove_var("MULTI_INSTALL_SUFFIX");
        std::env::remove_var("NODE_NAME");
        std::env::remove_var("DEBUG");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");

        // Test with MULTI_INSTALL_SUFFIX set
        std::env::set_var("MULTI_INSTALL_SUFFIX", "dev");
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("DEBUG", "false");
        std::env::set_var("SHIMS", "qemu");
        std::env::set_var("DEFAULT_SHIM", "qemu");

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

        // Cleanup
        std::env::remove_var("MULTI_INSTALL_SUFFIX");
        std::env::remove_var("NODE_NAME");
        std::env::remove_var("DEBUG");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_multi_install_suffix_different_values() {
        // Clean up ALL relevant env vars first (test isolation)
        std::env::remove_var("INSTALLATION_PREFIX");
        std::env::remove_var("MULTI_INSTALL_SUFFIX");
        std::env::remove_var("NODE_NAME");
        std::env::remove_var("DEBUG");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");

        // Test various suffix values
        let suffixes = vec!["staging", "prod", "v2", "test123"];

        for suffix in suffixes {
            std::env::set_var("MULTI_INSTALL_SUFFIX", suffix);
            std::env::set_var("NODE_NAME", "test-node");
            std::env::set_var("DEBUG", "false");
            std::env::set_var("SHIMS", "qemu");
            std::env::set_var("DEFAULT_SHIM", "qemu");

            let config = Config::from_env().unwrap();

            assert_eq!(config.multi_install_suffix, Some(suffix.to_string()));
            assert!(config.dest_dir.contains(&format!("-{}", suffix)));
            assert!(config
                .crio_drop_in_conf_file
                .contains(&format!("-{}", suffix)));

            // Cleanup after each iteration for better test isolation
            std::env::remove_var("MULTI_INSTALL_SUFFIX");
            std::env::remove_var("NODE_NAME");
            std::env::remove_var("DEBUG");
            std::env::remove_var("SHIMS");
            std::env::remove_var("DEFAULT_SHIM");
        }
    }

    #[test]
    fn test_multi_install_prefix_and_suffix() {
        // Clean up ALL relevant env vars first (test isolation)
        std::env::remove_var("INSTALLATION_PREFIX");
        std::env::remove_var("MULTI_INSTALL_SUFFIX");
        std::env::remove_var("NODE_NAME");
        std::env::remove_var("DEBUG");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");

        // Test combination of INSTALLATION_PREFIX and MULTI_INSTALL_SUFFIX
        std::env::set_var("INSTALLATION_PREFIX", "/custom");
        std::env::set_var("MULTI_INSTALL_SUFFIX", "dev");
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("DEBUG", "false");
        std::env::set_var("SHIMS", "qemu");
        std::env::set_var("DEFAULT_SHIM", "qemu");

        let config = Config::from_env().unwrap();

        assert_eq!(config.installation_prefix, Some("/custom".to_string()));
        assert_eq!(config.multi_install_suffix, Some("dev".to_string()));
        assert!(
            config.dest_dir.starts_with("/custom/opt/kata-dev")
                || config.dest_dir == "/custom/opt/kata-dev"
        );

        // Cleanup
        std::env::remove_var("INSTALLATION_PREFIX");
        std::env::remove_var("MULTI_INSTALL_SUFFIX");
        std::env::remove_var("NODE_NAME");
        std::env::remove_var("DEBUG");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_validate_empty_shims() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "");
        std::env::set_var("DEFAULT_SHIM", "qemu");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("SHIMS for the current architecture must not be empty"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_validate_default_shim_not_in_shims() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "clh");

        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("DEFAULT_SHIM"));
        assert!(err_msg.contains("must be one of"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_validate_hypervisor_annotation_invalid_shim() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var("ALLOWED_HYPERVISOR_ANNOTATIONS", "clh:some.annotation");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("references unknown shim 'clh'"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
        std::env::remove_var("ALLOWED_HYPERVISOR_ANNOTATIONS");
    }

    #[test]
    fn test_validate_agent_https_proxy_invalid_shim() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var("AGENT_HTTPS_PROXY", "clh=http://proxy:8080");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("AGENT_HTTPS_PROXY references unknown shim"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
        std::env::remove_var("AGENT_HTTPS_PROXY");
    }

    #[test]
    fn test_validate_snapshotter_mapping_invalid_shim() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var("SNAPSHOTTER_HANDLER_MAPPING", "clh:nydus");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("SNAPSHOTTER_HANDLER_MAPPING"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
        std::env::remove_var("SNAPSHOTTER_HANDLER_MAPPING");
    }

    #[test]
    fn test_validate_pull_type_mapping_invalid_shim() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var("PULL_TYPE_MAPPING", "clh:guest-pull");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("PULL_TYPE_MAPPING"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
        std::env::remove_var("PULL_TYPE_MAPPING");
    }

    #[test]
    fn test_validate_force_guest_pull_invalid_shim() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var("EXPERIMENTAL_FORCE_GUEST_PULL", "clh,dragonball");

        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("EXPERIMENTAL_FORCE_GUEST_PULL"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
        std::env::remove_var("EXPERIMENTAL_FORCE_GUEST_PULL");
    }

    #[test]
    fn test_validate_success() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc clh");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var(
            "ALLOWED_HYPERVISOR_ANNOTATIONS",
            "qemu:ann1,ann2 global-ann",
        );
        std::env::set_var("AGENT_HTTPS_PROXY", "qemu=http://proxy:8080");
        std::env::set_var("SNAPSHOTTER_HANDLER_MAPPING", "qemu:nydus,fc:default");
        std::env::set_var("PULL_TYPE_MAPPING", "qemu:guest-pull");
        std::env::set_var("EXPERIMENTAL_FORCE_GUEST_PULL", "qemu,fc");

        let result = Config::from_env();
        assert!(result.is_ok());

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
        std::env::remove_var("ALLOWED_HYPERVISOR_ANNOTATIONS");
        std::env::remove_var("AGENT_HTTPS_PROXY");
        std::env::remove_var("SNAPSHOTTER_HANDLER_MAPPING");
        std::env::remove_var("PULL_TYPE_MAPPING");
        std::env::remove_var("EXPERIMENTAL_FORCE_GUEST_PULL");
    }

    #[test]
    fn test_missing_node_name_fails() {
        std::env::remove_var("NODE_NAME");
        std::env::set_var("SHIMS", "qemu");
        std::env::set_var("DEFAULT_SHIM", "qemu");

        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("NODE_NAME"));

        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_empty_node_name_fails() {
        std::env::set_var("NODE_NAME", "");
        std::env::set_var("SHIMS", "qemu");
        std::env::set_var("DEFAULT_SHIM", "qemu");

        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("NODE_NAME"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_empty_default_shim_fails() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "");

        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("DEFAULT_SHIM"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_whitespace_only_default_shim_fails() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "   ");

        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("DEFAULT_SHIM"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_whitespace_only_shims_fails() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "   ");
        std::env::set_var("DEFAULT_SHIM", "qemu");

        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("SHIMS"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
    }

    #[test]
    fn test_agent_no_proxy_invalid_shim() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu fc");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var("AGENT_NO_PROXY", "clh=localhost,127.0.0.1");

        let result = Config::from_env();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("AGENT_NO_PROXY"));
        assert!(err_msg.contains("unknown shim"));

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
        std::env::remove_var("AGENT_NO_PROXY");
    }

    #[test]
    fn test_multi_install_suffix_empty_treated_as_none() {
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var("MULTI_INSTALL_SUFFIX", "");

        let result = Config::from_env();
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.multi_install_suffix.is_none());

        std::env::remove_var("NODE_NAME");
        std::env::remove_var("SHIMS");
        std::env::remove_var("DEFAULT_SHIM");
        std::env::remove_var("MULTI_INSTALL_SUFFIX");
    }

    #[test]
    fn test_arch_specific_all_variables() {
        // Test ALL architecture-specific variables work without base variables
        // This is the real-world use case where users set only arch-specific vars in Helm charts

        // Clean up all env vars first
        let vars_to_clean = vec![
            "NODE_NAME",
            "SHIMS",
            "SHIMS_X86_64",
            "DEFAULT_SHIM",
            "DEFAULT_SHIM_X86_64",
            "ALLOWED_HYPERVISOR_ANNOTATIONS",
            "ALLOWED_HYPERVISOR_ANNOTATIONS_X86_64",
            "SNAPSHOTTER_HANDLER_MAPPING",
            "SNAPSHOTTER_HANDLER_MAPPING_X86_64",
            "PULL_TYPE_MAPPING",
            "PULL_TYPE_MAPPING_X86_64",
            "EXPERIMENTAL_FORCE_GUEST_PULL",
            "EXPERIMENTAL_FORCE_GUEST_PULL_X86_64",
        ];
        for var in &vars_to_clean {
            std::env::remove_var(var);
        }

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

            // Cleanup
            for var in &vars_to_clean {
                std::env::remove_var(var);
            }

            // Test 2: Base vars set, arch-specific overrides
            std::env::set_var("NODE_NAME", "test-node");
            std::env::set_var("SHIMS", "qemu fc");
            std::env::set_var("SHIMS_X86_64", "qemu-coco-dev");
            std::env::set_var("DEFAULT_SHIM", "qemu");
            std::env::set_var("DEFAULT_SHIM_X86_64", "qemu-coco-dev");
            std::env::set_var("ALLOWED_HYPERVISOR_ANNOTATIONS", "qemu:image");
            std::env::set_var(
                "ALLOWED_HYPERVISOR_ANNOTATIONS_X86_64",
                "qemu-coco-dev:default_vcpus",
            );
            std::env::set_var("SNAPSHOTTER_HANDLER_MAPPING", "qemu:default");
            std::env::set_var("SNAPSHOTTER_HANDLER_MAPPING_X86_64", "qemu-coco-dev:nydus");
            std::env::set_var("PULL_TYPE_MAPPING", "qemu:default");
            std::env::set_var("PULL_TYPE_MAPPING_X86_64", "qemu-coco-dev:guest-pull");
            std::env::set_var("EXPERIMENTAL_FORCE_GUEST_PULL", "qemu");
            std::env::set_var("EXPERIMENTAL_FORCE_GUEST_PULL_X86_64", "qemu-coco-dev");

            let config2 = Config::from_env().unwrap();

            // On x86_64, should prefer ALL arch-specific over base
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

            // Cleanup
            for var in &vars_to_clean {
                std::env::remove_var(var);
            }
        }

        // Test 3: Only base vars set (backwards compatibility)
        std::env::set_var("NODE_NAME", "test-node");
        std::env::set_var("SHIMS", "qemu");
        std::env::set_var("DEFAULT_SHIM", "qemu");
        std::env::set_var("ALLOWED_HYPERVISOR_ANNOTATIONS", "qemu:image");
        std::env::set_var("SNAPSHOTTER_HANDLER_MAPPING", "qemu:nydus");
        std::env::set_var("PULL_TYPE_MAPPING", "qemu:guest-pull");
        std::env::set_var("EXPERIMENTAL_FORCE_GUEST_PULL", "qemu");

        let config3 = Config::from_env().unwrap();

        // Should use base vars when no arch-specific set
        assert_eq!(config3.shims_for_arch, vec!["qemu"]);
        assert_eq!(config3.default_shim_for_arch, "qemu");
        assert_eq!(
            config3.allowed_hypervisor_annotations_for_arch,
            vec!["qemu:image"]
        );
        assert_eq!(
            config3.snapshotter_handler_mapping_for_arch,
            Some("qemu:nydus".to_string())
        );
        assert_eq!(
            config3.pull_type_mapping_for_arch,
            Some("qemu:guest-pull".to_string())
        );
        assert_eq!(config3.experimental_force_guest_pull_for_arch, vec!["qemu"]);

        // Final cleanup
        for var in &vars_to_clean {
            std::env::remove_var(var);
        }
    }
}
