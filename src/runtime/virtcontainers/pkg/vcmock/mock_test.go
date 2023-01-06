// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package vcmock

import (
	"context"
	"reflect"
	"testing"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory"
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
	_, err := m.CreateSandbox(ctx, vc.SandboxConfig{}, nil)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig, hookFunc func(context.Context) error) (vc.VCSandbox, error) {
		return &Sandbox{}, nil
	}

	sandbox, err := m.CreateSandbox(ctx, vc.SandboxConfig{}, nil)
	assert.NoError(err)
	assert.Equal(sandbox, &Sandbox{})

	// reset
	m.CreateSandboxFunc = nil

	_, err = m.CreateSandbox(ctx, vc.SandboxConfig{}, nil)
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

func TestVCMockCleanupContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.CleanupContainerFunc)

	ctx := context.Background()
	err := m.CleanupContainer(ctx, testSandboxID, testContainerID, false)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CleanupContainerFunc = func(ctx context.Context, sandboxID, containerID string, force bool) error {
		return nil
	}

	err = m.CleanupContainer(ctx, testSandboxID, testContainerID, false)
	assert.NoError(err)

	// reset
	m.CleanupContainerFunc = nil

	err = m.CleanupContainer(ctx, testSandboxID, testContainerID, false)
	assert.Error(err)
	assert.True(IsMockError(err))
}

func TestVCMockForceCleanupContainer(t *testing.T) {
	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.CleanupContainerFunc)

	ctx := context.Background()
	err := m.CleanupContainer(ctx, testSandboxID, testContainerID, true)
	assert.Error(err)
	assert.True(IsMockError(err))

	m.CleanupContainerFunc = func(ctx context.Context, sandboxID, containerID string, force bool) error {
		return nil
	}

	err = m.CleanupContainer(ctx, testSandboxID, testContainerID, true)
	assert.NoError(err)

	// reset
	m.CleanupContainerFunc = nil

	err = m.CleanupContainer(ctx, testSandboxID, testContainerID, true)
	assert.Error(err)
	assert.True(IsMockError(err))
}
