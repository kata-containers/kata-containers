// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"testing"
)

func TestMockHypervisorInit(t *testing.T) {
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

	// wrong config
	if err := m.init(sandbox.config.ID, &sandbox.config.HypervisorConfig, sandbox.config.VMConfig, sandbox.storage); err == nil {
		t.Fatal()
	}

	sandbox.config.HypervisorConfig = HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	// right config
	if err := m.init(sandbox.config.ID, &sandbox.config.HypervisorConfig, sandbox.config.VMConfig, sandbox.storage); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorCreateSandbox(t *testing.T) {
	var m *mockHypervisor

	if err := m.createSandbox(); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorStartSandbox(t *testing.T) {
	var m *mockHypervisor

	if err := m.startSandbox(); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorWaitSandbox(t *testing.T) {
	var m *mockHypervisor

	if err := m.waitSandbox(0); err != nil {
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
