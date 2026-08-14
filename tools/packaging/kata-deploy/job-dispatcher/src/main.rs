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
mod nodes;

use anyhow::{bail, Context, Result};
use clap::Parser;
use job::{
    build_node_job, interpret_status, job_name, job_owned_by, sanitize_label_value, JobOutcome,
    OWNER_LABEL,
};
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{Node, Toleration};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::{Api, DeleteParams, ListParams, PostParams};
use kube::Client;
use log::{error, info};
use node_filter::{describe_taint, partition_by_tolerations, suggested_toleration, SkippedNode};
use nodes::{NodeFacts, NodeOps};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

/// How long a Job may stay unreadable before its node is failed.
///
/// A GET that keeps failing - RBAC changed under us, an apiserver that keeps
/// rejecting it - would otherwise be polled forever, and the run would end only
/// when something killed the dispatcher, with no result reported for any node.
const JOB_READ_ERROR_BUDGET: Duration = Duration::from_secs(300);

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

    /// Label a node `katacontainers.io/kata-runtime=<value>` once its Job
    /// succeeded, then lift any --remove-node-taints.
    ///
    /// The label is what admits Kata workloads to a node, and setting it from
    /// here rather than from a final stage inside the per-node Job is what allows
    /// those Jobs to carry no credentials at all. It also tightens the contract:
    /// the label goes on only after the Job as a whole succeeded AND the node
    /// reported Ready again, never from inside a pipeline that might still fail.
    #[arg(long)]
    node_label: Option<String>,

    /// Remove `katacontainers.io/kata-runtime` from a node before creating its
    /// Job, so the scheduler stops sending Kata workloads there before anything on
    /// the node is taken apart. For cleanup runs.
    #[arg(long, default_value_t = false)]
    remove_node_label: bool,

    /// Set `katacontainers.io/kata-runtime=false` on a node before creating its
    /// Job, unless it already carries the label.
    ///
    /// The value admits nothing - the RuntimeClasses require `true` - but it makes
    /// the node discoverable by an uninstall. An install that dies after placing
    /// artifacts but before the node is labelled would otherwise leave a modified
    /// node that the default cleanup selection cannot see.
    #[arg(long, default_value_t = false)]
    claim_node_pending: bool,

    /// Comma-separated CRI runtime handlers a node must be serving before it is
    /// labelled, as reported in its `.status.runtimeHandlers`. Empty skips it.
    ///
    /// The one direct answer about what the runtime loaded, rather than what the
    /// install wrote and hoped it would read - and it needs the apiserver, so it
    /// belongs here rather than in the credential-free per-node Jobs. Any one of
    /// them counts: which handlers a node can serve depends on its architecture.
    #[arg(long)]
    require_node_handlers: Option<String>,

    /// Comma-separated taints to lift after labelling a node, as `key` (any
    /// effect) or `key:effect`. These are the start-up taints that keep workloads
    /// off a node until Kata is actually installed on it.
    #[arg(long)]
    remove_node_taints: Option<String>,

    /// Seconds to wait for a node to report Ready after its install Job finished,
    /// before labelling it. 0 disables the wait.
    ///
    /// The install restarts the node's CRI runtime, and the Job can only report on
    /// the runtime's own unit; waiting for the kubelet to report Ready here is
    /// what keeps a node from being advertised as Kata-capable while it is still
    /// coming back.
    #[arg(long, default_value_t = 0)]
    wait_node_ready_secs: u64,

    /// Warn when a node's kubelet `runtimeRequestTimeout` is below this many
    /// seconds. 0 disables the check.
    ///
    /// Advisory: a timeout well below the time a large image needs can abort
    /// CreateContainer mid-pull, which is why the chart only asks for this when
    /// guest pull or image conversion is configured.
    #[arg(long, default_value_t = 0)]
    kubelet_timeout_warn_secs: u64,

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

    let node_ops = Arc::new(node_ops_from_args(&client, &args)?);

    let nodes = resolve_nodes(&client, &args, &template_tolerations(&template)).await?;
    if nodes.is_empty() {
        info!("no target nodes matched the selection; nothing to do");
        return Ok(());
    }

    let facts = collect_node_facts(&node_ops, &nodes).await;

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
        node_ops.clone(),
        &facts,
    )
    .await
}

fn comma_separated(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn node_ops_from_args(client: &Client, args: &Args) -> Result<NodeOps> {
    let mut ops = NodeOps::new(client);

    ops.label_value = args.node_label.clone();
    ops.remove_label = args.remove_node_label;
    ops.claim_pending = args.claim_node_pending;
    ops.remove_taints = comma_separated(args.remove_node_taints.as_deref());
    ops.require_handlers = comma_separated(args.require_node_handlers.as_deref());
    if args.wait_node_ready_secs > 0 {
        ops.wait_ready = Some(Duration::from_secs(args.wait_node_ready_secs));
    }
    if args.kubelet_timeout_warn_secs > 0 {
        ops.kubelet_timeout_warn = Some(Duration::from_secs(args.kubelet_timeout_warn_secs));
    }

    if !ops.remove_taints.is_empty() && ops.label_value.is_none() {
        bail!(
            "--remove-node-taints needs --node-label: a start-up taint may only be lifted once \
             the node is labelled Kata-capable, otherwise workloads reach it before the label \
             that is supposed to gate them"
        );
    }

    Ok(ops)
}

/// Nodes resolved from a selector were already listed in full, so their facts are
/// free; explicitly named nodes (`--nodes`) are fetched here.
///
/// A node missing from the result is not dispatched to. The per-node Jobs detect
/// their CRI runtime from this version string and hold no credentials to look it
/// up themselves, so dispatching one anyway would start a privileged pod that is
/// certain to fail on the first thing it does.
async fn collect_node_facts(ops: &NodeOps, nodes: &[Node]) -> HashMap<String, NodeFacts> {
    let mut facts = HashMap::new();

    for node in nodes {
        let Some(name) = node.metadata.name.clone() else {
            continue;
        };
        // A node the selector produced carries its full object; a named one is a
        // stub holding just the name.
        let known = node.status.is_some() || node.metadata.labels.is_some();
        if known {
            record_facts(&mut facts, NodeFacts::from_node(node));
            continue;
        }

        match ops.get(&name).await {
            Ok(fetched) => {
                record_facts(&mut facts, NodeFacts::from_node(&fetched));
            }
            Err(err) => {
                error!("node {name}: could not read its runtime details ({err:#})");
            }
        }
    }

    facts
}

fn record_facts(facts: &mut HashMap<String, NodeFacts>, node_facts: NodeFacts) {
    if node_facts.container_runtime_version.is_none() {
        error!(
            "node {}: reports no containerRuntimeVersion, so the per-node Job would have no way \
             to tell which CRI runtime to configure",
            node_facts.name
        );
        return;
    }

    facts.insert(node_facts.name.clone(), node_facts);
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
) -> Result<Vec<Node>> {
    if let Some(list) = args.nodes.as_deref() {
        let mut names: Vec<String> = list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        names.sort();
        names.dedup();
        // Named nodes are taken at face value - naming a node is unambiguous - so
        // these are stubs; whatever else is needed about them is fetched later.
        return Ok(names.into_iter().map(node_stub).collect());
    }

    let api: Api<Node> = Api::all(client.clone());
    let poll = Duration::from_secs(args.poll_interval_secs.max(1));
    let deadline = Instant::now() + Duration::from_secs(args.wait_for_nodes_secs);
    let mut announced_wait = false;
    let mut previous: Option<Vec<String>> = None;

    let report_skipped = |skipped: &[SkippedNode]| {
        for node in skipped {
            info!(
                "skipping node {}: it carries the taint {}, which this install does not tolerate \
                 (a DaemonSet would not have been scheduled there either)",
                node.name,
                describe_taint(&node.taint)
            );
        }
    };

    loop {
        let (admitted, skipped) = select_nodes(&api, args, tolerations).await?;
        let expired = Instant::now() >= deadline;

        if !admitted.is_empty() {
            let mut names: Vec<String> = admitted
                .iter()
                .filter_map(|node| node.metadata.name.clone())
                .collect();
            names.sort();

            // The set the first pass sees is not the set to install on: the labels
            // a selector matches are written per node by an add-on that is itself
            // still starting, so returning on the first match would install on
            // whichever node won that race and silently leave the rest out. Wait
            // until a whole pass adds nothing.
            if expired || previous.as_deref() == Some(names.as_slice()) {
                report_skipped(&skipped);
                return Ok(admitted);
            }

            info!(
                "{} node(s) eligible so far ({}); re-checking once more in case others are still \
                 becoming eligible",
                names.len(),
                names.join(", ")
            );
            previous = Some(names);
            tokio::time::sleep(poll).await;
            continue;
        }

        if expired {
            report_skipped(&skipped);
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
) -> Result<(Vec<Node>, Vec<SkippedNode>)> {
    let mut matched: Vec<Node> = Vec::new();
    for selector in selector_passes(&args.node_selector) {
        matched.extend(list_nodes(api, args, selector).await?);
    }

    if args.ignore_node_taints {
        return Ok((dedup_by_name(matched, None), Vec::new()));
    }

    let (admitted, skipped) = partition_by_tolerations(&matched, tolerations);
    Ok((dedup_by_name(matched, Some(&admitted)), skipped))
}

/// Collapse the per-selector duplicates a union produces, keeping the objects
/// themselves so the facts they carry can be handed to the per-node Jobs. With
/// `keep`, only those nodes survive.
fn dedup_by_name(nodes: Vec<Node>, keep: Option<&[String]>) -> Vec<Node> {
    let mut seen: Vec<String> = Vec::new();
    let mut unique: Vec<Node> = Vec::new();

    for node in nodes {
        let Some(name) = node.metadata.name.clone() else {
            continue;
        };
        if let Some(keep) = keep {
            if !keep.contains(&name) {
                continue;
            }
        }
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        unique.push(node);
    }

    unique.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));
    unique
}

/// A Node carrying nothing but its name, for nodes named explicitly rather than
/// selected (and therefore never listed).
fn node_stub(name: String) -> Node {
    Node {
        metadata: kube::core::ObjectMeta {
            name: Some(name),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Decide what "nothing is eligible" means once we have stopped looking.
///
/// Nodes matched but none tolerated is almost always a forgotten toleration -
/// classically when targeting control-plane nodes - so it fails with the fix.
/// Nothing matched at all is only a no-op when the caller never asked us to
/// wait: having waited means nodes were expected, and exiting 0 there would
/// leave the whole fleet uninstalled with nothing to show for it.
fn no_eligible_nodes(args: &Args, skipped: &[SkippedNode]) -> Result<Vec<Node>> {
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
///
/// `node_ops` brackets each node's Job with the node-level API work the Job
/// itself is deliberately not credentialed to do: claiming and unlabelling before
/// the Job, and - once it has actually succeeded - waiting for the node to report
/// Ready, checking its runtime serves what was installed, labelling it
/// Kata-capable and lifting its start-up taints.
#[allow(clippy::too_many_arguments)]
async fn run_fanout(
    jobs: &Api<Job>,
    template: &Job,
    nodes: &[Node],
    args: &Args,
    namespace: &str,
    parallelism: usize,
    owner: Option<&OwnerReference>,
    node_ops: Arc<NodeOps>,
    facts: &HashMap<String, NodeFacts>,
) -> Result<()> {
    let names: Vec<String> = nodes
        .iter()
        .filter_map(|node| node.metadata.name.clone())
        .collect();
    let mut queue: VecDeque<&String> = names.iter().collect();
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

    // A node's post-success work (readiness, handlers, label, taints) can take
    // minutes, so it runs off to the side. It holds a slot until it finishes -
    // the node is not done before it does - which keeps one unready node from
    // stalling every other node's Job.
    let mut post_work: JoinSet<Result<()>> = JoinSet::new();
    let mut post_nodes: HashMap<tokio::task::Id, String> = HashMap::new();
    // Since when each Job has been unreadable, cleared as soon as one reads.
    let mut unreadable: HashMap<String, Instant> = HashMap::new();

    loop {
        while in_flight.len() + post_work.len() < parallelism {
            let Some(node) = queue.pop_front() else {
                break;
            };

            // A failure here fails the node rather than proceeding: for cleanup
            // that would mean dismantling Kata on a node still advertising it.
            if let Err(err) = node_ops.before_dispatch(node).await {
                error!("node {node}: {err:#}");
                failed.push(node.clone());
                continue;
            }

            let Some(node_facts) = facts.get(node.as_str()) else {
                error!(
                    "node {node}: not dispatching a Job to it, since its runtime details could \
                     not be established (reported above)"
                );
                failed.push(node.clone());
                continue;
            };

            let name = job_name(&owner_value, node);
            let node_job = build_node_job(template, &name, node, &owner_value, owner, node_facts);
            match jobs.create(&post, &node_job).await {
                Ok(_) => info!("created job {name} (node {node})"),
                // A Job with this name already exists. Job names are derived from
                // the node and the release, so this is either this same run's own
                // Job (the dispatcher was restarted) or one left behind by a
                // previous run - and those two need opposite treatment.
                Err(kube::Error::Api(e)) if e.code == 409 => {
                    match adopt_or_replace(jobs, &name, &node_job, &owner_value, owner).await {
                        Ok(Adoption::Adopted) => {
                            info!("job {name} (node {node}) is this run's own, adopting it")
                        }
                        Ok(Adoption::Recreated) => {
                            info!("job {name} (node {node}) was left by an earlier run, recreated")
                        }
                        Err(err) => {
                            error!("node {node}: {err:#}");
                            failed.push(node.clone());
                            continue;
                        }
                    }
                }
                Err(e) => {
                    error!("failed to create job {name} (node {node}): {e}");
                    failed.push(node.clone());
                    continue;
                }
            }
            in_flight.insert(name, node.clone());
        }

        if in_flight.is_empty() && post_work.is_empty() {
            break;
        }

        // Nothing left to poll for: wait on the work that is finishing rather
        // than spinning on the poll interval.
        if in_flight.is_empty() {
            if let Some(joined) = post_work.join_next_with_id().await {
                record_post_work(joined, &mut post_nodes, &mut succeeded, &mut failed);
            }
            continue;
        }

        tokio::time::sleep(poll).await;

        // Poll each in-flight Job via GET so we only need the `get` verb on
        // batch/jobs (not `list`), matching the least-privilege Role.
        let mut finished: Vec<String> = Vec::new();
        for (name, node) in &in_flight {
            let j = match jobs.get(name).await {
                Ok(j) => j,
                // Deleted under us - garbage collection catching up with a
                // previous dispatcher, or someone clearing Jobs by hand. Waiting
                // for a Job that no longer exists never ends, so the node fails
                // and a later run retries it.
                Err(kube::Error::Api(e)) if e.code == 404 => {
                    error!(
                        "node {node}: job {name} no longer exists, so its result cannot be \
                         established; treating the node as failed"
                    );
                    failed.push(node.clone());
                    finished.push(name.clone());
                    continue;
                }
                Err(e) => {
                    let since = *unreadable.entry(name.clone()).or_insert_with(Instant::now);
                    if since.elapsed() >= JOB_READ_ERROR_BUDGET {
                        error!(
                            "node {node}: job {name} has been unreadable for {}s ({e}), so its \
                             result cannot be established; treating the node as failed",
                            since.elapsed().as_secs()
                        );
                        failed.push(node.clone());
                        finished.push(name.clone());
                    } else {
                        error!("failed to get job {name} (node {node}): {e}");
                    }
                    continue;
                }
            };
            unreadable.remove(name);
            match interpret_status(&j) {
                JobOutcome::Succeeded => {
                    finished.push(name.clone());
                    info!("node {node}: job {name} succeeded");
                    let ops = node_ops.clone();
                    let target = node.clone();
                    let handle = post_work.spawn(async move { ops.after_success(&target).await });
                    post_nodes.insert(handle.id(), node.clone());
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
            unreadable.remove(&name);
        }

        while let Some(joined) = post_work.try_join_next_with_id() {
            record_post_work(joined, &mut post_nodes, &mut succeeded, &mut failed);
        }

        info!(
            "progress: {succeeded} succeeded, {} failed, {} in-flight, {} finishing, {} queued",
            failed.len(),
            in_flight.len(),
            post_work.len(),
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

enum Adoption {
    Adopted,
    Recreated,
}

/// Decide what to do about a Job that already carries the name this run wants.
///
/// The owner label alone cannot tell the two cases apart: it holds the release's
/// name prefix, which is the same on every run. An `ownerReference` to *this*
/// dispatcher can - and the distinction matters, because a finished Job from the
/// previous release would otherwise be read as this upgrade's result, leaving the
/// node on the old version with nothing to show for it.
async fn adopt_or_replace(
    jobs: &Api<Job>,
    name: &str,
    desired: &Job,
    owner_value: &str,
    owner: Option<&OwnerReference>,
) -> Result<Adoption> {
    let existing = jobs
        .get(name)
        .await
        .with_context(|| format!("failed to fetch the pre-existing job {name}"))?;

    match job_disposition(&existing, owner_value, owner) {
        Disposition::NotOurs => bail!(
            "job {name} already exists but is not labeled {OWNER_LABEL}={owner_value}; refusing \
             to adopt a Job that is not this release's"
        ),
        Disposition::Stale => {
            replace_job(jobs, name, desired).await?;
            Ok(Adoption::Recreated)
        }
        Disposition::Current => Ok(Adoption::Adopted),
    }
}

#[derive(Debug, PartialEq)]
enum Disposition {
    NotOurs,
    Stale,
    Current,
}

fn job_disposition(
    existing: &Job,
    owner_value: &str,
    owner: Option<&OwnerReference>,
) -> Disposition {
    if !job_owned_by(existing, owner_value) {
        return Disposition::NotOurs;
    }

    // Without an owner of our own there is nothing to compare against, so the
    // label is all there is to go on.
    let Some(owner) = owner else {
        return Disposition::Current;
    };

    if existing
        .metadata
        .owner_references
        .iter()
        .flatten()
        .any(|reference| reference.uid == owner.uid)
    {
        Disposition::Current
    } else {
        Disposition::Stale
    }
}

/// How long to keep trying to create a Job whose predecessor is still going away.
const REPLACE_ATTEMPTS: u32 = 30;
const REPLACE_INTERVAL: Duration = Duration::from_secs(2);

/// Delete a Job and create the intended one in its place.
///
/// Deletion is asynchronous, and the name is only free once it completes, so the
/// create is retried while the apiserver still answers 409.
async fn replace_job(jobs: &Api<Job>, name: &str, desired: &Job) -> Result<()> {
    // Foreground, so that the Job outlives its pods rather than the other way
    // round: the name stays taken - and this loop keeps waiting - until the pod a
    // previous run left on the node is gone. Two of these pods on one node would
    // both be root on it, installing at the same time.
    let delete = DeleteParams::foreground();
    match jobs.delete(name, &delete).await {
        Ok(_) => (),
        Err(kube::Error::Api(e)) if e.code == 404 => (),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to delete the stale job {name}"))
        }
    }

    let post = PostParams::default();
    for _ in 0..REPLACE_ATTEMPTS {
        match jobs.create(&post, desired).await {
            Ok(_) => return Ok(()),
            Err(kube::Error::Api(e)) if e.code == 409 => {
                tokio::time::sleep(REPLACE_INTERVAL).await;
            }
            Err(err) => {
                return Err(err).with_context(|| format!("failed to recreate the job {name}"))
            }
        }
    }

    bail!(
        "the stale job {name} was still being deleted {}s after it was asked to go; giving up on \
         recreating it",
        (REPLACE_ATTEMPTS as u64) * REPLACE_INTERVAL.as_secs()
    )
}

/// A node whose Job passed but whose post-success work did not is a failed node:
/// an install is only complete once the node is labelled, and a later run comes
/// back to it.
fn record_post_work(
    joined: std::result::Result<(tokio::task::Id, Result<()>), tokio::task::JoinError>,
    post_nodes: &mut HashMap<tokio::task::Id, String>,
    succeeded: &mut usize,
    failed: &mut Vec<String>,
) {
    let (id, outcome) = match joined {
        Ok((id, outcome)) => (id, outcome),
        Err(err) => (err.id(), Err(anyhow::anyhow!("{err}"))),
    };
    let node = post_nodes
        .remove(&id)
        .unwrap_or_else(|| "<unknown>".to_string());

    match outcome {
        Ok(()) => *succeeded += 1,
        Err(err) => {
            error!("node {node}: install finished but {err:#}");
            failed.push(node);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::batch::v1::JobSpec;
    use k8s_openapi::api::core::v1::{PodSpec, PodTemplateSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

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

    fn owner(uid: &str) -> OwnerReference {
        OwnerReference {
            uid: uid.to_string(),
            name: "kata-deploy-install".to_string(),
            kind: "Job".to_string(),
            api_version: "batch/v1".to_string(),
            ..Default::default()
        }
    }

    fn existing_job(owner_label: &str, owned_by: Option<&OwnerReference>) -> Job {
        let mut job = Job {
            metadata: ObjectMeta {
                name: Some("kata-deploy-install-node1".to_string()),
                labels: Some(BTreeMap::from([(
                    OWNER_LABEL.to_string(),
                    owner_label.to_string(),
                )])),
                ..Default::default()
            },
            ..Default::default()
        };
        job.metadata.owner_references = owned_by.map(|reference| vec![reference.clone()]);
        job
    }

    #[test]
    fn a_job_from_another_release_is_never_adopted() {
        let existing = existing_job("someone-else", Some(&owner("uid-1")));
        assert_eq!(
            job_disposition(&existing, "kata-deploy-install", Some(&owner("uid-1"))),
            Disposition::NotOurs
        );
    }

    /// Job names repeat across runs, so a finished Job from the release being
    /// replaced would otherwise be reported as this upgrade's result.
    #[test]
    fn a_job_from_an_earlier_run_is_replaced() {
        let existing = existing_job("kata-deploy-install", Some(&owner("uid-old")));
        assert_eq!(
            job_disposition(&existing, "kata-deploy-install", Some(&owner("uid-new"))),
            Disposition::Stale
        );

        let orphan = existing_job("kata-deploy-install", None);
        assert_eq!(
            job_disposition(&orphan, "kata-deploy-install", Some(&owner("uid-new"))),
            Disposition::Stale
        );
    }

    #[test]
    fn this_runs_own_job_is_adopted() {
        let existing = existing_job("kata-deploy-install", Some(&owner("uid-1")));
        assert_eq!(
            job_disposition(&existing, "kata-deploy-install", Some(&owner("uid-1"))),
            Disposition::Current
        );

        // Nothing to compare against outside a Helm hook: the label is all there is.
        let existing = existing_job("kata-deploy-install", None);
        assert_eq!(
            job_disposition(&existing, "kata-deploy-install", None),
            Disposition::Current
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
