// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::Result;
use kata_types::{
    annotations::{EnvdAnnotation, KATA_ANNO_CFG_RUNTIME_ENVD},
    k8s::container_type,
    mount::{ENVD_CONTAINER_PATH, ENVD_GUEST_PATH},
};
use oci_spec::runtime as oci;

/// Called while holding the container-map write lock, before insertion.
pub(crate) fn select<'a>(
    sandbox_annotations: &HashMap<String, String>,
    current: &oci::Spec,
    mut previous: impl Iterator<Item = &'a oci::Spec>,
) -> Result<Option<EnvdAnnotation>> {
    if container_type(current).is_pod_sandbox()
        || previous.any(|s| !container_type(s).is_pod_sandbox())
    {
        return Ok(None);
    }
    sandbox_annotations
        .get(KATA_ANNO_CFG_RUNTIME_ENVD)
        .map(|raw| EnvdAnnotation::parse(raw))
        .transpose()
}

/// Inject only after host volume handling, so the source stays guest-local.
pub(crate) fn configure(spec: &mut oci::Spec, config: Option<EnvdAnnotation>) -> Result<()> {
    // Discard requests from workload annotations, including on later containers.
    let annotations = spec.annotations_mut().get_or_insert_with(Default::default);
    annotations.remove(KATA_ANNO_CFG_RUNTIME_ENVD);
    if let Some(mut config) = config {
        config.resolve_args();
        let guest_path = config.guest_path.take();
        annotations.insert(
            KATA_ANNO_CFG_RUNTIME_ENVD.to_string(),
            serde_json::to_string(&config)?,
        );
        spec.mounts_mut().get_or_insert_with(Vec::new).push(
            oci::MountBuilder::default()
                .source(guest_path.as_deref().unwrap_or(ENVD_GUEST_PATH))
                .destination(ENVD_CONTAINER_PATH)
                .typ("bind")
                .options(vec!["bind".to_string(), "ro".to_string()])
                .build()?,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_types::annotations::{cri_containerd, dockershim};

    fn spec(kind: Option<(&str, &str)>) -> oci::Spec {
        let mut spec = oci::Spec::default();
        if let Some((key, value)) = kind {
            spec.annotations_mut()
                .get_or_insert_with(Default::default)
                .insert(key.to_string(), value.to_string());
        }
        spec
    }

    #[test]
    fn selects_only_first_non_pause_container() {
        let annotations = HashMap::from([(
            KATA_ANNO_CFG_RUNTIME_ENVD.to_string(),
            r#"{"port":18080,"args":[]}"#.to_string(),
        )]);
        let standalone = spec(None);
        let pause = spec(Some((cri_containerd::CONTAINER_TYPE_LABEL_KEY, "sandbox")));
        let docker_pause = spec(Some((dockershim::CONTAINER_TYPE_LABEL_KEY, "podsandbox")));
        let workload = spec(Some((
            cri_containerd::CONTAINER_TYPE_LABEL_KEY,
            "container",
        )));
        for (current, previous, expected) in [
            (&standalone, vec![], true),
            (&pause, vec![], false),
            (&docker_pause, vec![], false),
            (&workload, vec![&pause, &docker_pause], true),
            (&standalone, vec![&pause], true),
            (&workload, vec![&pause, &workload], false),
            (&standalone, vec![&standalone], false),
        ] {
            let selected = select(&annotations, current, previous.into_iter()).unwrap();
            assert_eq!(selected.is_some(), expected);
        }
    }

    #[test]
    fn injects_guest_mount_and_resolves_args() {
        let mut spec = oci::Spec::default();
        let existing_mounts = spec.mounts().clone().unwrap_or_default();
        let config = EnvdAnnotation::parse(
            r#"{"port":65535,"args":["serve","--addr=:{port}","two words","$(literal)",""]}"#,
        )
        .unwrap();
        configure(&mut spec, Some(config)).unwrap();
        let raw = &spec.annotations().as_ref().unwrap()[KATA_ANNO_CFG_RUNTIME_ENVD];
        let config: EnvdAnnotation = serde_json::from_str(raw).unwrap();
        assert_eq!(config.guest_path, None);
        assert_eq!(
            config.args,
            ["serve", "--addr=:65535", "two words", "$(literal)", ""]
        );
        let mounts = spec.mounts().as_ref().unwrap();
        assert_eq!(&mounts[..existing_mounts.len()], existing_mounts);
        let mount = mounts.last().unwrap();
        assert_eq!(
            mount.source().as_deref(),
            Some(std::path::Path::new(ENVD_GUEST_PATH))
        );
        assert_eq!(
            mount.destination(),
            std::path::Path::new(ENVD_CONTAINER_PATH)
        );
        assert_eq!(mount.typ().as_deref(), Some("bind"));
        assert_eq!(mount.options().as_ref().unwrap(), &["bind", "ro"]);
    }

    #[test]
    fn uses_custom_guest_path_only_for_mount_source() {
        let path = "/run/kata-extensions/envd/bin/kata-envd";
        let config = EnvdAnnotation::parse(
            &serde_json::json!({
                "port": 49983,
                "args": ["serve", "--addr", ":{port}"],
                "guest_path": path,
            })
            .to_string(),
        )
        .unwrap();
        let mut spec = oci::Spec::default();
        configure(&mut spec, Some(config)).unwrap();

        let mount = spec.mounts().as_ref().unwrap().last().unwrap();
        assert_eq!(mount.source().as_deref(), Some(std::path::Path::new(path)));
        assert_eq!(
            mount.destination(),
            std::path::Path::new(ENVD_CONTAINER_PATH)
        );
        assert_eq!(mount.typ().as_deref(), Some("bind"));
        assert_eq!(mount.options().as_ref().unwrap(), &["bind", "ro"]);

        // The agent receives only launch arguments and still executes the fixed
        // container path; guest_path is consumed entirely by the runtime.
        let raw = &spec.annotations().as_ref().unwrap()[KATA_ANNO_CFG_RUNTIME_ENVD];
        let request: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(
            request,
            serde_json::json!({"port":49983,"args":["serve","--addr",":49983"]})
        );
    }

    #[test]
    fn discards_container_requests_and_uses_only_sandbox_request() {
        let mut current = spec(None);
        current
            .annotations_mut()
            .get_or_insert_with(Default::default)
            .insert(
                KATA_ANNO_CFG_RUNTIME_ENVD.to_string(),
                "invalid workload request".to_string(),
            );
        let mounts = current.mounts().clone();
        let config = select(&HashMap::new(), &current, std::iter::empty()).unwrap();
        configure(&mut current, config).unwrap();
        assert!(!current
            .annotations()
            .as_ref()
            .unwrap()
            .contains_key(KATA_ANNO_CFG_RUNTIME_ENVD));
        assert_eq!(current.mounts(), &mounts);

        let sandbox = HashMap::from([(
            KATA_ANNO_CFG_RUNTIME_ENVD.to_string(),
            r#"{"port":1,"args":[]}"#.to_string(),
        )]);
        let config = select(&sandbox, &current, std::iter::empty()).unwrap();
        configure(&mut current, config).unwrap();
        let raw = &current.annotations().as_ref().unwrap()[KATA_ANNO_CFG_RUNTIME_ENVD];
        assert!(EnvdAnnotation::parse(raw).unwrap().args.is_empty());
    }

    #[test]
    fn invalid_sandbox_request_fails_selection() {
        let annotations =
            HashMap::from([(KATA_ANNO_CFG_RUNTIME_ENVD.to_string(), "{}".to_string())]);
        assert!(select(&annotations, &spec(None), std::iter::empty()).is_err());
    }
}
