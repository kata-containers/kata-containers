//
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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

func testNewAgentConfig(t *testing.T, config PodConfig, expected interface{}) {
	agentConfig := newAgentConfig(config)
	if reflect.DeepEqual(agentConfig, expected) == false {
		t.Fatal()
	}
}

func TestNewAgentConfigFromNoopAgentType(t *testing.T) {
	var agentConfig interface{}

	podConfig := PodConfig{
		AgentType:   NoopAgentType,
		AgentConfig: agentConfig,
	}

	testNewAgentConfig(t, podConfig, agentConfig)
}

func TestNewAgentConfigFromHyperstartAgentType(t *testing.T) {
	agentConfig := HyperConfig{}

	podConfig := PodConfig{
		AgentType:   HyperstartAgent,
		AgentConfig: agentConfig,
	}

	testNewAgentConfig(t, podConfig, agentConfig)
}

func TestNewAgentConfigFromKataAgentType(t *testing.T) {
	agentConfig := KataAgentConfig{}

	podConfig := PodConfig{
		AgentType:   KataContainersAgent,
		AgentConfig: agentConfig,
	}

	testNewAgentConfig(t, podConfig, agentConfig)
}

func TestNewAgentConfigFromUnknownAgentType(t *testing.T) {
	var agentConfig interface{}

	testNewAgentConfig(t, PodConfig{}, agentConfig)
}
