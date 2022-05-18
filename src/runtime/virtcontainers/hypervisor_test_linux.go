package virtcontainers

import (
	"github.com/stretchr/testify/assert"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
)

func TestGenerateVMSocket(t *testing.T) {
	assert := assert.New(t)

	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	s, err := generateVMSocket("a", "")
	assert.NoError(err)
	vsock, ok := s.(types.VSock)
	assert.True(ok)
	defer assert.NoError(vsock.VhostFd.Close())
	assert.NotZero(vsock.VhostFd)
	assert.NotZero(vsock.ContextID)
	assert.NotZero(vsock.Port)
}
