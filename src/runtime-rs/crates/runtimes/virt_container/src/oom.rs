// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Telling CRI-O that a container was OOM killed.
//!
//! The guest kernel enforces a container's memory limit, so the host cgroup a
//! CRI would normally watch stays quiet and the kill reaches the kubelet as a
//! plain non-zero exit. containerd is told by the shim's TaskOOM event. CRI-O
//! ignores that event for VM runtimes and instead stats a file named `oom` in
//! the bundle of any such container that exits non-zero, as the Go runtime has
//! written since kata-containers/runtime#2961.
//!
//! The bundles are collected here because the OOM events arrive on the
//! sandbox's watcher while the bundle a container was created with is the
//! container manager's to know.

use std::collections::HashMap;
use std::path::PathBuf;

use kata_types::annotations::crio;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

const OOM_FILE_NAME: &str = "oom";

/// The bundles of a sandbox's CRI-O containers.
#[derive(Default)]
pub struct CrioOomNotifier {
    bundles: RwLock<HashMap<String, PathBuf>>,
}

impl CrioOomNotifier {
    /// Remember where to leave word for `container_id`. Only CRI-O's own
    /// containers are tracked, so an OOM on anyone else's writes nothing.
    pub async fn track(&self, container_id: &str, bundle: &str, spec: &oci::Spec) {
        if !is_crio_managed(spec) {
            return;
        }

        self.bundles
            .write()
            .await
            .insert(container_id.to_string(), PathBuf::from(bundle));
    }

    /// Drop a container that never came up. CRI-O stats the file while
    /// reporting a status the container has already exited from, so this is
    /// the one case where a bundle can go before the sandbox does.
    pub async fn forget(&self, container_id: &str) {
        self.bundles.write().await.remove(container_id);
    }

    /// Leave the file CRI-O reads once it sees the container exit.
    pub async fn notify(&self, container_id: &str) {
        let bundle = match self.bundles.read().await.get(container_id) {
            Some(bundle) => bundle.clone(),
            None => return,
        };

        let path = bundle.join(OOM_FILE_NAME);
        match tokio::fs::File::create(&path).await {
            Ok(_) => info!(
                sl!(),
                "wrote {} to notify CRI-O that {} was OOM killed",
                path.display(),
                container_id
            ),
            Err(e) => warn!(sl!(), "failed to write {}: {:?}", path.display(), e),
        }
    }
}

fn is_crio_managed(spec: &oci::Spec) -> bool {
    spec.annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(crio::CONTAINER_TYPE_LABEL_KEY))
        .is_some_and(|typ| typ == crio::SANDBOX || typ == crio::CONTAINER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_with_annotations(annotations: &[(&str, &str)]) -> oci::Spec {
        let mut spec = oci::Spec::default();
        spec.set_annotations(Some(
            annotations
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        ));
        spec
    }

    #[test]
    fn test_is_crio_managed() {
        assert!(is_crio_managed(&spec_with_annotations(&[(
            crio::CONTAINER_TYPE_LABEL_KEY,
            crio::CONTAINER,
        )])));
        assert!(is_crio_managed(&spec_with_annotations(&[(
            crio::CONTAINER_TYPE_LABEL_KEY,
            crio::SANDBOX,
        )])));

        // containerd's own container type key must not be mistaken for CRI-O's.
        assert!(!is_crio_managed(&spec_with_annotations(&[(
            kata_types::annotations::cri_containerd::CONTAINER_TYPE_LABEL_KEY,
            "container",
        )])));
        assert!(!is_crio_managed(&oci::Spec::default()));
    }

    #[tokio::test]
    async fn test_notify_writes_oom_file_for_tracked_container() {
        let bundle = tempfile::tempdir().unwrap();
        let notifier = CrioOomNotifier::default();
        let spec = spec_with_annotations(&[(crio::CONTAINER_TYPE_LABEL_KEY, crio::CONTAINER)]);

        notifier
            .track("cid", bundle.path().to_str().unwrap(), &spec)
            .await;
        notifier.notify("cid").await;

        assert!(bundle.path().join(OOM_FILE_NAME).exists());
    }

    #[tokio::test]
    async fn test_notify_is_silent_for_untracked_containers() {
        let bundle = tempfile::tempdir().unwrap();
        let notifier = CrioOomNotifier::default();

        // containerd's containers are never tracked, and one whose creation
        // failed is not written to either.
        notifier
            .track(
                "cid",
                bundle.path().to_str().unwrap(),
                &oci::Spec::default(),
            )
            .await;
        notifier.notify("cid").await;
        assert!(!bundle.path().join(OOM_FILE_NAME).exists());

        let spec = spec_with_annotations(&[(crio::CONTAINER_TYPE_LABEL_KEY, crio::CONTAINER)]);
        notifier
            .track("cid", bundle.path().to_str().unwrap(), &spec)
            .await;
        notifier.forget("cid").await;
        notifier.notify("cid").await;
        assert!(!bundle.path().join(OOM_FILE_NAME).exists());
    }
}
