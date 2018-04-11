// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package virtcontainers

// ContainerType defines a type of container.
type ContainerType string

// List different types of containers
var (
	PodContainer         ContainerType = "pod_container"
	PodSandbox           ContainerType = "pod_sandbox"
	UnknownContainerType ContainerType = "unknown_container_type"
)

// IsSandbox determines if the container type can be considered as a sandbox.
// We can consider a sandbox in case we have a PodSandbox or a RegularContainer.
func (cType ContainerType) IsSandbox() bool {
	if cType == PodSandbox {
		return true
	}

	return false
}
