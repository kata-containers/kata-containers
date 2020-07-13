// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package template

import (
	"context"
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
)

const testDisabledAsNonRoot = "Test disabled as requires root privileges"

func TestTemplateFactory(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	templateWaitForAgent = 1 * time.Microsecond

	testDir := fs.MockStorageRootPath()
	defer fs.MockStorageDestroy()

	hyperConfig := vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	vmConfig := vc.VMConfig{
		HypervisorType:   vc.MockHypervisor,
		HypervisorConfig: hyperConfig,
	}

	err := vmConfig.Valid()
	assert.Nil(err)

	ctx := context.Background()

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(fmt.Sprintf("mock://%s", vc.MockHybridVSockPath))
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	// New
	f, err := New(ctx, vmConfig, testDir)
	assert.Nil(err)

	// Config
	assert.Equal(f.Config(), vmConfig)

	// GetBaseVM
	vm, err := f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop()
	assert.Nil(err)

	// Fetch
	tt := template{
		statePath: testDir,
		config:    vmConfig,
	}

	assert.Equal(tt.Config(), vmConfig)

	err = tt.checkTemplateVM()
	assert.Error(err)

	_, err = os.Create(tt.statePath + "/memory")
	assert.Nil(err)
	err = tt.checkTemplateVM()
	assert.Error(err)

	_, err = os.Create(tt.statePath + "/state")
	assert.Nil(err)
	err = tt.checkTemplateVM()
	assert.Nil(err)

	err = tt.createTemplateVM(ctx)
	assert.Nil(err)

	vm, err = tt.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop()
	assert.Nil(err)

	vm, err = f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop()
	assert.Nil(err)

	err = tt.createTemplateVM(ctx)
	assert.Nil(err)

	vm, err = tt.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop()
	assert.Nil(err)

	vm, err = f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop()
	assert.Nil(err)

	// CloseFactory
	f.CloseFactory(ctx)
	tt.CloseFactory(ctx)
}
