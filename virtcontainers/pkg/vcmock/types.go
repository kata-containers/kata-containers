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

package vcmock

import (
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
)

// Pod is a fake Pod type used for testing
type Pod struct {
	MockID          string
	MockURL         string
	MockAnnotations map[string]string
	MockContainers  []*Container
}

// Container is a fake Container type used for testing
type Container struct {
	MockID          string
	MockURL         string
	MockToken       string
	MockProcess     vc.Process
	MockPid         int
	MockPod         *Pod
	MockAnnotations map[string]string
}

// VCMock is a type that provides an implementation of the VC interface.
// It is used for testing.
type VCMock struct {
	SetLoggerFunc func(logger logrus.FieldLogger)

	CreatePodFunc func(podConfig vc.PodConfig) (vc.VCPod, error)
	DeletePodFunc func(podID string) (vc.VCPod, error)
	ListPodFunc   func() ([]vc.PodStatus, error)
	PausePodFunc  func(podID string) (vc.VCPod, error)
	ResumePodFunc func(podID string) (vc.VCPod, error)
	RunPodFunc    func(podConfig vc.PodConfig) (vc.VCPod, error)
	StartPodFunc  func(podID string) (vc.VCPod, error)
	StatusPodFunc func(podID string) (vc.PodStatus, error)
	StopPodFunc   func(podID string) (vc.VCPod, error)

	CreateContainerFunc      func(podID string, containerConfig vc.ContainerConfig) (vc.VCPod, vc.VCContainer, error)
	DeleteContainerFunc      func(podID, containerID string) (vc.VCContainer, error)
	EnterContainerFunc       func(podID, containerID string, cmd vc.Cmd) (vc.VCPod, vc.VCContainer, *vc.Process, error)
	KillContainerFunc        func(podID, containerID string, signal syscall.Signal, all bool) error
	StartContainerFunc       func(podID, containerID string) (vc.VCContainer, error)
	StatusContainerFunc      func(podID, containerID string) (vc.ContainerStatus, error)
	StopContainerFunc        func(podID, containerID string) (vc.VCContainer, error)
	ProcessListContainerFunc func(podID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error)
}
