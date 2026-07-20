// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

//! FR-7 — complete mediation manifest.
//!
//! Total mediation requires that *every* agent RPC that a (possibly hostile) caller can
//! invoke passes through the policy enforcement point before it can act, with no
//! always-allowed escape hatch. This module is the machine-checkable record of that
//! property:
//!
//!  - [`MEDIATION_MANIFEST`] classifies every method of the agent ttRPC service by the
//!    enforcement point it must pass through.
//!  - The tests below fail the build if the proto service and this manifest drift apart
//!    (a new RPC added without a mediation classification), or if a handler that the
//!    manifest declares mediated does not actually invoke its enforcement point.
//!
//! In strict builds the default policy is closed-door (every request denied unless the
//! activated policy allows it), so a mediated RPC with no matching allow rule is denied.
//! The enforcement classes here document *how* each RPC is mediated, not *whether* a
//! particular policy happens to allow it.

/// The enforcement point an RPC must pass through before it can take effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnforcementClass {
    /// Mutates container/sandbox lifecycle state; gated by policy *and* by the FR-9
    /// occurrence state machine.
    LifecycleGated,
    /// Mutates guest/sandbox state; gated by the policy enforcement point (`is_allowed`).
    PolicyGated,
    /// Read-only / observational; still gated by the policy enforcement point.
    PolicyGatedQuery,
    /// The one-shot policy-activation endpoint; self-gated by the `SetPolicyRequest`
    /// rule in the currently active policy.
    PolicyActivation,
}

impl EnforcementClass {
    /// Source token that a handler in this class must contain to prove it reaches its
    /// enforcement point.
    fn required_gate_token(self) -> &'static str {
        match self {
            EnforcementClass::PolicyActivation => "do_set_policy",
            _ => "is_allowed",
        }
    }
}

/// Every method of the agent ttRPC service, the handler that implements it, and the
/// enforcement point it must pass through. This table is exhaustive: the proto-sync test
/// fails if any service method is missing or unknown.
pub const MEDIATION_MANIFEST: &[(&str, &str, EnforcementClass)] = &[
    // Container lifecycle (policy + occurrence state machine).
    ("CreateContainer", "create_container", EnforcementClass::LifecycleGated),
    ("StartContainer", "start_container", EnforcementClass::LifecycleGated),
    ("RemoveContainer", "remove_container", EnforcementClass::LifecycleGated),
    ("ExecProcess", "exec_process", EnforcementClass::LifecycleGated),
    ("SignalProcess", "signal_process", EnforcementClass::LifecycleGated),
    // State-mutating operations (policy gated).
    ("WaitProcess", "wait_process", EnforcementClass::PolicyGated),
    ("UpdateContainer", "update_container", EnforcementClass::PolicyGated),
    ("UpdateEphemeralMounts", "update_ephemeral_mounts", EnforcementClass::PolicyGated),
    ("PauseContainer", "pause_container", EnforcementClass::PolicyGated),
    ("ResumeContainer", "resume_container", EnforcementClass::PolicyGated),
    ("RemoveStaleVirtiofsShareMounts", "remove_stale_virtiofs_share_mounts", EnforcementClass::PolicyGated),
    ("WriteStdin", "write_stdin", EnforcementClass::PolicyGated),
    ("CloseStdin", "close_stdin", EnforcementClass::PolicyGated),
    ("TtyWinResize", "tty_win_resize", EnforcementClass::PolicyGated),
    ("UpdateInterface", "update_interface", EnforcementClass::PolicyGated),
    ("UpdateRoutes", "update_routes", EnforcementClass::PolicyGated),
    ("AddARPNeighbors", "add_arp_neighbors", EnforcementClass::PolicyGated),
    ("SetIPTables", "set_ip_tables", EnforcementClass::PolicyGated),
    ("MemAgentMemcgSet", "mem_agent_memcg_set", EnforcementClass::PolicyGated),
    ("MemAgentCompactSet", "mem_agent_compact_set", EnforcementClass::PolicyGated),
    ("CreateSandbox", "create_sandbox", EnforcementClass::PolicyGated),
    ("DestroySandbox", "destroy_sandbox", EnforcementClass::PolicyGated),
    ("OnlineCPUMem", "online_cpu_mem", EnforcementClass::PolicyGated),
    ("ReseedRandomDev", "reseed_random_dev", EnforcementClass::PolicyGated),
    ("MemHotplugByProbe", "mem_hotplug_by_probe", EnforcementClass::PolicyGated),
    ("SetGuestDateTime", "set_guest_date_time", EnforcementClass::PolicyGated),
    ("CopyFile", "copy_file", EnforcementClass::PolicyGated),
    ("AddSwap", "add_swap", EnforcementClass::PolicyGated),
    ("AddSwapPath", "add_swap_path", EnforcementClass::PolicyGated),
    ("ResizeVolume", "resize_volume", EnforcementClass::PolicyGated),
    // Read-only / observational (still policy gated).
    ("StatsContainer", "stats_container", EnforcementClass::PolicyGatedQuery),
    ("GetDiagnosticData", "get_diagnostic_data", EnforcementClass::PolicyGatedQuery),
    ("ReadStdout", "read_stdout", EnforcementClass::PolicyGatedQuery),
    ("ReadStderr", "read_stderr", EnforcementClass::PolicyGatedQuery),
    ("ListInterfaces", "list_interfaces", EnforcementClass::PolicyGatedQuery),
    ("ListRoutes", "list_routes", EnforcementClass::PolicyGatedQuery),
    ("GetIPTables", "get_ip_tables", EnforcementClass::PolicyGatedQuery),
    ("GetMetrics", "get_metrics", EnforcementClass::PolicyGatedQuery),
    ("GetGuestDetails", "get_guest_details", EnforcementClass::PolicyGatedQuery),
    ("GetOOMEvent", "get_oom_event", EnforcementClass::PolicyGatedQuery),
    ("GetVolumeStats", "get_volume_stats", EnforcementClass::PolicyGatedQuery),
    // Policy activation.
    ("SetPolicy", "set_policy", EnforcementClass::PolicyActivation),
    ("LoadPolicyFragment", "load_policy_fragment", EnforcementClass::PolicyGated),
];

/// Return the enforcement class declared for a service method, if any.
pub fn enforcement_class(rpc_method: &str) -> Option<EnforcementClass> {
    MEDIATION_MANIFEST
        .iter()
        .find(|(m, _, _)| *m == rpc_method)
        .map(|(_, _, c)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const AGENT_PROTO: &str = include_str!("../../libs/protocols/protos/agent.proto");
    const RPC_SOURCE: &str = include_str!("rpc.rs");

    /// Extract every `rpc <Name>(` method declared by the agent proto service.
    fn proto_rpc_methods() -> Vec<String> {
        AGENT_PROTO
            .lines()
            .filter_map(|line| {
                let l = line.trim();
                let rest = l.strip_prefix("rpc ")?;
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                (!name.is_empty()).then_some(name)
            })
            .collect()
    }

    /// TC3.9: complete-mediation coverage. Every RPC exposed by the service must be
    /// classified in the manifest, and the manifest must not classify a method the
    /// service does not expose. Adding a new RPC without classifying it fails the build.
    #[test]
    fn every_service_rpc_is_classified() {
        let proto: HashSet<String> = proto_rpc_methods().into_iter().collect();
        assert!(!proto.is_empty(), "failed to parse any rpc from agent.proto");

        let manifest: HashSet<String> =
            MEDIATION_MANIFEST.iter().map(|(m, _, _)| m.to_string()).collect();

        let unclassified: Vec<_> = proto.difference(&manifest).collect();
        assert!(
            unclassified.is_empty(),
            "agent RPC(s) exposed but not covered by the mediation manifest (FR-7 gap): {unclassified:?}"
        );

        let stale: Vec<_> = manifest.difference(&proto).collect();
        assert!(
            stale.is_empty(),
            "mediation manifest classifies method(s) the service no longer exposes: {stale:?}"
        );
    }

    /// Each handler the manifest declares mediated must actually reach its enforcement
    /// point (call `is_allowed`, or `do_set_policy` for policy activation). This closes
    /// the "handler silently skips the policy check" gap.
    #[test]
    fn every_mediated_handler_reaches_its_enforcement_point() {
        for (rpc, handler, class) in MEDIATION_MANIFEST {
            let token = class.required_gate_token();
            let decl = format!("async fn {handler}(");
            let start = RPC_SOURCE.find(&decl).unwrap_or_else(|| {
                panic!("handler `{handler}` for RPC `{rpc}` not found in rpc.rs")
            });
            // Bound the body at the next handler declaration to avoid leaking a gate
            // token from a neighbouring handler into this one.
            let after = &RPC_SOURCE[start + decl.len()..];
            let end = after.find("async fn ").map(|e| e).unwrap_or(after.len());
            let body = &after[..end];
            assert!(
                body.contains(token),
                "RPC `{rpc}` (handler `{handler}`, class {class:?}) does not reach its \
                 enforcement point (`{token}` not found in its body) — total-mediation gap"
            );
        }
    }

    /// No agent RPC may be left unmediated. Encoded as: every manifest entry belongs to a
    /// mediated class (there is no `Unmediated` variant), and the manifest is non-empty.
    #[test]
    fn no_always_allowed_escape_hatch() {
        assert!(!MEDIATION_MANIFEST.is_empty());
        for (rpc, _, class) in MEDIATION_MANIFEST {
            // All defined classes are enforcement points; this match must stay exhaustive
            // so adding a future non-mediated class forces a deliberate decision here.
            match class {
                EnforcementClass::LifecycleGated
                | EnforcementClass::PolicyGated
                | EnforcementClass::PolicyGatedQuery
                | EnforcementClass::PolicyActivation => {}
            }
            assert!(enforcement_class(rpc).is_some());
        }
    }
}
