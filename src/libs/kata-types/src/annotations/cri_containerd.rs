// Copyright (c) 2019 Alibaba Cloud
// Copyright (c) 2019 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(missing_docs)]

pub const CONTAINER_TYPE_LABEL_KEY: &str = "io.kubernetes.cri.container-type";
pub const CONTAINER_NAME_LABEL_KEY: &str = "io.kubernetes.cri.container-name";
pub const SANDBOX: &str = "sandbox";
pub const CONTAINER: &str = "container";

// SandboxID is the sandbox ID annotation
pub const SANDBOX_ID_LABEL_KEY: &str = "io.kubernetes.cri.sandbox-id";
// SandboxName is the name of the sandbox (pod)
pub const SANDBOX_NAME_LABEL_KEY: &str = "io.kubernetes.cri.sandbox-name";
// SandboxNamespace is the name of the namespace of the sandbox (pod)
pub const SANDBOX_NAMESPACE_LABEL_KEY: &str = "io.kubernetes.cri.sandbox-namespace";

// Ref: https://pkg.go.dev/github.com/containerd/containerd@v1.6.7/pkg/cri/annotations
// SandboxCPU annotations are based on the initial CPU configuration for the sandbox. This is calculated as the
// sum of container CPU resources, optionally provided by Kubelet (introduced in 1.23) as part of the PodSandboxConfig
pub const SANDBOX_CPU_QUOTA_KEY: &str = "io.kubernetes.cri.sandbox-cpu-quota";
pub const SANDBOX_CPU_PERIOD_KEY: &str = "io.kubernetes.cri.sandbox-cpu-period";
pub const SANDBOX_CPU_SHARE_KEY: &str = "io.kubernetes.cri.sandbox-cpu-shares";

// SandboxMemory is the initial amount of memory associated with this sandbox. This is calculated as the sum
// of container memory, optionally provided by Kubelet (introduced in 1.23) as part of the PodSandboxConfig
pub const SANDBOX_MEM_KEY: &str = "io.kubernetes.cri.sandbox-memory";
