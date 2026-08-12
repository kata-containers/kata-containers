// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! kata-deploy-job-dispatcher: a small, deployment-agnostic dispatcher that runs exactly
//! one node-pinned Job per selected node.
//!
//! Given a Job template (any `batch/v1` Job manifest) and a node selector, it
//! creates one Job per node — pinned to that node via `spec.nodeName` — keeps
//! at most `--parallelism` Jobs in flight at a time (refilling as they finish),
//! and exits non-zero if any node's Job failed. This gives paced rollouts with
//! *guaranteed per-node coverage*, which an Indexed Job / topology-spread
//! cannot guarantee once `parallelism < completions` (the scheduler ignores
//! completed pods when balancing the spread).
//!
//! It has no host dependencies and only needs RBAC to list nodes and to
//! create/get/delete Jobs in its namespace.

mod job;
mod node_filter;

use anyhow::{bail, Context, Result};
use clap::Parser;
use job::{
    build_node_job, interpret_status, job_name, job_owned_by, sanitize_label_value, JobOutcome,
    OWNER_LABEL,
};
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{Node, Toleration};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::{Api, ListParams, PostParams};
use kube::Client;
use log::{error, info};
use node_filter::{describe_taint, partition_by_tolerations, suggested_toleration, SkippedNode};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Run one node-pinned Job per selected node, paced and with guaranteed coverage."
)]
struct Args {
    /// Path to a YAML file containing the batch/v1 Job to run on each node.
    /// The dispatcher clones it per node and sets metadata.name + nodeName.
    #[arg(long)]
    job_template: String,

    /// Prefix for generated per-node Job names. Also recorded as the
    /// "kata-deploy-job-dispatcher/owner" label so the dispatcher tracks only its own Jobs.
    #[arg(long)]
    name_prefix: String,

    /// Namespace to create the per-node Jobs in. Defaults to $POD_NAMESPACE,
    /// then the in-cluster service-account namespace, then "default".
    #[arg(long)]
    namespace: Option<String>,

    /// Maximum number of per-node Jobs in flight at once.
    #[arg(long, default_value_t = 100)]
    parallelism: usize,

    /// Server-side label selector used to pick target nodes, e.g.
    /// "kubernetes.io/os=linux" or "node-role.kubernetes.io/control-plane".
    /// Supports the full label-selector grammar (In/NotIn/Exists/DoesNotExist).
    ///
    /// May be repeated, in which case the target set is the UNION of the
    /// per-selector matches. A single label selector expresses one AND-group of
    /// requirements, so repeating the flag is how OR is expressed - it mirrors
    /// nodeAffinity's `nodeSelectorTerms`, where terms are OR-ed while the
    /// requirements within a term are AND-ed.
    #[arg(long)]
    node_selector: Vec<String>,

    /// Server-side field selector used to pick target nodes (ANDed with the
    /// label selector).
    #[arg(long)]
    node_field_selector: Option<String>,

    /// Explicit comma-separated node names. When set, the node selectors are
    /// ignored and exactly these nodes are targeted.
    #[arg(long)]
    nodes: Option<String>,

    /// Target selector-matched nodes even when they carry a taint the Job's
    /// tolerations do not cover.
    ///
    /// By default such nodes are skipped, mirroring the scheduler's admission of
    /// an equivalent DaemonSet pod. Set this for reverse/cleanup runs, which must
    /// reach every node previously acted on even if it has been tainted since.
    #[arg(long, default_value_t = false)]
    ignore_node_taints: bool,

    /// Seconds to keep re-resolving the target nodes while none is eligible yet.
    ///
    /// A DaemonSet is level-triggered: it picks a node up whenever it becomes
    /// eligible. The dispatcher runs once, so on a fresh cluster it can resolve
    /// nodes before the labels its selectors match even exist - a cluster that
    /// installs node-feature-discovery alongside kata-deploy only grows the
    /// `feature.node.kubernetes.io/*` labels seconds later. Waiting closes that
    /// window; 0 resolves once and moves on.
    ///
    /// Setting this also declares that at least one node is expected, so an
    /// empty selection becomes an error once the wait expires instead of a
    /// silent no-op that would leave every node uninstalled.
    #[arg(long, default_value_t = 0)]
    wait_for_nodes_secs: u64,

    /// Optional owner Job name (in the dispatcher's namespace). When set, every
    /// per-node Job gets an ownerReference to it so they are garbage-collected
    /// together with the owner.
    #[arg(long)]
    owner_job_name: Option<String>,

    /// Seconds between status polls.
    #[arg(long, default_value_t = 5)]
    poll_interval_secs: u64,

    /// Page size used when listing nodes (server-side pagination).
    #[arg(long, default_value_t = 500)]
    node_page_size: u32,
}

// The dispatcher is overwhelmingly I/O-bound (apiserver round-trips); two worker
// threads are plenty and keep the footprint small.
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();

    let client = Client::try_default()
        .await
        .context("failed to create Kubernetes client")?;

    let namespace = resolve_namespace(args.namespace.clone());
    info!("kata-deploy-job-dispatcher starting (namespace: {namespace})");

    // Read the template up front: the tolerations in its pod spec decide which
    // tainted nodes the per-node Jobs are allowed to run on.
    let template_raw = std::fs::read_to_string(&args.job_template)
        .with_context(|| format!("failed to read job template {}", args.job_template))?;
    let template: Job = serde_yaml::from_str(&template_raw)
        .with_context(|| format!("failed to parse job template {}", args.job_template))?;

    let nodes = resolve_nodes(&client, &args, &template_tolerations(&template)).await?;
    if nodes.is_empty() {
        info!("no target nodes matched the selection; nothing to do");
        return Ok(());
    }

    let owner = match args.owner_job_name.as_deref() {
        Some(name) => Some(owner_ref_for_job(&client, &namespace, name).await?),
        None => None,
    };

    let jobs: Api<Job> = Api::namespaced(client.clone(), &namespace);

    let parallelism = args.parallelism.clamp(1, nodes.len());
    info!(
        "fanning out {} per-node Job(s) with parallelism {}",
        nodes.len(),
        parallelism
    );

    run_fanout(
        &jobs,
        &template,
        &nodes,
        &args,
        &namespace,
        parallelism,
        owner.as_ref(),
    )
    .await
}

/// Resolve the namespace to create Jobs in: explicit flag, then $POD_NAMESPACE,
/// then the in-cluster service-account namespace file, then "default".
fn resolve_namespace(flag: Option<String>) -> String {
    if let Some(ns) = flag.filter(|s| !s.trim().is_empty()) {
        return ns;
    }
    if let Ok(ns) = std::env::var("POD_NAMESPACE") {
        if !ns.trim().is_empty() {
            return ns;
        }
    }
    if let Ok(ns) =
        std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace")
    {
        let ns = ns.trim().to_string();
        if !ns.is_empty() {
            return ns;
        }
    }
    "default".to_string()
}

/// The tolerations the per-node Jobs will run with, read straight out of the
/// template's pod spec so node admission is checked against exactly what the
/// Jobs carry.
fn template_tolerations(template: &Job) -> Vec<Toleration> {
    template
        .spec
        .as_ref()
        .and_then(|spec| spec.template.spec.as_ref())
        .and_then(|pod| pod.tolerations.clone())
        .unwrap_or_default()
}

/// Turn the repeatable `--node-selector` flag into the list of LIST passes to
/// run. No selector at all means a single unfiltered pass (every node); each
/// selector otherwise contributes one pass, and the caller unions the results.
fn selector_passes(selectors: &[String]) -> Vec<Option<&str>> {
    if selectors.is_empty() {
        return vec![None];
    }
    selectors.iter().map(|s| Some(s.as_str())).collect()
}

/// Resolve the set of target node names: an explicit `--nodes` list when given,
/// otherwise a paginated, server-side-filtered LIST of nodes per selector, with
/// the results unioned (see `--node-selector`).
///
/// Selector-matched nodes are then held to the same taint rules a DaemonSet pod
/// would face, so job mode installs on the same nodes daemonset mode does. An
/// explicit `--nodes` list is taken verbatim: it has no daemonset equivalent and
/// names exact nodes, so it is honoured as a deliberate override.
///
/// Eligibility is re-checked until `--wait-for-nodes-secs` expires, so a node
/// that is still being labelled - or still carries a start-up taint - when the
/// dispatcher starts is picked up rather than missed.
async fn resolve_nodes(
    client: &Client,
    args: &Args,
    tolerations: &[Toleration],
) -> Result<Vec<String>> {
    if let Some(list) = args.nodes.as_deref() {
        let mut names: Vec<String> = list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        names.sort();
        names.dedup();
        return Ok(names);
    }

    let api: Api<Node> = Api::all(client.clone());
    let poll = Duration::from_secs(args.poll_interval_secs.max(1));
    let deadline = Instant::now() + Duration::from_secs(args.wait_for_nodes_secs);
    let mut announced_wait = false;

    loop {
        let (admitted, skipped) = select_nodes(&api, args, tolerations).await?;

        if !admitted.is_empty() || Instant::now() >= deadline {
            for node in &skipped {
                info!(
                    "skipping node {}: it carries the taint {}, which this install does not \
                     tolerate (a DaemonSet would not have been scheduled there either)",
                    node.name,
                    describe_taint(&node.taint)
                );
            }
            if !admitted.is_empty() {
                return Ok(admitted);
            }
            return no_eligible_nodes(args, &skipped);
        }

        if !announced_wait {
            info!(
                "no node is eligible yet; re-checking for up to {}s. Nodes become eligible \
                 asynchronously: the labels a selector matches are often written by an add-on \
                 (node-feature-discovery, say) that starts alongside this dispatcher, and \
                 start-up taints clear only once the node settles",
                args.wait_for_nodes_secs
            );
            announced_wait = true;
        }
        tokio::time::sleep(poll).await;
    }
}

/// One resolution pass: LIST the nodes matching each selector, union them, and
/// split the result by taint admission (unless `--ignore-node-taints`, where
/// every matched node is admitted).
async fn select_nodes(
    api: &Api<Node>,
    args: &Args,
    tolerations: &[Toleration],
) -> Result<(Vec<String>, Vec<SkippedNode>)> {
    let mut matched: Vec<Node> = Vec::new();
    for selector in selector_passes(&args.node_selector) {
        matched.extend(list_nodes(api, args, selector).await?);
    }

    if args.ignore_node_taints {
        let mut names: Vec<String> = matched
            .iter()
            .filter_map(|node| node.metadata.name.clone())
            .collect();
        names.sort();
        names.dedup();
        return Ok((names, Vec::new()));
    }

    Ok(partition_by_tolerations(&matched, tolerations))
}

/// Decide what "nothing is eligible" means once we have stopped looking.
///
/// Nodes matched but none tolerated is almost always a forgotten toleration -
/// classically when targeting control-plane nodes - so it fails with the fix.
/// Nothing matched at all is only a no-op when the caller never asked us to
/// wait: having waited means nodes were expected, and exiting 0 there would
/// leave the whole fleet uninstalled with nothing to show for it.
fn no_eligible_nodes(args: &Args, skipped: &[SkippedNode]) -> Result<Vec<String>> {
    let waited = if args.wait_for_nodes_secs > 0 {
        format!(" after waiting {}s", args.wait_for_nodes_secs)
    } else {
        String::new()
    };

    if let Some(blocked) = skipped.first() {
        bail!(
            "all {} selected node(s) carry a taint this install does not tolerate{}, so there is \
             nowhere to install. First blocker: node {} has taint {}. If you meant to target \
             these nodes, tolerate it by adding to your values:\n{}",
            skipped.len(),
            waited,
            blocked.name,
            describe_taint(&blocked.taint),
            suggested_toleration(&blocked.taint)
        );
    }

    if args.wait_for_nodes_secs > 0 {
        bail!(
            "no node matched the selection{}, so there is nowhere to install. A node matching \
             any one of these selectors is enough: {}. Check that whatever applies those labels \
             (node-feature-discovery, for one) is running, or set --wait-for-nodes-secs=0 \
             (job.waitForNodesSeconds in the Helm chart) to make an empty selection a no-op.",
            waited,
            describe_selectors(&args.node_selector)
        );
    }

    Ok(Vec::new())
}

/// The selectors we tried, spelled out for error messages.
fn describe_selectors(selectors: &[String]) -> String {
    if selectors.is_empty() {
        return "<none> (every node)".to_string();
    }
    selectors
        .iter()
        .map(|s| format!("[{s}]"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One paginated LIST of nodes matching `label_selector` (ANDed with the global
/// field selector, if any).
async fn list_nodes(
    api: &Api<Node>,
    args: &Args,
    label_selector: Option<&str>,
) -> Result<Vec<Node>> {
    let mut nodes = Vec::new();
    let mut continue_token: Option<String> = None;

    loop {
        let lp = ListParams {
            limit: Some(args.node_page_size.max(1)),
            label_selector: label_selector.map(str::to_string),
            field_selector: args.node_field_selector.clone(),
            continue_token: continue_token.clone(),
            ..Default::default()
        };

        let page = api.list(&lp).await.with_context(|| {
            format!(
                "failed to list nodes (label selector: {})",
                label_selector.unwrap_or("<none>")
            )
        })?;
        nodes.extend(page.items);

        match page.metadata.continue_ {
            Some(token) if !token.is_empty() => continue_token = Some(token),
            _ => break,
        }
    }

    Ok(nodes)
}

/// Fetch the owner Job and build an `ownerReference` to it (non-controller, so
/// it does not interfere with the Job controller's own ownership of pods).
async fn owner_ref_for_job(client: &Client, namespace: &str, name: &str) -> Result<OwnerReference> {
    let jobs: Api<Job> = Api::namespaced(client.clone(), namespace);
    let job = jobs
        .get(name)
        .await
        .with_context(|| format!("failed to get owner job {name}"))?;
    let uid = job
        .metadata
        .uid
        .ok_or_else(|| anyhow::anyhow!("owner job {name} has no uid"))?;
    Ok(OwnerReference {
        api_version: "batch/v1".to_string(),
        kind: "Job".to_string(),
        name: name.to_string(),
        uid,
        controller: Some(false),
        block_owner_deletion: Some(false),
    })
}

/// Create and watch per-node Jobs, keeping at most `parallelism` in flight.
/// Returns an error listing the nodes whose Jobs failed, if any.
async fn run_fanout(
    jobs: &Api<Job>,
    template: &Job,
    nodes: &[String],
    args: &Args,
    namespace: &str,
    parallelism: usize,
    owner: Option<&OwnerReference>,
) -> Result<()> {
    let mut queue: VecDeque<&String> = nodes.iter().collect();
    // job name -> node name
    let mut in_flight: HashMap<String, String> = HashMap::new();
    let mut succeeded = 0usize;
    let mut failed: Vec<String> = Vec::new();

    let post = PostParams::default();
    let poll = Duration::from_secs(args.poll_interval_secs.max(1));
    // The name prefix is recorded in OWNER_LABEL and reused as the Job-name
    // prefix; sanitize it once so it is a valid label value / DNS-1123 prefix
    // regardless of what the caller passed (e.g. a Helm release suffix).
    let owner_value = sanitize_label_value(&args.name_prefix);

    while !queue.is_empty() || !in_flight.is_empty() {
        // Refill the in-flight set up to the parallelism cap.
        while in_flight.len() < parallelism {
            let Some(node) = queue.pop_front() else {
                break;
            };
            let name = job_name(&owner_value, node);
            let node_job = build_node_job(template, &name, node, &owner_value, owner);
            match jobs.create(&post, &node_job).await {
                Ok(_) => info!("created job {name} (node {node})"),
                // A Job with this name already exists (e.g. left over from a
                // previous, interrupted run). Only adopt it if it actually
                // carries our owner label: status polling GETs each in-flight
                // Job by name, so adopting one that lacks it (or belongs to
                // someone else) would leave it stuck in-flight forever. If it
                // is not ours, fail the node instead of hanging.
                Err(kube::Error::Api(e)) if e.code == 409 => match jobs.get(&name).await {
                    Ok(existing) if job_owned_by(&existing, &owner_value) => {
                        info!("job {name} (node {node}) already exists and is ours, adopting it");
                    }
                    Ok(_) => {
                        error!(
                            "job {name} (node {node}) already exists but is not labeled \
                             {OWNER_LABEL}={owner_value}; refusing to adopt it"
                        );
                        failed.push(node.clone());
                        continue;
                    }
                    Err(e) => {
                        error!("failed to fetch pre-existing job {name} (node {node}): {e}");
                        failed.push(node.clone());
                        continue;
                    }
                },
                Err(e) => {
                    error!("failed to create job {name} (node {node}): {e}");
                    failed.push(node.clone());
                    continue;
                }
            }
            in_flight.insert(name, node.clone());
        }

        if in_flight.is_empty() {
            break;
        }

        tokio::time::sleep(poll).await;

        // Poll each in-flight Job via GET so we only need the `get` verb on
        // batch/jobs (not `list`), matching the least-privilege Role.
        let mut finished: Vec<String> = Vec::new();
        for (name, node) in &in_flight {
            let j = match jobs.get(name).await {
                Ok(j) => j,
                Err(e) => {
                    error!("failed to get job {name} (node {node}): {e}");
                    continue;
                }
            };
            match interpret_status(&j) {
                JobOutcome::Succeeded => {
                    succeeded += 1;
                    finished.push(name.clone());
                    info!("node {node}: job {name} succeeded");
                }
                JobOutcome::Failed => {
                    failed.push(node.clone());
                    finished.push(name.clone());
                    error!("node {node}: job {name} failed");
                }
                JobOutcome::Running => {}
            }
        }
        for name in finished {
            in_flight.remove(&name);
        }

        info!(
            "progress: {succeeded} succeeded, {} failed, {} in-flight, {} queued",
            failed.len(),
            in_flight.len(),
            queue.len()
        );
    }

    if !failed.is_empty() {
        failed.sort();
        failed.dedup();
        bail!(
            "{} node(s) failed: {}. Inspect the per-node Job logs with: \
             kubectl logs -n {} -l {}={} --all-containers --prefix",
            failed.len(),
            failed.join(", "),
            namespace,
            OWNER_LABEL,
            owner_value
        );
    }

    info!("all {succeeded} node(s) completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::batch::v1::JobSpec;
    use k8s_openapi::api::core::v1::{PodSpec, PodTemplateSpec};

    #[test]
    fn no_selector_means_one_unfiltered_pass() {
        assert_eq!(selector_passes(&[]), vec![None]);
    }

    #[test]
    fn one_selector_means_one_filtered_pass() {
        let selectors = vec!["kubernetes.io/os=linux".to_string()];
        assert_eq!(
            selector_passes(&selectors),
            vec![Some("kubernetes.io/os=linux")]
        );
    }

    #[test]
    fn repeated_selectors_become_one_pass_each() {
        let selectors = vec![
            "feature.node.kubernetes.io/cpu-cpuid.VMX=true".to_string(),
            "feature.node.kubernetes.io/cpu-cpuid.SVM=true".to_string(),
        ];
        assert_eq!(
            selector_passes(&selectors),
            vec![
                Some("feature.node.kubernetes.io/cpu-cpuid.VMX=true"),
                Some("feature.node.kubernetes.io/cpu-cpuid.SVM=true"),
            ]
        );
    }

    fn job_with_tolerations(tolerations: Option<Vec<Toleration>>) -> Job {
        Job {
            spec: Some(JobSpec {
                template: PodTemplateSpec {
                    spec: Some(PodSpec {
                        tolerations,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn template_tolerations_are_read_from_the_pod_spec() {
        let toleration = Toleration {
            key: Some("node-role.kubernetes.io/control-plane".to_string()),
            operator: Some("Exists".to_string()),
            ..Default::default()
        };
        let template = job_with_tolerations(Some(vec![toleration.clone()]));

        assert_eq!(template_tolerations(&template), vec![toleration]);
    }

    #[test]
    fn template_without_tolerations_yields_an_empty_list() {
        assert!(template_tolerations(&job_with_tolerations(None)).is_empty());
        assert!(template_tolerations(&Job::default()).is_empty());
    }

    fn args_from(extra: &[&str]) -> Args {
        let mut argv = vec![
            "kata-deploy-job-dispatcher",
            "--job-template=/etc/kata-job/install-job.yaml",
            "--name-prefix=kata-deploy-install",
        ];
        argv.extend_from_slice(extra);
        Args::parse_from(argv)
    }

    fn skipped_node(name: &str) -> SkippedNode {
        SkippedNode {
            name: name.to_string(),
            taint: k8s_openapi::api::core::v1::Taint {
                key: "node-role.kubernetes.io/control-plane".to_string(),
                effect: "NoSchedule".to_string(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn nodes_are_resolved_once_by_default() {
        assert_eq!(args_from(&[]).wait_for_nodes_secs, 0);
    }

    #[test]
    fn an_empty_selection_is_a_no_op_when_we_never_waited() {
        let nodes = no_eligible_nodes(&args_from(&[]), &[]).expect("empty selection is a no-op");
        assert!(nodes.is_empty());
    }

    #[test]
    fn an_empty_selection_fails_once_the_wait_expires() {
        let args = args_from(&[
            "--wait-for-nodes-secs=120",
            "--node-selector=feature.node.kubernetes.io/cpu-cpuid.SVM in (true)",
        ]);
        let err = no_eligible_nodes(&args, &[]).expect_err("waiting means nodes were expected");
        let msg = err.to_string();

        assert!(msg.contains("after waiting 120s"), "{msg}");
        assert!(
            msg.contains("[feature.node.kubernetes.io/cpu-cpuid.SVM in (true)]"),
            "{msg}"
        );
    }

    #[test]
    fn nodes_matched_but_all_tainted_fail_with_the_missing_toleration() {
        let err = no_eligible_nodes(&args_from(&[]), &[skipped_node("cp-0")])
            .expect_err("a fully tainted selection has nowhere to install");
        let msg = err.to_string();

        assert!(msg.contains("node cp-0"), "{msg}");
        assert!(
            msg.contains("node-role.kubernetes.io/control-plane:NoSchedule"),
            "{msg}"
        );
        assert!(msg.contains("operator: Exists"), "{msg}");
    }

    #[test]
    fn selectors_are_spelled_out_for_errors() {
        assert_eq!(describe_selectors(&[]), "<none> (every node)");
        assert_eq!(
            describe_selectors(&["a=b".to_string(), "c=d".to_string()]),
            "[a=b], [c=d]"
        );
    }
}
