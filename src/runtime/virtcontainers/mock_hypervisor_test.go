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

func TestMockHypervisorCreateSandbox(t *testing.T) {
	var m *mockHypervisor
	assert := assert.New(t)

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ID: "mock_sandbox",
			HypervisorConfig: HypervisorConfig{
				KernelPath:     "",
				ImagePath:      "",
				HypervisorPath: "",
			},
		},
	}

	ctx := context.Background()

	// wrong config
	err := m.createSandbox(ctx, sandbox.config.ID, NetworkNamespace{}, &sandbox.config.HypervisorConfig)
	assert.Error(err)

	sandbox.config.HypervisorConfig = HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	err = m.createSandbox(ctx, sandbox.config.ID, NetworkNamespace{}, &sandbox.config.HypervisorConfig)
	assert.NoError(err)
}

func TestMockHypervisorStartSandbox(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.startSandbox(context.Background(), vmStartTimeout))
}

func TestMockHypervisorStopSandbox(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.stopSandbox(context.Background(), false))
}

func TestMockHypervisorAddDevice(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.addDevice(context.Background(), nil, imgDev))
}

func TestMockHypervisorGetSandboxConsole(t *testing.T) {
	var m *mockHypervisor

	expected := ""
	expectedProto := ""
	proto, result, err := m.getSandboxConsole(context.Background(), "testSandboxID")
	assert.NoError(t, err)
	assert.Equal(t, result, expected)
	assert.Equal(t, proto, expectedProto)
}

func TestMockHypervisorSaveSandbox(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.saveSandbox())
}

func TestMockHypervisorDisconnect(t *testing.T) {
	var m *mockHypervisor

	m.disconnect(context.Background())
}

func TestMockHypervisorCheck(t *testing.T) {
	var m *mockHypervisor

	assert.NoError(t, m.check())
}

func TestMockGenerateSocket(t *testing.T) {
	var m *mockHypervisor

	i, err := m.generateSocket("a")
	assert.NoError(t, err)
	assert.NotNil(t, i)
}
