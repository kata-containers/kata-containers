// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	"context"
	"reflect"
	"syscall"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/factory"
	vcTypes "github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/types"
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
	ctx := context.Background()
	m.SetLogger(ctx, logger)
	assert.Equal(loggerTriggered, 0)

	m.SetLoggerFunc = func(ctx context.Context, logger *logrus.Entry) {
		loggerTriggered = 1
	}

	m.SetLogger(ctx, logger)
	assert.Equal(loggerTriggered, 1)
}

func TestVCMockCreateSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.CreateSandboxFunc)

	ctx := context.Background()
	_, err := m.CreateSandbox(ctx, vc.SandboxConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.CreateSandbox(ctx, vc.SandboxConfig{})
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.CreateSandboxFunc = nil

	_, err = m.CreateSandbox(ctx, vc.SandboxConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockDeleteSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.DeleteSandboxFunc)

	ctx := context.Background()
	_, err := m.DeleteSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.DeleteSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.DeleteSandbox(ctx, testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.DeleteSandboxFunc = nil

	_, err = m.DeleteSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockListSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.ListSandboxFunc)

	ctx := context.Background()
	_, err := m.ListSandbox(ctx)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ListSandboxFunc = func(ctx context.Context) ([]vc.SandboxStatus, error) {
		return []vc.SandboxStatus{}, nil
	}

	sandboxes, err := m.ListSandbox(ctx)
	assert.NoError(err)
	assert.Equal(sandboxes, []vc.SandboxStatus{})

	// reset
	m.ListSandboxFunc = nil

	_, err = m.ListSandbox(ctx)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockPauseSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.PauseSandboxFunc)

	ctx := context.Background()
	_, err := m.PauseSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.PauseSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.PauseSandbox(ctx, testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.PauseSandboxFunc = nil

	_, err = m.PauseSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockResumeSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.ResumeSandboxFunc)

	ctx := context.Background()
	_, err := m.ResumeSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ResumeSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.ResumeSandbox(ctx, testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.ResumeSandboxFunc = nil

	_, err = m.ResumeSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockRunSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.RunSandboxFunc)

	ctx := context.Background()
	_, err := m.RunSandbox(ctx, vc.SandboxConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))

	m.RunSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.RunSandbox(ctx, vc.SandboxConfig{})
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.RunSandboxFunc = nil

	_, err = m.RunSandbox(ctx, vc.SandboxConfig{})
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStartSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StartSandboxFunc)

	ctx := context.Background()
	_, err := m.StartSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StartSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.StartSandbox(ctx, testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.StartSandboxFunc = nil

	_, err = m.StartSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStatusSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StatusSandboxFunc)

	ctx := context.Background()
	_, err := m.StatusSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StatusSandboxFunc = func(ctx context.Context, sandboxID string) (vc.SandboxStatus, error) {
		return vc.SandboxStatus{}, nil
	}

	sandbox, err := m.StatusSandbox(ctx, testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, vc.SandboxStatus{})

	// reset
	m.StatusSandboxFunc = nil

	_, err = m.StatusSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStopSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StopSandboxFunc)

	ctx := context.Background()
	_, err := m.StopSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StopSandboxFunc = func(ctx context.Context, sandboxID string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.StopSandbox(ctx, testSandboxID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.StopSandboxFunc = nil

	_, err = m.StopSandbox(ctx, testSandboxID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockCreateContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.CreateContainerFunc)

	ctx := context.Background()
	config := vc.ContainerConfig{}
	_, _, err := m.CreateContainer(ctx, testSandboxID, config)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CreateContainerFunc = func(ctx context.Context, sandboxID string, containerConfig vc.ContainerConfig) (vc.VCSandbox, vc.VCContainer, error) {
		return &Sandbox{}, &Container{}, nil
	}

	sandbox, container, err := m.CreateContainer(ctx, testSandboxID, config)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})
	assert.Equal(container, &Container{})

	// reset
	m.CreateContainerFunc = nil

	_, _, err = m.CreateContainer(ctx, testSandboxID, config)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockDeleteContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.DeleteContainerFunc)

	ctx := context.Background()
	_, err := m.DeleteContainer(ctx, testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.DeleteContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.DeleteContainer(ctx, testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.DeleteContainerFunc = nil

	_, err = m.DeleteContainer(ctx, testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockEnterContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.EnterContainerFunc)

	ctx := context.Background()
	cmd := types.Cmd{}
	_, _, _, err := m.EnterContainer(ctx, testSandboxID, testContainerID, cmd)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.EnterContainerFunc = func(ctx context.Context, sandboxID, containerID string, cmd types.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error) {
		return &Sandbox{}, &Container{}, &vc.Process{}, nil
	}

	sandbox, container, process, err := m.EnterContainer(ctx, testSandboxID, testContainerID, cmd)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})
	assert.Equal(container, &Container{})
	assert.Equal(process, &vc.Process{})

	// reset
	m.EnterContainerFunc = nil

	_, _, _, err = m.EnterContainer(ctx, testSandboxID, testContainerID, cmd)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockKillContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.KillContainerFunc)

	ctx := context.Background()
	sig := syscall.SIGTERM

	for _, all := range []bool{true, false} {
		err := m.KillContainer(ctx, testSandboxID, testContainerID, sig, all)
		assert.Error(err)
		assert.True(IsMockError(err))
	}

	m.KillContainerFunc = func(ctx context.Context, sandboxID, containerID string, signal syscall.Signal, all bool) error {
		return nil
	}

	for _, all := range []bool{true, false} {
		err := m.KillContainer(ctx, testSandboxID, testContainerID, sig, all)
		assert.NoError(err)
	}

	// reset
	m.KillContainerFunc = nil

	for _, all := range []bool{true, false} {
		err := m.KillContainer(ctx, testSandboxID, testContainerID, sig, all)
		assert.Error(err)
		assert.True(IsMockError(err))
	}
}

func TestVCMockStartContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StartContainerFunc)

	ctx := context.Background()
	_, err := m.StartContainer(ctx, testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StartContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.StartContainer(ctx, testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.StartContainerFunc = nil

	_, err = m.StartContainer(ctx, testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStatusContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StatusContainerFunc)

	ctx := context.Background()
	_, err := m.StatusContainer(ctx, testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return vc.ContainerStatus{}, nil
	}

	status, err := m.StatusContainer(ctx, testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(status, vc.ContainerStatus{})

	// reset
	m.StatusContainerFunc = nil

	_, err = m.StatusContainer(ctx, testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStatsContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StatsContainerFunc)

	ctx := context.Background()
	_, err := m.StatsContainer(ctx, testSandboxID, testContainerID)

	assert.Error(err)
	assert.True(IsMockError(err))

	m.StatsContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStats, error) {
		return vc.ContainerStats{}, nil
	}

	stats, err := m.StatsContainer(ctx, testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(stats, vc.ContainerStats{})

	// reset
	m.StatsContainerFunc = nil

	_, err = m.StatsContainer(ctx, testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockStopContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.StopContainerFunc)

	ctx := context.Background()
	_, err := m.StopContainer(ctx, testSandboxID, testContainerID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.StopContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.VCContainer, error) {
		return &Container{}, nil
	}

	container, err := m.StopContainer(ctx, testSandboxID, testContainerID)
	assert.NoError(err)
	assert.Equal(container, &Container{})

	// reset
	m.StopContainerFunc = nil

	_, err = m.StopContainer(ctx, testSandboxID, testContainerID)
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

	ctx := context.Background()
	_, err := m.ProcessListContainer(ctx, testSandboxID, testContainerID, options)
	assert.Error(err)
	assert.True(IsMockError(err))

	processList := vc.ProcessList("hi")

	m.ProcessListContainerFunc = func(ctx context.Context, sandboxID, containerID string, options vc.ProcessListOptions) (vc.ProcessList, error) {
		return processList, nil
	}

	pList, err := m.ProcessListContainer(ctx, testSandboxID, testContainerID, options)
	assert.NoError(err)
	assert.Equal(pList, processList)

	// reset
	m.ProcessListContainerFunc = nil

	_, err = m.ProcessListContainer(ctx, testSandboxID, testContainerID, options)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockFetchSandbox(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.FetchSandboxFunc)

	ctx := context.Background()
	_, err := m.FetchSandbox(ctx, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.FetchSandboxFunc = func(ctx context.Context, id string) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.FetchSandbox(ctx, config.ID)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.FetchSandboxFunc = nil

	_, err = m.FetchSandbox(ctx, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

}

func TestVCMockPauseContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.PauseContainerFunc)

	ctx := context.Background()
	err := m.PauseContainer(ctx, config.ID, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.PauseContainerFunc = func(ctx context.Context, sid, cid string) error {
		return nil
	}

	err = m.PauseContainer(ctx, config.ID, config.ID)
	assert.NoError(err)

	// reset
	m.PauseContainerFunc = nil

	err = m.PauseContainer(ctx, config.ID, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockResumeContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.ResumeContainerFunc)

	ctx := context.Background()
	err := m.ResumeContainer(ctx, config.ID, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ResumeContainerFunc = func(ctx context.Context, sid, cid string) error {
		return nil
	}

	err = m.ResumeContainer(ctx, config.ID, config.ID)
	assert.NoError(err)

	// reset
	m.ResumeContainerFunc = nil

	err = m.ResumeContainer(ctx, config.ID, config.ID)
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

	ctx := context.Background()
	f, err := factory.NewFactory(ctx, factory.Config{VMConfig: vmConfig}, false)
	assert.Nil(err)

	assert.Equal(factoryTriggered, 0)
	m.SetFactory(ctx, f)
	assert.Equal(factoryTriggered, 0)

	m.SetFactoryFunc = func(ctx context.Context, factory vc.Factory) {
		factoryTriggered = 1
	}

	m.SetFactory(ctx, f)
	assert.Equal(factoryTriggered, 1)
}

func TestVCMockAddInterface(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.AddInterfaceFunc)

	ctx := context.Background()
	_, err := m.AddInterface(ctx, config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.AddInterfaceFunc = func(ctx context.Context, sid string, inf *vcTypes.Interface) (*vcTypes.Interface, error) {
		return nil, nil
	}

	_, err = m.AddInterface(ctx, config.ID, nil)
	assert.NoError(err)

	// reset
	m.AddInterfaceFunc = nil

	_, err = m.AddInterface(ctx, config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockRemoveInterface(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.RemoveInterfaceFunc)

	ctx := context.Background()
	_, err := m.RemoveInterface(ctx, config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.RemoveInterfaceFunc = func(ctx context.Context, sid string, inf *vcTypes.Interface) (*vcTypes.Interface, error) {
		return nil, nil
	}

	_, err = m.RemoveInterface(ctx, config.ID, nil)
	assert.NoError(err)

	// reset
	m.RemoveInterfaceFunc = nil

	_, err = m.RemoveInterface(ctx, config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockListInterfaces(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.ListInterfacesFunc)

	ctx := context.Background()
	_, err := m.ListInterfaces(ctx, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ListInterfacesFunc = func(ctx context.Context, sid string) ([]*vcTypes.Interface, error) {
		return nil, nil
	}

	_, err = m.ListInterfaces(ctx, config.ID)
	assert.NoError(err)

	// reset
	m.ListInterfacesFunc = nil

	_, err = m.ListInterfaces(ctx, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockUpdateRoutes(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.UpdateRoutesFunc)

	ctx := context.Background()
	_, err := m.UpdateRoutes(ctx, config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.UpdateRoutesFunc = func(ctx context.Context, sid string, routes []*vcTypes.Route) ([]*vcTypes.Route, error) {
		return nil, nil
	}

	_, err = m.UpdateRoutes(ctx, config.ID, nil)
	assert.NoError(err)

	// reset
	m.UpdateRoutesFunc = nil

	_, err = m.UpdateRoutes(ctx, config.ID, nil)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockListRoutes(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	config := &vc.SandboxConfig{}
	assert.Nil(m.ListRoutesFunc)

	ctx := context.Background()
	_, err := m.ListRoutes(ctx, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.ListRoutesFunc = func(ctx context.Context, sid string) ([]*vcTypes.Route, error) {
		return nil, nil
	}

	_, err = m.ListRoutes(ctx, config.ID)
	assert.NoError(err)

	// reset
	m.ListRoutesFunc = nil

	_, err = m.ListRoutes(ctx, config.ID)
	assert.Error(err)
	assert.True(IsMockError(err))
}
