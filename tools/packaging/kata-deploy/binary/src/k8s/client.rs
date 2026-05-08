// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, DeleteParams, DynamicObject, Patch, PatchParams},
    discovery::ApiResource,
    Client,
};
use log::info;
use serde_json::json;

pub struct K8sClient {
    client: Client,
    node_api: Api<Node>,
    node_name: String,
}

impl K8sClient {
    pub async fn new(node_name: &str) -> Result<Self> {
        let client = Client::try_default()
            .await
            .context("Failed to create Kubernetes client")?;
        // Node is a cluster-scoped resource
        let node_api: Api<Node> = Api::all(client.clone());

        Ok(K8sClient {
            client,
            node_api,
            node_name: node_name.to_string(),
        })
    }

    pub async fn get_node(&self) -> Result<Node> {
        self.node_api
            .get(&self.node_name)
            .await
            .with_context(|| format!("Failed to get node: {}", self.node_name))
    }

    /// Return `.status.nodeInfo.containerRuntimeVersion` for the bound node,
    /// or an error if the field isn't populated. Avoids deep-cloning the
    /// whole `Node` into a `serde_json::Value` tree just to walk a static
    /// path.
    pub async fn get_container_runtime_version(&self) -> Result<String> {
        let node = self.get_node().await?;
        node.status
            .as_ref()
            .and_then(|s| s.node_info.as_ref())
            .map(|i| i.container_runtime_version.clone())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Node '{}' is missing status.nodeInfo.containerRuntimeVersion",
                    self.node_name
                )
            })
    }

    /// Return the value of a single label from `.metadata.labels` on the
    /// bound node, or `None` if the label is absent.
    pub async fn get_node_label(&self, key: &str) -> Result<Option<String>> {
        let node = self.get_node().await?;
        Ok(node
            .metadata
            .labels
            .as_ref()
            .and_then(|labels| labels.get(key).cloned()))
    }

    pub async fn label_node(
        &self,
        label_key: &str,
        label_value: Option<&str>,
        overwrite: bool,
    ) -> Result<()> {
        let mut node = self.get_node().await?;

        let labels = node.metadata.labels.get_or_insert_with(Default::default);

        let patch = if let Some(value) = label_value {
            if overwrite || !labels.contains_key(label_key) {
                labels.insert(label_key.to_string(), value.to_string());
                info!(
                    "Setting label {}={} on node {}",
                    label_key, value, self.node_name
                );
            }
            Patch::Merge(json!({
                "metadata": {
                    "labels": labels
                }
            }))
        } else {
            labels.remove(label_key);
            info!("Removing label {} from node {}", label_key, self.node_name);
            // JSON merge patch: omit key = leave unchanged. To remove, set key to null.
            let mut patch_labels = serde_json::Map::new();
            patch_labels.insert(label_key.to_string(), serde_json::Value::Null);
            Patch::Merge(json!({
                "metadata": {
                    "labels": patch_labels
                }
            }))
        };

        let pp = PatchParams::default();
        self.node_api
            .patch(&self.node_name, &pp, &patch)
            .await
            .with_context(|| format!("Failed to patch node: {}", self.node_name))?;

        Ok(())
    }

    /// Returns whether a non-terminating DaemonSet with this exact name
    /// exists in the current namespace. Used to decide whether this pod is
    /// being restarted (true) or uninstalled (false).
    pub async fn own_daemonset_exists(&self, daemonset_name: &str) -> Result<bool> {
        use k8s_openapi::api::apps::v1::DaemonSet;
        use kube::api::Api;

        let ds_api: Api<DaemonSet> = Api::default_namespaced(self.client.clone());
        match ds_api.get_opt(daemonset_name).await? {
            Some(ds) => Ok(ds.metadata.deletion_timestamp.is_none()),
            None => Ok(false),
        }
    }

    /// Returns how many non-terminating DaemonSets across all namespaces
    /// have a name containing "kata-deploy". Used to decide whether shared
    /// node-level resources (node label, CRI restart) should be cleaned up:
    /// they are only safe to remove when no kata-deploy instance remains
    /// on the cluster.
    pub async fn count_any_kata_deploy_daemonsets(&self) -> Result<usize> {
        use k8s_openapi::api::apps::v1::DaemonSet;
        use kube::api::{Api, ListParams};

        let ds_api: Api<DaemonSet> = Api::all(self.client.clone());
        let daemonsets = ds_api.list(&ListParams::default()).await?;

        let count = daemonsets
            .iter()
            .filter(|ds| {
                ds.metadata.deletion_timestamp.is_none()
                    && ds
                        .metadata
                        .name
                        .as_ref()
                        .is_some_and(|n| n.contains("kata-deploy"))
            })
            .count();

        Ok(count)
    }

    pub async fn crd_exists(&self, crd_name: &str) -> Result<bool> {
        use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
        use kube::api::{Api, ListParams};

        let crd_api: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        // Use field selector to filter server-side for exact name match
        let lp = ListParams::default().fields(&format!("metadata.name={crd_name}"));
        let crds = crd_api.list(&lp).await?;

        // If any CRDs are returned, the CRD exists
        Ok(!crds.items.is_empty())
    }

    pub async fn apply_yaml(&self, yaml_content: &str) -> Result<()> {
        use kube::api::{Api, PostParams};
        use serde_yaml;

        // Parse YAML to determine resource type
        let value: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;
        let kind = value
            .get("kind")
            .and_then(|k| k.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'kind' in YAML"))?;

        match kind {
            "RuntimeClass" => {
                use k8s_openapi::api::node::v1::RuntimeClass;
                let runtimeclass: RuntimeClass = serde_yaml::from_str(yaml_content)?;
                let api: Api<RuntimeClass> = Api::all(self.client.clone());
                let pp = PostParams::default();
                api.create(&pp, &runtimeclass).await?;
            }
            "NodeFeatureRule" => {
                // NodeFeatureRule is a CRD, handle via dynamic API
                self.apply_dynamic_resource(yaml_content).await?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported resource kind: {kind}"));
            }
        }

        Ok(())
    }

    pub async fn delete_yaml(&self, yaml_content: &str, ignore_not_found: bool) -> Result<()> {
        use serde_yaml;

        let value: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;
        let kind = value
            .get("kind")
            .and_then(|k| k.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'kind' in YAML"))?;

        let name = value
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|n| n.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'metadata.name' in YAML"))?;

        // Extract expected instance label for ownership verification
        let expected_instance = value
            .get("metadata")
            .and_then(|m| m.get("labels"))
            .and_then(|l| l.get("kata-deploy/instance"))
            .and_then(|i| i.as_str());

        // If the YAML doesn't have our instance label, skip ownership verification
        // This handles old resources created before labeling was implemented
        let expected_instance = match expected_instance {
            Some(instance) => instance,
            None => {
                log::warn!(
                    "YAML for {} '{}' missing kata-deploy/instance label - skipping deletion for safety",
                    kind,
                    name
                );
                return Ok(());
            }
        };

        match kind {
            "RuntimeClass" => {
                use k8s_openapi::api::node::v1::RuntimeClass;
                let api: Api<RuntimeClass> = Api::all(self.client.clone());

                // Fetch the existing resource to verify ownership
                match api.get(name).await {
                    Ok(existing) => {
                        // Check if the instance label matches
                        let current_instance = existing
                            .metadata
                            .labels
                            .as_ref()
                            .and_then(|labels| labels.get("kata-deploy/instance"))
                            .map(|s| s.as_str());

                        match current_instance {
                            Some(instance) if instance == expected_instance => {
                                // We own this resource, safe to delete
                                info!("Deleting RuntimeClass '{}' (instance: {})", name, instance);
                                let dp = DeleteParams::default();
                                api.delete(name, &dp).await?;
                            }
                            Some(instance) => {
                                // Resource exists but owned by different instance
                                log::warn!(
                                    "Skipping deletion of RuntimeClass '{}' - owned by instance '{}', not '{}'",
                                    name,
                                    instance,
                                    expected_instance
                                );
                            }
                            None => {
                                // Resource exists but has no instance label
                                log::warn!(
                                    "Skipping deletion of RuntimeClass '{}' - missing kata-deploy/instance label",
                                    name
                                );
                            }
                        }
                    }
                    Err(kube::Error::Api(e)) if e.code == 404 => {
                        // Resource doesn't exist
                        if ignore_not_found {
                            log::debug!("RuntimeClass '{}' not found (already deleted)", name);
                        } else {
                            return Err(anyhow::anyhow!("RuntimeClass '{}' not found", name));
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            "NodeFeatureRule" => {
                self.delete_dynamic_resource(yaml_content, ignore_not_found)
                    .await?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported resource kind: {kind}"));
            }
        }

        Ok(())
    }

    async fn apply_dynamic_resource(&self, yaml_content: &str) -> Result<()> {
        // Parse the YAML into a DynamicObject
        let obj: DynamicObject = serde_yaml::from_str(yaml_content)
            .context("Failed to parse YAML for dynamic resource")?;

        // NodeFeatureRule is in the nfd.k8s-sigs.io API group
        // We know the CRD exists because we checked before calling this function
        let api_resource = ApiResource {
            group: "nfd.k8s-sigs.io".to_string(),
            version: "v1alpha1".to_string(),
            api_version: "nfd.k8s-sigs.io/v1alpha1".to_string(),
            kind: "NodeFeatureRule".to_string(),
            plural: "nodefeaturerules".to_string(),
        };

        // NodeFeatureRule is cluster-scoped (no namespace)
        let api: Api<DynamicObject> = Api::all_with(self.client.clone(), &api_resource);

        // Extract the resource name from the object
        let name = obj
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Resource missing name"))?;

        // Apply the resource (server-side apply)
        let pp = PatchParams::apply("kata-deploy");
        api.patch(name, &pp, &Patch::Apply(&obj)).await?;

        Ok(())
    }

    async fn delete_dynamic_resource(
        &self,
        yaml_content: &str,
        ignore_not_found: bool,
    ) -> Result<()> {
        // Parse the YAML to extract the resource name and expected instance label
        let obj: DynamicObject = serde_yaml::from_str(yaml_content)
            .context("Failed to parse YAML for dynamic resource")?;

        // NodeFeatureRule is in the nfd.k8s-sigs.io API group
        // We know the CRD exists because we checked before calling this function
        let api_resource = ApiResource {
            group: "nfd.k8s-sigs.io".to_string(),
            version: "v1alpha1".to_string(),
            api_version: "nfd.k8s-sigs.io/v1alpha1".to_string(),
            kind: "NodeFeatureRule".to_string(),
            plural: "nodefeaturerules".to_string(),
        };

        // NodeFeatureRule is cluster-scoped (no namespace)
        let api: Api<DynamicObject> = Api::all_with(self.client.clone(), &api_resource);

        // Extract the resource name from the object
        let name = obj
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Resource missing name"))?;

        // Extract the expected instance label from the YAML
        let expected_instance = obj
            .metadata
            .labels
            .as_ref()
            .and_then(|labels| labels.get("kata-deploy/instance"))
            .map(|s| s.as_str());

        // If the YAML doesn't have our instance label, skip ownership verification
        // This handles old resources created before labeling was implemented
        let expected_instance = match expected_instance {
            Some(instance) => instance,
            None => {
                log::warn!(
                    "YAML for {} '{}' missing kata-deploy/instance label - skipping deletion for safety",
                    api_resource.kind,
                    name
                );
                return Ok(());
            }
        };

        // Fetch the existing resource to verify ownership
        match api.get(name).await {
            Ok(existing) => {
                // Check if the instance label matches
                let current_instance = existing
                    .metadata
                    .labels
                    .as_ref()
                    .and_then(|labels| labels.get("kata-deploy/instance"))
                    .map(|s| s.as_str());

                match current_instance {
                    Some(instance) if instance == expected_instance => {
                        // We own this resource, safe to delete
                        info!(
                            "Deleting {} '{}' (instance: {})",
                            api_resource.kind, name, instance
                        );
                        let dp = DeleteParams::default();
                        api.delete(name, &dp).await?;
                        Ok(())
                    }
                    Some(instance) => {
                        // Resource exists but owned by different instance
                        log::warn!(
                            "Skipping deletion of {} '{}' - owned by instance '{}', not '{}'",
                            api_resource.kind,
                            name,
                            instance,
                            expected_instance
                        );
                        Ok(())
                    }
                    None => {
                        // Resource exists but has no instance label
                        log::warn!(
                            "Skipping deletion of {} '{}' - missing kata-deploy/instance label",
                            api_resource.kind,
                            name
                        );
                        Ok(())
                    }
                }
            }
            Err(kube::Error::Api(e)) if e.code == 404 => {
                // Resource doesn't exist
                if ignore_not_found {
                    log::debug!(
                        "{} '{}' not found (already deleted)",
                        api_resource.kind,
                        name
                    );
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "{} '{}' not found",
                        api_resource.kind,
                        name
                    ))
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Get all RuntimeClasses from Kubernetes
    pub async fn list_runtimeclasses(
        &self,
    ) -> Result<Vec<k8s_openapi::api::node::v1::RuntimeClass>> {
        use k8s_openapi::api::node::v1::RuntimeClass;
        use kube::api::{Api, ListParams};

        let api: Api<RuntimeClass> = Api::all(self.client.clone());
        let lp = ListParams::default();
        let runtimeclasses = api.list(&lp).await?;

        Ok(runtimeclasses.iter().cloned().collect())
    }

    /// Get a specific RuntimeClass by name
    #[cfg(test)]
    #[allow(dead_code)]
    pub async fn get_runtimeclass(
        &self,
        name: &str,
    ) -> Result<Option<k8s_openapi::api::node::v1::RuntimeClass>> {
        use k8s_openapi::api::node::v1::RuntimeClass;
        use kube::api::Api;

        let api: Api<RuntimeClass> = Api::all(self.client.clone());
        match api.get(name).await {
            Ok(rc) => Ok(Some(rc)),
            Err(kube::Error::Api(e)) if e.code == 404 => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update a RuntimeClass
    pub async fn update_runtimeclass(
        &self,
        runtimeclass: &k8s_openapi::api::node::v1::RuntimeClass,
    ) -> Result<()> {
        use k8s_openapi::api::node::v1::RuntimeClass;
        use kube::api::{Api, Patch, PatchParams};

        let api: Api<RuntimeClass> = Api::all(self.client.clone());
        let name = runtimeclass
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("RuntimeClass missing name"))?;

        let patch = Patch::Merge(runtimeclass);
        let pp = PatchParams::default();
        api.patch(name, &pp, &patch).await?;

        Ok(())
    }
}

// Public API functions that use the client
pub async fn get_container_runtime_version(config: &Config) -> Result<String> {
    let client = K8sClient::new(&config.node_name).await?;
    client.get_container_runtime_version().await
}

pub async fn get_node_label(config: &Config, key: &str) -> Result<Option<String>> {
    let client = K8sClient::new(&config.node_name).await?;
    client.get_node_label(key).await
}

pub async fn get_node_ready_status(config: &Config) -> Result<String> {
    let client = K8sClient::new(&config.node_name).await?;
    let node = client.get_node().await?;

    if let Some(status) = &node.status {
        if let Some(conditions) = &status.conditions {
            for condition in conditions {
                if condition.type_ == "Ready" {
                    return Ok(condition.status.clone());
                }
            }
        }
    }

    Ok("Unknown".to_string())
}

pub async fn label_node(
    config: &Config,
    label_key: &str,
    label_value: Option<&str>,
    overwrite: bool,
) -> Result<()> {
    let client = K8sClient::new(&config.node_name).await?;
    client.label_node(label_key, label_value, overwrite).await
}

pub async fn own_daemonset_exists(config: &Config) -> Result<bool> {
    let client = K8sClient::new(&config.node_name).await?;
    client.own_daemonset_exists(&config.daemonset_name).await
}

pub async fn count_any_kata_deploy_daemonsets(config: &Config) -> Result<usize> {
    let client = K8sClient::new(&config.node_name).await?;
    client.count_any_kata_deploy_daemonsets().await
}

pub async fn crd_exists(config: &Config, crd_name: &str) -> Result<bool> {
    let client = K8sClient::new(&config.node_name).await?;
    client.crd_exists(crd_name).await
}

pub async fn apply_yaml(config: &Config, yaml_content: &str) -> Result<()> {
    let client = K8sClient::new(&config.node_name).await?;
    client.apply_yaml(yaml_content).await
}

pub async fn delete_yaml(
    config: &Config,
    yaml_content: &str,
    ignore_not_found: bool,
) -> Result<()> {
    let client = K8sClient::new(&config.node_name).await?;
    client.delete_yaml(yaml_content, ignore_not_found).await
}

pub async fn list_runtimeclasses(
    config: &Config,
) -> Result<Vec<k8s_openapi::api::node::v1::RuntimeClass>> {
    let client = K8sClient::new(&config.node_name).await?;
    client.list_runtimeclasses().await
}

pub async fn update_runtimeclass(
    config: &Config,
    runtimeclass: &k8s_openapi::api::node::v1::RuntimeClass,
) -> Result<()> {
    let client = K8sClient::new(&config.node_name).await?;
    client.update_runtimeclass(runtimeclass).await
}
