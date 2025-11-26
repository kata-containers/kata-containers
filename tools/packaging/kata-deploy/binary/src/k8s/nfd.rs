// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::client as k8s;
use crate::config::Config;
use anyhow::Result;
use log::info;
use std::env;

pub async fn setup_nfd_rules(config: &Config) -> Result<bool> {
    if !k8s::crd_exists(config, "nodefeaturerules.nfd.k8s-sigs.io").await? {
        return Ok(false);
    }

    let arch = env::consts::ARCH;
    if arch == "x86_64" {
        let node_feature_rule_file =
            format!("/opt/kata-artifacts/node-feature-rules/{arch}-tee-keys.yaml");

        if std::path::Path::new(&node_feature_rule_file).exists() {
            let mut yaml_content = std::fs::read_to_string(&node_feature_rule_file)?;

            // Add instance-specific labels to track ownership
            // This ensures each kata-deploy installation only manages its own resources
            if !yaml_content.contains("kata-deploy/instance:") {
                // Determine the instance identifier
                let instance = config
                    .multi_install_suffix
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.as_str())
                    .unwrap_or("default");

                // Insert labels after 'metadata:' if it exists
                if let Some(pos) = yaml_content.find("metadata:") {
                    let insertion_point = yaml_content[pos..].find('\n').map(|i| pos + i + 1);
                    if let Some(idx) = insertion_point {
                        let labels = format!(
                            r#"  labels:
    app.kubernetes.io/managed-by: kata-deploy
    kata-deploy/instance: {}
"#,
                            instance
                        );
                        yaml_content.insert_str(idx, &labels);
                    }
                }
            }

            k8s::apply_yaml(config, &yaml_content).await?;

            info!("As NFD is deployed on the node, rules for {arch} TEEs have been created");

            return Ok(true);
        }
    }

    Ok(false)
}

pub async fn remove_nfd_rules(config: &Config) -> Result<()> {
    if !k8s::crd_exists(config, "nodefeaturerules.nfd.k8s-sigs.io").await? {
        return Ok(());
    }

    let arch = env::consts::ARCH;
    if arch == "x86_64" {
        let node_feature_rule_file =
            format!("/opt/kata-artifacts/node-feature-rules/{arch}-tee-keys.yaml");

        if std::path::Path::new(&node_feature_rule_file).exists() {
            // Only delete resources from THIS specific kata-deploy instance
            // Each instance is identified by its MULTI_INSTALL_SUFFIX via the kata-deploy/instance label
            // This prevents multiple installations from interfering with each other
            // Using --ignore-not-found ensures we handle cases where the resource
            // doesn't exist or was already deleted
            let yaml_content = std::fs::read_to_string(&node_feature_rule_file)?;
            k8s::delete_yaml(config, &yaml_content, true).await?;

            info!("As NFD is deployed on the node, rules for {arch} TEEs have been deleted");
        }
    }

    Ok(())
}
