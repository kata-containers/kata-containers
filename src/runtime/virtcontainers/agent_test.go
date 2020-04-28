// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func testSetAgentType(t *testing.T, value string, expected AgentType) {
	var agentType AgentType
	assert := assert.New(t)

	err := (&agentType).Set(value)
	assert.NoError(err)
	assert.Equal(agentType, expected)
}

func TestSetNoopAgentType(t *testing.T) {
	testSetAgentType(t, "noop", NoopAgentType)
}

func TestSetKataAgentType(t *testing.T) {
	testSetAgentType(t, "kata", KataContainersAgent)
}

func TestSetUnknownAgentType(t *testing.T) {
	var agentType AgentType
	assert := assert.New(t)

	err := (&agentType).Set("unknown")
	assert.Error(err)
	assert.NotEqual(agentType, NoopAgentType)
}

func testStringFromAgentType(t *testing.T, agentType AgentType, expected string) {
	agentTypeStr := (&agentType).String()
	assert.Equal(t, agentTypeStr, expected)
}

func TestStringFromNoopAgentType(t *testing.T) {
	testStringFromAgentType(t, NoopAgentType, "noop")
}

func TestStringFromKataAgentType(t *testing.T) {
	testStringFromAgentType(t, KataContainersAgent, "kata")
}

func TestStringFromUnknownAgentType(t *testing.T) {
	var agentType AgentType
	testStringFromAgentType(t, agentType, "")
}

func testNewAgentFromAgentType(t *testing.T, agentType AgentType, expected agent) {
	ag := newAgent(agentType)
	assert.Exactly(t, ag, expected)
}

func TestNewAgentFromNoopAgentType(t *testing.T) {
	testNewAgentFromAgentType(t, NoopAgentType, &noopAgent{})
}

func TestNewAgentFromKataAgentType(t *testing.T) {
	testNewAgentFromAgentType(t, KataContainersAgent, &kataAgent{})
}

func TestNewAgentFromUnknownAgentType(t *testing.T) {
	var agentType AgentType
	testNewAgentFromAgentType(t, agentType, &noopAgent{})
}

func testNewAgentConfig(t *testing.T, config SandboxConfig, expected interface{}) {
	agentConfig, err := newAgentConfig(config.AgentType, config.AgentConfig)
	assert.NoError(t, err)
	assert.Exactly(t, agentConfig, expected)
}

func TestNewAgentConfigFromNoopAgentType(t *testing.T) {
	var agentConfig interface{}

	sandboxConfig := SandboxConfig{
		AgentType:   NoopAgentType,
		AgentConfig: agentConfig,
	}

	testNewAgentConfig(t, sandboxConfig, agentConfig)
}

func TestNewAgentConfigFromKataAgentType(t *testing.T) {
	agentConfig := KataAgentConfig{UseVSock: true}

	sandboxConfig := SandboxConfig{
		AgentType:   KataContainersAgent,
		AgentConfig: agentConfig,
	}

	testNewAgentConfig(t, sandboxConfig, agentConfig)
}

func TestNewAgentConfigFromUnknownAgentType(t *testing.T) {
	var agentConfig interface{}

	testNewAgentConfig(t, SandboxConfig{}, agentConfig)
}
