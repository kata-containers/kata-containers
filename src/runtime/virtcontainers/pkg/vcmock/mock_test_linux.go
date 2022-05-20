package vcmock

import (
	"context"
	"github.com/stretchr/testify/assert"
	"testing"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/factory"
)

func TestVCMockSetVMFactory(t *testing.T) {
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
	f, err := factory.NewFactory(ctx, factory.Config{VMConfig: vmConfig}, false)
	assert.Nil(err)

	assert.Equal(factoryTriggered, 0)
	m.SetFactory(ctx, f)
	assert.Equal(factoryTriggered, 0)

	m.SetFactoryFunc = func(ctx context.Context, factory vc.Factory) {
		factoryTriggered = 1
	}

	m.SetFactory(ctx, f)
	assert.Equal(factoryTriggered, 1)
}
