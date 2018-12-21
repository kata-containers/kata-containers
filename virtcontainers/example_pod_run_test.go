// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers_test

import (
	"context"
	"fmt"
	"strings"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

const containerRootfs = "/var/lib/container/bundle/"

// This example creates and starts a single container sandbox,
// using qemu as the hypervisor and hyperstart as the VM agent.
func Example_createAndStartSandbox() {
	envs := []types.EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := types.Cmd{
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
		MemorySize:     1024,
	}

	// Use hyperstart default values for the agent.
	agConfig := vc.HyperConfig{}

	// The sandbox configuration:
	// - One container
	// - Hypervisor is QEMU
	// - Agent is hyperstart
	sandboxConfig := vc.SandboxConfig{
		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   vc.HyperstartAgent,
		AgentConfig: agConfig,

		Containers: []vc.ContainerConfig{container},
	}

	_, err := vc.RunSandbox(context.Background(), sandboxConfig, nil)
	if err != nil {
		fmt.Printf("Could not run sandbox: %s", err)
	}

	return
}
