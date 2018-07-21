// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package direct

import (
	"io/ioutil"
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

	// CloseFactory
	f.CloseFactory()
}
