// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package direct

import (
	"context"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/persist/fs"
)

var rootPathSave = fs.StorageRootPath()

func TestTemplateFactory(t *testing.T) {
	assert := assert.New(t)

	testDir, err := ioutil.TempDir("", "vmfactory-tmp-")
	fs.TestSetStorageRootPath(filepath.Join(testDir, "vc"))

	defer func() {
		os.RemoveAll(testDir)
		fs.TestSetStorageRootPath(rootPathSave)
	}()

	assert.Nil(err)

	runPathSave := fs.RunStoragePath()
	fs.TestSetRunStoragePath(filepath.Join(testDir, "vc", "run"))

	defer func() {
		os.RemoveAll(testDir)
		fs.TestSetRunStoragePath(runPathSave)
	}()

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

	// New
	f := New(ctx, vmConfig)

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
