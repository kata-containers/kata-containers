// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

type monitorTestAgent struct {
	mockAgent
	checkErr error
}

func (a *monitorTestAgent) check(ctx context.Context) error {
	return a.checkErr
}

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
	assert.True(resultErr == fakeErr, "monitor notification mismatch %v vs. %v", resultErr, fakeErr)

	m.stop()
}

func TestMonitorConfig(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)
	assert := assert.New(t)

	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	assert.NoError(err)
	defer cleanUp()

	m := newMonitor(s)
	assert.Equal(defaultCheckInterval, m.checkInterval)
	assert.Equal(uint32(defaultAgentCheckFailureThreshold), m.agentCheckFailureThreshold)

	s.config.MonitorCheckInterval = 7
	s.config.AgentCheckFailureThreshold = 3

	m = newMonitor(s)
	assert.Equal(7*time.Second, m.checkInterval)
	assert.Equal(uint32(3), m.agentCheckFailureThreshold)
}

func TestMonitorAgentCheckFailureThreshold(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)
	assert := assert.New(t)

	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	assert.NoError(err)
	defer cleanUp()

	fakeErr := errors.New("agent check failed")
	s.agent = &monitorTestAgent{checkErr: fakeErr}

	m := newMonitor(s)
	m.agentCheckFailureThreshold = 2
	m.running = true
	ch := make(chan error, watcherChannelSize)
	m.watchers = []chan error{ch}

	m.watchAgent(context.Background())
	assert.Equal(uint32(1), m.agentCheckFailures)
	select {
	case err := <-ch:
		t.Fatalf("unexpected monitor notification: %v", err)
	default:
	}

	m.watchAgent(context.Background())
	assert.Equal(uint32(2), m.agentCheckFailures)
	resultErr := <-ch
	assert.ErrorIs(resultErr, fakeErr)
}

func TestMonitorAgentCheckFailureReset(t *testing.T) {
	contID := "505"
	contConfig := newTestContainerConfigNoop(contID)
	hConfig := newHypervisorConfig(nil, nil)
	assert := assert.New(t)

	s, err := testCreateSandbox(t, testSandboxID, MockHypervisor, hConfig, NetworkConfig{}, []ContainerConfig{contConfig}, nil)
	assert.NoError(err)
	defer cleanUp()

	fakeErr := errors.New("agent check failed")
	testAgent := &monitorTestAgent{checkErr: fakeErr}
	s.agent = testAgent

	m := newMonitor(s)
	m.agentCheckFailureThreshold = 2
	m.running = true
	ch := make(chan error, watcherChannelSize)
	m.watchers = []chan error{ch}

	m.watchAgent(context.Background())
	assert.Equal(uint32(1), m.agentCheckFailures)

	testAgent.checkErr = nil
	m.watchAgent(context.Background())
	assert.Equal(uint32(0), m.agentCheckFailures)

	testAgent.checkErr = fakeErr
	m.watchAgent(context.Background())
	assert.Equal(uint32(1), m.agentCheckFailures)
	select {
	case err := <-ch:
		t.Fatalf("unexpected monitor notification: %v", err)
	default:
	}
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
