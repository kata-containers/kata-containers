// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"syscall"
	"time"

	"github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"golang.org/x/net/context"
)

// noopAgent a.k.a. NO-OP Agent is an empty Agent implementation, for testing and
// mocking purposes.
type noopAgent struct {
}

//start the proxy to watch the vm console. It does nothing.
func (n *noopAgent) startProxy(sandbox *Sandbox) error {
	return nil
}

// init initializes the Noop agent, i.e. it does nothing.
func (n *noopAgent) init(ctx context.Context, sandbox *Sandbox, config interface{}) error {
	return nil
}

// createSandbox is the Noop agent sandbox creation implementation. It does nothing.
func (n *noopAgent) createSandbox(sandbox *Sandbox) error {
	return nil
}

// capabilities returns empty capabilities, i.e no capabilties are supported.
func (n *noopAgent) capabilities() capabilities {
	return capabilities{}
}

// disconnect is the Noop agent connection closer. It does nothing.
func (n *noopAgent) disconnect() error {
	return nil
}

// exec is the Noop agent command execution implementation. It does nothing.
func (n *noopAgent) exec(sandbox *Sandbox, c Container, cmd Cmd) (*Process, error) {
	return nil, nil
}

// startSandbox is the Noop agent Sandbox starting implementation. It does nothing.
func (n *noopAgent) startSandbox(sandbox *Sandbox) error {
	return nil
}

// stopSandbox is the Noop agent Sandbox stopping implementation. It does nothing.
func (n *noopAgent) stopSandbox(sandbox *Sandbox) error {
	return nil
}

// createContainer is the Noop agent Container creation implementation. It does nothing.
func (n *noopAgent) createContainer(sandbox *Sandbox, c *Container) (*Process, error) {
	return &Process{}, nil
}

// startContainer is the Noop agent Container starting implementation. It does nothing.
func (n *noopAgent) startContainer(sandbox *Sandbox, c *Container) error {
	return nil
}

// stopContainer is the Noop agent Container stopping implementation. It does nothing.
func (n *noopAgent) stopContainer(sandbox *Sandbox, c Container) error {
	return nil
}

// signalProcess is the Noop agent Container signaling implementation. It does nothing.
func (n *noopAgent) signalProcess(c *Container, processID string, signal syscall.Signal, all bool) error {
	return nil
}

// processListContainer is the Noop agent Container ps implementation. It does nothing.
func (n *noopAgent) processListContainer(sandbox *Sandbox, c Container, options ProcessListOptions) (ProcessList, error) {
	return nil, nil
}

// updateContainer is the Noop agent Container update implementation. It does nothing.
func (n *noopAgent) updateContainer(sandbox *Sandbox, c Container, resources specs.LinuxResources) error {
	return nil
}

// onlineCPUMem is the Noop agent Container online CPU and Memory implementation. It does nothing.
func (n *noopAgent) onlineCPUMem(cpus uint32, cpuOnly bool) error {
	return nil
}

// updateInterface is the Noop agent Interface update implementation. It does nothing.
func (n *noopAgent) updateInterface(inf *types.Interface) (*types.Interface, error) {
	return nil, nil
}

// listInterfaces is the Noop agent Interfaces list implementation. It does nothing.
func (n *noopAgent) listInterfaces() ([]*types.Interface, error) {
	return nil, nil
}

// updateRoutes is the Noop agent Routes update implementation. It does nothing.
func (n *noopAgent) updateRoutes(routes []*types.Route) ([]*types.Route, error) {
	return nil, nil
}

// listRoutes is the Noop agent Routes list implementation. It does nothing.
func (n *noopAgent) listRoutes() ([]*types.Route, error) {
	return nil, nil
}

// check is the Noop agent health checker. It does nothing.
func (n *noopAgent) check() error {
	return nil
}

// statsContainer is the Noop agent Container stats implementation. It does nothing.
func (n *noopAgent) statsContainer(sandbox *Sandbox, c Container) (*ContainerStats, error) {
	return &ContainerStats{}, nil
}

// waitProcess is the Noop agent process waiter. It does nothing.
func (n *noopAgent) waitProcess(c *Container, processID string) (int32, error) {
	return 0, nil
}

// winsizeProcess is the Noop agent process tty resizer. It does nothing.
func (n *noopAgent) winsizeProcess(c *Container, processID string, height, width uint32) error {
	return nil
}

// writeProcessStdin is the Noop agent process stdin writer. It does nothing.
func (n *noopAgent) writeProcessStdin(c *Container, ProcessID string, data []byte) (int, error) {
	return 0, nil
}

// closeProcessStdin is the Noop agent process stdin closer. It does nothing.
func (n *noopAgent) closeProcessStdin(c *Container, ProcessID string) error {
	return nil
}

// readProcessStdout is the Noop agent process stdout reader. It does nothing.
func (n *noopAgent) readProcessStdout(c *Container, processID string, data []byte) (int, error) {
	return 0, nil
}

// readProcessStderr is the Noop agent process stderr reader. It does nothing.
func (n *noopAgent) readProcessStderr(c *Container, processID string, data []byte) (int, error) {
	return 0, nil
}

// pauseContainer is the Noop agent Container pause implementation. It does nothing.
func (n *noopAgent) pauseContainer(sandbox *Sandbox, c Container) error {
	return nil
}

// resumeContainer is the Noop agent Container resume implementation. It does nothing.
func (n *noopAgent) resumeContainer(sandbox *Sandbox, c Container) error {
	return nil
}

// configHypervisor is the Noop agent hypervisor configuration implementation. It does nothing.
func (n *noopAgent) configure(h hypervisor, id, sharePath string, builtin bool, config interface{}) error {
	return nil
}

// getVMPath is the Noop agent vm path getter. It does nothing.
func (n *noopAgent) getVMPath(id string) string {
	return ""
}

// getVMPath is the Noop agent share path getter. It does nothing.
func (n *noopAgent) getSharePath(id string) string {
	return ""
}

// reseedRNG is the Noop agent RND reseeder. It does nothing.
func (n *noopAgent) reseedRNG(data []byte) error {
	return nil
}

// reuseAgent is the Noop agent reuser. It does nothing.
func (n *noopAgent) reuseAgent(agent agent) error {
	return nil
}

// getAgentURL is the Noop agent url getter. It returns nothing.
func (n *noopAgent) getAgentURL() (string, error) {
	return "", nil
}

// setProxy is the Noop agent proxy setter. It does nothing.
func (n *noopAgent) setProxy(sandbox *Sandbox, proxy proxy, pid int, url string) error {
	return nil
}

// getGuestDetails is the Noop agent GuestDetails queryer. It does nothing.
func (n *noopAgent) getGuestDetails(*grpc.GuestDetailsRequest) (*grpc.GuestDetailsResponse, error) {
	return nil, nil
}

// setGuestDateTime is the Noop agent guest time setter. It does nothing.
func (n *noopAgent) setGuestDateTime(time.Time) error {
	return nil
}

// copyFile is the Noop agent copy file. It does nothing.
func (n *noopAgent) copyFile(src, dst string) error {
	return nil
}
