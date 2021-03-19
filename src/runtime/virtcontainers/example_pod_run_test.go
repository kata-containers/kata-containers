// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers_test

import (
	"context"
	"fmt"
	"strings"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var containerRootfs = vc.RootFs{Target: "/var/lib/container/bundle/", Mounted: true}

// This example creates and starts a single container sandbox,
// using qemu as the hypervisor and kata as the VM agent.
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

	// Use kata default values for the agent.
	agConfig := vc.KataAgentConfig{}

	// The sandbox configuration:
	// - One container
	// - Hypervisor is QEMU
	// - Agent is kata
	sandboxConfig := vc.SandboxConfig{
		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentConfig: agConfig,

		Containers: []vc.ContainerConfig{container},
	}

	// Create the sandbox
	s, err := vc.CreateSandbox(context.Background(), sandboxConfig, nil)
	if err != nil {
		fmt.Printf("Could not create sandbox: %s", err)
		return
	}

	// Start the sandbox
	err = s.Start(context.Background())
	if err != nil {
		fmt.Printf("Could not start sandbox: %s", err)
	}
}
