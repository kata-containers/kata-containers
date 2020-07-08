// Copyright (c) 2019 hyper.sh
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"errors"
	"syscall"
	"testing"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/types"
	"github.com/stretchr/testify/assert"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
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

func TestIsGRPCErrorCode(t *testing.T) {
	assert := assert.New(t)

	assert.True(isGRPCErrorCode(codes.Unimplemented, status.New(codes.Unimplemented, "foobar").Err()))
	assert.True(isGRPCErrorCode(codes.NotFound, status.New(codes.NotFound, "foobar").Err()))
	assert.False(isGRPCErrorCode(codes.Unimplemented, errors.New("foobar")))
}
