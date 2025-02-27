// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use crate::annotations;
use crate::container::ContainerType;
use oci_spec::runtime as oci;
use std::str::FromStr;
// K8S_EMPTY_DIR is the K8s specific path for `empty-dir` volumes
const K8S_EMPTY_DIR: &str = "kubernetes.io~empty-dir";
// K8S_CONFIGMAP is the K8s specific path for `configmap` volumes
const K8S_CONFIGMAP: &str = "kubernetes.io~configmap";
// K8S_SECRET is the K8s specific path for `secret` volumes
const K8S_SECRET: &str = "kubernetes.io~secret";

/// Check whether the path is a K8s empty directory.
pub fn is_empty_dir<P: AsRef<Path>>(path: P) -> bool {
    is_special_dir(path, K8S_EMPTY_DIR)
}

/// Check whether the path is a K8s configmap.
pub fn is_configmap<P: AsRef<Path>>(path: P) -> bool {
    is_special_dir(path, K8S_CONFIGMAP)
}

/// Check whether the path is a K8s secret.
pub fn is_secret<P: AsRef<Path>>(path: P) -> bool {
    is_special_dir(path, K8S_SECRET)
}

/// Check whether the path is a K8s empty directory, configmap, or secret.
///
/// For example, given a K8s EmptyDir, Kubernetes mounts
/// "/var/lib/kubelet/pods/<id>/volumes/kubernetes.io~empty-dir/<volumeMount name>"
/// to "/<mount-point>".
pub fn is_special_dir<P: AsRef<Path>>(path: P, dir_type: &str) -> bool {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        if let Some(pname) = parent.file_name() {
            if pname == dir_type && parent.parent().is_some() {
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
        if let Some(annotations) = spec.annotations() {
            if let Some(v) = annotations.get(k.to_owned()) {
                if let Ok(t) = ContainerType::from_str(v) {
                    return t;
                }
            }
        }
    }

    ContainerType::SingleContainer
}

/// Get K8S container name from OCI annotations.
pub fn container_name(spec: &oci::Spec) -> String {
    for k in [
        annotations::cri_containerd::CONTAINER_NAME_LABEL_KEY,
        annotations::crio::CONTAINER_NAME_LABEL_KEY,
    ]
    .iter()
    {
        if let Some(annotations) = spec.annotations() {
            if let Some(v) = annotations.get(k.to_owned()) {
                return v.clone();
            }
        }
    }

    String::new()
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
            if let Some(annotations) = spec.annotations() {
                if let Some(id) = annotations.get(k.to_owned()) {
                    sid = Some(id.to_string());
                    break;
                }
            }
        }
    }

    (container_type, sid)
}

// count_files will return the number of files within a given path.
// If the total number of
// files observed is greater than limit, break and return -1
fn count_files<P: AsRef<Path>>(path: P, limit: i32) -> std::io::Result<i32> {
    // First, Check to see if the path exists
    let src = std::fs::canonicalize(path)?;

    // Special case if this is just a file, not a directory:
    if !src.is_dir() {
        return Ok(1);
    }

    let mut num_files = 0;

    for entry in std::fs::read_dir(src)? {
        let file = entry?;
        let p = file.path();
        if p.is_dir() {
            let inc = count_files(&p, limit - num_files)?;
            if inc == -1 {
                return Ok(-1);
            }
            num_files += inc;
        } else {
            num_files += 1;
        }

        if num_files > limit {
            return Ok(-1);
        }
    }

    Ok(num_files)
}

/// Check if a volume should be processed as a watchable volume,
/// which adds inotify-like function for virtio-fs.
pub fn is_watchable_mount<P: AsRef<Path>>(path: P) -> bool {
    if !is_secret(&path) && !is_configmap(&path) {
        return false;
    }

    // we have a cap on number of FDs which can be present in mount
    // to determine if watchable. A similar Check exists within the agent,
    // which may or may not help handle case where extra files are added to
    // a mount after the fact
    let count = count_files(&path, 8).unwrap_or(0);
    count > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{annotations, container};
    use std::fs;
    use test_utils::skip_if_not_root;

    #[test]
    fn test_count_files() {
        let limit = 8;
        let test_tmp_dir = tempfile::tempdir().expect("failed to create tempdir");
        let work_path = test_tmp_dir.path().join("work");

        let result = fs::create_dir_all(&work_path);
        assert!(result.is_ok());

        let origin_dir = work_path.join("origin_dir");
        let result = fs::create_dir_all(&origin_dir);
        assert!(result.is_ok());
        for n in 0..limit {
            let tmp_file = origin_dir.join(format!("file{}", n));
            let res = fs::File::create(tmp_file);
            assert!(res.is_ok());
        }

        let symlink_origin_dir = work_path.join("symlink_origin_dir");
        let result = std::os::unix::fs::symlink(&origin_dir, &symlink_origin_dir);
        assert!(result.is_ok());
        for n in 0..2 {
            let tmp_file = work_path.join(format!("file{}", n));
            let res = fs::File::create(tmp_file);
            assert!(res.is_ok());
        }

        let count = count_files(&work_path, limit).unwrap_or(0);
        assert_eq!(count, -1);

        let count = count_files(&origin_dir, limit).unwrap_or(0);
        assert_eq!(count, limit);
    }

    #[test]
    fn test_is_watchable_mount() {
        skip_if_not_root!();

        let result = is_watchable_mount("");
        assert!(!result);

        // path does not exist, failure expected:
        let result = is_watchable_mount("/var/lib/kubelet/pods/5f0861a0-a987-4a3a-bb0f-1058ddb9678f/volumes/kubernetes.io~empty-dir/foobar");
        assert!(!result);

        let test_tmp_dir = tempfile::tempdir().expect("failed to create tempdir");

        // Verify secret is successful (single file mount):
        //   /tmppath/kubernetes.io~secret/super-secret-thing
        let secret_path = test_tmp_dir.path().join(K8S_SECRET);
        let result = fs::create_dir_all(&secret_path);
        assert!(result.is_ok());
        let secret_file = &secret_path.join("super-secret-thing");
        let result = fs::File::create(secret_file);
        assert!(result.is_ok());

        let result = is_watchable_mount(secret_file);
        assert!(result);

        // Verify that if we have too many files, it will no longer be watchable:
        // /tmp/kubernetes.io~configmap/amazing-dir-of-configs/
        //                                  | - c0
        //                                  | - c1
        //                                    ...
        //                                  | - c7
        // should be okay.
        //
        // 9 files should cause the mount to be deemed "not watchable"
        let configmap_path = test_tmp_dir
            .path()
            .join(K8S_CONFIGMAP)
            .join("amazing-dir-of-configs");
        let result = fs::create_dir_all(&configmap_path);
        assert!(result.is_ok());

        // not a watchable mount if no files available.
        let result = is_watchable_mount(&configmap_path);
        assert!(!result);

        for i in 0..8 {
            let configmap_file = &configmap_path.join(format!("c{}", i));
            let result = fs::File::create(configmap_file);
            assert!(result.is_ok());

            let result = is_watchable_mount(&configmap_path);
            assert!(result);
        }
        let configmap_file = &configmap_path.join("too_much_files");
        let result = fs::File::create(configmap_file);
        assert!(result.is_ok());

        let result = is_watchable_mount(&configmap_path);
        assert!(!result);
    }

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
    fn test_is_configmap() {
        let path = "/volumes/kubernetes.io~configmap/cm";
        assert!(is_configmap(path));

        let path = "/volumes/kubernetes.io~configmap//cm";
        assert!(is_configmap(path));

        let path = "/volumes/kubernetes.io~configmap-test/cm";
        assert!(!is_configmap(path));

        let path = "/volumes/kubernetes.io~configmap";
        assert!(!is_configmap(path));
    }

    #[test]
    fn test_is_secret() {
        let path = "/volumes/kubernetes.io~secret/test-serect";
        assert!(is_secret(path));

        let path = "/volumes/kubernetes.io~secret//test-serect";
        assert!(is_secret(path));

        let path = "/volumes/kubernetes.io~secret-test/test-serect";
        assert!(!is_secret(path));

        let path = "/volumes/kubernetes.io~secret";
        assert!(!is_secret(path));
    }

    #[test]
    fn test_container_type() {
        let sid = "sid".to_string();
        let mut spec = oci::Spec::default();

        // default
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::SingleContainer, None)
        );

        // crio sandbox
        spec.set_annotations(Some(
            [(
                annotations::crio::CONTAINER_TYPE_LABEL_KEY.to_string(),
                container::SANDBOX.to_string(),
            )]
            .iter()
            .cloned()
            .collect(),
        ));
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodSandbox, None)
        );

        // cri containerd sandbox
        spec.set_annotations(Some(
            [(
                annotations::crio::CONTAINER_TYPE_LABEL_KEY.to_string(),
                container::POD_SANDBOX.to_string(),
            )]
            .iter()
            .cloned()
            .collect(),
        ));
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodSandbox, None)
        );

        // docker shim sandbox
        spec.set_annotations(Some(
            [(
                annotations::crio::CONTAINER_TYPE_LABEL_KEY.to_string(),
                container::PODSANDBOX.to_string(),
            )]
            .iter()
            .cloned()
            .collect(),
        ));
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodSandbox, None)
        );

        // crio container
        spec.set_annotations(Some(
            [
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
            .collect(),
        ));
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodContainer, Some(sid.clone()))
        );

        // cri containerd container
        spec.set_annotations(Some(
            [
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
            .collect(),
        ));
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodContainer, Some(sid.clone()))
        );

        // docker shim container
        spec.set_annotations(Some(
            [
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
            .collect(),
        ));
        assert_eq!(
            container_type_with_id(&spec),
            (ContainerType::PodContainer, Some(sid))
        );
    }
}
