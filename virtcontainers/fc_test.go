// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func TestFCGenerateSocket(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	i, err := fc.generateSocket("a", false)
	assert.Error(err)
	assert.Nil(i)

	i, err = fc.generateSocket("a", true)
	assert.NoError(err)
	assert.NotNil(i)

	hvsock, ok := i.(types.HybridVSock)
	assert.True(ok)
	assert.NotEmpty(hvsock.UdsPath)
	assert.NotZero(hvsock.Port)
}
