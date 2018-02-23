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
	"reflect"
	"syscall"
	"testing"

	vc "github.com/containers/virtcontainers"
	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

const (
	testPodID       = "testPodID"
	testContainerID = "testContainerID"
)

var loggerTriggered = 0

func TestVCImplementations(t *testing.T) {
	// official implementation
	mainImpl := &vc.VCImpl{}

	// test implementation
	testImpl := &VCMock{}

	var interfaceType vc.VC

	// check that the official implementation implements the
	// interface
	mainImplType := reflect.TypeOf(mainImpl)
	mainImplementsIF := mainImplType.Implements(reflect.TypeOf(&interfaceType).Elem())
	assert.True(t, mainImplementsIF)

	// check that the test implementation implements the
	// interface
	testImplType := reflect.TypeOf(testImpl)
	testImplementsIF := testImplType.Implements(reflect.TypeOf(&interfaceType).Elem())
	assert.True(t, testImplementsIF)
}

func TestVCPodImplementations(t *testing.T) {
	// official implementation
	mainImpl := &vc.Pod{}

	// test implementation
	testImpl := &Pod{}

	var interfaceType vc.VCPod

	// check that the official implementation implements the
	// interface
	mainImplType := reflect.TypeOf(mainImpl)
	mainImplementsIF := mainImplType.Implements(reflect.TypeOf(&interfaceType).Elem())
	assert.True(t, mainImplementsIF)

	// check that the test implementation implements the
	// interface
	testImplType := reflect.TypeOf(testImpl)
	testImplementsIF := testImplType.Implements(reflect.TypeOf(&interfaceType).Elem())
	assert.True(t, testImplementsIF)
}

func TestVCContainerImplementations(t *testing.T) {
	// official implementation
	mainImpl := &vc.Container{}

	// test implementation
	testImpl := &Container{}

	var interfaceType vc.VCContainer

	// check that the official implementation implements the
	// interface
	mainImplType := reflect.TypeOf(mainImpl)
	mainImplementsIF := mainImplType.Implements(reflect.TypeOf(&interfaceType).Elem())
	assert.True(t, mainImplementsIF)

	// check that the test implementation implements the
	// interface
	testImplType := reflect.TypeOf(testImpl)
	testImplementsIF := testImplType.Implements(reflect.TypeOf(&interfaceType).Elem())
	assert.True(t, testImplementsIF)
}

func TestVCMockSetLogger(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.SetLoggerFunc)

	logger := logrus.New()

	assert.Equal(loggerTriggered, 0)
	m.SetLogger(logger)
	assert.Equal(loggerTriggered, 0)

	m.SetLoggerFunc = func(logger logrus.FieldLogger) {
		loggerTriggered = 1
	}

	m.SetLogger(logger)
	assert.Equal(loggerTriggered, 1)
}

func TestVCMockCreatePod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.CreatePodFunc)

	_, err := m.CreatePod(vc.PodConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return &Pod{}, nil
	}

	pod, err := m.CreatePod(vc.PodConfig{})
	assert.NoError(err)
	assert.Equal(pod, &Pod{})

	// reset
	m.CreatePodFunc = nil

	_, err = m.CreatePod(vc.PodConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockDeletePod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.DeletePodFunc)

	_, err := m.DeletePod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.DeletePodFunc = func(podID string) (vc.VCPod, error) {
		return &Pod{}, nil
	}

	pod, err := m.DeletePod(testPodID)
	assert.NoError(err)
	assert.Equal(pod, &Pod{})

	// reset
	m.DeletePodFunc = nil

	_, err = m.DeletePod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockListPod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.ListPodFunc)

	_, err := m.ListPod()
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}

	pods, err := m.ListPod()
	assert.NoError(err)
	assert.Equal(pods, []vc.PodStatus{})

	// reset
	m.ListPodFunc = nil

	_, err = m.ListPod()
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockPausePod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.PausePodFunc)

	_, err := m.PausePod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.PausePodFunc = func(podID string) (vc.VCPod, error) {
		return &Pod{}, nil
	}

	pod, err := m.PausePod(testPodID)
	assert.NoError(err)
	assert.Equal(pod, &Pod{})

	// reset
	m.PausePodFunc = nil

	_, err = m.PausePod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockResumePod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.ResumePodFunc)

	_, err := m.ResumePod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ResumePodFunc = func(podID string) (vc.VCPod, error) {
		return &Pod{}, nil
	}

	pod, err := m.ResumePod(testPodID)
	assert.NoError(err)
	assert.Equal(pod, &Pod{})

	// reset
	m.ResumePodFunc = nil

	_, err = m.ResumePod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockRunPod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.RunPodFunc)

	_, err := m.RunPod(vc.PodConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))

	m.RunPodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return &Pod{}, nil
	}

	pod, err := m.RunPod(vc.PodConfig{})
	assert.NoError(err)
	assert.Equal(pod, &Pod{})

	// reset
	m.RunPodFunc = nil

	_, err = m.RunPod(vc.PodConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStartPod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StartPodFunc)

	_, err := m.StartPod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StartPodFunc = func(podID string) (vc.VCPod, error) {
		return &Pod{}, nil
	}

	pod, err := m.StartPod(testPodID)
	assert.NoError(err)
	assert.Equal(pod, &Pod{})

	// reset
	m.StartPodFunc = nil

	_, err = m.StartPod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStatusPod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StatusPodFunc)

	_, err := m.StatusPod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StatusPodFunc = func(podID string) (vc.PodStatus, error) {
		return vc.PodStatus{}, nil
	}

	pod, err := m.StatusPod(testPodID)
	assert.NoError(err)
	assert.Equal(pod, vc.PodStatus{})

	// reset
	m.StatusPodFunc = nil

	_, err = m.StatusPod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStopPod(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StopPodFunc)

	_, err := m.StopPod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StopPodFunc = func(podID string) (vc.VCPod, error) {
		return &Pod{}, nil
	}

	pod, err := m.StopPod(testPodID)
	assert.NoError(err)
	assert.Equal(pod, &Pod{})

	// reset
	m.StopPodFunc = nil

	_, err = m.StopPod(testPodID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockCreateContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.CreateContainerFunc)

	config := vc.ContainerConfig{}
	_, _, err := m.CreateContainer(testPodID, config)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CreateContainerFunc = func(podID string, containerConfig vc.ContainerConfig) (vc.VCPod, vc.VCContainer, error) {
		return &Pod{}, &Container{}, nil
	}

	pod, container, err := m.CreateContainer(testPodID, config)
	assert.NoError(err)
	assert.Equal(pod, &Pod{})
	assert.Equal(container, &Container{})

	// reset
	m.CreateContainerFunc = nil

	_, _, err = m.CreateContainer(testPodID, config)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockDeleteContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.DeleteContainerFunc)

	_, err := m.DeleteContainer(testPodID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.DeleteContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.DeleteContainer(testPodID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.DeleteContainerFunc = nil

	_, err = m.DeleteContainer(testPodID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockEnterContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.EnterContainerFunc)

	cmd := vc.Cmd{}
	_, _, _, err := m.EnterContainer(testPodID, testContainerID, cmd)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.EnterContainerFunc = func(podID, containerID string, cmd vc.Cmd) (vc.VCPod, vc.VCContainer, *vc.Process, error) {
		return &Pod{}, &Container{}, &vc.Process{}, nil
	}

	pod, container, process, err := m.EnterContainer(testPodID, testContainerID, cmd)
	assert.NoError(err)
	assert.Equal(pod, &Pod{})
	assert.Equal(container, &Container{})
	assert.Equal(process, &vc.Process{})

	// reset
	m.EnterContainerFunc = nil

	_, _, _, err = m.EnterContainer(testPodID, testContainerID, cmd)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockKillContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.KillContainerFunc)

	sig := syscall.SIGTERM

	for _, all := range []bool{true, false} {
		err := m.KillContainer(testPodID, testContainerID, sig, all)
		assert.Error(err)
		assert.True(IsMockError(err))
	}

	m.KillContainerFunc = func(podID, containerID string, signal syscall.Signal, all bool) error {
		return nil
	}

	for _, all := range []bool{true, false} {
		err := m.KillContainer(testPodID, testContainerID, sig, all)
		assert.NoError(err)
	}

	// reset
	m.KillContainerFunc = nil

	for _, all := range []bool{true, false} {
		err := m.KillContainer(testPodID, testContainerID, sig, all)
		assert.Error(err)
		assert.True(IsMockError(err))
	}
}

func TestVCMockStartContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StartContainerFunc)

	_, err := m.StartContainer(testPodID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StartContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.StartContainer(testPodID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.StartContainerFunc = nil

	_, err = m.StartContainer(testPodID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStatusContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StatusContainerFunc)

	_, err := m.StatusContainer(testPodID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StatusContainerFunc = func(podID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{}, nil
	}

	status, err := m.StatusContainer(testPodID, testContainerID)
	assert.NoError(err)
	assert.Equal(status, vc.ContainerStatus{})

	// reset
	m.StatusContainerFunc = nil

	_, err = m.StatusContainer(testPodID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStopContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StopContainerFunc)

	_, err := m.StopContainer(testPodID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StopContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.StopContainer(testPodID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.StopContainerFunc = nil

	_, err = m.StopContainer(testPodID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockProcessListContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.ProcessListContainerFunc)

	options := vc.ProcessListOptions{
		Format: "json",
		Args:   []string{"-ef"},
	}

	_, err := m.ProcessListContainer(testPodID, testContainerID, options)
	assert.Error(err)
	assert.True(IsMockError(err))

	processList := vc.ProcessList("hi")

	m.ProcessListContainerFunc = func(podID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error) {
		return processList, nil
	}

	pList, err := m.ProcessListContainer(testPodID, testContainerID, options)
	assert.NoError(err)
	assert.Equal(pList, processList)

	// reset
	m.ProcessListContainerFunc = nil

	_, err = m.ProcessListContainer(testPodID, testContainerID, options)
	assert.Error(err)
	assert.True(IsMockError(err))
}
