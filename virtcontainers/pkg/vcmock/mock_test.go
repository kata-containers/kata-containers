// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	"reflect"
	"syscall"
	"testing"

	"github.com/kata-containers/agent/protocols/grpc"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/factory"
	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

const (
	testSandboxID   = "testSandboxID"
	testContainerID = "testContainerID"
)

var (
	loggerTriggered  = 0
	factoryTriggered = 0
)

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

func TestVCSandboxImplementations(t *testing.T) {
	// official implementation
	mainImpl := &vc.Sandbox{}

	// test implementation
	testImpl := &Sandbox{}

	var interfaceType vc.VCSandbox

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

	logger := logrus.NewEntry(logrus.New())

	assert.Equal(loggerTriggered, 0)
	m.SetLogger(logger)
	assert.Equal(loggerTriggered, 0)

	m.SetLoggerFunc = func(logger logrus.FieldLogger) {
		loggerTriggered = 1
	}

	m.SetLogger(logger)
	assert.Equal(loggerTriggered, 1)
}

func TestVCMockCreateSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.CreateSandboxFunc)

	_, err := m.CreateSandbox(vc.SandboxConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CreateSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.CreateSandbox(vc.SandboxConfig{})
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.CreateSandboxFunc = nil

	_, err = m.CreateSandbox(vc.SandboxConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockDeleteSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.DeleteSandboxFunc)

	_, err := m.DeleteSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.DeleteSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.DeleteSandbox(testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.DeleteSandboxFunc = nil

	_, err = m.DeleteSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockListSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.ListSandboxFunc)

	_, err := m.ListSandbox()
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ListSandboxFunc = func() ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{}, nil
	}

	sandboxes, err := m.ListSandbox()
	assert.NoError(err)
	assert.Equal(sandboxes, []vc.SandboxStatus{})

	// reset
	m.ListSandboxFunc = nil

	_, err = m.ListSandbox()
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockPauseSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.PauseSandboxFunc)

	_, err := m.PauseSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.PauseSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.PauseSandbox(testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.PauseSandboxFunc = nil

	_, err = m.PauseSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockResumeSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.ResumeSandboxFunc)

	_, err := m.ResumeSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ResumeSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.ResumeSandbox(testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.ResumeSandboxFunc = nil

	_, err = m.ResumeSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockRunSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.RunSandboxFunc)

	_, err := m.RunSandbox(vc.SandboxConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))

	m.RunSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.RunSandbox(vc.SandboxConfig{})
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.RunSandboxFunc = nil

	_, err = m.RunSandbox(vc.SandboxConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStartSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StartSandboxFunc)

	_, err := m.StartSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StartSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.StartSandbox(testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.StartSandboxFunc = nil

	_, err = m.StartSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStatusSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StatusSandboxFunc)

	_, err := m.StatusSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StatusSandboxFunc = func(sandboxID string) (vc.SandboxStatus, error) {
		return vc.SandboxStatus{}, nil
	}

	sandbox, err := m.StatusSandbox(testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, vc.SandboxStatus{})

	// reset
	m.StatusSandboxFunc = nil

	_, err = m.StatusSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStopSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StopSandboxFunc)

	_, err := m.StopSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StopSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.StopSandbox(testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.StopSandboxFunc = nil

	_, err = m.StopSandbox(testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockCreateContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.CreateContainerFunc)

	config := vc.ContainerConfig{}
	_, _, err := m.CreateContainer(testSandboxID, config)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CreateContainerFunc = func(sandboxID string, containerConfig vc.ContainerConfig) (vc.VCSandbox, vc.VCContainer, error) {
		return &Sandbox{}, &Container{}, nil
	}

	sandbox, container, err := m.CreateContainer(testSandboxID, config)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})
	assert.Equal(container, &Container{})

	// reset
	m.CreateContainerFunc = nil

	_, _, err = m.CreateContainer(testSandboxID, config)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockDeleteContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.DeleteContainerFunc)

	_, err := m.DeleteContainer(testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.DeleteContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.DeleteContainer(testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.DeleteContainerFunc = nil

	_, err = m.DeleteContainer(testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockEnterContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.EnterContainerFunc)

	cmd := vc.Cmd{}
	_, _, _, err := m.EnterContainer(testSandboxID, testContainerID, cmd)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.EnterContainerFunc = func(sandboxID, containerID string, cmd vc.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error) {
		return &Sandbox{}, &Container{}, &vc.Process{}, nil
	}

	sandbox, container, process, err := m.EnterContainer(testSandboxID, testContainerID, cmd)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})
	assert.Equal(container, &Container{})
	assert.Equal(process, &vc.Process{})

	// reset
	m.EnterContainerFunc = nil

	_, _, _, err = m.EnterContainer(testSandboxID, testContainerID, cmd)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockKillContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.KillContainerFunc)

	sig := syscall.SIGTERM

	for _, all := range []bool{true, false} {
		err := m.KillContainer(testSandboxID, testContainerID, sig, all)
		assert.Error(err)
		assert.True(IsMockError(err))
	}

	m.KillContainerFunc = func(sandboxID, containerID string, signal syscall.Signal, all bool) error {
		return nil
	}

	for _, all := range []bool{true, false} {
		err := m.KillContainer(testSandboxID, testContainerID, sig, all)
		assert.NoError(err)
	}

	// reset
	m.KillContainerFunc = nil

	for _, all := range []bool{true, false} {
		err := m.KillContainer(testSandboxID, testContainerID, sig, all)
		assert.Error(err)
		assert.True(IsMockError(err))
	}
}

func TestVCMockStartContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StartContainerFunc)

	_, err := m.StartContainer(testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StartContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.StartContainer(testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.StartContainerFunc = nil

	_, err = m.StartContainer(testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStatusContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StatusContainerFunc)

	_, err := m.StatusContainer(testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{}, nil
	}

	status, err := m.StatusContainer(testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(status, vc.ContainerStatus{})

	// reset
	m.StatusContainerFunc = nil

	_, err = m.StatusContainer(testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStatsContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StatsContainerFunc)

	_, err := m.StatsContainer(testSandboxID, testContainerID)

	assert.Error(err)
	assert.True(IsMockError(err))

	m.StatsContainerFunc = func(sandboxID, containerID string) (vc.ContainerStats, error) {
		return vc.ContainerStats{}, nil
	}

	stats, err := m.StatsContainer(testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(stats, vc.ContainerStats{})

	// reset
	m.StatsContainerFunc = nil

	_, err = m.StatsContainer(testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStopContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StopContainerFunc)

	_, err := m.StopContainer(testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StopContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.StopContainer(testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.StopContainerFunc = nil

	_, err = m.StopContainer(testSandboxID, testContainerID)
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

	_, err := m.ProcessListContainer(testSandboxID, testContainerID, options)
	assert.Error(err)
	assert.True(IsMockError(err))

	processList := vc.ProcessList("hi")

	m.ProcessListContainerFunc = func(sandboxID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error) {
		return processList, nil
	}

	pList, err := m.ProcessListContainer(testSandboxID, testContainerID, options)
	assert.NoError(err)
	assert.Equal(pList, processList)

	// reset
	m.ProcessListContainerFunc = nil

	_, err = m.ProcessListContainer(testSandboxID, testContainerID, options)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockFetchSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.FetchSandboxFunc)

	_, err := m.FetchSandbox(config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.FetchSandboxFunc = func(id string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.FetchSandbox(config.ID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.FetchSandboxFunc = nil

	_, err = m.FetchSandbox(config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

}

func TestVCMockPauseContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.PauseContainerFunc)

	err := m.PauseContainer(config.ID, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.PauseContainerFunc = func(sid, cid string) error {
		return nil
	}

	err = m.PauseContainer(config.ID, config.ID)
	assert.NoError(err)

	// reset
	m.PauseContainerFunc = nil

	err = m.PauseContainer(config.ID, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockResumeContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.ResumeContainerFunc)

	err := m.ResumeContainer(config.ID, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ResumeContainerFunc = func(sid, cid string) error {
		return nil
	}

	err = m.ResumeContainer(config.ID, config.ID)
	assert.NoError(err)

	// reset
	m.ResumeContainerFunc = nil

	err = m.ResumeContainer(config.ID, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockSetVMFactory(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.SetFactoryFunc)

	hyperConfig := vc.HypervisorConfig{
		KernelPath: "foobar",
		ImagePath:  "foobar",
	}
	vmConfig := vc.VMConfig{
		HypervisorType:   vc.MockHypervisor,
		AgentType:        vc.NoopAgentType,
		HypervisorConfig: hyperConfig,
	}

	f, err := factory.NewFactory(factory.Config{VMConfig: vmConfig}, false)
	assert.Nil(err)

	assert.Equal(factoryTriggered, 0)
	m.SetFactory(f)
	assert.Equal(factoryTriggered, 0)

	m.SetFactoryFunc = func(factory vc.Factory) {
		factoryTriggered = 1
	}

	m.SetFactory(f)
	assert.Equal(factoryTriggered, 1)
}

func TestVCMockAddInterface(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.AddInterfaceFunc)

	_, err := m.AddInterface(config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.AddInterfaceFunc = func(sid string, inf *grpc.Interface) (*grpc.Interface, error) {
		return nil, nil
	}

	_, err = m.AddInterface(config.ID, nil)
	assert.NoError(err)

	// reset
	m.AddInterfaceFunc = nil

	_, err = m.AddInterface(config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockRemoveInterface(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.RemoveInterfaceFunc)

	_, err := m.RemoveInterface(config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.RemoveInterfaceFunc = func(sid string, inf *grpc.Interface) (*grpc.Interface, error) {
		return nil, nil
	}

	_, err = m.RemoveInterface(config.ID, nil)
	assert.NoError(err)

	// reset
	m.RemoveInterfaceFunc = nil

	_, err = m.RemoveInterface(config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockListInterfaces(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.ListInterfacesFunc)

	_, err := m.ListInterfaces(config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ListInterfacesFunc = func(sid string) ([]*grpc.Interface, error) {
		return nil, nil
	}

	_, err = m.ListInterfaces(config.ID)
	assert.NoError(err)

	// reset
	m.ListInterfacesFunc = nil

	_, err = m.ListInterfaces(config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockUpdateRoutes(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.UpdateRoutesFunc)

	_, err := m.UpdateRoutes(config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.UpdateRoutesFunc = func(sid string, routes []*grpc.Route) ([]*grpc.Route, error) {
		return nil, nil
	}

	_, err = m.UpdateRoutes(config.ID, nil)
	assert.NoError(err)

	// reset
	m.UpdateRoutesFunc = nil

	_, err = m.UpdateRoutes(config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockListRoutes(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.ListRoutesFunc)

	_, err := m.ListRoutes(config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ListRoutesFunc = func(sid string) ([]*grpc.Route, error) {
		return nil, nil
	}

	_, err = m.ListRoutes(config.ID)
	assert.NoError(err)

	// reset
	m.ListRoutesFunc = nil

	_, err = m.ListRoutes(config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))
}
