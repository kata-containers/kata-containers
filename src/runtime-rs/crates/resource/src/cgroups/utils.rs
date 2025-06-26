// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use oci_spec::runtime::Spec;

const SANDBOXED_CGROUP_PATH: &str = "kata_sandboxed_pod";

/// Returns cgroup paths for sandbox and overhead, even though we don't
/// need the overhead.
///
/// For cgroup v1
/// - sandbox: "/sys/fs/cgroup/{subsystem}/{base}" (relative "{base}")
/// - overhead: "/sys/fs/cgroup/{subsystem}/kata_overhead/{sid}" (relative
///   "kata_overhead/{sid}")
///
/// For cgroup v2
/// - sandbox: "/sys/fs/cgroup/{base}/sandbox" (relative "{base}/sandbox")
/// - overhead: "/sys/fs/cgroup/{base}/overhead" (relative
///   "{base}/overhead")
///
/// # Returns
///  `(String, String)`: The first one is the sandbox cgroup path
///  (relative), and the second one is the overhead cgroup path (relative).
pub(crate) fn new_cgroup_paths(sid: &str, spec: Option<&Spec>, v2: bool) -> (String, String) {
    let base = if let Some(spec) = spec {
        spec.linux()
            .clone()
            .and_then(|linux| linux.cgroups_path().clone())
            .map(|path| {
                // The trim of '/' is important, because cgroup_path is a relative path.
                path.display()
                    .to_string()
                    .trim_start_matches('/')
                    .to_string()
            })
            .unwrap_or_default()
    } else {
        format!("{}/{}", SANDBOXED_CGROUP_PATH, sid)
    };

    if v2 {
        (format!("{}/sandbox", base), format!("{}/overhead", base))
    } else {
        (base, format!("kata_overhead/{}", sid))
    }
}
