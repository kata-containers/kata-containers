// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Package annotations defines CRI annotation keys used by Kata Containers.
//
// These constants are part of the Kubernetes CRI wire protocol
// (io.kubernetes.cri.*) and were formerly sourced from
// github.com/containerd/containerd/pkg/cri/annotations, which moved to an
// internal package in containerd v2 and is therefore no longer importable from
// outside the containerd module.
package annotations

const (
	// ContainerType is the annotation key used to identify whether a
	// container is a sandbox or a regular container.
	ContainerType = "io.kubernetes.cri.container-type"

	// ContainerTypeSandbox is the value of ContainerType for a pod sandbox.
	ContainerTypeSandbox = "sandbox"

	// ContainerTypeContainer is the value of ContainerType for a container
	// running within a pod.
	ContainerTypeContainer = "container"

	// SandboxID is the annotation key for the sandbox (pod) ID.
	SandboxID = "io.kubernetes.cri.sandbox-id"

	// SandboxName is the annotation key for the sandbox (pod) name.
	SandboxName = "io.kubernetes.cri.sandbox-name"

	// SandboxNamespace is the annotation key for the namespace of the sandbox (pod).
	SandboxNamespace = "io.kubernetes.cri.sandbox-namespace"

	// SandboxCPUPeriod is the annotation key for the CPU CFS period (µs) of the sandbox.
	SandboxCPUPeriod = "io.kubernetes.cri.sandbox-cpu-period"

	// SandboxCPUQuota is the annotation key for the CPU CFS quota (µs) of the sandbox.
	SandboxCPUQuota = "io.kubernetes.cri.sandbox-cpu-quota"

	// SandboxCPUShares is the annotation key for the CPU shares of the sandbox.
	SandboxCPUShares = "io.kubernetes.cri.sandbox-cpu-shares"

	// SandboxMem is the annotation key for the memory limit (bytes) of the sandbox.
	SandboxMem = "io.kubernetes.cri.sandbox-memory"
)
