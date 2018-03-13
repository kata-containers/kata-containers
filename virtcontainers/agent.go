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
	"fmt"
	"syscall"

	"github.com/mitchellh/mapstructure"
)

// AgentType describes the type of guest agent a Pod should run.
type AgentType string

// ProcessListOptions contains the options used to list running
// processes inside the container
type ProcessListOptions struct {
	// Format describes the output format to list the running processes.
	// Formats are unrelated to ps(1) formats, only two formats can be specified:
	// "json" and "table"
	Format string

	// Args contains the list of arguments to run ps(1) command.
	// If Args is empty the agent will use "-ef" as options to ps(1).
	Args []string
}

// ProcessList represents the list of running processes inside the container
type ProcessList []byte

const (
	// NoopAgentType is the No-Op agent.
	NoopAgentType AgentType = "noop"

	// HyperstartAgent is the Hyper hyperstart agent.
	HyperstartAgent AgentType = "hyperstart"

	// KataContainersAgent is the Kata Containers agent.
	KataContainersAgent AgentType = "kata"

	// SocketTypeVSOCK is a VSOCK socket type for talking to an agent.
	SocketTypeVSOCK = "vsock"

	// SocketTypeUNIX is a UNIX socket type for talking to an agent.
	// It typically means the agent is living behind a host proxy.
	SocketTypeUNIX = "unix"
)

// Set sets an agent type based on the input string.
func (agentType *AgentType) Set(value string) error {
	switch value {
	case "noop":
		*agentType = NoopAgentType
		return nil
	case "hyperstart":
		*agentType = HyperstartAgent
		return nil
	case "kata":
		*agentType = KataContainersAgent
		return nil
	default:
		return fmt.Errorf("Unknown agent type %s", value)
	}
}

// String converts an agent type to a string.
func (agentType *AgentType) String() string {
	switch *agentType {
	case NoopAgentType:
		return string(NoopAgentType)
	case HyperstartAgent:
		return string(HyperstartAgent)
	case KataContainersAgent:
		return string(KataContainersAgent)
	default:
		return ""
	}
}

// newAgent returns an agent from an agent type.
func newAgent(agentType AgentType) agent {
	switch agentType {
	case NoopAgentType:
		return &noopAgent{}
	case HyperstartAgent:
		return &hyper{}
	case KataContainersAgent:
		return &kataAgent{}
	default:
		return &noopAgent{}
	}
}

// newAgentConfig returns an agent config from a generic PodConfig interface.
func newAgentConfig(config PodConfig) interface{} {
	switch config.AgentType {
	case NoopAgentType:
		return nil
	case HyperstartAgent:
		var hyperConfig HyperConfig
		err := mapstructure.Decode(config.AgentConfig, &hyperConfig)
		if err != nil {
			return err
		}
		return hyperConfig
	case KataContainersAgent:
		var kataAgentConfig KataAgentConfig
		err := mapstructure.Decode(config.AgentConfig, &kataAgentConfig)
		if err != nil {
			return err
		}
		return kataAgentConfig
	default:
		return nil
	}
}

// agent is the virtcontainers agent interface.
// Agents are running in the guest VM and handling
// communications between the host and guest.
type agent interface {
	// init is used to pass agent specific configuration to the agent implementation.
	// agent implementations also will typically start listening for agent events from
	// init().
	// After init() is called, agent implementations should be initialized and ready
	// to handle all other Agent interface methods.
	init(pod *Pod, config interface{}) error

	// capabilities should return a structure that specifies the capabilities
	// supported by the agent.
	capabilities() capabilities

	// createPod will tell the agent to perform necessary setup for a Pod.
	createPod(pod *Pod) error

	// exec will tell the agent to run a command in an already running container.
	exec(pod *Pod, c Container, cmd Cmd) (*Process, error)

	// startPod will tell the agent to start all containers related to the Pod.
	startPod(pod Pod) error

	// stopPod will tell the agent to stop all containers related to the Pod.
	stopPod(pod Pod) error

	// createContainer will tell the agent to create a container related to a Pod.
	createContainer(pod *Pod, c *Container) (*Process, error)

	// startContainer will tell the agent to start a container related to a Pod.
	startContainer(pod Pod, c *Container) error

	// stopContainer will tell the agent to stop a container related to a Pod.
	stopContainer(pod Pod, c Container) error

	// killContainer will tell the agent to send a signal to a
	// container related to a Pod. If all is true, all processes in
	// the container will be sent the signal.
	killContainer(pod Pod, c Container, signal syscall.Signal, all bool) error

	// processListContainer will list the processes running inside the container
	processListContainer(pod Pod, c Container, options ProcessListOptions) (ProcessList, error)
}
