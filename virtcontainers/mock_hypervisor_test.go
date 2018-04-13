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

package virtcontainers

import (
	"fmt"
	"testing"
)

func TestMockHypervisorInit(t *testing.T) {
	var m *mockHypervisor

	sandbox := &Sandbox{
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				KernelPath:     "",
				ImagePath:      "",
				HypervisorPath: "",
			},
		},
	}

	// wrong config
	if err := m.init(sandbox); err == nil {
		t.Fatal()
	}

	sandbox.config.HypervisorConfig = HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	// right config
	if err := m.init(sandbox); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorCreateSandbox(t *testing.T) {
	var m *mockHypervisor

	config := SandboxConfig{}

	if err := m.createSandbox(config); err != nil {
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

	if result := m.getSandboxConsole("testSandboxID"); result != expected {
		t.Fatalf("Got %s\nExpecting %s", result, expected)
	}
}
