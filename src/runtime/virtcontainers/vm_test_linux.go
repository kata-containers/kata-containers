package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
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
