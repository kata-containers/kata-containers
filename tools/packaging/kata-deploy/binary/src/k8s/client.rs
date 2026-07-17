// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, DeleteParams, DynamicObject, GetParams, Patch, PatchParams},
    core::Request,
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

    pub async fn get_kubelet_runtime_request_timeout(&self) -> Result<Option<String>> {
        let request = Request::new(format!("/api/v1/nodes/{}/proxy", self.node_name))
            .get("configz", &GetParams::default())?;

        let configz: serde_json::Value = self.client.request(request).await.with_context(|| {
            format!(
                "Failed to query kubelet configz for node {}",
                self.node_name
            )
        })?;

        Ok(configz
            .get("kubeletconfig")
            .or_else(|| configz.get("kubeletConfig"))
            .and_then(|kubelet_config| kubelet_config.get("runtimeRequestTimeout"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()))
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

    /// Remove taints from the bound node.
    ///
    /// `matchers` is a list of `key` or `key:effect` entries. A bare key removes
    /// every taint with that key regardless of effect; `key:effect` removes only
    /// the taint matching both. Taints not matched are left untouched.
    ///
    /// Returns the matcher labels that matched and were removed. A matcher that
    /// matches nothing is not an error: the node simply had no such taint, which
    /// is the expected steady state on re-runs and pod restarts.
    pub async fn remove_node_taints(&self, matchers: &[String]) -> Result<Vec<String>> {
        if matchers.is_empty() {
            return Ok(Vec::new());
        }

        let node = self.get_node().await?;
        let current = node
            .spec
            .as_ref()
            .and_then(|s| s.taints.clone())
            .unwrap_or_default();

        if current.is_empty() {
            return Ok(Vec::new());
        }

        let (retained, removed) = partition_taints(current, matchers);

        if removed.is_empty() {
            return Ok(removed);
        }

        for label in &removed {
            info!("Removing taint {} from node {}", label, self.node_name);
        }

        // `.spec.taints` is an atomic list server-side, so we replace it wholesale
        // with the retained set. A JSON-merge patch on the whole array is
        // equivalent here; we use a merge patch for consistency with label_node
        // and to avoid resourceVersion juggling.
        let patch = Patch::Merge(json!({
            "spec": {
                "taints": retained,
            }
        }));

        let pp = PatchParams::default();
        self.node_api
            .patch(&self.node_name, &pp, &patch)
            .await
            .with_context(|| format!("Failed to patch node {} to remove taints", self.node_name))?;

        Ok(removed)
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

/// Split `taints` into (retained, removed-labels) according to `matchers`.
///
/// Each matcher is `key` (matches any effect) or `key:effect` (matches only that
/// effect). Pure and cluster-free so the matching rules can be unit-tested; the
/// async `remove_node_taints` method wraps this with the apiserver read/patch.
fn partition_taints(
    taints: Vec<k8s_openapi::api::core::v1::Taint>,
    matchers: &[String],
) -> (Vec<k8s_openapi::api::core::v1::Taint>, Vec<String>) {
    // Split each matcher into (key, optional effect) once up front.
    let parsed: Vec<(&str, Option<&str>)> = matchers
        .iter()
        .map(|m| match m.split_once(':') {
            Some((k, e)) => (k.trim(), Some(e.trim())),
            None => (m.trim(), None),
        })
        .filter(|(k, _)| !k.is_empty())
        .collect();

    let mut removed = Vec::new();
    let retained = taints
        .into_iter()
        .filter(|taint| {
            let hit = parsed.iter().find(|(key, effect)| {
                taint.key == *key && effect.map(|e| e == taint.effect).unwrap_or(true)
            });
            match hit {
                Some((key, effect)) => {
                    let label = match effect {
                        Some(e) => format!("{key}:{e}"),
                        None => (*key).to_string(),
                    };
                    removed.push(label);
                    false
                }
                None => true,
            }
        })
        .collect();

    (retained, removed)
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

pub async fn get_kubelet_runtime_request_timeout(config: &Config) -> Result<Option<String>> {
    let client = K8sClient::new(&config.node_name).await?;
    client.get_kubelet_runtime_request_timeout().await
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

pub async fn remove_node_taints(config: &Config, matchers: &[String]) -> Result<Vec<String>> {
    let client = K8sClient::new(&config.node_name).await?;
    client.remove_node_taints(matchers).await
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

#[cfg(test)]
mod tests {
    use super::partition_taints;
    use k8s_openapi::api::core::v1::Taint;
    use rstest::rstest;

    fn taint(key: &str, effect: &str) -> Taint {
        Taint {
            key: key.to_string(),
            effect: effect.to_string(),
            value: None,
            time_added: None,
        }
    }

    fn build(pairs: &[(&str, &str)]) -> Vec<Taint> {
        pairs.iter().map(|(k, e)| taint(k, e)).collect()
    }

    fn keys(taints: &[Taint]) -> Vec<(String, String)> {
        taints
            .iter()
            .map(|t| (t.key.clone(), t.effect.clone()))
            .collect()
    }

    /// `partition_taints` keeps every taint except those matched by a matcher.
    /// A bare key matches any effect; `key:effect` matches only that effect;
    /// matchers are trimmed; blank matchers and non-matches remove nothing.
    #[rstest]
    // bare key removes every effect for that key, leaving others untouched
    #[case::bare_key_removes_all_effects(
        &[("kata.io/not-ready", "NoSchedule"), ("kata.io/not-ready", "NoExecute"), ("other", "NoSchedule")],
        &["kata.io/not-ready"],
        &[("other", "NoSchedule")],
        &["kata.io/not-ready", "kata.io/not-ready"],
    )]
    // key:effect removes only the matching effect
    #[case::key_effect_removes_only_matching_effect(
        &[("kata.io/not-ready", "NoSchedule"), ("kata.io/not-ready", "NoExecute")],
        &["kata.io/not-ready:NoSchedule"],
        &[("kata.io/not-ready", "NoExecute")],
        &["kata.io/not-ready:NoSchedule"],
    )]
    // no matcher matches: everything retained, nothing removed
    #[case::no_match_retains_everything(
        &[("some-other-taint", "NoSchedule")],
        &["kata.io/not-ready"],
        &[("some-other-taint", "NoSchedule")],
        &[],
    )]
    // key matches but effect differs: not removed
    #[case::effect_mismatch_is_not_removed(
        &[("kata.io/not-ready", "NoExecute")],
        &["kata.io/not-ready:NoSchedule"],
        &[("kata.io/not-ready", "NoExecute")],
        &[],
    )]
    // empty / whitespace-only matchers remove nothing
    #[case::blank_matchers_remove_nothing(
        &[("kata.io/not-ready", "NoSchedule")],
        &["", "   "],
        &[("kata.io/not-ready", "NoSchedule")],
        &[],
    )]
    // surrounding whitespace in a key:effect matcher is trimmed before matching
    #[case::whitespace_around_matcher_is_trimmed(
        &[("kata.io/not-ready", "NoSchedule")],
        &["  kata.io/not-ready : NoSchedule "],
        &[],
        &["kata.io/not-ready:NoSchedule"],
    )]
    fn test_partition_taints(
        #[case] taints: &[(&str, &str)],
        #[case] matchers: &[&str],
        #[case] expected_retained: &[(&str, &str)],
        #[case] expected_removed: &[&str],
    ) {
        let matchers: Vec<String> = matchers.iter().map(|s| s.to_string()).collect();
        let (retained, removed) = partition_taints(build(taints), &matchers);

        assert_eq!(
            keys(&retained),
            build(expected_retained)
                .iter()
                .map(|t| (t.key.clone(), t.effect.clone()))
                .collect::<Vec<_>>(),
            "retained taints mismatch",
        );
        assert_eq!(
            removed,
            expected_removed
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            "removed labels mismatch",
        );
    }
}
