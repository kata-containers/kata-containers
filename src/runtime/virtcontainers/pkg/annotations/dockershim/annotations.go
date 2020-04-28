// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package dockershim

const (
	// Copied from k8s.io/pkg/kubelet/dockershim/docker_service.go,
	// used to identify whether a docker container is a sandbox or
	// a regular container, will be removed after defining those as
	// public fields in dockershim.

	// ContainerTypeLabelKey is the container type (podsandbox or container) annotation
	ContainerTypeLabelKey = "io.kubernetes.docker.type"

	// ContainerTypeLabelSandbox represents a sandbox sandbox container
	ContainerTypeLabelSandbox = "podsandbox"

	// ContainerTypeLabelContainer represents a container running within a sandbox
	ContainerTypeLabelContainer = "container"

	// SandboxIDLabelKey is the sandbox ID annotation
	SandboxIDLabelKey = "io.kubernetes.sandbox.id"
)
