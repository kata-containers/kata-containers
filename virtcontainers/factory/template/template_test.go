// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package template

import (
	"io/ioutil"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/runtime/virtcontainers"
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
		HypervisorConfig: hyperConfig,
	}

	// New
	f := New(vmConfig)

	// Config
	assert.Equal(f.Config(), vmConfig)

	// GetBaseVM
	_, err := f.GetBaseVM()
	assert.Nil(err)

	// Fetch
	tt := template{
		statePath: testDir,
		config:    vmConfig,
	}

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

	err = tt.createTemplateVM()
	assert.Nil(err)

	_, err = tt.GetBaseVM()
	assert.Nil(err)

	// CloseFactory
	f.CloseFactory()
	tt.CloseFactory()
}
