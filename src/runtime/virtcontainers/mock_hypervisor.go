// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"os"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var MockHybridVSockPath = "/tmp/kata-mock-hybrid-vsock.socket"

type mockHypervisor struct {
	mockPid int
}

func (m *mockHypervisor) capabilities() types.Capabilities {
	return types.Capabilities{}
}

func (m *mockHypervisor) hypervisorConfig() HypervisorConfig {
	return HypervisorConfig{}
}

func (m *mockHypervisor) createSandbox(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig) error {
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

func (m *mockHypervisor) getSandboxConsole(sandboxID string) (string, string, error) {
	return "", "", nil
}

func (m *mockHypervisor) resizeMemory(memMB uint32, memorySectionSizeMB uint32, probe bool) (uint32, memoryDevice, error) {
	return 0, memoryDevice{}, nil
}
func (m *mockHypervisor) resizeVCPUs(cpus uint32) (uint32, uint32, error) {
	return 0, 0, nil
}

func (m *mockHypervisor) disconnect() {
}

func (m *mockHypervisor) getThreadIDs() (vcpuThreadIDs, error) {
	vcpus := map[int]int{0: os.Getpid()}
	return vcpuThreadIDs{vcpus}, nil
}

func (m *mockHypervisor) cleanup() error {
	return nil
}

func (m *mockHypervisor) getPids() []int {
	return []int{m.mockPid}
}

func (m *mockHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("mockHypervisor is not supported by VM cache")
}

func (m *mockHypervisor) toGrpc() ([]byte, error) {
	return nil, errors.New("mockHypervisor is not supported by VM cache")
}

func (m *mockHypervisor) save() (s persistapi.HypervisorState) {
	return
}

func (m *mockHypervisor) load(s persistapi.HypervisorState) {}

func (m *mockHypervisor) check() error {
	return nil
}

func (m *mockHypervisor) generateSocket(id string) (interface{}, error) {
	return types.MockHybridVSock{
		UdsPath: MockHybridVSockPath,
	}, nil
}

func (m *mockHypervisor) isRateLimiterBuiltin() bool {
	return false
}
