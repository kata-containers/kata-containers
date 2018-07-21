// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"reflect"
	"testing"
)

func testSetAgentType(t *testing.T, value string, expected AgentType) {
	var agentType AgentType

	err := (&agentType).Set(value)
	if err != nil {
		t.Fatal(err)
	}

	if agentType != expected {
		t.Fatal(err)
	}
}

func TestSetNoopAgentType(t *testing.T) {
	testSetAgentType(t, "noop", NoopAgentType)
}

func TestSetHyperstartAgentType(t *testing.T) {
	testSetAgentType(t, "hyperstart", HyperstartAgent)
}

func TestSetKataAgentType(t *testing.T) {
	testSetAgentType(t, "kata", KataContainersAgent)
}

func TestSetUnknownAgentType(t *testing.T) {
	var agentType AgentType

	err := (&agentType).Set("unknown")
	if err == nil {
		t.Fatal()
	}

	if agentType == NoopAgentType ||
		agentType == HyperstartAgent {
		t.Fatal()
	}
}

func testStringFromAgentType(t *testing.T, agentType AgentType, expected string) {
	agentTypeStr := (&agentType).String()
	if agentTypeStr != expected {
		t.Fatal()
	}
}

func TestStringFromNoopAgentType(t *testing.T) {
	testStringFromAgentType(t, NoopAgentType, "noop")
}

func TestStringFromHyperstartAgentType(t *testing.T) {
	testStringFromAgentType(t, HyperstartAgent, "hyperstart")
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

	if reflect.DeepEqual(ag, expected) == false {
		t.Fatal()
	}
}

func TestNewAgentFromNoopAgentType(t *testing.T) {
	testNewAgentFromAgentType(t, NoopAgentType, &noopAgent{})
}

func TestNewAgentFromHyperstartAgentType(t *testing.T) {
	testNewAgentFromAgentType(t, HyperstartAgent, &hyper{})
}

func TestNewAgentFromKataAgentType(t *testing.T) {
	testNewAgentFromAgentType(t, KataContainersAgent, &kataAgent{})
}

func TestNewAgentFromUnknownAgentType(t *testing.T) {
	var agentType AgentType
	testNewAgentFromAgentType(t, agentType, &noopAgent{})
}

func testNewAgentConfig(t *testing.T, config SandboxConfig, expected interface{}) {
	agentConfig := newAgentConfig(config.AgentType, config.AgentConfig)
	if reflect.DeepEqual(agentConfig, expected) == false {
		t.Fatal()
	}
}

func TestNewAgentConfigFromNoopAgentType(t *testing.T) {
	var agentConfig interface{}

	sandboxConfig := SandboxConfig{
		AgentType:   NoopAgentType,
		AgentConfig: agentConfig,
	}

	testNewAgentConfig(t, sandboxConfig, agentConfig)
}

func TestNewAgentConfigFromHyperstartAgentType(t *testing.T) {
	agentConfig := HyperConfig{}

	sandboxConfig := SandboxConfig{
		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,
	}

	testNewAgentConfig(t, sandboxConfig, agentConfig)
}

func TestNewAgentConfigFromKataAgentType(t *testing.T) {
	agentConfig := KataAgentConfig{}

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
