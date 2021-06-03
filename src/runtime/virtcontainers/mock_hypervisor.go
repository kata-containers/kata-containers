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

func (m *mockHypervisor) capabilities(ctx context.Context) types.Capabilities {
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

func (m *mockHypervisor) startSandbox(ctx context.Context, timeout int) error {
	return nil
}

func (m *mockHypervisor) stopSandbox(ctx context.Context, waitOnly bool) error {
	return nil
}

func (m *mockHypervisor) pauseSandbox(ctx context.Context) error {
	return nil
}

func (m *mockHypervisor) resumeSandbox(ctx context.Context) error {
	return nil
}

func (m *mockHypervisor) saveSandbox() error {
	return nil
}

func (m *mockHypervisor) addDevice(ctx context.Context, devInfo interface{}, devType deviceType) error {
	return nil
}

func (m *mockHypervisor) hotplugAddDevice(ctx context.Context, devInfo interface{}, devType deviceType) (interface{}, error) {
	switch devType {
	case cpuDev:
		return devInfo.(uint32), nil
	case memoryDev:
		memdev := devInfo.(*memoryDevice)
		return memdev.sizeMB, nil
	}
	return nil, nil
}

func (m *mockHypervisor) hotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType deviceType) (interface{}, error) {
	switch devType {
	case cpuDev:
		return devInfo.(uint32), nil
	case memoryDev:
		return 0, nil
	}
	return nil, nil
}

func (m *mockHypervisor) getSandboxConsole(ctx context.Context, sandboxID string) (string, string, error) {
	return "", "", nil
}

func (m *mockHypervisor) resizeMemory(ctx context.Context, memMB uint32, memorySectionSizeMB uint32, probe bool) (uint32, memoryDevice, error) {
	return 0, memoryDevice{}, nil
}
func (m *mockHypervisor) resizeVCPUs(ctx context.Context, cpus uint32) (uint32, uint32, error) {
	return 0, 0, nil
}

func (m *mockHypervisor) disconnect(ctx context.Context) {
}

func (m *mockHypervisor) getThreadIDs(ctx context.Context) (vcpuThreadIDs, error) {
	vcpus := map[int]int{0: os.Getpid()}
	return vcpuThreadIDs{vcpus}, nil
}

func (m *mockHypervisor) cleanup(ctx context.Context) error {
	return nil
}

func (m *mockHypervisor) getPids() []int {
	return []int{m.mockPid}
}

func (m *mockHypervisor) getVirtioFsPid() *int {
	return nil
}

func (m *mockHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("mockHypervisor is not supported by VM cache")
}

func (m *mockHypervisor) toGrpc(ctx context.Context) ([]byte, error) {
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

func (m *mockHypervisor) setSandbox(sandbox *Sandbox) {
}
