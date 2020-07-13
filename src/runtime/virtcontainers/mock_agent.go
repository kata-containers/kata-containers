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

// mockAgent is an empty Agent implementation, for testing and
// mocking purposes.
type mockAgent struct {
}

func NewMockAgent() agent {
	return &mockAgent{}
}

// init initializes the Noop agent, i.e. it does nothing.
func (n *mockAgent) init(ctx context.Context, sandbox *Sandbox, config KataAgentConfig) (bool, error) {
	return false, nil
}

func (n *mockAgent) longLiveConn() bool {
	return false
}

// createSandbox is the Noop agent sandbox creation implementation. It does nothing.
func (n *mockAgent) createSandbox(sandbox *Sandbox) error {
	return nil
}

// capabilities returns empty capabilities, i.e no capabilties are supported.
func (n *mockAgent) capabilities() types.Capabilities {
	return types.Capabilities{}
}

// disconnect is the Noop agent connection closer. It does nothing.
func (n *mockAgent) disconnect() error {
	return nil
}

// exec is the Noop agent command execution implementation. It does nothing.
func (n *mockAgent) exec(sandbox *Sandbox, c Container, cmd types.Cmd) (*Process, error) {
	return nil, nil
}

// startSandbox is the Noop agent Sandbox starting implementation. It does nothing.
func (n *mockAgent) startSandbox(sandbox *Sandbox) error {
	return nil
}

// stopSandbox is the Noop agent Sandbox stopping implementation. It does nothing.
func (n *mockAgent) stopSandbox(sandbox *Sandbox) error {
	return nil
}

// createContainer is the Noop agent Container creation implementation. It does nothing.
func (n *mockAgent) createContainer(sandbox *Sandbox, c *Container) (*Process, error) {
	return &Process{}, nil
}

// startContainer is the Noop agent Container starting implementation. It does nothing.
func (n *mockAgent) startContainer(sandbox *Sandbox, c *Container) error {
	return nil
}

// stopContainer is the Noop agent Container stopping implementation. It does nothing.
func (n *mockAgent) stopContainer(sandbox *Sandbox, c Container) error {
	return nil
}

// signalProcess is the Noop agent Container signaling implementation. It does nothing.
func (n *mockAgent) signalProcess(c *Container, processID string, signal syscall.Signal, all bool) error {
	return nil
}

// processListContainer is the Noop agent Container ps implementation. It does nothing.
func (n *mockAgent) processListContainer(sandbox *Sandbox, c Container, options ProcessListOptions) (ProcessList, error) {
	return nil, nil
}

// updateContainer is the Noop agent Container update implementation. It does nothing.
func (n *mockAgent) updateContainer(sandbox *Sandbox, c Container, resources specs.LinuxResources) error {
	return nil
}

// memHotplugByProbe is the Noop agent notify meomory hotplug event via probe interface implementation. It does nothing.
func (n *mockAgent) memHotplugByProbe(addr uint64, sizeMB uint32, memorySectionSizeMB uint32) error {
	return nil
}

// onlineCPUMem is the Noop agent Container online CPU and Memory implementation. It does nothing.
func (n *mockAgent) onlineCPUMem(cpus uint32, cpuOnly bool) error {
	return nil
}

// updateInterface is the Noop agent Interface update implementation. It does nothing.
func (n *mockAgent) updateInterface(inf *pbTypes.Interface) (*pbTypes.Interface, error) {
	return nil, nil
}

// listInterfaces is the Noop agent Interfaces list implementation. It does nothing.
func (n *mockAgent) listInterfaces() ([]*pbTypes.Interface, error) {
	return nil, nil
}

// updateRoutes is the Noop agent Routes update implementation. It does nothing.
func (n *mockAgent) updateRoutes(routes []*pbTypes.Route) ([]*pbTypes.Route, error) {
	return nil, nil
}

// listRoutes is the Noop agent Routes list implementation. It does nothing.
func (n *mockAgent) listRoutes() ([]*pbTypes.Route, error) {
	return nil, nil
}

// check is the Noop agent health checker. It does nothing.
func (n *mockAgent) check() error {
	return nil
}

// statsContainer is the Noop agent Container stats implementation. It does nothing.
func (n *mockAgent) statsContainer(sandbox *Sandbox, c Container) (*ContainerStats, error) {
	return &ContainerStats{}, nil
}

// waitProcess is the Noop agent process waiter. It does nothing.
func (n *mockAgent) waitProcess(c *Container, processID string) (int32, error) {
	return 0, nil
}

// winsizeProcess is the Noop agent process tty resizer. It does nothing.
func (n *mockAgent) winsizeProcess(c *Container, processID string, height, width uint32) error {
	return nil
}

// writeProcessStdin is the Noop agent process stdin writer. It does nothing.
func (n *mockAgent) writeProcessStdin(c *Container, ProcessID string, data []byte) (int, error) {
	return 0, nil
}

// closeProcessStdin is the Noop agent process stdin closer. It does nothing.
func (n *mockAgent) closeProcessStdin(c *Container, ProcessID string) error {
	return nil
}

// readProcessStdout is the Noop agent process stdout reader. It does nothing.
func (n *mockAgent) readProcessStdout(c *Container, processID string, data []byte) (int, error) {
	return 0, nil
}

// readProcessStderr is the Noop agent process stderr reader. It does nothing.
func (n *mockAgent) readProcessStderr(c *Container, processID string, data []byte) (int, error) {
	return 0, nil
}

// pauseContainer is the Noop agent Container pause implementation. It does nothing.
func (n *mockAgent) pauseContainer(sandbox *Sandbox, c Container) error {
	return nil
}

// resumeContainer is the Noop agent Container resume implementation. It does nothing.
func (n *mockAgent) resumeContainer(sandbox *Sandbox, c Container) error {
	return nil
}

// configHypervisor is the Noop agent hypervisor configuration implementation. It does nothing.
func (n *mockAgent) configure(h hypervisor, id, sharePath string, config interface{}) error {
	return nil
}

func (n *mockAgent) configureFromGrpc(h hypervisor, id string, config interface{}) error {
	return nil
}

// reseedRNG is the Noop agent RND reseeder. It does nothing.
func (n *mockAgent) reseedRNG(data []byte) error {
	return nil
}

// reuseAgent is the Noop agent reuser. It does nothing.
func (n *mockAgent) reuseAgent(agent agent) error {
	return nil
}

// getAgentURL is the Noop agent url getter. It returns nothing.
func (n *mockAgent) getAgentURL() (string, error) {
	return "", nil
}

// setAgentURL is the Noop agent url setter. It does nothing.
func (n *mockAgent) setAgentURL() error {
	return nil
}

// getGuestDetails is the Noop agent GuestDetails queryer. It does nothing.
func (n *mockAgent) getGuestDetails(*grpc.GuestDetailsRequest) (*grpc.GuestDetailsResponse, error) {
	return nil, nil
}

// setGuestDateTime is the Noop agent guest time setter. It does nothing.
func (n *mockAgent) setGuestDateTime(time.Time) error {
	return nil
}

// copyFile is the Noop agent copy file. It does nothing.
func (n *mockAgent) copyFile(src, dst string) error {
	return nil
}

func (n *mockAgent) markDead() {
}

func (n *mockAgent) cleanup(s *Sandbox) {
}

// save is the Noop agent state saver. It does nothing.
func (n *mockAgent) save() (s persistapi.AgentState) {
	return
}

// load is the Noop agent state loader. It does nothing.
func (n *mockAgent) load(s persistapi.AgentState) {}

func (n *mockAgent) getOOMEvent() (string, error) {
	return "", nil
}

func (k *mockAgent) getAgentMetrics(req *grpc.GetMetricsRequest) (*grpc.Metrics, error) {
	return nil, nil
}
