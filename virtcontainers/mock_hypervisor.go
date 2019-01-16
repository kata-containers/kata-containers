// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"

	"github.com/kata-containers/runtime/virtcontainers/types"
)

type mockHypervisor struct {
}

func (m *mockHypervisor) capabilities() types.Capabilities {
	return types.Capabilities{}
}

func (m *mockHypervisor) hypervisorConfig() HypervisorConfig {
	return HypervisorConfig{}
}

func (m *mockHypervisor) createSandbox(ctx context.Context, id string, hypervisorConfig *HypervisorConfig, storage resourceStorage) error {
	err := hypervisorConfig.valid()
	if err != nil {
		return err
	}

	return nil
}

func (m *mockHypervisor) startSandbox(timeout int) error {
	return nil
}

func (m *mockHypervisor) stopSandbox() error {
	return nil
}

func (m *mockHypervisor) pauseSandbox() error {
	return nil
}

func (m *mockHypervisor) resumeSandbox() error {
	return nil
}

func (m *mockHypervisor) saveSandbox() error {
	return nil
}

func (m *mockHypervisor) addDevice(devInfo interface{}, devType deviceType) error {
	return nil
}

func (m *mockHypervisor) hotplugAddDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	switch devType {
	case cpuDev:
		return devInfo.(uint32), nil
	case memoryDev:
		memdev := devInfo.(*memoryDevice)
		return memdev.sizeMB, nil
	}
	return nil, nil
}

func (m *mockHypervisor) hotplugRemoveDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	switch devType {
	case cpuDev:
		return devInfo.(uint32), nil
	case memoryDev:
		return 0, nil
	}
	return nil, nil
}

func (m *mockHypervisor) getSandboxConsole(sandboxID string) (string, error) {
	return "", nil
}

func (m *mockHypervisor) resizeMemory(memMB uint32, memorySectionSizeMB uint32) (uint32, error) {
	return 0, nil
}
func (m *mockHypervisor) resizeVCPUs(cpus uint32) (uint32, uint32, error) {
	return 0, 0, nil
}

func (m *mockHypervisor) disconnect() {
}

func (m *mockHypervisor) getThreadIDs() (*threadIDs, error) {
	vcpus := []int{os.Getpid()}
	return &threadIDs{vcpus}, nil
}

func (m *mockHypervisor) cleanup() error {
	return nil
}
