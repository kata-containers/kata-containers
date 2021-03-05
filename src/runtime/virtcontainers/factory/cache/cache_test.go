// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package cache

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory/direct"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
)

func TestTemplateFactory(t *testing.T) {
	assert := assert.New(t)

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

	ctx := vc.WithNewAgentFunc(context.Background(), vc.NewMockAgent)

	// New
	f := New(ctx, 2, direct.New(ctx, vmConfig))

	// Config
	assert.Equal(f.Config(), vmConfig)

	// GetBaseVM
	vm, err := f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop(ctx)
	assert.Nil(err)

	// CloseFactory
	f.CloseFactory(ctx)
}
