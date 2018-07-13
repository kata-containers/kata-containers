// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package factory

import (
	"io/ioutil"
	"testing"

	"github.com/stretchr/testify/assert"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

func TestNewFactory(t *testing.T) {
	var config Config

	assert := assert.New(t)

	_, err := NewFactory(config, true)
	assert.Error(err)
	_, err = NewFactory(config, false)
	assert.Error(err)

	config.VMConfig = vc.VMConfig{
		HypervisorType: vc.MockHypervisor,
		AgentType:      vc.NoopAgentType,
	}

	_, err = NewFactory(config, false)
	assert.Error(err)

	testDir, err := ioutil.TempDir("", "vmfactory-tmp-")
	assert.Nil(err)

	config.VMConfig.HypervisorConfig = vc.HypervisorConfig{
		KernelPath: testDir,
		ImagePath:  testDir,
	}

	_, err = NewFactory(config, false)
	assert.Nil(err)

	config.Cache = 10
	_, err = NewFactory(config, true)
	assert.Error(err)
}
