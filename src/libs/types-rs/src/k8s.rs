// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use crate::annotations;
use crate::container::ContainerType;

// K8S_EMPTY_DIR is the k8s specific path for `empty-dir` volumes
const K8S_EMPTY_DIR: &str = "kubernetes.io~empty-dir";

/// Check whether the path is a K8S empty directory.
///
/// For a K8S EmptyDir, Kubernetes mounts
/// "/var/lib/kubelet/pods/<id>/volumes/kubernetes.io~empty-dir/<volumeMount name>"
/// to "/<mount-point>".
pub fn is_k8s_empty_dir<P: AsRef<Path>>(path: P) -> bool {
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
    for k in [
        annotations::crio::CONTAINER_TYPE_LABEL_KEY,
        annotations::cri_containerd::CONTAINER_TYPE_LABEL_KEY,
        annotations::dockershim::CONTAINER_TYPE_LABLE_KEY,
    ]
    .iter()
    {
        if let Some(v) = spec.annotations.get(k.to_owned()) {
            match v.as_str() {
                // Containerd & CRI-O
                "sandbox" => return ContainerType::PodSandbox,
                // dockershim
                "podsandbox" => return ContainerType::PodSandbox,
                // Containerd & CRI-O & dockershim
                "container" => return ContainerType::PodContainer,
                _ => {}
            }
        }
    }

    ContainerType::PodSandbox
}

/// Determine the k8s sandbox ID from OCI annotations.
///
/// This function is expected to be called only when the container type is "PodContainer".
pub fn sandbox_id(spec: &oci::Spec) -> Option<String> {
    for k in [
        annotations::crio::SANDBOX_ID_LABEL_KEY,
        annotations::cri_containerd::SANDBOX_ID_LABEL_KEY,
        annotations::dockershim::SANDBOX_ID_LABLE_KEY,
    ]
    .iter()
    {
        if let Some(id) = spec.annotations.get(k.to_owned()) {
            return Some(id.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_empty_dir() {
        let empty_dir = "/volumes/kubernetes.io~empty-dir/shm";
        assert!(is_k8s_empty_dir(empty_dir));

        let empty_dir = "/volumes/kubernetes.io~empty-dir//shm";
        assert!(is_k8s_empty_dir(empty_dir));

        let empty_dir = "/volumes/kubernetes.io~empty-dir-test/shm";
        assert!(!is_k8s_empty_dir(empty_dir));

        let empty_dir = "/volumes/kubernetes.io~empty-dir";
        assert!(!is_k8s_empty_dir(empty_dir));

        let empty_dir = "kubernetes.io~empty-dir";
        assert!(!is_k8s_empty_dir(empty_dir));

        let empty_dir = "/kubernetes.io~empty-dir/shm";
        assert!(is_k8s_empty_dir(empty_dir));
    }
}
