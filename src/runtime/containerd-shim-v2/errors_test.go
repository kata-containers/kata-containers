// Copyright (c) 2019 hyper.sh
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"syscall"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/stretchr/testify/assert"
)

func TestToGRPC(t *testing.T) {
	assert := assert.New(t)

	for _, err := range []error{vc.ErrNeedSandbox, vc.ErrNeedSandboxID,
		vc.ErrNeedContainerID, vc.ErrNeedState, syscall.EINVAL, vc.ErrNoSuchContainer, syscall.ENOENT} {
		assert.False(isGRPCError(err))
		err = toGRPC(err)
		assert.True(isGRPCError(err))
		err = toGRPC(err)
		assert.True(isGRPCError(err))
		err = toGRPCf(err, "appending")
		assert.True(isGRPCError(err))
	}
}
