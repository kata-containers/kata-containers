// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use oci::Spec;
use std::path::Path;

// Prepend a kata specific string to oci cgroup path to
// form a different cgroup path, thus cAdvisor couldn't
// find kata containers cgroup path on host to prevent it
// from grabbing the stats data.
const CGROUP_KATA_PREFIX: &str = "kata";

// DEFAULT_RESOURCE_CONTROLLER_ID runtime-determined location in the cgroups hierarchy.
const DEFAULT_RESOURCE_CONTROLLER_ID: &str = "vc";

// When the Kata overhead threads (I/O, VMM, etc) are not
// placed in the sandbox resource controller (A cgroup on Linux),
// they are moved to a specific, unconstrained resource controller.
// On Linux, assuming the cgroup mount point is at /sys/fs/cgroup/,
// on a cgroup v1 system, the Kata overhead memory cgroup will be at
// /sys/fs/cgroup/memory/kata_overhead/$CGPATH where $CGPATH is
// defined by the orchestrator.
pub(crate) fn gen_overhead_path(path: &str) -> String {
    format!("kata_overhead/{}", path.trim_start_matches('/'))
}

// add_kata_prefix_for_cgroup_path takes a cgroup path as a parameter and returns a modified cgroup path
// with the Kata prefix added. If the input path is empty or "/", it returns the default
// resource controller ID.
pub(crate) fn add_kata_prefix_for_cgroup_path(path: &str) -> String {
    let cgroup_path = Path::new(if path.is_empty() || path == "/" {
        DEFAULT_RESOURCE_CONTROLLER_ID
    } else {
        path
    });

    let cgroup_path_name = format!(
        "{}_{}",
        CGROUP_KATA_PREFIX,
        cgroup_path.file_name().unwrap().to_string_lossy(),
    );

    let cgroup_path_dir = cgroup_path
        .parent()
        .unwrap_or(Path::new(""))
        .to_string_lossy();
    if cgroup_path_dir.is_empty() {
        cgroup_path_name
    } else {
        format!("{}/{}", cgroup_path_dir, cgroup_path_name)
    }
}

pub(crate) fn generate_paths(sid: &str, spec: &Spec, threaded_mode: bool) -> (String, String) {
    let path = spec
        .linux
        .clone()
        // The trim of '/' is important, because cgroup_path is a relative path.
        .map(|linux| linux.cgroups_path.trim_start_matches('/').to_string())
        .unwrap_or_default();
    let sandbox_path = add_kata_prefix_for_cgroup_path(&path);
    if threaded_mode {
        (
            format!("{}/sandbox", sandbox_path),
            format!("{}/overhead", sandbox_path),
        )
    } else {
        (
            sandbox_path,
            add_kata_prefix_for_cgroup_path(&gen_overhead_path(sid)),
        )
    }
}

pub(crate) fn determine_controllers(threaded_mode: bool) -> Option<Vec<String>> {
    // In cgroup v2, the sandbox and overhead cgroup in threaded mode are placed under
    // the same cgroup in domain threaded mode.
    // In this way, vCPU threads and VMM processes can be separated into two cgroups.
    // For details, please refer to host cgroups design document
    // https://github.com/kata-containers/kata-containers/blob/main/docs/design/host-cgroups.md.
    if threaded_mode {
        Some(vec![String::from("cpuset"), String::from("cpu")])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci::Spec;

    #[derive(Debug)]
    struct TestData {
        path: String,
        cgroup_path: String,
    }

    #[test]
    fn test_add_kata_prefix_for_cgroup_path() {
        let tests = &[
            TestData {
                path: "".to_string(),
                cgroup_path: format!("{}_{}", CGROUP_KATA_PREFIX, DEFAULT_RESOURCE_CONTROLLER_ID,),
            },
            TestData {
                path: "/".to_string(),
                cgroup_path: format!("{}_{}", CGROUP_KATA_PREFIX, DEFAULT_RESOURCE_CONTROLLER_ID,),
            },
            TestData {
                path: "hello".to_string(),
                cgroup_path: "kata_hello".to_string(),
            },
            TestData {
                path: "default/hello".to_string(),
                cgroup_path: "default/kata_hello".to_string(),
            },
        ];

        for t in tests.iter() {
            let path = add_kata_prefix_for_cgroup_path(&t.path);
            assert_eq!(path, t.cgroup_path);
        }
    }

    #[test]
    fn test_generate_paths_threaded_mode() {
        let sid = "test_sid";
        let spec = Spec {
            // Provide the necessary Linux structure for testing.
            linux: Some(oci::Linux {
                cgroups_path: String::from(
                    "/k8s.io/a0af2a15c35ea1c3f72a7f91c72b9c313b3700439631d475a33cc41bc925db77",
                ),
                ..Default::default()
            }),
            ..Default::default()
        };
        let threaded_mode = true;

        let (sandbox, overhead) = generate_paths(sid, &spec, threaded_mode);

        // Verify the generated paths in threaded mode.
        assert_eq!(
            sandbox,
            "k8s.io/kata_a0af2a15c35ea1c3f72a7f91c72b9c313b3700439631d475a33cc41bc925db77/sandbox"
        );
        assert_eq!(
            overhead,
            "k8s.io/kata_a0af2a15c35ea1c3f72a7f91c72b9c313b3700439631d475a33cc41bc925db77/overhead"
        );
    }

    #[test]
    fn test_generate_paths_non_threaded_mode() {
        let sid = "a0af2a15c35ea1c3f72a7f91c72b9c313b3700439631d475a33cc41bc925db77";
        let spec = Spec {
            // Provide the necessary Linux structure for testing.
            linux: Some(oci::Linux {
                cgroups_path: String::from(
                    "/k8s.io/a0af2a15c35ea1c3f72a7f91c72b9c313b3700439631d475a33cc41bc925db77",
                ),
                ..Default::default()
            }),
            ..Default::default()
        };
        let threaded_mode = false;

        let (sandbox, overhead) = generate_paths(sid, &spec, threaded_mode);

        // Verify the generated paths in non-threaded mode.
        assert_eq!(
            sandbox,
            "k8s.io/kata_a0af2a15c35ea1c3f72a7f91c72b9c313b3700439631d475a33cc41bc925db77"
        );
        assert_eq!(
            overhead,
            "kata_overhead/kata_a0af2a15c35ea1c3f72a7f91c72b9c313b3700439631d475a33cc41bc925db77"
        );
    }
}
