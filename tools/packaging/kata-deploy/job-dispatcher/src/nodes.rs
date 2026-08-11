// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! Everything the dispatcher does *to* a node, and everything it reads *from* one
//! on the per-node Jobs' behalf.
//!
//! This exists so the per-node Jobs can run with no ServiceAccount token at all.
//! They are the privileged, host-mutating half of the install and they run on
//! every node Kata is installed on, including nodes that also run untrusted
//! workloads; root there can read any token mounted into a pod on that node. The
//! dispatcher, by contrast, is one unprivileged pod that an operator can pin to
//! trusted nodes, so the node-scoped API work belongs here.

use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::{Node, Taint};
use kube::api::{Api, GetParams, Patch, PatchParams, Request};
use kube::Client;
use log::{info, warn};
use serde_json::json;
use std::time::{Duration, Instant};

/// The RuntimeClasses select on this label, so it is the switch that admits Kata
/// workloads to a node.
pub const KATA_RUNTIME_LABEL: &str = "katacontainers.io/kata-runtime";

/// The facts about a node that the per-node Job would otherwise have to read from
/// the apiserver itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeFacts {
    pub name: String,
    /// `.status.nodeInfo.containerRuntimeVersion`, e.g. `containerd://2.1.5-k3s1`.
    pub container_runtime_version: Option<String>,
}

impl NodeFacts {
    pub fn from_node(node: &Node) -> Self {
        let name = node.metadata.name.clone().unwrap_or_default();
        let container_runtime_version = node
            .status
            .as_ref()
            .and_then(|status| status.node_info.as_ref())
            .map(|info| info.container_runtime_version.clone())
            .filter(|version| !version.is_empty());

        Self {
            name,
            container_runtime_version,
        }
    }

    /// Reached when a node was named explicitly and the GET for it failed; the
    /// install is left to look the rest up itself.
    pub fn name_only(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }
}

/// The node-level work the dispatcher performs around each per-node Job.
pub struct NodeOps {
    api: Api<Node>,
    client: Client,
    pub label_value: Option<String>,
    pub remove_label: bool,
    /// Matchers, each `key` (any effect) or `key:effect`.
    pub remove_taints: Vec<String>,
    pub wait_ready: Option<Duration>,
    pub kubelet_timeout_warn: Option<Duration>,
}

impl NodeOps {
    pub fn new(client: &Client) -> Self {
        Self {
            api: Api::all(client.clone()),
            client: client.clone(),
            label_value: None,
            remove_label: false,
            remove_taints: Vec::new(),
            wait_ready: None,
            kubelet_timeout_warn: None,
        }
    }

    pub async fn get(&self, node: &str) -> Result<Node> {
        self.api
            .get(node)
            .await
            .with_context(|| format!("failed to get node {node}"))
    }

    /// Unlabelling first is what makes cleanup safe: the scheduler stops placing
    /// Kata workloads on the node before anything on it is taken apart.
    pub async fn before_dispatch(&self, node: &str) -> Result<()> {
        if self.remove_label {
            self.set_label(node, None).await?;
        }
        if let Some(threshold) = self.kubelet_timeout_warn {
            self.warn_on_low_kubelet_timeout(node, threshold).await;
        }
        Ok(())
    }

    /// Order matters: the node has to be Ready (its CRI runtime was just
    /// restarted) before it is advertised as Kata-capable, and the start-up
    /// taints may only be lifted once that advertisement is in place - they are
    /// what keeps workloads off the node until then.
    pub async fn after_success(&self, node: &str) -> Result<()> {
        let Some(value) = self.label_value.clone() else {
            return Ok(());
        };

        if let Some(timeout) = self.wait_ready {
            self.wait_till_ready(node, timeout).await?;
        }

        self.set_label(node, Some(&value)).await?;
        self.verify_label(node, &value).await?;
        self.lift_taints(node).await;

        Ok(())
    }

    /// Set (or, with `None`, remove) the Kata runtime label on `node`.
    async fn set_label(&self, node: &str, value: Option<&str>) -> Result<()> {
        // A JSON merge patch removes a key by setting it to null; omitting it
        // would leave it untouched.
        let patch = match value {
            Some(value) => json!({"metadata": {"labels": {KATA_RUNTIME_LABEL: value}}}),
            None => json!({"metadata": {"labels": {KATA_RUNTIME_LABEL: serde_json::Value::Null}}}),
        };

        self.api
            .patch(node, &PatchParams::default(), &Patch::Merge(&patch))
            .await
            .with_context(|| match value {
                Some(value) => format!("failed to label node {node} {KATA_RUNTIME_LABEL}={value}"),
                None => format!("failed to remove label {KATA_RUNTIME_LABEL} from node {node}"),
            })?;

        match value {
            Some(value) => info!("node {node}: labelled {KATA_RUNTIME_LABEL}={value}"),
            None => info!("node {node}: removed label {KATA_RUNTIME_LABEL}"),
        }
        Ok(())
    }

    /// Read the label back, so a silently dropped patch (a mutating webhook, say)
    /// fails the node instead of leaving it advertised-but-unlabelled.
    async fn verify_label(&self, node: &str, expected: &str) -> Result<()> {
        let observed = self
            .get(node)
            .await?
            .metadata
            .labels
            .as_ref()
            .and_then(|labels| labels.get(KATA_RUNTIME_LABEL))
            .cloned();

        match observed.as_deref() {
            Some(value) if value == expected => Ok(()),
            other => anyhow::bail!(
                "node {node} does not carry {KATA_RUNTIME_LABEL}={expected} after patching it \
                 (observed: {}); workloads would not be scheduled there",
                other.unwrap_or("<absent>")
            ),
        }
    }

    /// Best-effort on purpose: the runtime is installed and the node is labelled
    /// by now, and a taint left in place only keeps workloads away - the safe
    /// direction - so a failure here warns and leaves a later run to retry rather
    /// than failing an otherwise complete install.
    async fn lift_taints(&self, node: &str) {
        if self.remove_taints.is_empty() {
            return;
        }

        match self.try_lift_taints(node).await {
            Ok(removed) if removed.is_empty() => {
                info!(
                    "node {node}: no matching start-up taint to remove ({})",
                    self.remove_taints.join(", ")
                );
            }
            Ok(removed) => info!(
                "node {node}: removed start-up taint(s) {}",
                removed.join(", ")
            ),
            Err(err) => warn!(
                "node {node}: could not remove start-up taint(s) {} ({err:#}). Kata is installed \
                 and the node is labelled, but workloads will stay off it until the taint goes; \
                 a later run retries",
                self.remove_taints.join(", ")
            ),
        }
    }

    async fn try_lift_taints(&self, node: &str) -> Result<Vec<String>> {
        let current = self
            .get(node)
            .await?
            .spec
            .and_then(|spec| spec.taints)
            .unwrap_or_default();
        if current.is_empty() {
            return Ok(Vec::new());
        }

        let (retained, removed) = partition_taints(current, &self.remove_taints);
        if removed.is_empty() {
            return Ok(removed);
        }

        // `.spec.taints` is an atomic list server-side, so the retained set
        // replaces it wholesale.
        let patch = json!({"spec": {"taints": retained}});
        self.api
            .patch(node, &PatchParams::default(), &Patch::Merge(&patch))
            .await
            .with_context(|| format!("failed to patch taints on node {node}"))?;

        Ok(removed)
    }

    /// Wait for the node's Ready condition, which is how we know the CRI runtime
    /// the install just restarted is serving again.
    async fn wait_till_ready(&self, node: &str, timeout: Duration) -> Result<()> {
        let start = Instant::now();
        let mut announced = false;

        loop {
            let ready = match self.get(node).await {
                Ok(n) => node_ready_condition(&n).unwrap_or_else(|| "Unknown".to_string()),
                Err(err) => {
                    warn!("node {node}: could not read readiness ({err:#})");
                    "Unknown".to_string()
                }
            };

            if ready == "True" {
                return Ok(());
            }

            if start.elapsed() >= timeout {
                anyhow::bail!(
                    "node {node} did not become Ready within {}s of its install finishing (last \
                     seen: {ready}); not labelling it, so workloads are not sent to a node whose \
                     CRI runtime may still be restarting",
                    timeout.as_secs()
                );
            }

            if !announced {
                info!(
                    "node {node}: waiting up to {}s for it to report Ready after the CRI runtime \
                     restart",
                    timeout.as_secs()
                );
                announced = true;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Advisory pre-flight: a kubelet `runtimeRequestTimeout` well below the time
    /// a large image needs can abort `CreateContainer` mid-pull.
    ///
    /// Warning-only, and deliberately never fatal - it used to run inside the
    /// per-node Job, and the only reason it moved here is that it needs the
    /// apiserver (`nodes/proxy`), which those Jobs no longer have.
    async fn warn_on_low_kubelet_timeout(&self, node: &str, threshold: Duration) {
        let timeout = match self.kubelet_runtime_request_timeout(node).await {
            Ok(Some(value)) => value,
            Ok(None) => {
                warn!("node {node}: kubelet /configz did not report runtimeRequestTimeout");
                return;
            }
            Err(err) => {
                warn!("node {node}: could not read kubelet runtimeRequestTimeout ({err:#})");
                return;
            }
        };

        let parsed = match humantime::parse_duration(&timeout) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    "node {node}: could not parse kubelet runtimeRequestTimeout {timeout} ({err})"
                );
                return;
            }
        };

        if parsed < threshold {
            warn!(
                "node {node}: kubelet runtimeRequestTimeout is {timeout} ({}s). Pulling a large \
                 image, or converting one, happens during CreateContainer and can exceed it; \
                 consider raising it to at least {}s on nodes running Kata",
                parsed.as_secs(),
                threshold.as_secs()
            );
        } else {
            info!(
                "node {node}: kubelet runtimeRequestTimeout is {timeout} ({}s)",
                parsed.as_secs()
            );
        }
    }

    async fn kubelet_runtime_request_timeout(&self, node: &str) -> Result<Option<String>> {
        let request = Request::new(format!("/api/v1/nodes/{node}/proxy"))
            .get("configz", &GetParams::default())?;
        let configz: serde_json::Value = self
            .client
            .request(request)
            .await
            .with_context(|| format!("failed to query kubelet /configz for node {node}"))?;

        Ok(configz
            .get("kubeletconfig")
            .or_else(|| configz.get("kubeletConfig"))
            .and_then(|config| config.get("runtimeRequestTimeout"))
            .and_then(|value| value.as_str())
            .map(str::to_string))
    }
}

/// The node's `Ready` condition status, if it reports one.
fn node_ready_condition(node: &Node) -> Option<String> {
    node.status
        .as_ref()?
        .conditions
        .as_ref()?
        .iter()
        .find(|condition| condition.type_ == "Ready")
        .map(|condition| condition.status.clone())
}

/// Each matcher is `key` (any effect) or `key:effect` (that effect only). A
/// matcher that matches nothing is not an error: on a re-run the taint is simply
/// already gone, which is the expected steady state.
fn partition_taints(taints: Vec<Taint>, matchers: &[String]) -> (Vec<Taint>, Vec<String>) {
    let parsed: Vec<(&str, Option<&str>)> = matchers
        .iter()
        .map(|matcher| match matcher.split_once(':') {
            Some((key, effect)) => (key.trim(), Some(effect.trim())),
            None => (matcher.trim(), None),
        })
        .filter(|(key, _)| !key.is_empty())
        .collect();

    let mut retained = Vec::new();
    let mut removed = Vec::new();

    for taint in taints {
        let matched = parsed.iter().any(|(key, effect)| {
            taint.key == *key && effect.map(|e| e == taint.effect).unwrap_or(true)
        });
        if matched {
            removed.push(format!("{}:{}", taint.key, taint.effect));
        } else {
            retained.push(taint);
        }
    }

    (retained, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{NodeCondition, NodeStatus, NodeSystemInfo};
    fn taint(key: &str, effect: &str) -> Taint {
        Taint {
            key: key.to_string(),
            effect: effect.to_string(),
            value: None,
            time_added: None,
        }
    }

    fn keys(taints: &[Taint]) -> Vec<(String, String)> {
        taints
            .iter()
            .map(|t| (t.key.clone(), t.effect.clone()))
            .collect()
    }

    /// A bare key removes the taint whatever its effect; `key:effect` is exact.
    #[test]
    fn taint_matchers_respect_the_effect() {
        let taints = vec![
            taint("kata/startup", "NoSchedule"),
            taint("kata/startup", "NoExecute"),
            taint("other", "NoSchedule"),
        ];

        let (retained, removed) = partition_taints(taints.clone(), &["kata/startup".to_string()]);
        assert_eq!(keys(&retained), vec![("other".into(), "NoSchedule".into())]);
        assert_eq!(removed.len(), 2);

        let (retained, removed) = partition_taints(taints, &["kata/startup:NoExecute".to_string()]);
        assert_eq!(removed, vec!["kata/startup:NoExecute".to_string()]);
        assert_eq!(keys(&retained).len(), 2);
    }

    /// A matcher that matches nothing leaves the taints alone and reports nothing
    /// removed, so a re-run is a no-op rather than a failure.
    #[test]
    fn unmatched_matchers_change_nothing() {
        let taints = vec![taint("other", "NoSchedule")];
        let (retained, removed) = partition_taints(taints, &["kata/startup".to_string()]);
        assert_eq!(keys(&retained), vec![("other".into(), "NoSchedule".into())]);
        assert!(removed.is_empty());
    }

    /// The facts handed to a per-node Job come straight off the Node object the
    /// dispatcher already listed, so no extra API call is needed to collect them.
    #[test]
    fn facts_are_read_off_the_node() {
        let node = Node {
            metadata: kube::core::ObjectMeta {
                name: Some("node-1".to_string()),
                ..Default::default()
            },
            status: Some(NodeStatus {
                node_info: Some(NodeSystemInfo {
                    container_runtime_version: "containerd://2.1.5".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let facts = NodeFacts::from_node(&node);
        assert_eq!(facts.name, "node-1");
        assert_eq!(
            facts.container_runtime_version.as_deref(),
            Some("containerd://2.1.5")
        );
    }

    /// A node with no runtime version reported yields no override, so the install
    /// falls back to looking it up rather than acting on an empty string.
    #[test]
    fn missing_facts_are_absent_not_empty() {
        let node = Node {
            metadata: kube::core::ObjectMeta {
                name: Some("node-2".to_string()),
                ..Default::default()
            },
            status: Some(NodeStatus {
                node_info: Some(NodeSystemInfo {
                    container_runtime_version: String::new(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let facts = NodeFacts::from_node(&node);
        assert!(facts.container_runtime_version.is_none());
    }

    /// Readiness is read from the Ready condition, not from the first condition.
    #[test]
    fn ready_condition_is_picked_by_type() {
        let node = Node {
            status: Some(NodeStatus {
                conditions: Some(vec![
                    NodeCondition {
                        type_: "MemoryPressure".to_string(),
                        status: "False".to_string(),
                        ..Default::default()
                    },
                    NodeCondition {
                        type_: "Ready".to_string(),
                        status: "True".to_string(),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(node_ready_condition(&node).as_deref(), Some("True"));
        assert_eq!(node_ready_condition(&Node::default()), None);
    }
}
