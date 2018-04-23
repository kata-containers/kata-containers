// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os/exec"
)

type ccProxy struct {
}

// start is the proxy start implementation for ccProxy.
func (p *ccProxy) start(sandbox *Sandbox, params proxyParams) (int, string, error) {
	config, err := newProxyConfig(sandbox.config)
	if err != nil {
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
