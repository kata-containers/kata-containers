// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/stretchr/testify/assert"
)

func TestVMConfigGrpc(t *testing.T) {
	assert := assert.New(t)
	config := VMConfig{
		HypervisorType:   QemuHypervisor,
		HypervisorConfig: newQemuConfig(),
		AgentConfig: KataAgentConfig{
			LongLiveConn:       true,
			Debug:              false,
			Trace:              false,
			EnableDebugConsole: false,
			ContainerPipeSize:  0,
			KernelModules:      []string{}},
	}

	p, err := config.ToGrpc()
	assert.Nil(err)

	config2, err := GrpcToVMConfig(p)
	assert.Nil(err)

	assert.True(utils.DeepCompare(config, *config2))
}
