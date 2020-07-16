// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func TestFCGenerateSocket(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}
	i, err := fc.generateSocket("a")
	assert.NoError(err)
	assert.NotNil(i)

	hvsock, ok := i.(types.HybridVSock)
	assert.True(ok)
	assert.NotEmpty(hvsock.UdsPath)
	assert.NotZero(hvsock.Port)
}

func TestFCTruncateID(t *testing.T) {
	assert := assert.New(t)

	fc := firecracker{}

	testLongID := "3ef98eb7c6416be11e0accfed2f4e6560e07f8e33fa8d31922fd4d61747d7ead"
	expectedID := "3ef98eb7c6416be11e0accfed2f4e656"
	id := fc.truncateID(testLongID)
	assert.Equal(expectedID, id)

	testShortID := "3ef98eb7c6416be11"
	expectedID = "3ef98eb7c6416be11"
	id = fc.truncateID(testShortID)
	assert.Equal(expectedID, id)
}

func TestRevertBytes(t *testing.T) {
	assert := assert.New(t)

	//10MB
	testNum := uint64(10000000)
	expectedNum := uint64(10485760)

	num := revertBytes(testNum)
	assert.Equal(expectedNum, num)
}
