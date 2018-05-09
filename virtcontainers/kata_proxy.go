// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os/exec"
	"syscall"
)

// This is the Kata Containers implementation of the proxy interface.
// This is pretty simple since it provides the same interface to both
// runtime and shim as if they were talking directly to the agent.
type kataProxy struct {
}

// start is kataProxy start implementation for proxy interface.
func (p *kataProxy) start(sandbox *Sandbox, params proxyParams) (int, string, error) {
	if sandbox.agent == nil {
		return -1, "", fmt.Errorf("No agent")
	}

	if params.agentURL == "" {
		return -1, "", fmt.Errorf("AgentURL cannot be empty")
	}

	config, err := newProxyConfig(sandbox.config)
	if err != nil {
		return -1, "", err
	}

	// construct the socket path the proxy instance will use
	proxyURL, err := defaultProxyURL(sandbox, SocketTypeUNIX)
	if err != nil {
		return -1, "", err
	}

	args := []string{config.Path, "-listen-socket", proxyURL, "-mux-socket", params.agentURL}
	if config.Debug {
		args = append(args, "-log", "debug")
		console, err := sandbox.hypervisor.getSandboxConsole(sandbox.id)
		if err != nil {
			return -1, "", err
		}

		args = append(args, "-agent-logs-socket", console)
	}

	cmd := exec.Command(args[0], args[1:]...)
	if err := cmd.Start(); err != nil {
		return -1, "", err
	}

	return cmd.Process.Pid, proxyURL, nil
}

// stop is kataProxy stop implementation for proxy interface.
func (p *kataProxy) stop(sandbox *Sandbox, pid int) error {
	// Signal the proxy with SIGTERM.
	return syscall.Kill(pid, syscall.SIGTERM)
}
