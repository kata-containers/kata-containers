// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestIOStream(t *testing.T) {
	hConfig := newHypervisorConfig(nil, nil)
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, []ContainerConfig{}, nil)
	assert.NoError(t, err)
	defer cleanUp()

	contID := "foo"
	processID := "bar"
	config := newTestContainerConfigNoop(contID)
	c := &Container{
		sandbox: s,
		config:  &config,
	}

	stream := newIOStream(s, c, processID)
	stdin := stream.stdin()
	stdout := stream.stdout()
	stderr := stream.stderr()

	buffer := []byte("randombufferdata")
	_, err = stdin.Write(buffer)
	assert.Nil(t, err, "stdin write failed: %s", err)

	_, err = stdout.Read(buffer)
	assert.Nil(t, err, "stdout read failed: %s", err)

	_, err = stderr.Read(buffer)
	assert.Nil(t, err, "stderr read failed: %s", err)

	err = stdin.Close()
	assert.Nil(t, err, "stream close failed: %s", err)

	_, err = stdin.Write(buffer)
	assert.NotNil(t, err, "stdin write closed should fail")

	_, err = stdout.Read(buffer)
	assert.NotNil(t, err, "stdout read closed should fail")

	_, err = stderr.Read(buffer)
	assert.NotNil(t, err, "stderr read closed should fail")

	err = stdin.Close()
	assert.NotNil(t, err, "stdin close closed should fail")
}
