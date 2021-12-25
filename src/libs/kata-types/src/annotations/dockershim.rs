// Copyright (c) 2019 Alibaba Cloud
// Copyright (c) 2019 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(missing_docs)]

//! Copied from k8s.io/pkg/kubelet/dockershim/docker_service.go, used to identify whether a docker
//! container is a sandbox or a regular container, will be removed after defining those as public
//! fields in dockershim.

///  ContainerTypeLabelKey is the container type (podsandbox or container) annotation.
pub const CONTAINER_TYPE_LABLE_KEY: &str = "io.kubernetes.docker.type";

/// ContainerTypeLabelSandbox represents a sandbox sandbox container.
pub const SANDBOX: &str = "podsandbox";

/// ContainerTypeLabelContainer represents a container running within a sandbox.
pub const CONTAINER: &str = "container";

/// SandboxIDLabelKey is the sandbox ID annotation.
pub const SANDBOX_ID_LABLE_KEY: &str = "io.kubernetes.sandbox.id";
