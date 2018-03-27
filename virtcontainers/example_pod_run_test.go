//
// Copyright (c) 2016 Intel Corporation
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

package virtcontainers_test

import (
	"fmt"
	"strings"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

const containerRootfs = "/var/lib/container/bundle/"

// This example creates and starts a single container pod,
// using qemu as the hypervisor and hyperstart as the VM agent.
func Example_createAndStartPod() {
	envs := []vc.EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := vc.Cmd{
		Args:    strings.Split("/bin/sh", " "),
		Envs:    envs,
		WorkDir: "/",
	}

	// Define the container command and bundle.
	container := vc.ContainerConfig{
		ID:     "1",
		RootFs: containerRootfs,
		Cmd:    cmd,
	}

	// Sets the hypervisor configuration.
	hypervisorConfig := vc.HypervisorConfig{
		KernelPath:     "/usr/share/kata-containers/vmlinux.container",
		ImagePath:      "/usr/share/kata-containers/kata-containers.img",
		HypervisorPath: "/usr/bin/qemu-system-x86_64",
	}

	// Use hyperstart default values for the agent.
	agConfig := vc.HyperConfig{}

	// VM resources
	vmConfig := vc.Resources{
		Memory: 1024,
	}

	// The pod configuration:
	// - One container
	// - Hypervisor is QEMU
	// - Agent is hyperstart
	podConfig := vc.PodConfig{
		VMConfig: vmConfig,

		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   vc.HyperstartAgent,
		AgentConfig: agConfig,

		Containers: []vc.ContainerConfig{container},
	}

	_, err := vc.RunPod(podConfig)
	if err != nil {
		fmt.Printf("Could not run pod: %s", err)
	}

	return
}
