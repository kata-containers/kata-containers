// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers_test

import (
	"fmt"
	"strings"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

const containerRootfs = "/var/lib/container/bundle/"

// This example creates and starts a single container sandbox,
// using qemu as the hypervisor and hyperstart as the VM agent.
func Example_createAndStartSandbox() {
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

	// The sandbox configuration:
	// - One container
	// - Hypervisor is QEMU
	// - Agent is hyperstart
	sandboxConfig := vc.SandboxConfig{
		VMConfig: vmConfig,

		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   vc.HyperstartAgent,
		AgentConfig: agConfig,

		Containers: []vc.ContainerConfig{container},
	}

	_, err := vc.RunSandbox(sandboxConfig)
	if err != nil {
		fmt.Printf("Could not run sandbox: %s", err)
	}

	return
}
