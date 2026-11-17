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

	// Closing stdin must not tear down the output streams: a process can
	// still write to stdout/stderr long after its input hit EOF. Killing
	// these readers here truncates `docker exec -i` output.
	_, err = stdout.Read(buffer)
	assert.Nil(t, err, "stdout read after stdin close should still work: %s", err)

	_, err = stderr.Read(buffer)
	assert.Nil(t, err, "stderr read after stdin close should still work: %s", err)

	err = stdin.Close()
	assert.NotNil(t, err, "stdin close closed should fail")
}

// TestIOStreamStdinCloseKeepsOutput does not need root, so it also runs in
// environments where TestIOStream is skipped.
//
// Closing a process' stdin must not disable its stdout/stderr readers: a
// process keeps writing output long after its input hit EOF. Sharing the
// closed state between the three streams made CloseIO truncate `docker exec`
// output as soon as stdin was closed.
func TestIOStreamStdinCloseKeepsOutput(t *testing.T) {
	assert := assert.New(t)

	sandbox := &Sandbox{
		id:     testSandboxID,
		config: &SandboxConfig{},
		agent:  &mockAgent{},
	}
	contID := "foo"
	contConfig := newTestContainerConfigNoop(contID)
	c := &Container{
		id:      contID,
		sandbox: sandbox,
		config:  &contConfig,
	}

	stream := newIOStream(sandbox, c, "bar")
	stdin := stream.stdin()
	stdout := stream.stdout()
	stderr := stream.stderr()

	buffer := []byte("randombufferdata")

	_, err := stdin.Write(buffer)
	assert.NoError(err, "stdin write failed: %v", err)

	_, err = stdout.Read(buffer)
	assert.NoError(err, "stdout read failed: %v", err)

	_, err = stderr.Read(buffer)
	assert.NoError(err, "stderr read failed: %v", err)

	assert.NoError(stdin.Close(), "stdin close failed")

	_, err = stdin.Write(buffer)
	assert.Error(err, "stdin write after close should fail")

	_, err = stdout.Read(buffer)
	assert.NoError(err, "stdout read after stdin close should still work: %v", err)

	_, err = stderr.Read(buffer)
	assert.NoError(err, "stderr read after stdin close should still work: %v", err)

	assert.Error(stdin.Close(), "stdin double close should fail")
}
