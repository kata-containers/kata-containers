// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

// ContainerType defines a type of container.
type ContainerType string

// List different types of containers
const (
	// PodContainer identifies a container that should be associated with an existing pod
	PodContainer ContainerType = "pod_container"
	// PodSandbox identifies an infra container that will be used to create the pod
	PodSandbox ContainerType = "pod_sandbox"
	// SingleContainer is utilized to describe a container that didn't have a container/sandbox
	// annotation applied. This is expected when dealing with non-pod container (ie, running
	// from ctr, podman, etc).
	SingleContainer ContainerType = "single_container"
	// UnknownContainerType specifies a container that provides container type annotation, but
	// it is unknown.
	UnknownContainerType ContainerType = "unknown_container_type"
)

// IsSandbox determines if the container type can be considered as a sandbox.
// We can consider a sandbox in case we have a PodSandbox or a "regular" container
func (cType ContainerType) IsSandbox() bool {
	return cType == PodSandbox || cType == SingleContainer
}

func (t ContainerType) IsCriSandbox() bool {
	return t == PodSandbox
}

// "Regular" Container
func (t ContainerType) IsSingleContainer() bool {
	return t == SingleContainer
}
