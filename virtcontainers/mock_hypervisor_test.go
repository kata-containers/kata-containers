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

	pod := &Pod{
		config: &PodConfig{
			HypervisorConfig: HypervisorConfig{
				KernelPath:     "",
				ImagePath:      "",
				HypervisorPath: "",
			},
		},
	}

	// wrong config
	if err := m.init(pod); err == nil {
		t.Fatal()
	}

	pod.config.HypervisorConfig = HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	// right config
	if err := m.init(pod); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorCreatePod(t *testing.T) {
	var m *mockHypervisor

	config := PodConfig{}

	if err := m.createPod(config); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorStartPod(t *testing.T) {
	var m *mockHypervisor

	if err := m.startPod(); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorWaitPod(t *testing.T) {
	var m *mockHypervisor

	if err := m.waitPod(0); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorStopPod(t *testing.T) {
	var m *mockHypervisor

	if err := m.stopPod(); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorAddDevice(t *testing.T) {
	var m *mockHypervisor

	if err := m.addDevice(nil, imgDev); err != nil {
		t.Fatal(err)
	}
}

func TestMockHypervisorGetPodConsole(t *testing.T) {
	var m *mockHypervisor

	expected := ""

	if result := m.getPodConsole("testPodID"); result != expected {
		t.Fatalf("Got %s\nExpecting %s", result, expected)
	}
}
