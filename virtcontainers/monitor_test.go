// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestMonitorSuccess(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)

	// create a sandbox
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	m := newMonitor(s)

	ch, err := m.newWatcher()
	assert.Nil(t, err, "newWatcher failed: %v", err)

	fakeErr := errors.New("foobar error")
	m.notify(fakeErr)
	resultErr := <-ch
	assert.True(t, resultErr == fakeErr, "monitor notification mismatch %v vs. %v", resultErr, fakeErr)

	m.stop()
}

func TestMonitorClosedChannel(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)

	// create a sandbox
	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NoopAgentType, NoopNetworkModel, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer cleanUp()

	m := newMonitor(s)

	ch, err := m.newWatcher()
	assert.Nil(t, err, "newWatcher failed: %v", err)

	close(ch)
	fakeErr := errors.New("foobar error")
	m.notify(fakeErr)

	m.stop()
}
