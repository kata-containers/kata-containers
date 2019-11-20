// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package cache

import (
	"context"
	"io/ioutil"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/factory/direct"
	"github.com/kata-containers/runtime/virtcontainers/store"
)

func TestTemplateFactory(t *testing.T) {
	assert := assert.New(t)

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")
	hyperConfig := vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	vmConfig := vc.VMConfig{
		HypervisorType:   vc.MockHypervisor,
		AgentType:        vc.NoopAgentType,
		ProxyType:        vc.NoopProxyType,
		HypervisorConfig: hyperConfig,
	}

	ctx := context.Background()

	ConfigStoragePathSaved := store.ConfigStoragePath
	RunStoragePathSaved := store.RunStoragePath
	// allow the tests to run without affecting the host system.
	store.ConfigStoragePath = func() string { return filepath.Join(testDir, store.StoragePathSuffix, "config") }
	store.RunStoragePath = func() string { return filepath.Join(testDir, store.StoragePathSuffix, "run") }
	defer func() {
		store.ConfigStoragePath = ConfigStoragePathSaved
		store.RunStoragePath = RunStoragePathSaved
	}()

	// New
	f := New(ctx, 2, direct.New(ctx, vmConfig))

	// Config
	assert.Equal(f.Config(), vmConfig)

	// GetBaseVM
	vm, err := f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = vm.Stop()
	assert.Nil(err)

	// CloseFactory
	f.CloseFactory(ctx)
}
