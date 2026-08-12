// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! DaemonSet-equivalent node admission for job mode.
//!
//! In daemonset mode the scheduler keeps kata-deploy off nodes whose taints its
//! pods do not tolerate - that, rather than any selector, is what keeps it off
//! control-plane nodes by default. Job mode cannot lean on that: the dispatcher
//! pins each per-node Job via `spec.nodeName`, which bypasses the scheduler
//! entirely, and `NoSchedule` is a scheduler-side check the kubelet never
//! repeats. Without the check below, job mode would install Kata on tainted
//! nodes a DaemonSet would have skipped.
//!
//! So we replicate it here, against the very tolerations the per-node Jobs will
//! run with, which keeps "which nodes get Kata" identical in both modes.

use k8s_openapi::api::core::v1::{Node, Taint, Toleration};

/// Taint effects that keep a pod off a node. `PreferNoSchedule` expresses a
/// scheduling preference and never blocks admission, so it is not listed.
const BLOCKING_EFFECTS: [&str; 2] = ["NoSchedule", "NoExecute"];

/// A node left out of the rollout, and the taint responsible for it.
pub struct SkippedNode {
    pub name: String,
    pub taint: Taint,
}

/// Render a taint the way `kubectl taint` spells it, for logs and errors.
pub fn describe_taint(taint: &Taint) -> String {
    match taint.value.as_deref() {
        Some(value) if !value.is_empty() => {
            format!("{}={}:{}", taint.key, value, taint.effect)
        }
        _ => format!("{}:{}", taint.key, taint.effect),
    }
}

/// The toleration a user needs to add to reach a node blocked by `taint`.
pub fn suggested_toleration(taint: &Taint) -> String {
    format!(
        "tolerations:\n  - key: {}\n    operator: Exists\n    effect: {}",
        taint.key, taint.effect
    )
}

/// Tolerations the DaemonSet controller silently adds to every DaemonSet pod.
/// We add the same set so job mode still reaches the nodes a DaemonSet would
/// have - most visibly cordoned (`unschedulable`) and resource-pressured nodes.
///
/// `node.kubernetes.io/network-unavailable` is deliberately absent: the
/// controller only adds it for host-network pods, and kata-deploy does not use
/// host networking.
fn daemonset_controller_tolerations() -> Vec<Toleration> {
    [
        ("node.kubernetes.io/not-ready", "NoExecute"),
        ("node.kubernetes.io/unreachable", "NoExecute"),
        ("node.kubernetes.io/disk-pressure", "NoSchedule"),
        ("node.kubernetes.io/memory-pressure", "NoSchedule"),
        ("node.kubernetes.io/pid-pressure", "NoSchedule"),
        ("node.kubernetes.io/unschedulable", "NoSchedule"),
    ]
    .into_iter()
    .map(|(key, effect)| Toleration {
        key: Some(key.to_string()),
        operator: Some("Exists".to_string()),
        effect: Some(effect.to_string()),
        ..Default::default()
    })
    .collect()
}

/// Whether `toleration` tolerates `taint`, following Kubernetes' own matching
/// rules: an empty effect matches any effect, an empty key matches any key, and
/// the operator defaults to `Equal` (which compares values).
pub fn tolerates(toleration: &Toleration, taint: &Taint) -> bool {
    if let Some(effect) = toleration.effect.as_deref() {
        if !effect.is_empty() && effect != taint.effect {
            return false;
        }
    }
    if let Some(key) = toleration.key.as_deref() {
        if !key.is_empty() && key != taint.key {
            return false;
        }
    }

    match toleration.operator.as_deref().unwrap_or("Equal") {
        "Exists" => true,
        _ => toleration.value.as_deref().unwrap_or("") == taint.value.as_deref().unwrap_or(""),
    }
}

/// The first blocking taint on the node that nothing tolerates, if any.
pub fn untolerated_taint<'a>(taints: &'a [Taint], tolerations: &[Toleration]) -> Option<&'a Taint> {
    taints.iter().find(|taint| {
        BLOCKING_EFFECTS.contains(&taint.effect.as_str())
            && !tolerations.iter().any(|tol| tolerates(tol, taint))
    })
}

/// Split the selected nodes into those the per-node Jobs may run on and those
/// blocked by a taint they do not tolerate, applying the same admission rules a
/// DaemonSet pod would face.
pub fn partition_by_tolerations(
    nodes: &[Node],
    tolerations: &[Toleration],
) -> (Vec<String>, Vec<SkippedNode>) {
    let mut effective = tolerations.to_vec();
    effective.extend(daemonset_controller_tolerations());

    let mut admitted = Vec::new();
    let mut skipped = Vec::new();

    for node in nodes {
        let Some(name) = node.metadata.name.clone() else {
            continue;
        };
        let taints = node
            .spec
            .as_ref()
            .and_then(|spec| spec.taints.as_deref())
            .unwrap_or(&[]);

        match untolerated_taint(taints, &effective) {
            Some(taint) => skipped.push(SkippedNode {
                name,
                taint: taint.clone(),
            }),
            None => admitted.push(name),
        }
    }

    // A node matching several selectors is listed once per match, and both
    // halves are reported to the user ("N node(s) skipped"), so collapse the
    // repeats rather than inflating the counts.
    admitted.sort();
    admitted.dedup();
    skipped.sort_by(|a, b| a.name.cmp(&b.name));
    skipped.dedup_by(|a, b| a.name == b.name);
    (admitted, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::NodeSpec;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn taint(key: &str, value: Option<&str>, effect: &str) -> Taint {
        Taint {
            key: key.to_string(),
            value: value.map(str::to_string),
            effect: effect.to_string(),
            ..Default::default()
        }
    }

    fn toleration(key: Option<&str>, operator: &str, effect: Option<&str>) -> Toleration {
        Toleration {
            key: key.map(str::to_string),
            operator: Some(operator.to_string()),
            effect: effect.map(str::to_string),
            ..Default::default()
        }
    }

    fn node(name: &str, taints: Vec<Taint>) -> Node {
        Node {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..Default::default()
            },
            spec: Some(NodeSpec {
                taints: Some(taints),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn control_plane_taint() -> Taint {
        taint("node-role.kubernetes.io/control-plane", None, "NoSchedule")
    }

    #[test]
    fn a_node_matched_by_several_selectors_is_reported_once() {
        let admitted_twice = node("worker-1", vec![]);
        let skipped_twice = node("cp-1", vec![control_plane_taint()]);
        let nodes = vec![
            admitted_twice.clone(),
            skipped_twice.clone(),
            admitted_twice,
            skipped_twice,
        ];

        let (admitted, skipped) = partition_by_tolerations(&nodes, &[]);

        assert_eq!(admitted, vec!["worker-1".to_string()]);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].name, "cp-1");
    }

    #[test]
    fn exists_toleration_matches_regardless_of_value() {
        let tol = toleration(
            Some("node-role.kubernetes.io/control-plane"),
            "Exists",
            Some("NoSchedule"),
        );
        assert!(tolerates(&tol, &control_plane_taint()));
    }

    #[test]
    fn empty_key_with_exists_tolerates_everything() {
        let tol = toleration(None, "Exists", None);
        assert!(tolerates(&tol, &control_plane_taint()));
        assert!(tolerates(&tol, &taint("custom", Some("v"), "NoExecute")));
    }

    #[test]
    fn effect_must_match_when_specified() {
        let tol = toleration(
            Some("node-role.kubernetes.io/control-plane"),
            "Exists",
            Some("NoExecute"),
        );
        assert!(!tolerates(&tol, &control_plane_taint()));
    }

    #[test]
    fn equal_operator_compares_values() {
        let mut tol = toleration(Some("dedicated"), "Equal", None);
        tol.value = Some("kata".to_string());

        assert!(tolerates(
            &tol,
            &taint("dedicated", Some("kata"), "NoSchedule")
        ));
        assert!(!tolerates(
            &tol,
            &taint("dedicated", Some("other"), "NoSchedule")
        ));
    }

    #[test]
    fn prefer_no_schedule_never_blocks() {
        let taints = vec![taint("spot", None, "PreferNoSchedule")];
        assert!(untolerated_taint(&taints, &[]).is_none());
    }

    #[test]
    fn control_plane_node_is_skipped_without_a_toleration() {
        let nodes = vec![
            node("worker-1", vec![]),
            node("cp-1", vec![control_plane_taint()]),
        ];

        let (admitted, skipped) = partition_by_tolerations(&nodes, &[]);

        assert_eq!(admitted, vec!["worker-1"]);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].name, "cp-1");
        assert_eq!(
            describe_taint(&skipped[0].taint),
            "node-role.kubernetes.io/control-plane:NoSchedule"
        );
    }

    #[test]
    fn control_plane_node_is_admitted_once_tolerated() {
        let nodes = vec![node("cp-1", vec![control_plane_taint()])];
        let tolerations = vec![toleration(
            Some("node-role.kubernetes.io/control-plane"),
            "Exists",
            Some("NoSchedule"),
        )];

        let (admitted, skipped) = partition_by_tolerations(&nodes, &tolerations);

        assert_eq!(admitted, vec!["cp-1"]);
        assert!(skipped.is_empty());
    }

    #[test]
    fn cordoned_and_pressured_nodes_stay_admitted_like_a_daemonset() {
        // The DaemonSet controller tolerates these implicitly, so job mode must
        // too, otherwise a cordoned node silently misses the install.
        let nodes = vec![
            node(
                "cordoned",
                vec![taint(
                    "node.kubernetes.io/unschedulable",
                    None,
                    "NoSchedule",
                )],
            ),
            node(
                "pressured",
                vec![taint(
                    "node.kubernetes.io/memory-pressure",
                    None,
                    "NoSchedule",
                )],
            ),
        ];

        let (admitted, skipped) = partition_by_tolerations(&nodes, &[]);

        assert_eq!(admitted, vec!["cordoned", "pressured"]);
        assert!(skipped.is_empty());
    }

    #[test]
    fn nodes_without_a_spec_or_taints_are_admitted() {
        let bare = Node {
            metadata: ObjectMeta {
                name: Some("bare".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let (admitted, skipped) = partition_by_tolerations(&[bare], &[]);

        assert_eq!(admitted, vec!["bare"]);
        assert!(skipped.is_empty());
    }

    #[test]
    fn suggested_toleration_names_the_blocking_taint() {
        let suggestion = suggested_toleration(&control_plane_taint());

        assert!(suggestion.contains("key: node-role.kubernetes.io/control-plane"));
        assert!(suggestion.contains("effect: NoSchedule"));
    }

    #[test]
    fn describe_taint_includes_the_value_when_present() {
        assert_eq!(
            describe_taint(&taint("dedicated", Some("kata"), "NoSchedule")),
            "dedicated=kata:NoSchedule"
        );
    }
}
