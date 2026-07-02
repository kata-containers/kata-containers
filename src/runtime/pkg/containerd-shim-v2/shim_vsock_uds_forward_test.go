// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGuestCIDFromAgentURL(t *testing.T) {
	cid, err := guestCIDFromAgentURL("vsock://3:1024")
	assert.NoError(t, err)
	assert.Equal(t, uint32(3), cid)

	cid, err = guestCIDFromAgentURL("vsock://16187:1024")
	assert.NoError(t, err)
	assert.Equal(t, uint32(16187), cid)

	_, err = guestCIDFromAgentURL("hvsock:///tmp/kata.hvsock:1024")
	assert.Error(t, err)
}
