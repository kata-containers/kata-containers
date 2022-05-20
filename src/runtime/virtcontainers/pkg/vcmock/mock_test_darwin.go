package vcmock

import (
	"context"
	"testing"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory"
	"github.com/stretchr/testify/assert"
)

func TestVCMockNewFactory(t *testing.T) {

	assert := assert.New(t)

	m := &VCMock{}
	assert.Nil(m.SetFactoryFunc)

	hyperConfig := vc.HypervisorConfig{
		KernelPath: "foobar",
		ImagePath:  "foobar",
	}
	vmConfig := vc.VMConfig{
		HypervisorType:   vc.MockHypervisor,
		HypervisorConfig: hyperConfig,
	}

	ctx := context.Background()
	_, err := factory.NewFactory(ctx, factory.Config{VMConfig: vmConfig}, false)
	assert.Error(err)

}
