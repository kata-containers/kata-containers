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

use anyhow::{bail, Context, Result};
use clap::Parser;
use job::{
    build_node_job, interpret_status, job_name, job_owned_by, sanitize_label_value, JobOutcome,
    OWNER_LABEL,
};
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::Node;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::{Api, ListParams, PostParams};
use kube::Client;
use log::{error, info};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

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
    #[arg(long)]
    node_selector: Option<String>,

    /// Server-side field selector used to pick target nodes (ANDed with the
    /// label selector).
    #[arg(long)]
    node_field_selector: Option<String>,

    /// Explicit comma-separated node names. When set, the node selectors are
    /// ignored and exactly these nodes are targeted.
    #[arg(long)]
    nodes: Option<String>,

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

    let nodes = resolve_nodes(&client, &args).await?;
    if nodes.is_empty() {
        info!("no target nodes matched the selection; nothing to do");
        return Ok(());
    }

    let template_raw = std::fs::read_to_string(&args.job_template)
        .with_context(|| format!("failed to read job template {}", args.job_template))?;
    let template: Job = serde_yaml::from_str(&template_raw)
        .with_context(|| format!("failed to parse job template {}", args.job_template))?;

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

/// Resolve the set of target node names: an explicit `--nodes` list when given,
/// otherwise a paginated, server-side-filtered LIST of nodes.
async fn resolve_nodes(client: &Client, args: &Args) -> Result<Vec<String>> {
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
    let mut names = Vec::new();
    let mut continue_token: Option<String> = None;

    loop {
        let lp = ListParams {
            limit: Some(args.node_page_size.max(1)),
            label_selector: args.node_selector.clone(),
            field_selector: args.node_field_selector.clone(),
            continue_token: continue_token.clone(),
            ..Default::default()
        };

        let page = api.list(&lp).await.context("failed to list nodes")?;
        for node in &page.items {
            if let Some(name) = node.metadata.name.clone() {
                names.push(name);
            }
        }

        match page.metadata.continue_ {
            Some(token) if !token.is_empty() => continue_token = Some(token),
            _ => break,
        }
    }

    names.sort();
    names.dedup();
    Ok(names)
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
