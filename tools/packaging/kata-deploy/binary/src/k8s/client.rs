// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, GetParams, Patch, PatchParams},
    core::Request,
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
