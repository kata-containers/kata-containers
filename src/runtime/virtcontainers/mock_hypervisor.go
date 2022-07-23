// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"errors"
	"os"

	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

var MockHybridVSockPath = "/tmp/kata-mock-hybrid-vsock.socket"

type mockHypervisor struct {
	config  HypervisorConfig
	mockPid int
}

func (m *mockHypervisor) Capabilities(ctx context.Context) types.Capabilities {
	caps := types.Capabilities{}
	caps.SetFsSharingSupport()
	return caps
}

func (m *mockHypervisor) HypervisorConfig() HypervisorConfig {
	return m.config
}

func (m *mockHypervisor) setConfig(config *HypervisorConfig) error {
	m.config = *config
	return nil
}

func (m *mockHypervisor) CreateVM(ctx context.Context, id string, network Network, hypervisorConfig *HypervisorConfig) error {
	if err := m.setConfig(hypervisorConfig); err != nil {
		return err
	}
	m.config.MemSlots = 0
	return nil
}

func (m *mockHypervisor) StartVM(ctx context.Context, timeout int) error {
	return nil
}

func (m *mockHypervisor) StopVM(ctx context.Context, waitOnly bool) error {
	return nil
}

func (m *mockHypervisor) PauseVM(ctx context.Context) error {
	return nil
}

func (m *mockHypervisor) ResumeVM(ctx context.Context) error {
	return nil
}

func (m *mockHypervisor) SaveVM() error {
	return nil
}

func (m *mockHypervisor) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	return nil
}

func (m *mockHypervisor) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	switch devType {
	case CpuDev:
		return devInfo.(uint32), nil
	case MemoryDev:
		memdev := devInfo.(*MemoryDevice)
		return memdev.SizeMB, nil
	}
	return nil, nil
}

func (m *mockHypervisor) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	switch devType {
	case CpuDev:
		return devInfo.(uint32), nil
	case MemoryDev:
		return 0, nil
	}
	return nil, nil
}

func (m *mockHypervisor) GetVMConsole(ctx context.Context, sandboxID string) (string, string, error) {
	return "", "", nil
}

func (m *mockHypervisor) ResizeMemory(ctx context.Context, memMB uint32, memorySectionSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {
	if m.config.MemorySize != memMB {
		// For testing, we'll use MemSlots to track how many times we resized memory
		m.config.MemSlots += 1
		m.config.MemorySize = memMB
	}
	return 0, MemoryDevice{}, nil
}
func (m *mockHypervisor) ResizeVCPUs(ctx context.Context, cpus uint32) (uint32, uint32, error) {
	return 0, 0, nil
}

func (m *mockHypervisor) GetTotalMemoryMB(ctx context.Context) uint32 {
	return m.config.MemorySize
}
func (m *mockHypervisor) Disconnect(ctx context.Context) {
}

func (m *mockHypervisor) GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error) {
	vcpus := map[int]int{0: os.Getpid()}
	return VcpuThreadIDs{vcpus}, nil
}

func (m *mockHypervisor) Cleanup(ctx context.Context) error {
	return nil
}

func (m *mockHypervisor) GetPids() []int {
	return []int{m.mockPid}
}

func (m *mockHypervisor) GetVirtioFsPid() *int {
	return nil
}

func (m *mockHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("mockHypervisor is not supported by VM cache")
}

func (m *mockHypervisor) toGrpc(ctx context.Context) ([]byte, error) {
	return nil, errors.New("mockHypervisor is not supported by VM cache")
}

func (m *mockHypervisor) Save() (s hv.HypervisorState) {
	return
}

func (m *mockHypervisor) Load(s hv.HypervisorState) {}

func (m *mockHypervisor) Check() error {
	return nil
}

func (m *mockHypervisor) GenerateSocket(id string) (interface{}, error) {
	return types.MockHybridVSock{
		UdsPath: MockHybridVSockPath,
	}, nil
}

func (m *mockHypervisor) IsRateLimiterBuiltin() bool {
	return false
}
