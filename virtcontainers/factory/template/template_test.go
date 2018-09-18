// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package template

import (
	"context"
	"io/ioutil"
	"os"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

func TestTemplateFactory(t *testing.T) {
	assert := assert.New(t)

	templateWaitForAgent = 1 * time.Microsecond

	testDir, _ := ioutil.TempDir("", "vmfactory-tmp-")
	hyperConfig := vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}
	vmConfig := vc.VMConfig{
		HypervisorType:   vc.MockHypervisor,
		HypervisorConfig: hyperConfig,
		AgentType:        vc.NoopAgentType,
		ProxyType:        vc.NoopProxyType,
	}

	err := vmConfig.Valid()
	assert.Nil(err)

	ctx := context.Background()

	// New
	f := New(ctx, vmConfig)

	// Config
	assert.Equal(f.Config(), vmConfig)

	// GetBaseVM
	_, err = f.GetBaseVM(ctx, vmConfig)
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
	assert.Error(err)

	templateProxyType = vc.NoopProxyType
	_, err = tt.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	_, err = f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	err = tt.createTemplateVM(ctx)
	assert.Nil(err)

	_, err = tt.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	_, err = f.GetBaseVM(ctx, vmConfig)
	assert.Nil(err)

	// CloseFactory
	f.CloseFactory(ctx)
	tt.CloseFactory(ctx)
}
