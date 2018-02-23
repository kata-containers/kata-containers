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

package vcMock

import (
	vc "github.com/containers/virtcontainers"
)

// ID implements the VCPod function of the same name.
func (p *Pod) ID() string {
	return p.MockID
}

// Annotations implements the VCPod function of the same name.
func (p *Pod) Annotations(key string) (string, error) {
	return p.MockAnnotations[key], nil
}

// SetAnnotations implements the VCPod function of the same name.
func (p *Pod) SetAnnotations(annotations map[string]string) error {
	return nil
}

// GetAnnotations implements the VCPod function of the same name.
func (p *Pod) GetAnnotations() map[string]string {
	return p.MockAnnotations
}

// GetAllContainers implements the VCPod function of the same name.
func (p *Pod) GetAllContainers() []vc.VCContainer {
	var ifa = make([]vc.VCContainer, len(p.MockContainers))

	for i, v := range p.MockContainers {
		ifa[i] = v
	}

	return ifa
}

// GetContainer implements the VCPod function of the same name.
func (p *Pod) GetContainer(containerID string) vc.VCContainer {
	for _, c := range p.MockContainers {
		if c.MockID == containerID {
			return c
		}
	}
	return &Container{}
}
