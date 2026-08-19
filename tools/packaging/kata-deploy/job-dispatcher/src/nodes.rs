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
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// The RuntimeClasses select on this label, so it is the switch that admits Kata
/// workloads to a node.
pub const KATA_RUNTIME_LABEL: &str = "katacontainers.io/kata-runtime";

/// Marks a node as being installed on: enough for `helm uninstall` to find it,
/// not enough for a workload to be scheduled there.
pub const KATA_RUNTIME_PENDING: &str = "false";

/// The kubelet republishes node status every ~10s, and `runtimeHandlers` trails
/// the runtime restart by a sync or two.
const HANDLER_WAIT: Duration = Duration::from_secs(120);

/// A kubelet that re-registers after the CRI restart republishes its cached
/// labels over ours, so one confirmation proves nothing. Six spaced ones outlive
/// a status-update period comfortably.
const LABEL_STABILITY_CHECKS: u32 = 6;
const LABEL_CHECK_INTERVAL: Duration = Duration::from_secs(2);
const LABEL_APPLY_ATTEMPTS: u32 = 12;

/// A concurrent taint update is a lost race, not a broken node.
const TAINT_PATCH_ATTEMPTS: u32 = 3;

/// Same for a node that gained the label while it was being claimed.
const CLAIM_PATCH_ATTEMPTS: u32 = 3;

/// Same for a node whose labels another install rewrote mid-decision.
const LABEL_PATCH_ATTEMPTS: u32 = 5;

/// Every install marks its own nodes with a label named after its
/// multiInstallSuffix. Without it, an uninstall removing the shared
/// [`KATA_RUNTIME_LABEL`] could not tell whether it leaves another install's
/// workloads with nowhere to run.
const INSTANCE_LABEL_PREFIX: &str = "kata-deploy.katacontainers.io/";

/// The instance name of an install that set no multiInstallSuffix.
const DEFAULT_INSTANCE: &str = "default";

/// The marker label of one install.
pub fn instance_label(suffix: Option<&str>) -> String {
    format!(
        "{INSTANCE_LABEL_PREFIX}{}",
        suffix.unwrap_or(DEFAULT_INSTANCE)
    )
}

/// What the marks of the other installs on a node say about the label they share.
#[derive(Debug, PartialEq, Eq)]
enum SharedLabel {
    /// At least one other install is serving Kata here: the shared label stays `true`.
    Keep,
    /// The other installs here are all mid-flight or failed. Nothing may be scheduled
    /// on the strength of it, but the key has to stay so those installs' own uninstalls
    /// can still find the node.
    Demote,
    /// Ours was the last mark: the key goes.
    Remove,
}

/// Read the marks other installs left on a node.
///
/// The value matters, not just the key: a `false` mark is a claim on a node whose
/// install has not finished, and reading it as "Kata is served here" would leave the
/// node advertised with nothing behind it.
fn shared_label_after(labels: &BTreeMap<String, String>, ours: &str) -> SharedLabel {
    let mut any = false;
    for (key, value) in labels {
        if key == ours || !key.starts_with(INSTANCE_LABEL_PREFIX) {
            continue;
        }
        any = true;
        if value != KATA_RUNTIME_PENDING {
            return SharedLabel::Keep;
        }
    }

    if any {
        SharedLabel::Demote
    } else {
        SharedLabel::Remove
    }
}

/// The kubelet may be unreachable through the apiserver proxy, and this check is
/// only advisory: it must not hold up the node it is about, let alone the queue
/// behind it.
const KUBELET_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// The facts about a node that the per-node Job would otherwise have to read from
/// the apiserver itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeFacts {
    pub name: String,
    /// `.status.nodeInfo.containerRuntimeVersion`, e.g. `containerd://2.1.5-k3s1`.
    pub container_runtime_version: Option<String>,
    pub machine_id: Option<String>,
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
        let machine_id = node
            .status
            .as_ref()
            .and_then(|status| status.node_info.as_ref())
            .map(|info| info.machine_id.clone())
            .filter(|id| !id.is_empty());

        Self {
            name,
            container_runtime_version,
            machine_id,
        }
    }
}

/// The node-level work the dispatcher performs around each per-node Job.
#[derive(Clone)]
pub struct NodeOps {
    api: Api<Node>,
    client: Client,
    pub label_value: Option<String>,
    pub remove_label: bool,
    /// Claim the node as being installed on before its Job starts, so a failure
    /// anywhere in that Job still leaves something for `helm uninstall` to find.
    pub claim_pending: bool,
    /// Matchers, each `key` (any effect) or `key:effect`.
    pub remove_taints: Vec<String>,
    pub wait_ready: Option<Duration>,
    /// Handlers the node's CRI runtime must be serving before the node may be
    /// labelled. Empty skips the check.
    pub require_handlers: Vec<String>,
    pub kubelet_timeout_warn: Option<Duration>,
    /// This install's own marker label; see [`INSTANCE_LABEL_PREFIX`].
    pub instance_label: String,
}

impl NodeOps {
    pub fn new(client: &Client) -> Self {
        Self {
            api: Api::all(client.clone()),
            client: client.clone(),
            label_value: None,
            remove_label: false,
            claim_pending: false,
            remove_taints: Vec::new(),
            wait_ready: None,
            require_handlers: Vec::new(),
            kubelet_timeout_warn: None,
            instance_label: instance_label(None),
        }
    }

    pub async fn get(&self, node: &str) -> Result<Node> {
        self.api
            .get(node)
            .await
            .with_context(|| format!("failed to get node {node}"))
    }

    async fn ensure_uid(&self, node: &str, expected_uid: &str) -> Result<()> {
        let current = self.get(node).await?;
        anyhow::ensure!(
            current.metadata.uid.as_deref() == Some(expected_uid),
            "node {node} changed identity: expected UID {expected_uid}, found {:?}",
            current.metadata.uid
        );
        Ok(())
    }

    /// Demoting first is what makes cleanup safe: nothing new is scheduled onto a
    /// node whose label no longer says `true` (the RuntimeClasses require exactly
    /// that value), so the node stops taking Kata workloads before anything on it
    /// is taken apart.
    ///
    /// Demoted rather than removed, because the label's *key* is what an uninstall
    /// selects on. A cleanup Job that fails - or is never created - would otherwise
    /// leave a node with Kata still installed that the next `helm uninstall` cannot
    /// even see. The key goes once the node's cleanup Job has actually succeeded.
    pub async fn before_dispatch(&self, node: &str, expected_uid: &str) -> Result<()> {
        self.ensure_uid(node, expected_uid).await?;
        if self.remove_label {
            self.demote(node, expected_uid).await?;
        }
        if self.claim_pending {
            self.claim(node, expected_uid).await?;
        }
        if let Some(threshold) = self.kubelet_timeout_warn {
            self.warn_on_low_kubelet_timeout(node, threshold).await;
        }
        self.ensure_uid(node, expected_uid).await?;
        Ok(())
    }

    /// Order matters: the node has to be Ready (its CRI runtime was just
    /// restarted) before it is advertised as Kata-capable, and the start-up
    /// taints may only be lifted once that advertisement is in place - they are
    /// what keeps workloads off the node until then.
    pub async fn after_success(&self, node: &str, expected_uid: &str) -> Result<()> {
        self.ensure_uid(node, expected_uid).await?;
        // Cleanup: the node kept the label's key through its Job so that a failure
        // anywhere in it would still be found by the next uninstall. That reason is
        // spent now, and leaving the key behind would have a later uninstall clean
        // a node that has nothing left on it.
        if self.remove_label {
            self.release(node, expected_uid).await?;
            return self.ensure_uid(node, expected_uid).await;
        }

        let Some(value) = self.label_value.clone() else {
            return Ok(());
        };

        if let Some(timeout) = self.wait_ready {
            self.wait_till_ready(node, timeout).await?;
        }

        self.ensure_uid(node, expected_uid).await?;
        self.verify_handlers(node).await?;
        self.ensure_uid(node, expected_uid).await?;
        self.label_until_stable(node, &value, expected_uid).await?;
        self.ensure_uid(node, expected_uid).await?;
        self.lift_taints(node, expected_uid).await;

        Ok(())
    }

    /// Take a node out of service for the cleanup about to happen, keeping the key
    /// an uninstall selects on. A node that never had the label is left alone: it
    /// was never installed on, and adding a key here would only invite the next
    /// uninstall to come back for it.
    async fn demote(&self, node: &str, expected_uid: &str) -> Result<()> {
        self.rewrite_labels(node, expected_uid, |labels, ours| {
            let mut updates: Vec<(String, Option<String>)> = Vec::new();

            if labels.contains_key(ours) {
                updates.push((ours.to_string(), Some(KATA_RUNTIME_PENDING.to_string())));
            }

            // Our RuntimeClasses select our own mark, so demoting that is what keeps
            // this install's workloads off the node. The shared label is another
            // question: an install still serving Kata here needs it.
            let shared = labels.get(KATA_RUNTIME_LABEL).map(String::as_str);
            let verdict = shared_label_after(labels, ours);
            match (shared, verdict) {
                (None, _) | (Some(KATA_RUNTIME_PENDING), _) => (),
                (Some(_), SharedLabel::Keep) => info!(
                    "node {node}: leaving {KATA_RUNTIME_LABEL} in place, another kata-deploy \
                     install is still serving Kata from this node"
                ),
                (Some(_), _) => updates.push((
                    KATA_RUNTIME_LABEL.to_string(),
                    Some(KATA_RUNTIME_PENDING.to_string()),
                )),
            }

            updates
        })
        .await
    }

    /// Give the node back: this install's marker goes, and the label it shares with
    /// every other install goes with it only if no other install is left holding
    /// this node.
    async fn release(&self, node: &str, expected_uid: &str) -> Result<()> {
        self.rewrite_labels(node, expected_uid, |labels, ours| {
            let mut updates: Vec<(String, Option<String>)> = Vec::new();
            if labels.contains_key(ours) {
                updates.push((ours.to_string(), None));
            }

            let verdict = shared_label_after(labels, ours);
            match verdict {
                SharedLabel::Keep => info!(
                    "node {node}: keeping {KATA_RUNTIME_LABEL}, another kata-deploy install is \
                     still serving Kata from this node"
                ),
                // Unfinished installs elsewhere: they need the key to find this node
                // again, but nothing may be scheduled here meanwhile.
                SharedLabel::Demote => {
                    if labels.get(KATA_RUNTIME_LABEL).map(String::as_str)
                        != Some(KATA_RUNTIME_PENDING)
                    {
                        info!(
                            "node {node}: leaving {KATA_RUNTIME_LABEL}={KATA_RUNTIME_PENDING}, \
                             one or more kata-deploy installs have claimed this node but none \
                             has finished installing on it"
                        );
                        updates.push((
                            KATA_RUNTIME_LABEL.to_string(),
                            Some(KATA_RUNTIME_PENDING.to_string()),
                        ));
                    }
                }
                SharedLabel::Remove => {
                    if labels.contains_key(KATA_RUNTIME_LABEL) {
                        updates.push((KATA_RUNTIME_LABEL.to_string(), None));
                    }
                }
            }

            updates
        })
        .await
    }

    /// Read a node's labels, decide what to write from them, and write it only if
    /// nothing else changed the node in between.
    ///
    /// The decision is read from labels other installs write at the same time, so an
    /// unconditional write could act on a node that has already moved on: two
    /// uninstalls each seeing the other's mark, each removing their own, leaving a
    /// node advertising Kata with nothing installed.
    async fn rewrite_labels<F>(&self, node: &str, expected_uid: &str, decide: F) -> Result<()>
    where
        F: Fn(&BTreeMap<String, String>, &str) -> Vec<(String, Option<String>)>,
    {
        for attempt in 1..=LABEL_PATCH_ATTEMPTS {
            let fetched = self.get(node).await?;
            anyhow::ensure!(
                fetched.metadata.uid.as_deref() == Some(expected_uid),
                "node {node} changed identity before its labels could be rewritten"
            );
            let version = fetched
                .metadata
                .resource_version
                .clone()
                .unwrap_or_default();
            let labels = fetched.metadata.labels.unwrap_or_default();

            let updates = decide(&labels, &self.instance_label);
            if updates.is_empty() {
                return Ok(());
            }

            match self
                .patch_labels_guarded(node, expected_uid, &version, &updates)
                .await
            {
                Ok(()) => return Ok(()),
                Err(err) if err.is_conflict => {
                    info!(
                        "node {node}: its labels changed while they were being rewritten \
                         (attempt {attempt}/{LABEL_PATCH_ATTEMPTS}); reading them again"
                    );
                }
                Err(err) => return Err(err.error),
            }
        }

        anyhow::bail!(
            "gave up rewriting the labels on node {node} after {LABEL_PATCH_ATTEMPTS} attempts: \
             something keeps changing them concurrently"
        )
    }

    /// One label write, rejected if the node changed since `version` was read.
    async fn patch_labels_guarded(
        &self,
        node: &str,
        expected_uid: &str,
        version: &str,
        updates: &[(String, Option<String>)],
    ) -> std::result::Result<(), GuardedPatchError> {
        let mut ops = vec![
            json!({"op": "test", "path": "/metadata/uid", "value": expected_uid}),
            json!({"op": "test", "path": "/metadata/resourceVersion", "value": version}),
        ];
        for (key, value) in updates {
            let path = format!("/metadata/labels/{}", escape_pointer(key));
            match value {
                Some(value) => ops.push(json!({"op": "add", "path": path, "value": value})),
                // `remove` is what makes this rejectable: it fails when the key is
                // already gone, which is another way of saying we read a stale node.
                None => ops.push(json!({"op": "remove", "path": path})),
            }
        }

        let patch: json_patch::Patch =
            serde_json::from_value(json!(ops)).map_err(|err| GuardedPatchError {
                is_conflict: false,
                error: anyhow::Error::new(err).context("failed to build the label patch"),
            })?;

        match self
            .api
            .patch(node, &PatchParams::default(), &Patch::Json::<Node>(patch))
            .await
        {
            Ok(_) => {
                info!("node {node}: labels {}", describe_updates(updates));
                Ok(())
            }
            Err(err) => Err(GuardedPatchError {
                is_conflict: is_precondition_failure(&err),
                error: anyhow::Error::new(err).context(format!(
                    "failed to write labels {} on node {node}",
                    describe_updates(updates)
                )),
            }),
        }
    }

    /// Conditional on the label still being absent when the write lands. Two
    /// dispatchers can be mid-flight over one node - an upgrade racing the install
    /// it replaces - and claiming a node another one has just finished labelling
    /// `true` would de-advertise a node that is serving Kata.
    async fn claim(&self, node: &str, expected_uid: &str) -> Result<()> {
        for attempt in 1..=CLAIM_PATCH_ATTEMPTS {
            let fetched = self.get(node).await.with_context(|| {
                format!("could not read node {node} to claim it before install")
            })?;
            anyhow::ensure!(
                fetched.metadata.uid.as_deref() == Some(expected_uid),
                "node {node} changed identity before it could be claimed"
            );

            let labels = fetched.metadata.labels.unwrap_or_default();
            // Any value means someone has been here: a `true` must not be
            // downgraded mid-upgrade, and a `false` is already the claim.
            let missing: Vec<&str> = [KATA_RUNTIME_LABEL, self.instance_label.as_str()]
                .into_iter()
                .filter(|key| !labels.contains_key(*key))
                .collect();
            if missing.is_empty() {
                return Ok(());
            }

            let version = fetched
                .metadata
                .resource_version
                .clone()
                .unwrap_or_default();
            // `add` needs its parent to exist; a Node without labels is only ever
            // seen in tests, but the patch has to be valid for it too.
            let mut ops = vec![
                json!({"op": "test", "path": "/metadata/uid", "value": expected_uid}),
                json!({"op": "test", "path": "/metadata/resourceVersion", "value": version}),
            ];
            if labels.is_empty() {
                let claimed: BTreeMap<&str, &str> = missing
                    .iter()
                    .map(|key| (*key, KATA_RUNTIME_PENDING))
                    .collect();
                ops.push(json!({"op": "add", "path": "/metadata/labels", "value": claimed}));
            } else {
                for key in &missing {
                    ops.push(json!({"op": "add",
                                    "path": format!("/metadata/labels/{}", escape_pointer(key)),
                                    "value": KATA_RUNTIME_PENDING}));
                }
            }

            let patch: json_patch::Patch = serde_json::from_value(json!(ops))
                .context("could not build the node claim patch")?;

            match self
                .api
                .patch(node, &PatchParams::default(), &Patch::Json::<Node>(patch))
                .await
            {
                Ok(_) => {
                    info!("node {node}: marked as being installed on");
                    return Ok(());
                }
                Err(err) if is_precondition_failure(&err) => {
                    info!(
                        "node {node}: its labels changed while it was being claimed \
                         (attempt {attempt}/{CLAIM_PATCH_ATTEMPTS}); reading them again"
                    );
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!(
                            "could not claim node {node} before install; refusing to mutate a host \
                             that a later uninstall could not discover"
                        )
                    });
                }
            }
        }

        anyhow::bail!(
            "gave up claiming node {node} after {CLAIM_PATCH_ATTEMPTS} attempts: something keeps \
             changing its labels; refusing to mutate a host that a later uninstall could not \
             discover"
        )
    }

    /// Refuse to advertise a node whose CRI runtime is not serving what the
    /// install wrote.
    ///
    /// `.status.runtimeHandlers` is the node's own answer about what its runtime
    /// loaded, as opposed to what we wrote and hoped it would read - and asking
    /// needs the apiserver, which is exactly what the per-node Jobs no longer
    /// have. A node reporting none of the handlers has a runtime that never read
    /// the configuration, and labelling it would advertise a node that cannot run
    /// a single Kata pod.
    async fn verify_handlers(&self, node: &str) -> Result<()> {
        if self.require_handlers.is_empty() {
            return Ok(());
        }

        let start = Instant::now();
        loop {
            let served = match self.get(node).await {
                Ok(fetched) => served_handlers(&fetched),
                // Not an answer, and must not be read as one: giving up here would
                // label the node on the strength of a failed request.
                Err(err) => {
                    if start.elapsed() >= HANDLER_WAIT {
                        return Err(err).with_context(|| {
                            format!(
                                "could not check whether node {node} is serving {:?} within {}s \
                                 of its install finishing; not labelling it, since nothing has \
                                 confirmed its CRI runtime read what was installed",
                                self.require_handlers,
                                HANDLER_WAIT.as_secs()
                            )
                        });
                    }
                    warn!(
                        "node {node}: could not read its runtime handlers ({err:#}); trying again"
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            match handler_verdict(&self.require_handlers, served.as_deref()) {
                HandlerVerdict::Serving { serving, missing } => {
                    info!("node {node}: CRI runtime is serving {serving:?}");
                    // Expected on a mixed-architecture release, and a real symptom
                    // when it is not: a handler this node should serve and does not
                    // means pods asking for it will not start here.
                    if !missing.is_empty() {
                        warn!(
                            "node {node}: CRI runtime is not serving {missing:?}. Expected for \
                             handlers built for another architecture; otherwise pods requesting \
                             them will not start on this node"
                        );
                    }
                    return Ok(());
                }
                // Older clusters, and runtimes that report nothing, cannot answer
                // this. Never a reason to fail an install that otherwise worked.
                HandlerVerdict::Unanswerable => {
                    info!(
                        "node {node}: does not report runtime handlers at all (Kubernetes below \
                         1.30, or a kubelet that does not publish them), so what its runtime \
                         loaded cannot be checked from here"
                    );
                    return Ok(());
                }
                HandlerVerdict::NotServing if start.elapsed() >= HANDLER_WAIT => {
                    anyhow::bail!(
                        "node {node} reports none of {:?} among its runtime handlers {}s after its \
                         install finished, so its CRI runtime is not serving the Kata \
                         configuration that install wrote. Check the runtime's logs for a rejected \
                         or unread configuration file; not labelling the node, since no Kata pod \
                         could run there",
                        self.require_handlers,
                        HANDLER_WAIT.as_secs()
                    )
                }
                HandlerVerdict::NotServing => (),
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Apply the label and require it to stay applied.
    ///
    /// On k3s and RKE2 the CRI restart takes the kubelet with it, and a kubelet
    /// coming back re-registers its node with the labels it had cached, silently
    /// undoing ours. `Ready` does not rule that out: the observation can predate
    /// the kubelet's own restart. So the label has to be seen to hold, and be
    /// re-applied when it drifts.
    async fn label_until_stable(&self, node: &str, value: &str, expected_uid: &str) -> Result<()> {
        // Both labels, because the RuntimeClasses select both: a marker the kubelet
        // clobbered leaves the node advertised but unusable by this install.
        let wanted = [KATA_RUNTIME_LABEL, self.instance_label.as_str()];
        let updates: Vec<(String, Option<String>)> = wanted
            .iter()
            .map(|key| (key.to_string(), Some(value.to_string())))
            .collect();

        for attempt in 1..=LABEL_APPLY_ATTEMPTS {
            self.rewrite_labels(node, expected_uid, |_, _| updates.clone())
                .await?;

            let mut stable = 0;
            while stable < LABEL_STABILITY_CHECKS {
                tokio::time::sleep(LABEL_CHECK_INTERVAL).await;

                match self.read_labels(node, expected_uid).await {
                    Ok(labels) => {
                        let drifted: Vec<String> = wanted
                            .iter()
                            .filter(|key| labels.get(**key).map(String::as_str) != Some(value))
                            .map(|key| format!("{key}={:?}", labels.get(*key).map(String::as_str)))
                            .collect();

                        if drifted.is_empty() {
                            stable += 1;
                            continue;
                        }

                        warn!(
                            "node {node}: {} after {stable}/{LABEL_STABILITY_CHECKS} stable \
                             observation(s); re-applying (attempt \
                             {attempt}/{LABEL_APPLY_ATTEMPTS})",
                            drifted.join(", ")
                        );
                        break;
                    }
                    Err(err) => {
                        warn!(
                            "node {node}: could not confirm its labels ({err:#}); re-applying \
                             (attempt {attempt}/{LABEL_APPLY_ATTEMPTS})"
                        );
                        break;
                    }
                }
            }

            if stable >= LABEL_STABILITY_CHECKS {
                info!("node {node}: {} are holding", describe_updates(&updates));
                return Ok(());
            }
        }

        anyhow::bail!(
            "node {node} did not hold {} for {LABEL_STABILITY_CHECKS} consecutive checks over \
             {LABEL_APPLY_ATTEMPTS} attempts; something on the node keeps removing them, and \
             workloads would not be scheduled there",
            describe_updates(&updates)
        )
    }

    async fn read_labels(
        &self,
        node: &str,
        expected_uid: &str,
    ) -> Result<BTreeMap<String, String>> {
        let fetched = self.get(node).await?;
        anyhow::ensure!(
            fetched.metadata.uid.as_deref() == Some(expected_uid),
            "node {node} changed identity while its labels were being verified"
        );
        Ok(fetched.metadata.labels.unwrap_or_default())
    }

    /// Best-effort on purpose: the runtime is installed and the node is labelled
    /// by now, and a taint left in place only keeps workloads away - the safe
    /// direction - so a failure here warns and leaves a later run to retry rather
    /// than failing an otherwise complete install.
    async fn lift_taints(&self, node: &str, expected_uid: &str) {
        if self.remove_taints.is_empty() {
            return;
        }

        match self.try_lift_taints(node, expected_uid).await {
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

    /// `.spec.taints` is an atomic list server-side, so removing one means writing
    /// the whole list back - which would silently drop a taint some controller
    /// added in the meantime, in the direction that admits workloads. A JSON Patch
    /// that tests the resourceVersion it read makes that a rejected write instead,
    /// and rejection just means reading again.
    async fn try_lift_taints(&self, node: &str, expected_uid: &str) -> Result<Vec<String>> {
        for attempt in 1..=TAINT_PATCH_ATTEMPTS {
            let fetched = self.get(node).await?;
            anyhow::ensure!(
                fetched.metadata.uid.as_deref() == Some(expected_uid),
                "node {node} changed identity before its taints could be removed"
            );
            let version = fetched
                .metadata
                .resource_version
                .clone()
                .unwrap_or_default();
            let current = fetched
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

            let patch: json_patch::Patch = serde_json::from_value(json!([
                {"op": "test", "path": "/metadata/uid", "value": expected_uid},
                {"op": "test", "path": "/metadata/resourceVersion", "value": version},
                {"op": "replace", "path": "/spec/taints", "value": retained},
            ]))
            .context("failed to build the taint patch")?;

            match self
                .api
                .patch(node, &PatchParams::default(), &Patch::Json::<Node>(patch))
                .await
            {
                Ok(_) => return Ok(removed),
                Err(err) if is_precondition_failure(&err) => {
                    info!(
                        "node {node}: its taints changed while they were being lifted \
                         (attempt {attempt}/{TAINT_PATCH_ATTEMPTS}); reading them again"
                    );
                }
                Err(err) => {
                    return Err(err)
                        .with_context(|| format!("failed to patch taints on node {node}"))
                }
            }
        }

        anyhow::bail!(
            "gave up lifting taints on node {node} after {TAINT_PATCH_ATTEMPTS} attempts: \
             something keeps changing them concurrently"
        )
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
        let probe = tokio::time::timeout(
            KUBELET_PROBE_TIMEOUT,
            self.kubelet_runtime_request_timeout(node),
        );

        let timeout = match probe.await {
            Ok(Ok(Some(value))) => value,
            Ok(Ok(None)) => {
                warn!("node {node}: kubelet /configz did not report runtimeRequestTimeout");
                return;
            }
            Ok(Err(err)) => {
                warn!("node {node}: could not read kubelet runtimeRequestTimeout ({err:#})");
                return;
            }
            Err(_) => {
                warn!(
                    "node {node}: kubelet /configz did not answer within {}s; skipping the \
                     runtimeRequestTimeout warning",
                    KUBELET_PROBE_TIMEOUT.as_secs()
                );
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

/// A label write that was rejected, and whether re-reading the node could make it
/// succeed.
struct GuardedPatchError {
    is_conflict: bool,
    error: anyhow::Error,
}

fn describe_updates(updates: &[(String, Option<String>)]) -> String {
    updates
        .iter()
        .map(|(key, value)| match value {
            Some(value) => format!("{key}={value}"),
            None => format!("{key} (removed)"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A label key as a JSON Pointer token: `~` and `/` are the two characters with a
/// meaning of their own there (RFC 6901).
fn escape_pointer(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// A rejected precondition, as opposed to a request that failed on its merits.
/// The apiserver answers a failing JSON Patch `test` with 422, and a genuine
/// write conflict with 409.
fn is_precondition_failure(err: &kube::Error) -> bool {
    matches!(err, kube::Error::Api(status) if status.code == 409 || status.code == 422)
}

/// What the node says its CRI runtime is serving.
///
/// `None` only when the node does not report the field at all - clusters below
/// Kubernetes 1.30, or a kubelet that does not publish it. A node that reports an
/// empty list has answered, and the answer is "nothing": that is a runtime which
/// read no Kata configuration, not a cluster that cannot be asked.
fn served_handlers(node: &Node) -> Option<Vec<String>> {
    Some(
        node.status
            .as_ref()?
            .runtime_handlers
            .as_ref()?
            .iter()
            .filter_map(|handler| handler.name.clone())
            .collect(),
    )
}

#[derive(Debug, PartialEq, Eq)]
enum HandlerVerdict {
    /// At least one expected handler is served. `missing` are the rest, which for
    /// a mixed-architecture release is the normal case.
    Serving {
        serving: Vec<String>,
        missing: Vec<String>,
    },
    NotServing,
    Unanswerable,
}

/// Any one of the expected handlers is enough to pass.
///
/// The chart cannot know a node's architecture, so it names every handler the
/// release could install, and a node legitimately serves only the subset built for
/// its own arch. Demanding all of them would fail every heterogeneous fleet, so
/// "none of them" is the verdict worth failing on: it means the runtime never read
/// what the install wrote. The remainder is reported, not enforced.
fn handler_verdict(expected: &[String], served: Option<&[String]>) -> HandlerVerdict {
    let Some(served) = served else {
        return HandlerVerdict::Unanswerable;
    };

    let (serving, missing): (Vec<String>, Vec<String>) = expected
        .iter()
        .cloned()
        .partition(|handler| served.contains(handler));

    if serving.is_empty() {
        HandlerVerdict::NotServing
    } else {
        HandlerVerdict::Serving { serving, missing }
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
    use k8s_openapi::api::core::v1::{
        NodeCondition, NodeRuntimeHandler, NodeStatus, NodeSystemInfo,
    };

    fn node_serving(handlers: &[&str]) -> Node {
        Node {
            status: Some(NodeStatus {
                runtime_handlers: Some(
                    handlers
                        .iter()
                        .map(|name| NodeRuntimeHandler {
                            name: Some(name.to_string()),
                            features: None,
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn expected(handlers: &[&str]) -> Vec<String> {
        handlers.iter().map(|h| h.to_string()).collect()
    }

    /// The chart names every handler the release could install, but a node only
    /// serves those built for its own architecture, so one is enough.
    #[test]
    fn one_expected_handler_is_enough() {
        let node = node_serving(&["runc", "kata-qemu"]);
        assert_eq!(
            handler_verdict(
                &expected(&["kata-qemu", "kata-qemu-snp"]),
                served_handlers(&node).as_deref()
            ),
            HandlerVerdict::Serving {
                serving: vec!["kata-qemu".to_string()],
                missing: vec!["kata-qemu-snp".to_string()],
            }
        );
    }

    /// A runtime serving nothing of ours never read what the install wrote.
    #[test]
    fn no_expected_handler_is_a_runtime_that_ignored_us() {
        let node = node_serving(&["runc"]);
        assert_eq!(
            handler_verdict(&expected(&["kata-qemu"]), served_handlers(&node).as_deref()),
            HandlerVerdict::NotServing
        );
    }

    /// A node that reports no handlers at all (an older kubelet, a CRI that does
    /// not report them) cannot answer, which must never fail an install.
    #[test]
    fn a_node_that_cannot_answer_never_fails_an_install() {
        assert_eq!(served_handlers(&Node::default()), None);
        assert_eq!(
            handler_verdict(&expected(&["kata-qemu"]), None),
            HandlerVerdict::Unanswerable
        );
    }

    /// An empty list is an answer, and the answer is "nothing of ours". Reading it
    /// as "cannot be asked" would label a node whose runtime read no Kata
    /// configuration at all.
    #[test]
    fn an_empty_list_is_an_answer() {
        assert_eq!(served_handlers(&node_serving(&[])), Some(Vec::new()));
        assert_eq!(
            handler_verdict(
                &expected(&["kata-qemu"]),
                served_handlers(&node_serving(&[])).as_deref()
            ),
            HandlerVerdict::NotServing
        );
    }

    #[test]
    fn an_install_is_named_after_its_suffix() {
        assert_eq!(
            instance_label(None),
            "kata-deploy.katacontainers.io/default"
        );
        assert_eq!(
            instance_label(Some("dev")),
            "kata-deploy.katacontainers.io/dev"
        );
    }

    /// The shared label may only be taken away when it is nobody else's.
    #[test]
    fn only_another_installs_mark_counts() {
        let ours = instance_label(Some("dev"));
        let mark = |keys: &[&str]| -> BTreeMap<String, String> {
            keys.iter()
                .map(|key| (key.to_string(), "true".to_string()))
                .collect()
        };

        assert_eq!(
            shared_label_after(&mark(&[&ours]), &ours),
            SharedLabel::Remove
        );
        assert_eq!(
            shared_label_after(
                &mark(&[&ours, KATA_RUNTIME_LABEL, "kubernetes.io/hostname"]),
                &ours
            ),
            SharedLabel::Remove,
            "neither the shared label nor an unrelated one is another install's mark"
        );
        assert_eq!(
            shared_label_after(&mark(&[&ours, &instance_label(None)]), &ours),
            SharedLabel::Keep
        );
        assert_eq!(
            shared_label_after(&mark(&[&instance_label(Some("prod"))]), &ours),
            SharedLabel::Keep
        );
    }

    /// Installs that have claimed the node but not finished on it must not keep it
    /// advertised as able to run Kata - and must still be able to find it.
    #[test]
    fn unfinished_installs_hold_the_key_without_the_promise() {
        let ours = instance_label(Some("dev"));
        let labels = BTreeMap::from([
            (ours.clone(), "true".to_string()),
            (instance_label(None), KATA_RUNTIME_PENDING.to_string()),
        ]);

        assert_eq!(shared_label_after(&labels, &ours), SharedLabel::Demote);

        // However many of them there are.
        let labels = BTreeMap::from([
            (instance_label(None), KATA_RUNTIME_PENDING.to_string()),
            (
                instance_label(Some("prod")),
                KATA_RUNTIME_PENDING.to_string(),
            ),
        ]);

        assert_eq!(shared_label_after(&labels, &ours), SharedLabel::Demote);
    }

    /// One install serving Kata is enough to keep the shared label, however many
    /// unfinished ones are read before it - and labels are read in key order, so
    /// "default" here is read before "prod".
    #[test]
    fn a_serving_install_outweighs_unfinished_ones() {
        let ours = instance_label(Some("dev"));
        let labels = BTreeMap::from([
            (instance_label(None), KATA_RUNTIME_PENDING.to_string()),
            (instance_label(Some("prod")), "true".to_string()),
        ]);

        assert_eq!(shared_label_after(&labels, &ours), SharedLabel::Keep);
    }

    /// The label key has a `/` in it, which is a path separator inside a JSON
    /// Pointer: unescaped, the claim would patch a key called "kata-runtime"
    /// nested under one called "katacontainers.io".
    #[test]
    fn a_label_key_is_escaped_for_a_json_pointer() {
        assert_eq!(
            escape_pointer(KATA_RUNTIME_LABEL),
            "katacontainers.io~1kata-runtime"
        );
        assert_eq!(escape_pointer("a~b/c"), "a~0b~1c");
    }

    #[test]
    fn only_conflicts_are_retried() {
        let status = |code: u16| {
            kube::Error::Api(kube::error::ErrorResponse {
                status: String::new(),
                message: String::new(),
                reason: String::new(),
                code,
            })
        };

        assert!(is_precondition_failure(&status(409)));
        assert!(is_precondition_failure(&status(422)));
        assert!(!is_precondition_failure(&status(403)));
        assert!(!is_precondition_failure(&status(404)));
    }
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
