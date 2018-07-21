// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

type mockHypervisor struct {
	vCPUs uint32
}

func (m *mockHypervisor) init(id string, hypervisorConfig *HypervisorConfig, vmConfig Resources, storage resourceStorage) error {
	err := hypervisorConfig.valid()
	if err != nil {
		return err
	}

	return nil
}

func (m *mockHypervisor) capabilities() capabilities {
	return capabilities{}
}

func (m *mockHypervisor) createSandbox() error {
	return nil
}

func (m *mockHypervisor) startSandbox() error {
	return nil
}

func (m *mockHypervisor) waitSandbox(timeout int) error {
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
		return m.vCPUs, nil
	}
	return nil, nil
}

func (m *mockHypervisor) hotplugRemoveDevice(devInfo interface{}, devType deviceType) (interface{}, error) {
	switch devType {
	case cpuDev:
		return m.vCPUs, nil
	}
	return nil, nil
}

func (m *mockHypervisor) getSandboxConsole(sandboxID string) (string, error) {
	return "", nil
}
