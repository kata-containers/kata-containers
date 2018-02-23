//
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
//

package dockershim

const (
	// Copied from k8s.io/pkg/kubelet/dockershim/docker_service.go,
	// used to identify whether a docker container is a sandbox or
	// a regular container, will be removed after defining those as
	// public fields in dockershim.

	// ContainerTypeLabelKey is the container type (podsandbox or container) annotation
	ContainerTypeLabelKey = "io.kubernetes.docker.type"

	// ContainerTypeLabelSandbox represents a pod sandbox container
	ContainerTypeLabelSandbox = "podsandbox"

	// ContainerTypeLabelContainer represents a container running within a pod
	ContainerTypeLabelContainer = "container"

	// SandboxIDLabelKey is the sandbox ID annotation
	SandboxIDLabelKey = "io.kubernetes.sandbox.id"
)
