// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os/exec"
)

type ccProxy struct {
}

// start is the proxy start implementation for ccProxy.
func (p *ccProxy) start(sandbox *Sandbox, params proxyParams) (int, string, error) {
	if sandbox.config == nil {
		return -1, "", fmt.Errorf("Sandbox config cannot be nil")
	}

	config := sandbox.config.ProxyConfig
	if err := validateProxyConfig(config); err != nil {
		return -1, "", err
	}

	// construct the socket path the proxy instance will use
	proxyURL, err := defaultProxyURL(sandbox, SocketTypeUNIX)
	if err != nil {
		return -1, "", err
	}

	args := []string{config.Path, "-uri", proxyURL}
	if config.Debug {
		args = append(args, "-log", "debug")
	}

	cmd := exec.Command(args[0], args[1:]...)
	if err := cmd.Start(); err != nil {
		return -1, "", err
	}

	return cmd.Process.Pid, proxyURL, nil
}

func (p *ccProxy) stop(sandbox *Sandbox, pid int) error {
	return nil
}

// The ccproxy doesn't need to watch the vm console, thus return false always.
func (p *ccProxy) consoleWatched() bool {
	return false
}
