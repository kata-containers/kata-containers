// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

use kata_types::annotations::KATA_ANNO_CFG_RUNTIME_ENVD;
use oci_spec::runtime as oci;

/// Forward the sandbox request only for containers that opt in.
pub(crate) fn inject_if_enabled(spec: &mut oci::Spec, sandbox_request: Option<&str>) {
    // Use the sandbox request only after checking the container's opt-in.
    if let Some(annotations) = spec.annotations_mut() {
        annotations.remove(KATA_ANNO_CFG_RUNTIME_ENVD);
    }
    let Some(raw) = sandbox_request else {
        return;
    };
    let enabled = spec
        .process()
        .as_ref()
        .and_then(|process| process.env().as_ref())
        .and_then(|env| {
            env.iter()
                .rev()
                .find_map(|entry| entry.strip_prefix("KATA_ENVD_ENABLED="))
        });
    if enabled != Some("true") {
        return;
    }

    spec.annotations_mut()
        .get_or_insert_with(Default::default)
        .insert(KATA_ANNO_CFG_RUNTIME_ENVD.to_string(), raw.to_owned());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(env: &[&str]) -> oci::Spec {
        let mut spec = oci::Spec::default();
        let mut process = oci::Process::default();
        process.set_env(Some(env.iter().map(|value| value.to_string()).collect()));
        spec.set_process(Some(process));
        spec
    }

    #[test]
    fn injects_only_into_opted_in_containers() {
        let request = r#"{"args":[]}"#;
        let enabled = "KATA_ENVD_ENABLED=true";
        let disabled = "KATA_ENVD_ENABLED=false";
        for (env, expected) in [
            (&[][..], false),
            (&[disabled][..], false),
            (&["KATA_ENVD_ENABLED=True"][..], false),
            (&[enabled][..], true),
            (&[enabled, disabled][..], false),
            (&[disabled, enabled][..], true),
        ] {
            let mut current = spec(env);
            current
                .annotations_mut()
                .get_or_insert_with(Default::default)
                .insert(KATA_ANNO_CFG_RUNTIME_ENVD.to_string(), request.to_string());
            let mounts = current.mounts().clone();
            inject_if_enabled(&mut current, Some(request));
            let annotated = current
                .annotations()
                .as_ref()
                .is_some_and(|annotations| annotations.contains_key(KATA_ANNO_CFG_RUNTIME_ENVD));
            assert_eq!(annotated, expected, "{env:?}");
            assert_eq!(current.mounts(), &mounts, "{env:?}");
        }
    }

    #[test]
    fn injects_the_original_request() {
        let mut spec = spec(&["KATA_ENVD_ENABLED=true"]);
        let request = r#"{
            "args": ["serve", "--addr", ":49983", "two words", ""]
        }"#;
        inject_if_enabled(&mut spec, Some(request));
        let raw = &spec.annotations().as_ref().unwrap()[KATA_ANNO_CFG_RUNTIME_ENVD];
        assert_eq!(raw, request);
    }

    #[test]
    fn discards_container_requests_and_uses_only_sandbox_request() {
        let mut current = spec(&["KATA_ENVD_ENABLED=true"]);
        current
            .annotations_mut()
            .get_or_insert_with(Default::default)
            .insert(
                KATA_ANNO_CFG_RUNTIME_ENVD.to_string(),
                "invalid workload request".to_string(),
            );
        let mounts = current.mounts().clone();
        inject_if_enabled(&mut current, None);
        assert!(!current
            .annotations()
            .as_ref()
            .unwrap()
            .contains_key(KATA_ANNO_CFG_RUNTIME_ENVD));
        assert_eq!(current.mounts(), &mounts);

        let request = r#"{"args":[]}"#;
        inject_if_enabled(&mut current, Some(request));
        let raw = &current.annotations().as_ref().unwrap()[KATA_ANNO_CFG_RUNTIME_ENVD];
        assert_eq!(raw, request);
    }
}
