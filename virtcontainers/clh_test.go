// Copyright (c) 2019 Ericsson Eurolab Deutschland G.m.b.H.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"github.com/stretchr/testify/assert"
	"testing"
)

func TestCloudHypervisorAddVSock(t *testing.T) {
	assert := assert.New(t)
	clh := cloudHypervisor{}

	clh.addVSock(1, "path")
	assert.Equal(clh.vmconfig.Vsock[0].Cid, int64(1))
	assert.Equal(clh.vmconfig.Vsock[0].Sock, "path")

	clh.addVSock(2, "path2")
	assert.Equal(clh.vmconfig.Vsock[1].Cid, int64(2))
	assert.Equal(clh.vmconfig.Vsock[1].Sock, "path2")
}
