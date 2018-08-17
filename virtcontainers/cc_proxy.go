// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "os/exec"

type ccProxy struct {
}

// start is the proxy start implementation for ccProxy.
func (p *ccProxy) start(params proxyParams) (int, string, error) {
	if err := validateProxyParams(params); err != nil {
		return -1, "", err
	}

	params.logger.Info("Starting cc proxy")

	// construct the socket path the proxy instance will use
	proxyURL, err := defaultProxyURL(params.id, SocketTypeUNIX)
	if err != nil {
		return -1, "", err
	}

	args := []string{params.path, "-uri", proxyURL}
	if params.debug {
		args = append(args, "-log", "debug")
	}

	cmd := exec.Command(args[0], args[1:]...)
	if err := cmd.Start(); err != nil {
		return -1, "", err
	}

	return cmd.Process.Pid, proxyURL, nil
}

func (p *ccProxy) stop(pid int) error {
	return nil
}

// The ccproxy doesn't need to watch the vm console, thus return false always.
func (p *ccProxy) consoleWatched() bool {
	return false
}
