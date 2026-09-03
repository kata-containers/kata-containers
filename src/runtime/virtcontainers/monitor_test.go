// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
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
	assert.True(resultErr == fakeErr, "monitor notification mismatch %v vs. %v", resultErr, fakeErr)

	m.stop()
}

type failingAgent struct {
	kataAgent
	shouldFail     bool
	markDeadCalled bool
}

func (a *failingAgent) check(ctx context.Context) error {
	if a.shouldFail {
		return errors.New("agent unreachable")
	}
	return nil
}

func (a *failingAgent) markDead(ctx context.Context) {
	a.markDeadCalled = true
}

func TestWatchAgentFailureThreshold(t *testing.T) {
	assert := assert.New(t)

	agent := &failingAgent{shouldFail: true}
	s := &Sandbox{}
	s.agent = agent

	m := newMonitor(s)
	m.failureThreshold = 3

	ch, err := m.newWatcher(context.Background())
	assert.Nil(err)

	// Simulate check failures below threshold — no notification expected
	for i := 0; i < 2; i++ {
		m.watchAgent(context.Background())
		assert.Equal(i+1, m.consecutiveFailures)
		select {
		case <-ch:
			t.Fatal("should not notify before threshold")
		default:
		}
	}

	// Third failure hits threshold — notification expected
	m.watchAgent(context.Background())
	assert.Equal(3, m.consecutiveFailures)
	select {
	case err := <-ch:
		assert.Contains(err.Error(), "failed to ping agent")
	default:
		t.Fatal("should have notified at threshold")
	}

	m.stop()
}

func TestWatchAgentRecovery(t *testing.T) {
	assert := assert.New(t)

	agent := &failingAgent{shouldFail: true}
	s := &Sandbox{}
	s.agent = agent

	m := newMonitor(s)
	m.failureThreshold = 5

	_, err := m.newWatcher(context.Background())
	assert.Nil(err)

	// Accumulate some failures
	m.watchAgent(context.Background())
	m.watchAgent(context.Background())
	assert.Equal(2, m.consecutiveFailures)

	// Simulate recovery
	agent.shouldFail = false
	m.watchAgent(context.Background())
	assert.Equal(0, m.consecutiveFailures)

	m.stop()
}

func TestWatchAgentRecoveryResetsThreshold(t *testing.T) {
	assert := assert.New(t)

	agent := &failingAgent{shouldFail: true}
	s := &Sandbox{}
	s.agent = agent

	m := newMonitor(s)
	m.failureThreshold = 3

	ch, err := m.newWatcher(context.Background())
	assert.Nil(err)

	// Fail twice, then recover
	m.watchAgent(context.Background())
	m.watchAgent(context.Background())
	assert.Equal(2, m.consecutiveFailures)

	agent.shouldFail = false
	m.watchAgent(context.Background())
	assert.Equal(0, m.consecutiveFailures)

	// Fail again — should need 3 more failures from zero, not 1
	agent.shouldFail = true
	m.watchAgent(context.Background())
	m.watchAgent(context.Background())
	assert.Equal(2, m.consecutiveFailures)

	select {
	case <-ch:
		t.Fatal("should not notify before threshold after recovery")
	default:
	}

	// Third failure post-recovery hits threshold
	m.watchAgent(context.Background())
	assert.Equal(3, m.consecutiveFailures)

	select {
	case err := <-ch:
		assert.Contains(err.Error(), "failed to ping agent")
	default:
		t.Fatal("should have notified at threshold")
	}

	m.stop()
}

func TestWatchAgentThresholdCallsMarkDead(t *testing.T) {
	assert := assert.New(t)

	agent := &failingAgent{shouldFail: true}
	s := &Sandbox{}
	s.agent = agent

	m := newMonitor(s)
	m.failureThreshold = 1

	_, err := m.newWatcher(context.Background())
	assert.Nil(err)

	m.watchAgent(context.Background())
	assert.True(agent.markDeadCalled, "notify should have called markDead")

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
