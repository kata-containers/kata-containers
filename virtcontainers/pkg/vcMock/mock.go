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

// Description: A mock implementation of virtcontainers that can be used
// for testing.
//
// This implementation calls the function set in the object that
// corresponds to the name of the method (for example, when CreatePod()
// is called, that method will try to call CreatePodFunc). If no
// function is defined for the method, it will return an error in a
// well-known format. Callers can detect this scenario by calling
// IsMockError().

package vcMock

import (
	"fmt"
	"syscall"

	vc "github.com/containers/virtcontainers"
	"github.com/sirupsen/logrus"
)

// mockErrorPrefix is a string that all errors returned by the mock
// implementation itself will contain as a prefix.
const mockErrorPrefix = "vcMock forced failure"

// SetLogger implements the VC function of the same name.
func (m *VCMock) SetLogger(logger logrus.FieldLogger) {
	if m.SetLoggerFunc != nil {
		m.SetLoggerFunc(logger)
	}
}

// CreatePod implements the VC function of the same name.
func (m *VCMock) CreatePod(podConfig vc.PodConfig) (vc.VCPod, error) {
	if m.CreatePodFunc != nil {
		return m.CreatePodFunc(podConfig)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podConfig: %v", mockErrorPrefix, getSelf(), m, podConfig)
}

// DeletePod implements the VC function of the same name.
func (m *VCMock) DeletePod(podID string) (vc.VCPod, error) {
	if m.DeletePodFunc != nil {
		return m.DeletePodFunc(podID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v", mockErrorPrefix, getSelf(), m, podID)
}

// StartPod implements the VC function of the same name.
func (m *VCMock) StartPod(podID string) (vc.VCPod, error) {
	if m.StartPodFunc != nil {
		return m.StartPodFunc(podID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v", mockErrorPrefix, getSelf(), m, podID)
}

// StopPod implements the VC function of the same name.
func (m *VCMock) StopPod(podID string) (vc.VCPod, error) {
	if m.StopPodFunc != nil {
		return m.StopPodFunc(podID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v", mockErrorPrefix, getSelf(), m, podID)
}

// RunPod implements the VC function of the same name.
func (m *VCMock) RunPod(podConfig vc.PodConfig) (vc.VCPod, error) {
	if m.RunPodFunc != nil {
		return m.RunPodFunc(podConfig)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podConfig: %v", mockErrorPrefix, getSelf(), m, podConfig)
}

// ListPod implements the VC function of the same name.
func (m *VCMock) ListPod() ([]vc.PodStatus, error) {
	if m.ListPodFunc != nil {
		return m.ListPodFunc()
	}

	return nil, fmt.Errorf("%s: %s", mockErrorPrefix, getSelf())
}

// StatusPod implements the VC function of the same name.
func (m *VCMock) StatusPod(podID string) (vc.PodStatus, error) {
	if m.StatusPodFunc != nil {
		return m.StatusPodFunc(podID)
	}

	return vc.PodStatus{}, fmt.Errorf("%s: %s (%+v): podID: %v", mockErrorPrefix, getSelf(), m, podID)
}

// PausePod implements the VC function of the same name.
func (m *VCMock) PausePod(podID string) (vc.VCPod, error) {
	if m.PausePodFunc != nil {
		return m.PausePodFunc(podID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v", mockErrorPrefix, getSelf(), m, podID)
}

// ResumePod implements the VC function of the same name.
func (m *VCMock) ResumePod(podID string) (vc.VCPod, error) {
	if m.ResumePodFunc != nil {
		return m.ResumePodFunc(podID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v", mockErrorPrefix, getSelf(), m, podID)
}

// CreateContainer implements the VC function of the same name.
func (m *VCMock) CreateContainer(podID string, containerConfig vc.ContainerConfig) (vc.VCPod, vc.VCContainer, error) {
	if m.CreateContainerFunc != nil {
		return m.CreateContainerFunc(podID, containerConfig)
	}

	return nil, nil, fmt.Errorf("%s: %s (%+v): podID: %v, containerConfig: %v", mockErrorPrefix, getSelf(), m, podID, containerConfig)
}

// DeleteContainer implements the VC function of the same name.
func (m *VCMock) DeleteContainer(podID, containerID string) (vc.VCContainer, error) {
	if m.DeleteContainerFunc != nil {
		return m.DeleteContainerFunc(podID, containerID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, podID, containerID)
}

// StartContainer implements the VC function of the same name.
func (m *VCMock) StartContainer(podID, containerID string) (vc.VCContainer, error) {
	if m.StartContainerFunc != nil {
		return m.StartContainerFunc(podID, containerID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, podID, containerID)
}

// StopContainer implements the VC function of the same name.
func (m *VCMock) StopContainer(podID, containerID string) (vc.VCContainer, error) {
	if m.StopContainerFunc != nil {
		return m.StopContainerFunc(podID, containerID)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, podID, containerID)
}

// EnterContainer implements the VC function of the same name.
func (m *VCMock) EnterContainer(podID, containerID string, cmd vc.Cmd) (vc.VCPod, vc.VCContainer, *vc.Process, error) {
	if m.EnterContainerFunc != nil {
		return m.EnterContainerFunc(podID, containerID, cmd)
	}

	return nil, nil, nil, fmt.Errorf("%s: %s (%+v): podID: %v, containerID: %v, cmd: %v", mockErrorPrefix, getSelf(), m, podID, containerID, cmd)
}

// StatusContainer implements the VC function of the same name.
func (m *VCMock) StatusContainer(podID, containerID string) (vc.ContainerStatus, error) {
	if m.StatusContainerFunc != nil {
		return m.StatusContainerFunc(podID, containerID)
	}

	return vc.ContainerStatus{}, fmt.Errorf("%s: %s (%+v): podID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, podID, containerID)
}

// KillContainer implements the VC function of the same name.
func (m *VCMock) KillContainer(podID, containerID string, signal syscall.Signal, all bool) error {
	if m.KillContainerFunc != nil {
		return m.KillContainerFunc(podID, containerID, signal, all)
	}

	return fmt.Errorf("%s: %s (%+v): podID: %v, containerID: %v, signal: %v, all: %v", mockErrorPrefix, getSelf(), m, podID, containerID, signal, all)
}

// ProcessListContainer implements the VC function of the same name.
func (m *VCMock) ProcessListContainer(podID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error) {
	if m.ProcessListContainerFunc != nil {
		return m.ProcessListContainerFunc(podID, containerID, options)
	}

	return nil, fmt.Errorf("%s: %s (%+v): podID: %v, containerID: %v", mockErrorPrefix, getSelf(), m, podID, containerID)
}
