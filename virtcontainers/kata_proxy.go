// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os/exec"
	"syscall"
)

// This is the Kata Containers implementation of the proxy interface.
// This is pretty simple since it provides the same interface to both
// runtime and shim as if they were talking directly to the agent.
type kataProxy struct {
}

// The kata proxy doesn't need to watch the vm console, thus return false always.
func (p *kataProxy) consoleWatched() bool {
	return false
}

// start is kataProxy start implementation for proxy interface.
func (p *kataProxy) start(params proxyParams) (int, string, error) {
	if err := validateProxyParams(params); err != nil {
		return -1, "", err
	}

	params.logger.Debug("Starting regular Kata proxy rather than built-in")

	// construct the socket path the proxy instance will use
	proxyURL, err := defaultProxyURL(params.id, SocketTypeUNIX)
	if err != nil {
		return -1, "", err
	}

	args := []string{
		params.path,
		"-listen-socket", proxyURL,
		"-mux-socket", params.agentURL,
		"-sandbox", params.id,
	}

	if params.debug {
		args = append(args, "-log", "debug", "-agent-logs-socket", params.consoleURL)
	}

	cmd := exec.Command(args[0], args[1:]...)
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setsid: true,
	}
	if err := cmd.Start(); err != nil {
		return -1, "", err
	}

	go cmd.Wait()

	return cmd.Process.Pid, proxyURL, nil
}

// stop is kataProxy stop implementation for proxy interface.
func (p *kataProxy) stop(pid int) error {
	// Signal the proxy with SIGTERM.
	return syscall.Kill(pid, syscall.SIGTERM)
}
