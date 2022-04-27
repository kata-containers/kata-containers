// Copyright Red Hat.
//
// SPDX-License-Identifier: Apache-2.0
//

package fs

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestMockAutoInit(t *testing.T) {
	assert := assert.New(t)
	orgMockRootPath := mockRootPath
	defer func() {
		mockRootPath = orgMockRootPath
	}()

	mockRootPath = ""

	fsd, err := MockAutoInit()
	assert.Nil(fsd)
	assert.NoError(err)

	// Testing mock driver
	mockRootPath = t.TempDir()
	fsd, err = MockAutoInit()
	assert.NoError(err)
	expectedFS, err := MockFSInit(MockStorageRootPath())
	assert.NoError(err)
	assert.Equal(expectedFS, fsd)
}
