// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestMockHypervisorCreateVM(t *testing.T) {
	m := &mockHypervisor{}
	assert := assert.New(t)

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ID: "mock_sandbox",
			HypervisorConfig: HypervisorConfig{
				KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
				ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
				HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
			},
		},
	}

	network, err := NewNetwork()
	assert.NoError(err)

	ctx := context.Background()

	err = m.CreateVM(ctx, sandbox.config.ID, network, &sandbox.config.HypervisorConfig)
	assert.NoError(err)
}

func TestMockHypervisorStartSandbox(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.StartVM(context.Background(), VmStartTimeout))
}

func TestMockHypervisorStopSandbox(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.StopVM(context.Background(), false))
}

func TestMockHypervisorAddDevice(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.AddDevice(context.Background(), nil, ImgDev))
}

func TestMockHypervisorGetSandboxConsole(t *testing.T) {
	var m *mockHypervisor

	expected := ""
	expectedProto := ""
	proto, result, err := m.GetVMConsole(context.Background(), "testSandboxID")
	assert.NoError(t, err)
	assert.Equal(t, result, expected)
	assert.Equal(t, proto, expectedProto)
}

func TestMockHypervisorSaveSandbox(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.SaveVM())
}

func TestMockHypervisorDisconnect(t *testing.T) {
	var m *mockHypervisor

	m.Disconnect(context.Background())
}

func TestMockHypervisorCheck(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.Check())
}

func TestMockGenerateSocket(t *testing.T) {
	var m *mockHypervisor

	i, err := m.GenerateSocket("a")
	assert.NoError(t, err)
	assert.NotNil(t, i)
}
