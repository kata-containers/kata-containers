// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-8 — structured, rule-attributable decision objects.
//!
//! When the policy denies a request, the agent must be able to explain *why* in a way
//! that is auditable but leaks nothing sensitive. A [`DecisionObject`] records:
//!
//!  - the **endpoint** that was evaluated (e.g. `CreateContainerRequest`);
//!  - the **decision** (`deny`);
//!  - the **failed rule** that produced the denial (the Rego query path that evaluated to
//!    false), giving rule attribution;
//!  - the **bound state keys**: the *names* of the top-level fields present in the request
//!    that were bound during evaluation.
//!
//! Crucially, the decision object never contains request **values**: no environment
//! variable values, no sealed secrets, no policy text — only field names and rule names.
//! This lets an operator see which request shape hit which rule without exposing the
//! workload's data or the policy's contents.

use serde::Serialize;

/// A structured, redaction-safe record of a policy decision (emitted on denial).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DecisionObject {
    /// The policy endpoint / request kind that was evaluated.
    pub endpoint: String,
    /// The decision. Always `deny` for objects produced on denial.
    pub decision: &'static str,
    /// The Rego rule (query path) that produced the denial — rule attribution.
    pub failed_rule: String,
    /// Names of the top-level request fields bound during evaluation. Field *names* only,
    /// never their values.
    pub bound_state_keys: Vec<String>,
}

impl DecisionObject {
    /// Build a denial decision object for `endpoint`, extracting only the top-level field
    /// names from the (JSON) request input. Values are deliberately discarded so no
    /// request data can leak into the audit record.
    pub fn for_denial(endpoint: &str, request_input_json: &str) -> Self {
        let mut bound_state_keys = extract_top_level_keys(request_input_json);
        // Deterministic ordering for stable, comparable audit records.
        bound_state_keys.sort();
        DecisionObject {
            endpoint: endpoint.to_string(),
            decision: "deny",
            // The denied rule is the endpoint's query path; it evaluated to false.
            failed_rule: format!("data.agent_policy.{endpoint}"),
            bound_state_keys,
        }
    }

    /// Serialize to a single-line JSON audit record. Serialization only ever includes the
    /// (redaction-safe) fields of this struct.
    pub fn to_json(&self) -> String {
        // The struct contains no values, so this cannot leak request data.
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(
                "{{\"endpoint\":\"{}\",\"decision\":\"deny\"}}",
                self.endpoint
            )
        })
    }
}

/// Extract the names of the top-level object fields of a JSON document. Nested values are
/// never traversed and never included; a non-object document yields no keys.
fn extract_top_level_keys(json: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TC6.5: a denial emits a structured object with endpoint, failed rule, and the
    /// bound state keys (field names).
    #[test]
    fn denial_object_has_attribution_and_keys() {
        let input = r#"{"container_id":"c1","OCI":{"process":{}},"storages":[]}"#;
        let d = DecisionObject::for_denial("CreateContainerRequest", input);
        assert_eq!(d.endpoint, "CreateContainerRequest");
        assert_eq!(d.decision, "deny");
        assert_eq!(d.failed_rule, "data.agent_policy.CreateContainerRequest");
        assert_eq!(
            d.bound_state_keys,
            vec![
                "OCI".to_string(),
                "container_id".to_string(),
                "storages".to_string()
            ]
        );
    }

    /// TC6.6: the decision object (and its serialization) contains no request values —
    /// no env values, no sealed secrets, no policy text.
    #[test]
    fn decision_object_leaks_no_values() {
        // A request laden with sensitive values.
        let input = r#"{
            "container_id":"c1",
            "OCI":{"process":{"env":["API_KEY=supersecretvalue","DB_PASSWORD=hunter2"]}},
            "sealed_secret":"SEALED.eyJz.aGVsbG8",
            "policy":"package agent_policy\ndefault CreateContainerRequest := true"
        }"#;
        let d = DecisionObject::for_denial("CreateContainerRequest", input);
        let json = d.to_json();

        // Field names may appear; values must not.
        for leaked in [
            "supersecretvalue",
            "hunter2",
            "API_KEY=supersecretvalue",
            "SEALED.eyJz.aGVsbG8",
            "package agent_policy",
            "default CreateContainerRequest := true",
        ] {
            assert!(
                !json.contains(leaked),
                "decision object leaked a sensitive value: {leaked}\nobject: {json}"
            );
        }
        // The bound keys are names only (no values attached).
        assert!(d.bound_state_keys.contains(&"sealed_secret".to_string()));
        assert!(d.bound_state_keys.contains(&"policy".to_string()));
        assert!(!json.contains("supersecret"));
    }

    #[test]
    fn non_object_input_yields_no_keys() {
        let d = DecisionObject::for_denial("SomeRequest", "\"not-an-object\"");
        assert!(d.bound_state_keys.is_empty());
        assert_eq!(d.decision, "deny");
    }
}
