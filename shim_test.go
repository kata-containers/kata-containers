// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os/signal"
	"sync"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestNewShim(t *testing.T) {
	agent := testSetup(t)
	defer testTearDown(agent)

	id := "foobar"
	pid, err := agent.addContainer(id)
	assert.Nil(t, err, "%s", err)

	shim, err := newShim(mockSockAddr, id, pid)
	assert.Nil(t, err, "%s", err)
	defer shim.agent.Close()

	_, err = newShim(badMockAddr, id, pid)
	assert.NotNil(t, err, "New shim with wrong socket address should fail")
}

func TestShimOps(t *testing.T) {
	agent := testSetup(t)
	defer testTearDown(agent)

	id := "foobar"
	pid, err := agent.addContainer(id)
	assert.Nil(t, err, "%s", err)

	shim, err := newShim(mockSockAddr, id, pid)
	assert.Nil(t, err, "%s", err)
	defer shim.agent.Close()

	shim.resizeTty()

	wg := &sync.WaitGroup{}
	shim.proxyStdio(wg)

	sigc := shim.forwardAllSignals()
	defer signal.Stop(sigc)

	shim.monitorTtySize()

	status, err := shim.wait()
	assert.Nil(t, err, "%s", err)
	assert.Equal(t, status, int32(0), "process fail status %d", status)
}
