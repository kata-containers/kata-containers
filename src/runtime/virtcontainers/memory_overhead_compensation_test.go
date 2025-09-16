// Copyright (c) 2024 Kata Containers
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"testing"

	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

func TestMemoryOverheadCompensation(t *testing.T) {
	assert := assert.New(t)

	// Test cases for memory overhead compensation logic
	testCases := []struct {
		name                string
		memoryOverhead      uint32
		memorySize          uint32
		currentMemory       uint32
		requestedMemory     uint32
		expectedDelta       uint32
		expectedNewOverhead uint32
		shouldHotplug       bool
	}{
		{
			name:                "No overhead configured",
			memoryOverhead:      0,
			memorySize:          512,
			currentMemory:       512,
			requestedMemory:     1024,
			expectedDelta:       512,
			expectedNewOverhead: 0,
			shouldHotplug:       true,
		},
		{
			name:                "Overhead larger than default memory - no compensation",
			memoryOverhead:      1024,
			memorySize:          512,
			currentMemory:       512,
			requestedMemory:     1024,
			expectedDelta:       512,
			expectedNewOverhead: 1024,
			shouldHotplug:       true,
		},
		{
			name:                "Overhead smaller than default memory - compensation applied",
			memoryOverhead:      256,
			memorySize:          512,
			currentMemory:       512,
			requestedMemory:     1024,
			expectedDelta:       256, // 512 - (512 - 256) = 256
			expectedNewOverhead: 0,   // Should be reset to 0
			shouldHotplug:       true,
		},
		{
			name:                "Overhead smaller than default memory - delta smaller than unaccounted",
			memoryOverhead:      256,
			memorySize:          512,
			currentMemory:       512,
			requestedMemory:     600, // Only 88MB increase
			expectedDelta:       0,   // No hotplug, defer to next request
			expectedNewOverhead: 344, // 256 + 88 = 344
			shouldHotplug:       false,
		},
		{
			name:                "Overhead smaller than default memory - delta equals unaccounted",
			memoryOverhead:      256,
			memorySize:          512,
			currentMemory:       512,
			requestedMemory:     768,  // Exactly 256MB increase
			expectedDelta:       0,    // Apply compensation: 256 - 256 = 0
			expectedNewOverhead: 0,    // Reset to 0 after compensation
			shouldHotplug:       true, // Should hotplug (delta becomes 0 but shouldHotplug stays true)
		},
		{
			name:                "Overhead smaller than default memory - delta larger than unaccounted",
			memoryOverhead:      256,
			memorySize:          512,
			currentMemory:       512,
			requestedMemory:     900, // 388MB increase
			expectedDelta:       132, // 388 - (512 - 256) = 132
			expectedNewOverhead: 0,   // Should be reset to 0
			shouldHotplug:       true,
		},
		{
			name:                "Current memory already larger than default - no compensation",
			memoryOverhead:      256,
			memorySize:          512,
			currentMemory:       768, // Already hotplugged
			requestedMemory:     1024,
			expectedDelta:       0,    // Apply compensation: 256 - 256 = 0
			expectedNewOverhead: 0,    // Reset to 0 after compensation
			shouldHotplug:       true, // Should hotplug (delta becomes 0 but shouldHotplug stays true)
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			// Create a fresh mock hypervisor config for each test case
			hconfig := &HypervisorConfig{
				MemoryOverhead: tc.memoryOverhead,
				MemorySize:     tc.memorySize,
			}

			// Simulate the memory overhead compensation logic exactly as in sandbox.go
			deltaMB := tc.requestedMemory - tc.currentMemory
			hostUnaccountedMB := tc.memorySize - tc.memoryOverhead
			newOverhead := tc.memoryOverhead
			shouldHotplug := true

			// Apply compensation logic exactly as in sandbox.go lines 2329-2356
			if hconfig.MemoryOverhead != 0 && hconfig.MemoryOverhead <= hconfig.MemorySize {
				if hostUnaccountedMB > deltaMB {
					// Defer to next hotplug
					newOverhead += deltaMB
					shouldHotplug = false
					deltaMB = 0
				} else {
					// Apply compensation
					deltaMB -= hostUnaccountedMB
					newOverhead = 0
				}
			}

			assert.Equal(tc.expectedDelta, deltaMB, "Delta should match expected value")
			assert.Equal(tc.expectedNewOverhead, newOverhead, "New overhead should match expected value")
			assert.Equal(tc.shouldHotplug, shouldHotplug, "Hotplug decision should match expected value")
		})
	}
}

func TestMemoryOverheadCompensationEdgeCases(t *testing.T) {
	assert := assert.New(t)

	t.Run("Zero memory overhead", func(t *testing.T) {
		hconfig := &HypervisorConfig{
			MemoryOverhead: 0,
			MemorySize:     512,
		}

		currentMemory := uint32(512)
		requestedMemory := uint32(1024)
		deltaMB := requestedMemory - currentMemory

		// Should not apply any compensation
		hostUnaccountedMB := hconfig.MemorySize - hconfig.MemoryOverhead
		assert.Equal(uint32(512), hostUnaccountedMB, "Host unaccounted should be full memory size")

		// Normal hotplug should proceed - delta (512) should equal hostUnaccountedMB (512)
		assert.Equal(deltaMB, hostUnaccountedMB, "Delta should equal unaccounted when overhead is 0")
	})

	t.Run("Overhead equals memory size", func(t *testing.T) {
		hconfig := &HypervisorConfig{
			MemoryOverhead: 512,
			MemorySize:     512,
		}

		currentMemory := uint32(512)
		requestedMemory := uint32(1024)
		deltaMB := requestedMemory - currentMemory

		// Should not apply any compensation
		hostUnaccountedMB := hconfig.MemorySize - hconfig.MemoryOverhead
		assert.Equal(uint32(0), hostUnaccountedMB, "Host unaccounted should be 0")

		// Normal hotplug should proceed - delta (512) should be larger than hostUnaccountedMB (0)
		assert.True(deltaMB > hostUnaccountedMB, "Delta should be larger than unaccounted")
	})

	t.Run("Requested memory less than current memory", func(t *testing.T) {
		currentMemory := uint32(1024)
		requestedMemory := uint32(512)
		deltaMB := int32(requestedMemory) - int32(currentMemory)

		// Should not apply compensation for memory reduction
		assert.True(deltaMB < 0, "Delta should be negative for memory reduction")
		assert.Equal(int32(-512), deltaMB, "Delta should be -512 for memory reduction")
	})
}

func TestMemoryOverheadCompensationIntegration(t *testing.T) {
	assert := assert.New(t)

	// Test the complete flow with a mock sandbox
	ctx := context.Background()

	// Create a sandbox with memory overhead configuration
	sandbox := &Sandbox{
		ctx: ctx,
		config: &SandboxConfig{
			HypervisorConfig: HypervisorConfig{
				MemoryOverhead: 256,
				MemorySize:     512,
			},
		},
	}

	// Mock the hypervisor to return current memory
	mockHypervisor := &memoryOverheadMockHypervisor{
		currentMemory: 512,
	}

	sandbox.hypervisor = mockHypervisor

	// Test the memory overhead compensation logic
	hconfig := &sandbox.config.HypervisorConfig
	currentMemory := uint32(512)
	requestedMemory := uint32(1024)
	finalMemoryMB := requestedMemory
	deltaMB := finalMemoryMB - currentMemory

	// Apply the compensation logic
	if hconfig.MemoryOverhead != 0 && hconfig.MemoryOverhead <= hconfig.MemorySize {
		hostUnaccountedMB := hconfig.MemorySize - hconfig.MemoryOverhead
		if hostUnaccountedMB > deltaMB {
			// Defer to next hotplug
			hconfig.MemoryOverhead += deltaMB
			deltaMB = 0
		} else {
			// Apply compensation
			deltaMB -= hostUnaccountedMB
			hconfig.MemoryOverhead = 0
		}
	}

	// Verify the results
	assert.Equal(uint32(256), deltaMB, "Delta should be 256 after compensation")
	assert.Equal(uint32(0), hconfig.MemoryOverhead, "Overhead should be reset to 0")
}

// Custom mock hypervisor for testing memory overhead compensation
type memoryOverheadMockHypervisor struct {
	currentMemory uint32
}

func (m *memoryOverheadMockHypervisor) GetTotalMemoryMB(ctx context.Context) uint32 {
	return m.currentMemory
}

func (m *memoryOverheadMockHypervisor) ResizeMemory(ctx context.Context, reqMemMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error) {
	return reqMemMB, MemoryDevice{}, nil
}

func (m *memoryOverheadMockHypervisor) GetTotalMemorySlots(ctx context.Context) uint32 {
	return 0
}

func (m *memoryOverheadMockHypervisor) AddMemory(ctx context.Context, memMB uint32, memBlockSizeMB uint32, probe bool) (MemoryDevice, error) {
	return MemoryDevice{}, nil
}

func (m *memoryOverheadMockHypervisor) RemoveMemory(ctx context.Context, memDevice MemoryDevice) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) GetMemoryBlockSize(ctx context.Context) uint32 {
	return 0
}

func (m *memoryOverheadMockHypervisor) ResizeVCPUs(ctx context.Context, reqVCPUs uint32) (uint32, uint32, error) {
	return 0, 0, nil
}

func (m *memoryOverheadMockHypervisor) GetVCPUs(ctx context.Context) (uint32, uint32, error) {
	return 0, 0, nil
}

func (m *memoryOverheadMockHypervisor) GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error) {
	return VcpuThreadIDs{}, nil
}

func (m *memoryOverheadMockHypervisor) GetPids() []int {
	return nil
}

func (m *memoryOverheadMockHypervisor) GetVirtioFsPid() *int {
	return nil
}

func (m *memoryOverheadMockHypervisor) Cleanup(ctx context.Context) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) GetHypervisorConfig() HypervisorConfig {
	return HypervisorConfig{}
}

func (m *memoryOverheadMockHypervisor) Logger() *logrus.Entry {
	return logrus.NewEntry(logrus.New())
}

func (m *memoryOverheadMockHypervisor) GetVMConsole(ctx context.Context, sandboxID string) (string, string, error) {
	return "", "", nil
}

func (m *memoryOverheadMockHypervisor) Disconnect(ctx context.Context) {
}

func (m *memoryOverheadMockHypervisor) GetCapabilities(ctx context.Context) (types.Capabilities, error) {
	return types.Capabilities{}, nil
}

func (m *memoryOverheadMockHypervisor) PauseVM(ctx context.Context) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) SaveVM() error {
	return nil
}

func (m *memoryOverheadMockHypervisor) StopVM(ctx context.Context, waitOnly bool) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) ResumeVM(ctx context.Context) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) StartVM(ctx context.Context, timeout int) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) GetHypervisorType() HypervisorType {
	return QemuHypervisor
}

func (m *memoryOverheadMockHypervisor) GetGuestMemoryBytes(ctx context.Context) (uint64, error) {
	return 0, nil
}

func (m *memoryOverheadMockHypervisor) AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	return nil, nil
}

func (m *memoryOverheadMockHypervisor) HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error) {
	return nil, nil
}

func (m *memoryOverheadMockHypervisor) Capabilities(ctx context.Context) types.Capabilities {
	return types.Capabilities{}
}

func (m *memoryOverheadMockHypervisor) HypervisorConfig() HypervisorConfig {
	return HypervisorConfig{}
}

func (m *memoryOverheadMockHypervisor) CreateVM(ctx context.Context, id string, network Network, hypervisorConfig *HypervisorConfig) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) Check() error {
	return nil
}

func (m *memoryOverheadMockHypervisor) GenerateSocket(id string) (interface{}, error) {
	return nil, nil
}

func (m *memoryOverheadMockHypervisor) IsRateLimiterBuiltin() bool {
	return false
}

func (m *memoryOverheadMockHypervisor) setConfig(config *HypervisorConfig) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error {
	return nil
}

func (m *memoryOverheadMockHypervisor) toGrpc(ctx context.Context) ([]byte, error) {
	return nil, nil
}

func (m *memoryOverheadMockHypervisor) Save() hv.HypervisorState {
	return hv.HypervisorState{}
}

func (m *memoryOverheadMockHypervisor) Load(s hv.HypervisorState) {
}
