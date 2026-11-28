// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package template

import (
	"context"
	"fmt"
	"os"
	"runtime"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/mock"
)

const testDisabledAsNonRoot = "Test disabled as requires root privileges"

func TestTemplateFactory(t *testing.T) {
	// template is broken on arm64, so, temporarily disable it on arm64
	if runtime.GOARCH == "arm64" || os.Geteuid() != 0 {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	testDir := t.TempDir()

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

	url, err := mock.GenerateKataMockHybridVSock()
	assert.NoError(err)
	defer mock.RemoveKataMockHybridVSock(url)
	vc.MockHybridVSockPath = url

	hybridVSockTTRPCMock := mock.HybridVSockTTRPCMock{}
	err = hybridVSockTTRPCMock.Start(url)
	assert.NoError(err)
	defer hybridVSockTTRPCMock.Stop()

	// Create 2 sets of instance-specific directories for per-VM storage
	runStorePath1 := t.TempDir()
	vmStorePath1 := t.TempDir()
	runStorePath2 := t.TempDir()
	vmStorePath2 := t.TempDir()

	// Create a new Template Factory
	f, err := New(ctx, vmConfig, testDir)
	assert.Nil(err)

	// Config
	assert.Equal(f.Config(), vmConfig)

	// GetBaseVM with first instance paths
	vmConfig1 := vmConfig
	vmConfig1.HypervisorConfig.RunStorePath = runStorePath1
	vmConfig1.HypervisorConfig.VMStorePath = vmStorePath1

	// Test the creation of a new VM from the template factory
	vm, err := f.GetBaseVM(ctx, vmConfig1)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	// Fetch
	tt := template{
		statePath: testDir,
		config:    vmConfig,
	}

	assert.Equal(tt.Config(), vmConfig)

	// Checking that template VM check fails
	// if the corresponding memory and state files are absent
	err = tt.checkTemplateVM()
	assert.Error(err)

	memFile, err := os.Create(tt.statePath + "/memory")
	assert.Nil(err)
	memFile.Close()
	err = tt.checkTemplateVM()
	assert.Error(err)

	devFile, err := os.Create(tt.deviceStatePath())
	assert.Nil(err)
	devFile.Close()

	// After creating state and memory files, checkTemplateVM should succeed
	err = tt.checkTemplateVM()
	assert.Nil(err)

	// Recreate the template VM, which should succeed
	err = tt.createTemplateVM(ctx)
	assert.Nil(err)

	// Ensuring that directly calling template's GetBaseVM function
	// returns a VM instance similar to the one returned by the factory's GetBaseVM function
	vm, err = tt.GetBaseVM(ctx, vmConfig1)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	vm, err = f.GetBaseVM(ctx, vmConfig1)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	// Overwriting the template VM should succeed
	err = tt.createTemplateVM(ctx)
	assert.Nil(err)

	// Create second instance with different storage paths
	vmConfig2 := vmConfig
	vmConfig2.HypervisorConfig.RunStorePath = runStorePath2
	vmConfig2.HypervisorConfig.VMStorePath = vmStorePath2

	vm, err = tt.GetBaseVM(ctx, vmConfig2)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	vm, err = f.GetBaseVM(ctx, vmConfig2)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	// make tt.statePath is busy
	os.Chdir(tt.statePath)

	// CloseFactory, there is no need to call tt.CloseFactory(ctx)
	f.CloseFactory(ctx)

	//umount may take more time. Check periodically if the mount exists
	waitTime, delay := 20, 1*time.Second
	for check := waitTime; check > 0; {
		// expect tt.statePath not exist, if exist, it means this case failed.
		_, err = os.Stat(tt.statePath)
		if err != nil {
			break
		}
		check -= 1
		time.Sleep(delay)
	}
	assert.True(os.IsNotExist(err), fmt.Sprintf("mount still present after waiting %d seconds", waitTime))
}
