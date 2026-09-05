// Copyright (c) 2019 Alibaba Cloud
// Copyright (c) 2019 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(missing_docs)]

use serde::Deserialize;

pub const CONTAINER_TYPE_LABEL_KEY: &str = "io.kubernetes.cri-o.ContainerType";
pub const CONTAINER_NAME_LABEL_KEY: &str = "io.kubernetes.cri-o.ContainerName";
pub const SANDBOX: &str = "sandbox";
pub const CONTAINER: &str = "container";

pub const SANDBOX_ID_LABEL_KEY: &str = "io.kubernetes.cri-o.SandboxID";
pub const SANDBOX_NAME_LABEL_KEY: &str = "io.kubernetes.cri-o.KubeName";
pub const SANDBOX_NAMESPACE_LABEL_KEY: &str = "io.kubernetes.cri-o.Namespace";

// The sum of the pod's container resources, JSON-encoded as a CRI
// LinuxContainerResources. This is CRI-O's counterpart to containerd's
// io.kubernetes.cri.sandbox-{cpu-quota,cpu-period,memory} annotations, which
// it does not set.
//
// CRI-O renamed its annotations to the *.crio.io namespace and keeps the
// io.kubernetes.cri-o.* spelling as a deprecated alias, so both are accepted
// with the current name winning.
pub const POD_LINUX_RESOURCES_KEY: &str = "pod-linux-resources.crio.io";
pub const POD_LINUX_RESOURCES_KEY_DEPRECATED: &str = "io.kubernetes.cri-o.PodLinuxResources";

/// The `pod-linux-resources.crio.io` payload.
///
/// Only the fields Kata sizes a sandbox from are named; the rest of
/// LinuxContainerResources (cpuset_cpus, unified, ...) is ignored.
#[derive(Debug, Default, Deserialize)]
pub struct PodLinuxResources {
    #[serde(default)]
    pub cpu_period: u64,
    #[serde(default)]
    pub cpu_quota: i64,
    #[serde(default)]
    pub memory_limit_in_bytes: i64,
}
