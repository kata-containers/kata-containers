// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"testing"
)

func TestMockHypervisorCreateSandbox(t *testing.T) {
	var m *mockHypervisor

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ID: "mock_sandbox",
			HypervisorConfig: HypervisorConfig{
				KernelPath:     "",
				ImagePath:      "",
				HypervisorPath: "",
			},
		},
		storage: &filesystem{},
	}

	ctx := context.Background()

	// wrong config
	if err := m.createSandbox(ctx, sandbox.config.ID, &sandbox.config.HypervisorConfig, sandbox.storage); err == nil {
		t.Fatal()
	}

	sandbox.config.HypervisorConfig = HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	if err := m.createSandbox(ctx, sandbox.config.ID, &sandbox.config.HypervisorConfig, sandbox.storage); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorStartSandbox(t *testing.T) {
	var m *mockHypervisor

	if err := m.startSandbox(vmStartTimeout); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorStopSandbox(t *testing.T) {
	var m *mockHypervisor

	if err := m.stopSandbox(); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorAddDevice(t *testing.T) {
	var m *mockHypervisor

	if err := m.addDevice(nil, imgDev); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorGetSandboxConsole(t *testing.T) {
	var m *mockHypervisor

	expected := ""

	result, err := m.getSandboxConsole("testSandboxID")
	if err != nil {
		t.Fatal(err)
	}

	if result != expected {
		t.Fatalf("Got %s\nExpecting %s", result, expected)
	}
}

func TestMockHypervisorSaveSandbox(t *testing.T) {
	var m *mockHypervisor

	if err := m.saveSandbox(); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorDisconnect(t *testing.T) {
	var m *mockHypervisor

	m.disconnect()
}
