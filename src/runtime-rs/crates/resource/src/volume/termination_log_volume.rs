// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use oci_spec::runtime as oci;

/// A container path, and the same annotation the agent resolves the file
/// through when it reads the message back out.
const TERMINATION_MESSAGE_PATH: &str = "io.kubernetes.container.terminationMessagePath";

/// Kubelet hands the container an empty file for its exit message, which
/// without filesystem sharing is unreachable from the guest. Nothing has to be
/// transferred, so the mount passes through untouched and the agent creates
/// the file during create_container instead.
///
/// Matched on the annotation rather than a literal /dev/termination-log, since
/// terminationMessagePath is the pod's to choose and the agent resolves it the
/// same way. Not gated on terminationMessagePolicy, because the mount is there
/// either way and skipping it would only send the file back to copy_file.
pub(crate) fn is_termination_log_mount(m: &oci::Mount, spec: &oci::Spec) -> bool {
    let Some(annotations) = spec.annotations().as_ref() else {
        return false;
    };

    let Some(path) = annotations.get(TERMINATION_MESSAGE_PATH) else {
        return false;
    };

    !path.is_empty() && m.destination() == Path::new(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn spec_with(annotations: Option<HashMap<String, String>>) -> oci::Spec {
        let mut spec = oci::Spec::default();
        spec.set_annotations(annotations);
        spec
    }

    fn spec_with_path(path: &str) -> oci::Spec {
        let mut a = HashMap::new();
        a.insert(TERMINATION_MESSAGE_PATH.to_string(), path.to_string());
        spec_with(Some(a))
    }

    fn mount_at(destination: &str) -> oci::Mount {
        let mut m = oci::Mount::default();
        m.set_destination(Path::new(destination).to_path_buf());
        m
    }

    #[test]
    fn matches_the_annotated_path() {
        let spec = spec_with_path("/dev/termination-log");

        assert!(is_termination_log_mount(
            &mount_at("/dev/termination-log"),
            &spec
        ));
        assert!(!is_termination_log_mount(&mount_at("/etc/hosts"), &spec));
    }

    #[test]
    fn follows_a_pod_chosen_path() {
        let spec = spec_with_path("/var/log/goodbye");

        assert!(is_termination_log_mount(
            &mount_at("/var/log/goodbye"),
            &spec
        ));
        // The default is only a default, so it must not be assumed.
        assert!(!is_termination_log_mount(
            &mount_at("/dev/termination-log"),
            &spec
        ));
    }

    #[test]
    fn needs_the_annotation() {
        assert!(!is_termination_log_mount(
            &mount_at("/dev/termination-log"),
            &spec_with(None)
        ));
        assert!(!is_termination_log_mount(
            &mount_at("/dev/termination-log"),
            &spec_with(Some(HashMap::new()))
        ));
        assert!(!is_termination_log_mount(
            &mount_at("/dev/termination-log"),
            &spec_with_path("")
        ));
    }
}
