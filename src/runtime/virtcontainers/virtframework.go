//
// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

//go:build darwin
// +build darwin

package virtcontainers

import (
	"context"

	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/pkg/errors"
)

// virtFramework is a Hypervisor interface implementation for Darwin Virtualization.framework.
type virtFramework struct{}

func (vfw *virtFramework) CreateVM(ctx context.Context, id string, network Network, hypervisorConfig *HypervisorConfig) error {
	return nil
}

func (vfw *virtFramework) StartVM(ctx context.Context, timeout int) error {
	return nil
}

// If wait is set, don't actively stop the sandbox:
// just perform cleanup.
func (vfw *virtFramework) StopVM(ctx context.Context, waitOnly bool) error {
	return nil
}

func (vfw *virtFramework) PauseVM(ctx context.Context) error {
	return nil
}

func (vfw *virtFramework) SaveVM() error {
	return nil
}

func (vfw *virtFramework) ResumeVM(ctx context.Context) error {
	return nil
}

func (vfw *virtFramework) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	return nil
}

func (vfw *virtFramework) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	return nil, nil
}

func (vfw *virtFramework) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	return nil, nil
}

func (vfw *virtFramework) ResizeMemory(ctx context.Context, memMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {
	return 0, MemoryDevice{}, nil
}

func (vfw *virtFramework) ResizeVCPUs(ctx context.Context, vcpus uint32) (uint32, uint32, error) {
	return 0, 0, nil
}

func (vfw *virtFramework) GetVMConsole(ctx context.Context, sandboxID string) (string, string, error) {
	return "", "", nil
}

func (vfw *virtFramework) Disconnect(ctx context.Context) {
}

func (vfw *virtFramework) Capabilities(ctx context.Context) types.Capabilities {
	return types.Capabilities{}
}

func (vfw *virtFramework) HypervisorConfig() HypervisorConfig {
	return HypervisorConfig{}
}

func (vfw *virtFramework) GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error) {
	var vcpuInfo VcpuThreadIDs

	vcpuInfo.vcpus = make(map[int]int)

	return vcpuInfo, nil
}

func (vfw *virtFramework) Cleanup(ctx context.Context) error {
	return nil
}

func (vfw *virtFramework) GetTotalMemoryMB(ctx context.Context) uint32 {
	return 0
}

func (vfw *virtFramework) setConfig(config *HypervisorConfig) error {
	return nil
}

func (vfw *virtFramework) GetPids() []int {
	return nil
}

func (vfw *virtFramework) GetVirtioFsPid() *int {
	return nil
}

func (vfw *virtFramework) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return errors.New("Darwin is not supported by VM cache")
}

func (vfw *virtFramework) toGrpc(ctx context.Context) ([]byte, error) {
	return nil, errors.New("Darwin is not supported by VM cache")
}

func (vfw *virtFramework) Check() error {
	return nil
}

func (vfw *virtFramework) Save() hv.HypervisorState {
	return hv.HypervisorState{}
}

func (vfw *virtFramework) Load(hv.HypervisorState) {
}

func (vfw *virtFramework) GenerateSocket(id string) (interface{}, error) {
	return nil, nil
}

func (vfw *virtFramework) IsRateLimiterBuiltin() bool {
	return false
}
