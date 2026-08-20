// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, GetParams, ListParams, Patch, PatchParams},
    core::Request,
    Client,
};
use log::info;
use serde_json::json;

/// A concurrent taint update is a lost race, not a broken node.
const TAINT_PATCH_ATTEMPTS: u32 = 3;

/// Same for labels another install rewrote while we were deciding what to write.
const LABEL_PATCH_ATTEMPTS: u32 = 5;

/// A rejected precondition, as opposed to a request that failed on its merits.
/// The apiserver answers a failing JSON Patch `test` with 422, and a genuine write
/// conflict with 409.
/// A label key as a JSON Pointer token: `~` and `/` are the two characters with a
/// meaning of their own there (RFC 6901).
fn escape_pointer(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn is_precondition_failure(err: &kube::Error) -> bool {
    matches!(err, kube::Error::Api(status) if status.code == 409 || status.code == 422)
}

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

    pub async fn get_node_labels(&self) -> Result<std::collections::BTreeMap<String, String>> {
        Ok(self.get_node().await?.metadata.labels.unwrap_or_default())
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

    /// Read the bound node's labels, decide what to write from them, and write it
    /// only if nothing else changed the node in between.
    ///
    /// The decision is read from marks other installs write at the same time, so an
    /// unconditional write could act on a node that has already moved on: two
    /// uninstalls each seeing the other's mark, each removing their own, leaving a
    /// node advertising Kata with nothing installed.
    ///
    /// `decide` returns the label writes (`None` removes) and whatever the caller
    /// concluded from the labels it saw.
    pub async fn rewrite_node_labels<F, T>(&self, decide: F) -> Result<T>
    where
        F: Fn(&std::collections::BTreeMap<String, String>) -> (Vec<(String, Option<String>)>, T),
    {
        for attempt in 1..=LABEL_PATCH_ATTEMPTS {
            let node = self.get_node().await?;
            let version = node.metadata.resource_version.clone().unwrap_or_default();
            let labels = node.metadata.labels.unwrap_or_default();

            let (updates, outcome) = decide(&labels);
            if updates.is_empty() {
                return Ok(outcome);
            }

            let mut ops =
                vec![json!({"op": "test", "path": "/metadata/resourceVersion", "value": version})];
            for (key, value) in &updates {
                let path = format!("/metadata/labels/{}", escape_pointer(key));
                match value {
                    Some(value) => ops.push(json!({"op": "add", "path": path, "value": value})),
                    // `remove` is what makes this rejectable: it fails when the key
                    // is already gone, which is another way of saying we read a
                    // stale node.
                    None => ops.push(json!({"op": "remove", "path": path})),
                }
            }

            let patch: json_patch::Patch =
                serde_json::from_value(json!(ops)).context("Failed to build the label patch")?;

            match self
                .node_api
                .patch(
                    &self.node_name,
                    &PatchParams::default(),
                    &Patch::Json::<Node>(patch),
                )
                .await
            {
                Ok(_) => {
                    for (key, value) in &updates {
                        match value {
                            Some(value) => {
                                info!("Set label {}={} on node {}", key, value, self.node_name)
                            }
                            None => {
                                info!("Removed label {} from node {}", key, self.node_name)
                            }
                        }
                    }
                    return Ok(outcome);
                }
                Err(e) if is_precondition_failure(&e) => {
                    info!(
                        "Labels on node {} changed while they were being rewritten (attempt \
                         {}/{}); reading them again",
                        self.node_name, attempt, LABEL_PATCH_ATTEMPTS
                    );
                }
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!("Failed to patch the labels on node {}", self.node_name)
                    })
                }
            }
        }

        anyhow::bail!(
            "Gave up rewriting the labels on node {} after {} attempts: something keeps changing \
             them concurrently",
            self.node_name,
            LABEL_PATCH_ATTEMPTS
        )
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
    ///
    /// `.spec.taints` is an atomic list server-side, so removing one means writing
    /// the whole list back - which would silently drop a taint some controller
    /// added in the meantime, in the direction that admits workloads. Testing the
    /// resourceVersion that was read makes that a rejected write instead, and a
    /// rejection just means reading again.
    pub async fn remove_node_taints(&self, matchers: &[String]) -> Result<Vec<String>> {
        if matchers.is_empty() {
            return Ok(Vec::new());
        }

        for attempt in 1..=TAINT_PATCH_ATTEMPTS {
            let node = self.get_node().await?;
            let version = node.metadata.resource_version.clone().unwrap_or_default();
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

            let patch: json_patch::Patch = serde_json::from_value(json!([
                {"op": "test", "path": "/metadata/resourceVersion", "value": version},
                {"op": "replace", "path": "/spec/taints", "value": retained},
            ]))
            .context("Failed to build the taint patch")?;

            match self
                .node_api
                .patch(
                    &self.node_name,
                    &PatchParams::default(),
                    &Patch::Json::<Node>(patch),
                )
                .await
            {
                Ok(_) => return Ok(removed),
                Err(e) if is_precondition_failure(&e) => {
                    info!(
                        "Taints on node {} changed while they were being removed (attempt {}/{}); \
                         reading them again",
                        self.node_name, attempt, TAINT_PATCH_ATTEMPTS
                    );
                }
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!("Failed to patch node {} to remove taints", self.node_name)
                    })
                }
            }
        }

        anyhow::bail!(
            "Gave up removing taints from node {} after {} attempts: something keeps changing \
             them concurrently",
            self.node_name,
            TAINT_PATCH_ATTEMPTS
        )
    }

    /// Whether this DaemonSet still wants a pod on the bound node.
    ///
    /// Existence alone cannot distinguish a rolling restart from selector
    /// shrinkage: in both cases the old pod terminates while the DaemonSet
    /// remains. Cleanup is skipped only in the first case.
    pub async fn own_daemonset_selects_node(&self, daemonset_name: &str) -> Result<bool> {
        use k8s_openapi::api::apps::v1::DaemonSet;
        use kube::api::Api;

        let ds_api: Api<DaemonSet> = Api::default_namespaced(self.client.clone());
        match ds_api.get_opt(daemonset_name).await? {
            Some(ds) if ds.metadata.deletion_timestamp.is_none() => {
                let node = self.get_node().await?;
                Ok(daemonset_selects_node(&ds, &node))
            }
            _ => Ok(false),
        }
    }

    /// Whether another kata-deploy DaemonSet actually selects this node.
    ///
    /// A cluster-wide DaemonSet count is not ownership: a release selecting a
    /// different pool must not preserve this node's label. Legacy releases have
    /// no per-install marker, so their live pod is the node-local evidence.
    pub async fn other_kata_deploy_daemonset_selects_node(&self, ours: &str) -> Result<bool> {
        use k8s_openapi::api::apps::v1::DaemonSet;

        let node = self.get_node().await?;
        let own_uid = Api::<DaemonSet>::default_namespaced(self.client.clone())
            .get_opt(ours)
            .await?
            .and_then(|daemonset| daemonset.metadata.uid);
        let daemonsets: Api<DaemonSet> = Api::all(self.client.clone());
        Ok(daemonsets
            .list(&ListParams::default())
            .await?
            .items
            .into_iter()
            .any(|daemonset| {
                daemonset.metadata.deletion_timestamp.is_none()
                    && daemonset.metadata.uid != own_uid
                    && daemonset
                        .metadata
                        .name
                        .as_deref()
                        .is_some_and(|name| name.contains("kata-deploy"))
                    && daemonset_selects_node(&daemonset, &node)
            }))
    }
}

fn daemonset_selects_node(daemonset: &k8s_openapi::api::apps::v1::DaemonSet, node: &Node) -> bool {
    let Some(pod_spec) = daemonset
        .spec
        .as_ref()
        .and_then(|spec| spec.template.spec.as_ref())
    else {
        return false;
    };
    let labels = node.metadata.labels.as_ref().cloned().unwrap_or_default();
    if pod_spec.node_selector.as_ref().is_some_and(|selector| {
        selector
            .iter()
            .any(|(key, value)| labels.get(key) != Some(value))
    }) {
        return false;
    }

    let required = pod_spec
        .affinity
        .as_ref()
        .and_then(|affinity| affinity.node_affinity.as_ref())
        .and_then(|affinity| {
            affinity
                .required_during_scheduling_ignored_during_execution
                .as_ref()
        });
    let Some(required) = required else {
        return true;
    };

    required.node_selector_terms.iter().any(|term| {
        // Kubernetes reads a term with no requirement in it as matching no nodes,
        // while `all` over nothing is true. Left alone, an empty term would hand
        // the whole cluster to a DaemonSet that asked for none of it.
        if term.match_expressions.iter().flatten().next().is_none()
            && term.match_fields.iter().flatten().next().is_none()
        {
            return false;
        }
        term.match_expressions.iter().flatten().all(|requirement| {
            selector_requirement_matches(
                labels.get(&requirement.key).map(String::as_str),
                &requirement.operator,
                requirement.values.as_deref().unwrap_or_default(),
            )
        }) && term.match_fields.iter().flatten().all(|requirement| {
            let value = match requirement.key.as_str() {
                "metadata.name" => node.metadata.name.as_deref(),
                _ => None,
            };
            selector_requirement_matches(
                value,
                &requirement.operator,
                requirement.values.as_deref().unwrap_or_default(),
            )
        })
    })
}

fn selector_requirement_matches(value: Option<&str>, operator: &str, values: &[String]) -> bool {
    match operator {
        "In" => value.is_some_and(|value| values.iter().any(|candidate| candidate == value)),
        "NotIn" => value.is_none_or(|value| values.iter().all(|candidate| candidate != value)),
        "Exists" => value.is_some(),
        "DoesNotExist" => value.is_none(),
        "Gt" => value
            .and_then(|value| value.parse::<i64>().ok())
            .zip(values.first().and_then(|value| value.parse::<i64>().ok()))
            .is_some_and(|(actual, threshold)| actual > threshold),
        "Lt" => value
            .and_then(|value| value.parse::<i64>().ok())
            .zip(values.first().and_then(|value| value.parse::<i64>().ok()))
            .is_some_and(|(actual, threshold)| actual < threshold),
        _ => false,
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

pub async fn get_node_labels(
    config: &Config,
) -> Result<std::collections::BTreeMap<String, String>> {
    let client = K8sClient::new(&config.node_name).await?;
    client.get_node_labels().await
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

/// The CRI runtime handlers the kubelet reports this node's runtime as serving.
///
/// `None` means the node does not report them at all - the kubelet only fills
/// this in with RecursiveReadOnlyMounts or UserNamespacesSupport enabled, and an
/// older runtime returns none - which is not the same as kata being missing. An
/// empty list, on the other hand, is an answer: a runtime serving no handler at
/// all.
pub async fn get_node_runtime_handlers(config: &Config) -> Result<Option<Vec<String>>> {
    let client = K8sClient::new(&config.node_name).await?;
    let node = client.get_node().await?;

    Ok(node
        .status
        .and_then(|status| status.runtime_handlers)
        .map(|handlers| handlers.into_iter().filter_map(|h| h.name).collect()))
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

pub async fn rewrite_node_labels<F, T>(config: &Config, decide: F) -> Result<T>
where
    F: Fn(&std::collections::BTreeMap<String, String>) -> (Vec<(String, Option<String>)>, T),
{
    let client = K8sClient::new(&config.node_name).await?;
    client.rewrite_node_labels(decide).await
}

pub async fn remove_node_taints(config: &Config, matchers: &[String]) -> Result<Vec<String>> {
    let client = K8sClient::new(&config.node_name).await?;
    client.remove_node_taints(matchers).await
}

pub async fn own_daemonset_selects_node(config: &Config) -> Result<bool> {
    let client = K8sClient::new(&config.node_name).await?;
    client
        .own_daemonset_selects_node(&config.daemonset_name)
        .await
}

pub async fn other_kata_deploy_daemonset_selects_node(config: &Config) -> Result<bool> {
    let client = K8sClient::new(&config.node_name).await?;
    client
        .other_kata_deploy_daemonset_selects_node(&config.daemonset_name)
        .await
}

#[cfg(test)]
mod tests {
    use super::{daemonset_selects_node, partition_taints};
    use k8s_openapi::api::apps::v1::DaemonSet;
    use k8s_openapi::api::core::v1::{Node, Taint};
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

    #[test]
    fn daemonset_ownership_follows_node_selector_and_required_affinity() {
        let daemonset: DaemonSet = serde_yaml::from_str(
            r#"
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: kata-deploy
spec:
  selector:
    matchLabels:
      name: kata-deploy
  template:
    metadata:
      labels:
        name: kata-deploy
    spec:
      nodeSelector:
        pool: kata
      affinity:
        nodeAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
            nodeSelectorTerms:
              - matchExpressions:
                  - key: kubernetes.io/arch
                    operator: In
                    values: [amd64]
      containers:
        - name: kata-deploy
          image: example.invalid/kata-deploy
"#,
        )
        .unwrap();
        let mut node: Node = serde_yaml::from_str(
            r#"
apiVersion: v1
kind: Node
metadata:
  name: worker
  labels:
    pool: kata
    kubernetes.io/arch: amd64
"#,
        )
        .unwrap();

        assert!(daemonset_selects_node(&daemonset, &node));
        node.metadata
            .labels
            .as_mut()
            .unwrap()
            .insert("pool".to_string(), "other".to_string());
        assert!(!daemonset_selects_node(&daemonset, &node));
    }

    /// An empty required term, or an empty list of them, selects no nodes in
    /// Kubernetes. Reading either as "every node" would let a DaemonSet that wants
    /// nothing keep this node's label or block its cleanup.
    #[rstest]
    #[case::empty_term("\n              - {}")]
    #[case::no_terms(" []")]
    fn a_required_affinity_that_matches_nothing_selects_no_node(#[case] terms: &str) {
        let daemonset: DaemonSet = serde_yaml::from_str(&format!(
            r#"
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: kata-deploy
spec:
  selector:
    matchLabels:
      name: kata-deploy
  template:
    metadata:
      labels:
        name: kata-deploy
    spec:
      affinity:
        nodeAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
            nodeSelectorTerms:{terms}
      containers:
        - name: kata-deploy
          image: example.invalid/kata-deploy
"#
        ))
        .unwrap();
        let node: Node = serde_yaml::from_str(
            r#"
apiVersion: v1
kind: Node
metadata:
  name: worker
  labels:
    pool: kata
"#,
        )
        .unwrap();

        assert!(!daemonset_selects_node(&daemonset, &node));
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
