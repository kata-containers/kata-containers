// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"syscall"
	"time"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	pbTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/grpc"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"golang.org/x/net/context"
)

type newAgentFuncKey struct{}

type newAgentFuncType func() agent

// getAgentFunc used to pass mock agent creation func to CreateSandbox passed in `ctx`
func getNewAgentFunc(ctx context.Context) newAgentFuncType {
	v := ctx.Value(newAgentFuncKey{})
	if v != nil {
		if vv, ok := v.(newAgentFuncType); ok {
			return vv
		}
	}
	return newKataAgent
}

// WithNewAgentFunc set newAgentFuncKey in `ctx`
func WithNewAgentFunc(ctx context.Context, f newAgentFuncType) context.Context {
	return context.WithValue(ctx, newAgentFuncKey{}, f)
}

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
	// SocketTypeVSOCK is a VSOCK socket type for talking to an agent.
	SocketTypeVSOCK = "vsock"
)

// agent is the virtcontainers agent interface.
// Agents are running in the guest VM and handling
// communications between the host and guest.
type agent interface {
	// init is used to pass agent specific configuration to the agent implementation.
	// agent implementations also will typically start listening for agent events from
	// init().
	// After init() is called, agent implementations should be initialized and ready
	// to handle all other Agent interface methods.
	init(ctx context.Context, sandbox *Sandbox, config KataAgentConfig) (disableVMShutdown bool, err error)

	// capabilities should return a structure that specifies the capabilities
	// supported by the agent.
	capabilities() types.Capabilities

	// check will check the agent liveness
	check(ctx context.Context) error

	// tell whether the agent is long  live connected or not
	longLiveConn() bool

	// disconnect will disconnect the connection to the agent
	disconnect(ctx context.Context) error

	// get agent url
	getAgentURL() (string, error)

	// set agent url
	setAgentURL() error

	// update the agent using some elements from another agent
	reuseAgent(agent agent) error

	// createSandbox will tell the agent to perform necessary setup for a Sandbox.
	createSandbox(ctx context.Context, sandbox *Sandbox) error

	// exec will tell the agent to run a command in an already running container.
	exec(ctx context.Context, sandbox *Sandbox, c Container, cmd types.Cmd) (*Process, error)

	// startSandbox will tell the agent to start all containers related to the Sandbox.
	startSandbox(ctx context.Context, sandbox *Sandbox) error

	// stopSandbox will tell the agent to stop all containers related to the Sandbox.
	stopSandbox(ctx context.Context, sandbox *Sandbox) error

	// createContainer will tell the agent to create a container related to a Sandbox.
	createContainer(ctx context.Context, sandbox *Sandbox, c *Container) (*Process, error)

	// startContainer will tell the agent to start a container related to a Sandbox.
	startContainer(ctx context.Context, sandbox *Sandbox, c *Container) error

	// stopContainer will tell the agent to stop a container related to a Sandbox.
	stopContainer(ctx context.Context, sandbox *Sandbox, c Container) error

	// signalProcess will tell the agent to send a signal to a
	// container or a process related to a Sandbox. If all is true, all processes in
	// the container will be sent the signal.
	signalProcess(ctx context.Context, c *Container, processID string, signal syscall.Signal, all bool) error

	// winsizeProcess will tell the agent to set a process' tty size
	winsizeProcess(ctx context.Context, c *Container, processID string, height, width uint32) error

	// writeProcessStdin will tell the agent to write a process stdin
	writeProcessStdin(ctx context.Context, c *Container, ProcessID string, data []byte) (int, error)

	// closeProcessStdin will tell the agent to close a process stdin
	closeProcessStdin(ctx context.Context, c *Container, ProcessID string) error

	// readProcessStdout will tell the agent to read a process stdout
	readProcessStdout(ctx context.Context, c *Container, processID string, data []byte) (int, error)

	// readProcessStderr will tell the agent to read a process stderr
	readProcessStderr(ctx context.Context, c *Container, processID string, data []byte) (int, error)

	// updateContainer will update the resources of a running container
	updateContainer(ctx context.Context, sandbox *Sandbox, c Container, resources specs.LinuxResources) error

	// waitProcess will wait for the exit code of a process
	waitProcess(ctx context.Context, c *Container, processID string) (int32, error)

	// onlineCPUMem will online CPUs and Memory inside the Sandbox.
	// This function should be called after hot adding vCPUs or Memory.
	// cpus specifies the number of CPUs that were added and the agent should online
	// cpuOnly specifies that we should online cpu or online memory or both
	onlineCPUMem(ctx context.Context, cpus uint32, cpuOnly bool) error

	// memHotplugByProbe will notify the guest kernel about memory hotplug event through
	// probe interface.
	// This function should be called after hot adding Memory and before online memory.
	// addr specifies the address of the recently hotplugged or unhotplugged memory device.
	memHotplugByProbe(ctx context.Context, addr uint64, sizeMB uint32, memorySectionSizeMB uint32) error

	// statsContainer will tell the agent to get stats from a container related to a Sandbox
	statsContainer(ctx context.Context, sandbox *Sandbox, c Container) (*ContainerStats, error)

	// pauseContainer will pause a container
	pauseContainer(ctx context.Context, sandbox *Sandbox, c Container) error

	// resumeContainer will resume a paused container
	resumeContainer(ctx context.Context, sandbox *Sandbox, c Container) error

	// configure will update agent settings based on provided arguments
	configure(ctx context.Context, h hypervisor, id, sharePath string, config KataAgentConfig) error

	// configureFromGrpc will update agent settings based on provided arguments which from Grpc
	configureFromGrpc(h hypervisor, id string, config KataAgentConfig) error

	// reseedRNG will reseed the guest random number generator
	reseedRNG(ctx context.Context, data []byte) error

	// updateInterface will tell the agent to update a nic for an existed Sandbox.
	updateInterface(ctx context.Context, inf *pbTypes.Interface) (*pbTypes.Interface, error)

	// listInterfaces will tell the agent to list interfaces of an existed Sandbox
	listInterfaces(ctx context.Context) ([]*pbTypes.Interface, error)

	// updateRoutes will tell the agent to update route table for an existed Sandbox.
	updateRoutes(ctx context.Context, routes []*pbTypes.Route) ([]*pbTypes.Route, error)

	// listRoutes will tell the agent to list routes of an existed Sandbox
	listRoutes(ctx context.Context) ([]*pbTypes.Route, error)

	// getGuestDetails will tell the agent to get some information of guest
	getGuestDetails(context.Context, *grpc.GuestDetailsRequest) (*grpc.GuestDetailsResponse, error)

	// setGuestDateTime asks the agent to set guest time to the provided one
	setGuestDateTime(context.Context, time.Time) error

	// copyFile copies file from host to container's rootfs
	copyFile(ctx context.Context, src, dst string) error

	// markDead tell agent that the guest is dead
	markDead(ctx context.Context)

	// cleanup removes all on disk information generated by the agent
	cleanup(ctx context.Context, s *Sandbox)

	// return data for saving
	save() persistapi.AgentState

	// load data from disk
	load(persistapi.AgentState)

	// getOOMEvent will wait on OOM events that occur in the sandbox.
	// Will return the ID of the container where the event occurred.
	getOOMEvent(ctx context.Context) (string, error)

	// getAgentMetrics get metrics of agent and guest through agent
	getAgentMetrics(context.Context, *grpc.GetMetricsRequest) (*grpc.Metrics, error)
}
