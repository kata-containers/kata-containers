//
// Copyright (c) 2017 Intel Corporation
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
	"os/exec"
	"syscall"
)

// This is the Kata Containers implementation of the proxy interface.
// This is pretty simple since it provides the same interface to both
// runtime and shim as if they were talking directly to the agent.
type kataProxy struct {
}

// start is kataProxy start implementation for proxy interface.
func (p *kataProxy) start(pod Pod, params proxyParams) (int, string, error) {
	if pod.agent == nil {
		return -1, "", fmt.Errorf("No agent")
	}

	if params.agentURL == "" {
		return -1, "", fmt.Errorf("AgentURL cannot be empty")
	}

	config, err := newProxyConfig(pod.config)
	if err != nil {
		return -1, "", err
	}

	// construct the socket path the proxy instance will use
	proxyURL, err := defaultProxyURL(pod, SocketTypeUNIX)
	if err != nil {
		return -1, "", err
	}

	args := []string{config.Path, "-listen-socket", proxyURL, "-mux-socket", params.agentURL}
	if config.Debug {
		args = append(args, "-log", "debug")
		args = append(args, "-agent-logs-socket", pod.hypervisor.getPodConsole(pod.id))
	}

	cmd := exec.Command(args[0], args[1:]...)
	if err := cmd.Start(); err != nil {
		return -1, "", err
	}

	return cmd.Process.Pid, proxyURL, nil
}

// stop is kataProxy stop implementation for proxy interface.
func (p *kataProxy) stop(pod Pod, pid int) error {
	// Signal the proxy with SIGTERM.
	return syscall.Kill(pid, syscall.SIGTERM)
}
