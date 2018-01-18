// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"os/signal"
	"sync"
	"testing"

	"github.com/kr/pty"
	"github.com/stretchr/testify/assert"
)

func TestNewShim(t *testing.T) {
	agent := testSetup(t)
	defer testTearDown(agent)

	contID := "foobarContainer"
	execID := "testExec"
	err := agent.addContainer(contID, execID)
	assert.Nil(t, err, "%s", err)

	shim, err := newShim(mockSockAddr, contID, execID)
	assert.Nil(t, err, "%s", err)
	defer shim.agent.Close()

	_, err = newShim(badMockAddr, contID, execID)
	assert.NotNil(t, err, "New shim with wrong socket address should fail")
}

func TestShimOps(t *testing.T) {
	agent := testSetup(t)
	defer testTearDown(agent)

	contID := "foobarContainer"
	execID := "testExec"
	err := agent.addContainer(contID, execID)
	assert.Nil(t, err, "%s", err)

	shim, err := newShim(mockSockAddr, contID, execID)
	assert.Nil(t, err, "%s", err)
	defer shim.agent.Close()

	_, tty, err := pty.Open()
	assert.Nil(t, err, "%s", err)
	defer tty.Close()

	shim.resizeTty(tty)

	wg := &sync.WaitGroup{}
	shim.proxyStdio(wg)

	sigc := shim.forwardAllSignals()
	defer signal.Stop(sigc)

	shim.monitorTtySize(tty)

	status, err := shim.wait()
	assert.Nil(t, err, "%s", err)
	assert.Equal(t, status, int32(0), "process fail status %d", status)
}
