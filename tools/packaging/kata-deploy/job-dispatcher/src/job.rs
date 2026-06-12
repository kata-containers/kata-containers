// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// Label applied to every per-node Job, set to the dispatcher's name prefix.
/// Used as a server-side selector so the dispatcher only ever sees the Jobs it
/// created (and not unrelated Jobs in the namespace).
pub const OWNER_LABEL: &str = "kata-deploy-job-dispatcher/owner";

/// Label carrying the (sanitized) target node name, for human inspection.
pub const NODE_LABEL: &str = "kata-deploy-job-dispatcher/node";

/// Annotation carrying the full, unmodified target node name. Node names can
/// exceed the 63-char label-value limit or contain characters invalid in a
/// label value, so the authoritative value lives in an annotation.
pub const NODE_ANNOTATION: &str = "kata-deploy-job-dispatcher/node-name";

/// Maximum length of a DNS-1123 label and of a Kubernetes label value.
pub const MAX_LABEL_LEN: usize = 63;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    Running,
    Succeeded,
    Failed,
}

/// Lowercase a node name and replace any character that is not a valid
/// DNS-1123 label character (`[a-z0-9-]`) with `-`, then trim leading/trailing
/// dashes. The result is safe to embed in a Job name and label value.
pub fn sanitize_node(node: &str) -> String {
    let lowered = node.to_ascii_lowercase();
    let mapped: String = lowered
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    mapped.trim_matches('-').to_string()
}

/// Short, stable hex digest of an arbitrary string. Used to keep generated
/// Job names unique when the sanitized/truncated form would otherwise collide.
fn short_hash(s: &str) -> String {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:08x}", (hasher.finish() & 0xffff_ffff) as u32)
}

/// Build a deterministic, RFC1123-label-safe Job name (`<= 63` chars) for a
/// node. When `<prefix>-<sanitized-node>` fits it is used verbatim; otherwise
/// it is truncated and a short hash of the *full* node name is appended so two
/// different long node names cannot collide.
pub fn job_name(prefix: &str, node: &str) -> String {
    let sanitized = sanitize_node(node);
    let base = format!("{prefix}-{sanitized}");
    if base.len() <= MAX_LABEL_LEN {
        return base;
    }
    let hash = short_hash(node);
    // Reserve room for "-" + hash.
    let keep = MAX_LABEL_LEN.saturating_sub(hash.len() + 1);
    let truncated = base.chars().take(keep).collect::<String>();
    format!("{}-{}", truncated.trim_end_matches('-'), hash)
}

/// Sanitize an arbitrary string into a value safe to use BOTH as the prefix of
/// a DNS-1123 Job name and as a Kubernetes label value: lowercased, every
/// non-`[a-z0-9-]` character replaced with `-`, leading/trailing `-` trimmed,
/// and truncated to [`MAX_LABEL_LEN`] (re-trimming any trailing `-` left by the
/// truncation). The dispatcher records its `--name-prefix` in [`OWNER_LABEL`]
/// and reuses it as the Job-name prefix, so callers can pass a raw value (e.g.
/// a Helm release/suffix) without risking an invalid or over-long label.
pub fn sanitize_label_value(value: &str) -> String {
    let sanitized = sanitize_node(value);
    if sanitized.len() <= MAX_LABEL_LEN {
        return sanitized;
    }
    sanitized
        .chars()
        .take(MAX_LABEL_LEN)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

/// True if `job` carries [`OWNER_LABEL`] set to exactly `owner_value`. Used to
/// decide whether a pre-existing (409) Job is safe to adopt: the dispatcher
/// only ever LISTs Jobs by that label, so adopting one that lacks it would
/// leave it stuck in-flight forever.
pub fn job_owned_by(job: &Job, owner_value: &str) -> bool {
    job.metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get(OWNER_LABEL))
        .map(|value| value == owner_value)
        .unwrap_or(false)
}

/// Clone the template Job and specialize it for a single node:
///   - set a unique `metadata.name`,
///   - pin the pod to `node` via `spec.template.spec.nodeName`,
///   - add owner/node tracking labels (+ a full-name annotation),
///   - optionally attach an `ownerReference` for garbage collection.
///
/// `owner_value` is the dispatcher's name prefix, recorded in [`OWNER_LABEL`] so
/// the dispatcher can list back only its own Jobs.
pub fn build_node_job(
    template: &Job,
    name: &str,
    node: &str,
    owner_value: &str,
    owner: Option<&OwnerReference>,
) -> Job {
    let mut job = template.clone();

    job.metadata.name = Some(name.to_string());
    // A template may carry generateName; an explicit name wins, drop it to
    // avoid the apiserver rejecting both being set.
    job.metadata.generate_name = None;

    let labels = job.metadata.labels.get_or_insert_with(BTreeMap::new);
    labels.insert(OWNER_LABEL.to_string(), owner_value.to_string());
    labels.insert(NODE_LABEL.to_string(), sanitize_node(node));

    let annotations = job.metadata.annotations.get_or_insert_with(BTreeMap::new);
    annotations.insert(NODE_ANNOTATION.to_string(), node.to_string());

    if let Some(owner_ref) = owner {
        job.metadata.owner_references = Some(vec![owner_ref.clone()]);
    }

    let spec = job.spec.get_or_insert_with(Default::default);

    // Mirror the owner label onto the pod template so the pods are easy to
    // find too.
    let tmpl_meta = spec.template.metadata.get_or_insert_with(Default::default);
    let tmpl_labels = tmpl_meta.labels.get_or_insert_with(BTreeMap::new);
    tmpl_labels.insert(OWNER_LABEL.to_string(), owner_value.to_string());

    let pod_spec = spec.template.spec.get_or_insert_with(Default::default);
    pod_spec.node_name = Some(node.to_string());

    job
}

/// Interpret a Job's `.status` into a coarse outcome. Prefers the explicit
/// `Complete`/`Failed` conditions; falls back to the succeeded counter.
pub fn interpret_status(job: &Job) -> JobOutcome {
    let Some(status) = job.status.as_ref() else {
        return JobOutcome::Running;
    };

    if let Some(conditions) = status.conditions.as_ref() {
        for c in conditions {
            if c.status != "True" {
                continue;
            }
            match c.type_.as_str() {
                "Failed" => return JobOutcome::Failed,
                "Complete" => return JobOutcome::Succeeded,
                _ => {}
            }
        }
    }

    if status.succeeded.unwrap_or(0) >= 1 {
        return JobOutcome::Succeeded;
    }

    JobOutcome::Running
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("worker-0", "worker-0")]
    #[case("Worker.Example.COM", "worker-example-com")]
    #[case("--node--", "node")]
    #[case("a_b/c", "a-b-c")]
    fn test_sanitize_node(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(sanitize_node(input), expected);
    }

    #[rstest]
    #[case("kata-deploy-install", "kata-deploy-install")]
    #[case("Kata_Deploy.Install", "kata-deploy-install")]
    #[case("--weird--", "weird")]
    fn test_sanitize_label_value_short(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(sanitize_label_value(input), expected);
    }

    #[test]
    fn test_sanitize_label_value_truncates() {
        let out = sanitize_label_value(&"a".repeat(100));
        assert_eq!(out.len(), MAX_LABEL_LEN);
        assert!(
            !out.ends_with('-'),
            "truncation must not leave a trailing dash"
        );
    }

    #[test]
    fn test_job_owned_by() {
        let mut job = Job::default();
        assert!(!job_owned_by(&job, "kata-deploy-install"));
        job.metadata
            .labels
            .get_or_insert_with(BTreeMap::new)
            .insert(OWNER_LABEL.to_string(), "kata-deploy-install".to_string());
        assert!(job_owned_by(&job, "kata-deploy-install"));
        assert!(!job_owned_by(&job, "other-owner"));
    }

    #[rstest]
    #[case("kata-deploy-install", "worker-0", "kata-deploy-install-worker-0")]
    #[case("kata-deploy-cleanup", "Worker.0", "kata-deploy-cleanup-worker-0")]
    fn test_job_name_short(#[case] prefix: &str, #[case] node: &str, #[case] expected: &str) {
        assert_eq!(job_name(prefix, node), expected);
    }

    #[test]
    fn test_job_name_truncated_and_unique() {
        let prefix = "kata-deploy-install";
        let long_a = "node-with-a-really-really-really-really-really-long-name-aaaaaaa";
        let long_b = "node-with-a-really-really-really-really-really-long-name-bbbbbbb";

        let name_a = job_name(prefix, long_a);
        let name_b = job_name(prefix, long_b);

        assert!(
            name_a.len() <= 63,
            "name too long: {} ({})",
            name_a,
            name_a.len()
        );
        assert!(
            name_b.len() <= 63,
            "name too long: {} ({})",
            name_b,
            name_b.len()
        );
        assert_ne!(
            name_a, name_b,
            "different node names must yield different job names"
        );
    }

    #[test]
    fn test_build_node_job_pins_node_and_labels() {
        let template: Job = serde_yaml::from_str(
            r#"
apiVersion: batch/v1
kind: Job
metadata:
  name: ignored
spec:
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: c
          image: busybox
"#,
        )
        .unwrap();

        let owner = OwnerReference {
            api_version: "batch/v1".to_string(),
            kind: "Job".to_string(),
            name: "dispatcher".to_string(),
            uid: "abc-123".to_string(),
            controller: Some(false),
            block_owner_deletion: Some(false),
        };

        let job = build_node_job(
            &template,
            "kata-deploy-install-node1",
            "node1",
            "kata-deploy-install",
            Some(&owner),
        );

        assert_eq!(
            job.metadata.name.as_deref(),
            Some("kata-deploy-install-node1")
        );
        let labels = job.metadata.labels.unwrap();
        assert_eq!(
            labels.get(OWNER_LABEL).map(String::as_str),
            Some("kata-deploy-install")
        );
        assert_eq!(labels.get(NODE_LABEL).map(String::as_str), Some("node1"));
        let annotations = job.metadata.annotations.unwrap();
        assert_eq!(
            annotations.get(NODE_ANNOTATION).map(String::as_str),
            Some("node1")
        );
        assert_eq!(job.metadata.owner_references.unwrap().len(), 1);
        let pod_spec = job.spec.unwrap().template.spec.unwrap();
        assert_eq!(pod_spec.node_name.as_deref(), Some("node1"));
    }

    fn job_with_status(status_yaml: &str) -> Job {
        let yaml = format!(
            "apiVersion: batch/v1\nkind: Job\nmetadata:\n  name: j\nstatus:\n{status_yaml}"
        );
        serde_yaml::from_str(&yaml).unwrap()
    }

    #[rstest]
    #[case(
        "  conditions:\n    - type: Complete\n      status: \"True\"\n",
        JobOutcome::Succeeded
    )]
    #[case(
        "  conditions:\n    - type: Failed\n      status: \"True\"\n",
        JobOutcome::Failed
    )]
    #[case(
        "  conditions:\n    - type: Complete\n      status: \"False\"\n",
        JobOutcome::Running
    )]
    #[case("  succeeded: 1\n", JobOutcome::Succeeded)]
    fn test_interpret_status(#[case] status_yaml: &str, #[case] expected: JobOutcome) {
        assert_eq!(interpret_status(&job_with_status(status_yaml)), expected);
    }

    #[test]
    fn test_interpret_status_running_when_unset() {
        assert_eq!(interpret_status(&Job::default()), JobOutcome::Running);
    }
}
