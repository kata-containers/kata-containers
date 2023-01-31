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

	templateWaitForAgent = 1 * time.Microsecond

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

	// New
	f, err := New(ctx, vmConfig, testDir)
	assert.Nil(err)

	// Config
	assert.Equal(f.Config(), vmConfig)

	// GetBaseVM
	vm, err := f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
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

	err = vm.Stop(ctx)
	assert.Nil(err)

	vm, err = f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	err = tt.createTemplateVM(ctx)
	assert.Nil(err)

	vm, err = tt.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	vm, err = f.GetBaseVM(ctx, vmConfig)
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
