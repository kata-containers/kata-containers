// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestPipe(t *testing.T) {
	agent := testSetup(t)
	defer testTearDown(agent)

	containerId := "testContainer"
	pid, err := agent.addContainer(containerId)
	assert.Nil(t, err, "failed to add new container: %s", err)

	inPipe, outPipe, errPipe := shimStdioPipe(agent.ctx, agent.client, containerId, pid)

	buf := []byte("foobar")
	size, err := inPipe.Write(buf[:])
	assert.Nil(t, err, "failed to write stdin pipe: %s", err)
	assert.Equal(t, size, len(buf), "unmatched write stdin pipe len %d:%d", len(buf), size)

	size, err = outPipe.Read(buf)
	assert.Nil(t, err, "failed to read stdout pipe: %s", err)
	assert.Equal(t, size, 0, "unmatched write stdin pipe len %d:%d", 0, size)

	size, err = errPipe.Read(buf)
	assert.Nil(t, err, "failed to read stderr pipe: %s", err)
	assert.Equal(t, size, 0, "unmatched write stdin pipe len %d:%d", 0, size)

	// wrong process
	inPipe, outPipe, errPipe = shimStdioPipe(agent.ctx, agent.client, containerId, pid+100)
	_, err = inPipe.Write(buf[:])
	assert.NotNil(t, err, "Unexpected success writing stdin pipe")

	_, err = outPipe.Read(buf)
	assert.NotNil(t, err, "Unexpected success reading stdout pipe")

	_, err = errPipe.Read(buf)
	assert.NotNil(t, err, "Unexpected success reading stderr pipe")
}
