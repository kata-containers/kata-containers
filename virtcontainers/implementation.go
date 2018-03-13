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

// Description: The true virtcontainers function of the same name.
// This indirection is required to allow an alternative implemenation to be
// used for testing purposes.

package virtcontainers

import (
	"syscall"

	"github.com/sirupsen/logrus"
)

// VCImpl is the official virtcontainers function of the same name.
type VCImpl struct {
}

// SetLogger implements the VC function of the same name.
func (impl *VCImpl) SetLogger(logger logrus.FieldLogger) {
	SetLogger(logger)
}

// CreatePod implements the VC function of the same name.
func (impl *VCImpl) CreatePod(podConfig PodConfig) (VCPod, error) {
	return CreatePod(podConfig)
}

// DeletePod implements the VC function of the same name.
func (impl *VCImpl) DeletePod(podID string) (VCPod, error) {
	return DeletePod(podID)
}

// StartPod implements the VC function of the same name.
func (impl *VCImpl) StartPod(podID string) (VCPod, error) {
	return StartPod(podID)
}

// StopPod implements the VC function of the same name.
func (impl *VCImpl) StopPod(podID string) (VCPod, error) {
	return StopPod(podID)
}

// RunPod implements the VC function of the same name.
func (impl *VCImpl) RunPod(podConfig PodConfig) (VCPod, error) {
	return RunPod(podConfig)
}

// ListPod implements the VC function of the same name.
func (impl *VCImpl) ListPod() ([]PodStatus, error) {
	return ListPod()
}

// StatusPod implements the VC function of the same name.
func (impl *VCImpl) StatusPod(podID string) (PodStatus, error) {
	return StatusPod(podID)
}

// PausePod implements the VC function of the same name.
func (impl *VCImpl) PausePod(podID string) (VCPod, error) {
	return PausePod(podID)
}

// ResumePod implements the VC function of the same name.
func (impl *VCImpl) ResumePod(podID string) (VCPod, error) {
	return ResumePod(podID)
}

// CreateContainer implements the VC function of the same name.
func (impl *VCImpl) CreateContainer(podID string, containerConfig ContainerConfig) (VCPod, VCContainer, error) {
	return CreateContainer(podID, containerConfig)
}

// DeleteContainer implements the VC function of the same name.
func (impl *VCImpl) DeleteContainer(podID, containerID string) (VCContainer, error) {
	return DeleteContainer(podID, containerID)
}

// StartContainer implements the VC function of the same name.
func (impl *VCImpl) StartContainer(podID, containerID string) (VCContainer, error) {
	return StartContainer(podID, containerID)
}

// StopContainer implements the VC function of the same name.
func (impl *VCImpl) StopContainer(podID, containerID string) (VCContainer, error) {
	return StopContainer(podID, containerID)
}

// EnterContainer implements the VC function of the same name.
func (impl *VCImpl) EnterContainer(podID, containerID string, cmd Cmd) (VCPod, VCContainer, *Process, error) {
	return EnterContainer(podID, containerID, cmd)
}

// StatusContainer implements the VC function of the same name.
func (impl *VCImpl) StatusContainer(podID, containerID string) (ContainerStatus, error) {
	return StatusContainer(podID, containerID)
}

// KillContainer implements the VC function of the same name.
func (impl *VCImpl) KillContainer(podID, containerID string, signal syscall.Signal, all bool) error {
	return KillContainer(podID, containerID, signal, all)
}

// ProcessListContainer implements the VC function of the same name.
func (impl *VCImpl) ProcessListContainer(podID, containerID string, options ProcessListOptions) (ProcessList, error) {
	return ProcessListContainer(podID, containerID, options)
}
