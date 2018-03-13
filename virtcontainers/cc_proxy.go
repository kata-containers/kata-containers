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
	"os/exec"
)

type ccProxy struct {
}

// start is the proxy start implementation for ccProxy.
func (p *ccProxy) start(pod Pod, params proxyParams) (int, string, error) {
	config, err := newProxyConfig(pod.config)
	if err != nil {
		return -1, "", err
	}

	// construct the socket path the proxy instance will use
	proxyURL, err := defaultProxyURL(pod, SocketTypeUNIX)
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

func (p *ccProxy) stop(pod Pod, pid int) error {
	return nil
}
