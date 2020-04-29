// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

// ContainerType defines a type of container.
type ContainerType string

// List different types of containers
const (
	PodContainer         ContainerType = "pod_container"
	PodSandbox           ContainerType = "pod_sandbox"
	UnknownContainerType ContainerType = "unknown_container_type"
)

// IsSandbox determines if the container type can be considered as a sandbox.
// We can consider a sandbox in case we have a PodSandbox or a RegularContainer.
func (cType ContainerType) IsSandbox() bool {
	return cType == PodSandbox
}
