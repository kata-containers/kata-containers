// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestMonitorSuccess(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)
	assert := assert.New(t)

	// create a sandbox
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	assert.NoError(err)
	defer cleanUp()

	m := newMonitor(s)

	ch, err := m.newWatcher(context.Background())
	assert.Nil(err, "newWatcher failed: %v", err)

	fakeErr := errors.New("foobar error")
	m.notify(context.Background(), fakeErr)
	resultErr := <-ch
	assert.True(strings.Contains(resultErr.Error(), fakeErr.Error()), "monitor notification mismatch %v does not contain %v", resultErr, fakeErr)

	m.stop()
}

func TestMonitorClosedChannel(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)
	assert := assert.New(t)

	// create a sandbox
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	assert.NoError(err)
	defer cleanUp()

	m := newMonitor(s)

	ch, err := m.newWatcher(context.Background())
	assert.Nil(err, "newWatcher failed: %v", err)

	close(ch)
	fakeErr := errors.New("foobar error")
	m.notify(context.Background(), fakeErr)

	m.stop()
}
