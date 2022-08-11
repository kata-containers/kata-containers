// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use crate::annotations;
use crate::container::ContainerType;
use std::str::FromStr;

// K8S_EMPTY_DIR is the k8s specific path for `empty-dir` volumes
const K8S_EMPTY_DIR: &str = "kubernetes.io~empty-dir";

/// Check whether the path is a K8S empty directory.
///
/// For a K8S EmptyDir, Kubernetes mounts
/// "/var/lib/kubelet/pods/<id>/volumes/kubernetes.io~empty-dir/<volumeMount name>"
/// to "/<mount-point>".
pub fn is_empty_dir<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        if let Some(pname) = parent.file_name() {
            if pname == K8S_EMPTY_DIR && parent.parent().is_some() {
                return true;
            }
        }
    }

    false
}

/// Get K8S container type from OCI annotations.
pub fn container_type(spec: &oci::Spec) -> ContainerType {
    // PodSandbox:  "sandbox" (Containerd & CRI-O), "podsandbox" (dockershim)
    // PodContainer: "container" (Containerd & CRI-O & dockershim)
    for k in [
        annotations::crio::CONTAINER_TYPE_LABEL_KEY,
        annotations::cri_containerd::CONTAINER_TYPE_LABEL_KEY,
        annotations::dockershim::CONTAINER_TYPE_LABEL_KEY,
    ]
    .iter()
    {
        if let Some(v) = spec.annotations.get(k.to_owned()) {
            if let Ok(t) = ContainerType::from_str(v) {
                return t;
            }
        }
    }

    ContainerType::PodSandbox
}

/// Determine the k8s sandbox ID from OCI annotations.
///
/// This function is expected to be called only when the container type is "PodContainer".
pub fn container_type_with_id(spec: &oci::Spec) -> (ContainerType, Option<String>) {
    let container_type = container_type(spec);
    let mut sid = None;
    if container_type == ContainerType::PodContainer {
        for k in [
            annotations::crio::SANDBOX_ID_LABEL_KEY,
            annotations::cri_containerd::SANDBOX_ID_LABEL_KEY,
            annotations::dockershim::SANDBOX_ID_LABEL_KEY,
        ]
        .iter()
        {
            if let Some(id) = spec.annotations.get(k.to_owned()) {
                sid = Some(id.to_string());
                break;
            }
        }
    }

    (container_type, sid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{annotations, container};

    #[test]
    fn test_is_empty_dir() {
        let empty_dir = "/volumes/kubernetes.io~empty-dir/shm";
        assert!(is_empty_dir(empty_dir));

        let empty_dir = "/volumes/kubernetes.io~empty-dir//shm";
        assert!(is_empty_dir(empty_dir));

        let empty_dir = "/volumes/kubernetes.io~empty-dir-test/shm";
        assert!(!is_empty_dir(empty_dir));

        let empty_dir = "/volumes/kubernetes.io~empty-dir";
        assert!(!is_empty_dir(empty_dir));

        let empty_dir = "kubernetes.io~empty-dir";
        assert!(!is_empty_dir(empty_dir));

        let empty_dir = "/kubernetes.io~empty-dir/shm";
        assert!(is_empty_dir(empty_dir));
    }

    #[test]
    fn test_container_type() {
        let sid = "sid".to_string();
        let mut spec = oci::Spec::default();

        // default
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodSandbox, None)
        );

        // crio sandbox
        spec.annotations = [(
            annotations::crio::CONTAINER_TYPE_LABEL_KEY.to_string(),
            container::SANDBOX.to_string(),
        )]
        .iter()
        .cloned()
        .collect();
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodSandbox, None)
        );

        // cri containerd sandbox
        spec.annotations = [(
            annotations::crio::CONTAINER_TYPE_LABEL_KEY.to_string(),
            container::POD_SANDBOX.to_string(),
        )]
        .iter()
        .cloned()
        .collect();
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodSandbox, None)
        );

        // docker shim sandbox
        spec.annotations = [(
            annotations::crio::CONTAINER_TYPE_LABEL_KEY.to_string(),
            container::PODSANDBOX.to_string(),
        )]
        .iter()
        .cloned()
        .collect();
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodSandbox, None)
        );

        // crio container
        spec.annotations = [
            (
                annotations::crio::CONTAINER_TYPE_LABEL_KEY.to_string(),
                container::CONTAINER.to_string(),
            ),
            (
                annotations::crio::SANDBOX_ID_LABEL_KEY.to_string(),
                sid.clone(),
            ),
        ]
        .iter()
        .cloned()
        .collect();
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodContainer, Some(sid.clone()))
        );

        // cri containerd container
        spec.annotations = [
            (
                annotations::cri_containerd::CONTAINER_TYPE_LABEL_KEY.to_string(),
                container::POD_CONTAINER.to_string(),
            ),
            (
                annotations::cri_containerd::SANDBOX_ID_LABEL_KEY.to_string(),
                sid.clone(),
            ),
        ]
        .iter()
        .cloned()
        .collect();
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodContainer, Some(sid.clone()))
        );

        // docker shim container
        spec.annotations = [
            (
                annotations::dockershim::CONTAINER_TYPE_LABEL_KEY.to_string(),
                container::CONTAINER.to_string(),
            ),
            (
                annotations::dockershim::SANDBOX_ID_LABEL_KEY.to_string(),
                sid.clone(),
            ),
        ]
        .iter()
        .cloned()
        .collect();
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodContainer, Some(sid))
        );
    }
}
