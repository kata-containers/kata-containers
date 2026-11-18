//go:build linux

// SPDX-License-Identifier: Apache-2.0
package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/manager"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

func TestFCReadOnlyFailedAttachReleasesSlot(t *testing.T) {
	cfg := HypervisorConfig{BlockDeviceReadOnlySlots: 1}
	fc := &firecracker{config: cfg, jailerRoot: t.TempDir()}
	s := &Sandbox{config: &SandboxConfig{HypervisorType: FirecrackerHypervisor, HypervisorConfig: cfg}, hypervisor: fc,
		state: types.SandboxState{BlockIndexMap: make(map[int]struct{})}, devManager: manager.NewDeviceManager(config.VirtioMmio, false, "", 0, nil)}
	missing := filepath.Join(t.TempDir(), "missing")
	dev, err := s.devManager.NewDevice(config.DeviceInfo{HostPath: missing, ContainerPath: missing, DevType: "b", Major: -1, ReadOnly: true})
	if err != nil {
		t.Fatal(err)
	}
	if err := s.devManager.AttachDevice(context.Background(), dev.DeviceID(), s); err == nil {
		t.Fatal("missing backing file accepted")
	}
	if dev.GetAttachCount() != 0 || len(s.state.BlockIndexMap) != 0 {
		t.Fatal("failed attachment leaked a reference or slot")
	}
	index, err := s.GetAndSetSandboxBlockIndex(true)
	if err != nil || index != FirecrackerDiskPoolSize-1 {
		t.Fatalf("read-only slot unavailable after failure: %d, %v", index, err)
	}
}

func TestNonFirecrackerBlockAllocationIgnoresReadOnlyPool(t *testing.T) {
	s := &Sandbox{config: &SandboxConfig{HypervisorType: QemuHypervisor, HypervisorConfig: HypervisorConfig{BlockDeviceReadOnlySlots: 9}},
		state: types.SandboxState{BlockIndexMap: make(map[int]struct{})}}
	for i := 0; i < 2*FirecrackerDiskPoolSize; i++ {
		index, err := s.GetAndSetSandboxBlockIndex(i%2 == 0)
		if err != nil || index != i {
			t.Fatalf("non-Firecracker allocation restricted: %d, %v", index, err)
		}
	}
}

func TestFCReadOnlyPoolAllocationAndReuse(t *testing.T) {
	for slots := uint32(0); slots < FirecrackerDiskPoolSize; slots++ {
		t.Run(fmt.Sprint(slots), func(t *testing.T) {
			cfg := HypervisorConfig{BlockDeviceReadOnlySlots: slots}
			fc := firecracker{config: cfg, jailerRoot: t.TempDir(), fcConfig: &types.FcConfig{}}
			if err := fc.createDiskPool(context.Background()); err != nil {
				t.Fatal(err)
			}
			s := Sandbox{config: &SandboxConfig{HypervisorType: FirecrackerHypervisor, HypervisorConfig: cfg}, state: types.SandboxState{BlockIndexMap: map[int]struct{}{}}}
			if len(fc.fcConfig.Drives) != FirecrackerDiskPoolSize {
				t.Fatal("pool size changed")
			}
			// Interleave modes, then exhaust each range. Every allocated index must
			// correspond to the permissions actually advertised by the VMM at boot.
			for n := 0; n < FirecrackerDiskPoolSize; n++ {
				for _, readOnly := range []bool{true, false} {
					index, err := s.GetAndSetSandboxBlockIndex(readOnly)
					capacity := FirecrackerDiskPoolSize - int(slots)
					if readOnly {
						capacity = int(slots)
					}
					if n >= capacity {
						if err == nil {
							t.Fatal("exhausted mode spilled into another range")
						}
						continue
					}
					if err != nil {
						t.Fatal(err)
					}
					drive := fc.fcConfig.Drives[index]
					if *drive.IsReadOnly != readOnly || *drive.DriveID != fcDriveIndexToID(index) {
						t.Fatal("allocator and VMM slot disagree", index)
					}
				}
			}
			// Releasing a slot may only make that slot available to its original mode.
			index := FirecrackerDiskPoolSize - 1
			readOnly := slots > 0
			if err := s.UnsetSandboxBlockIndex(index); err != nil {
				t.Fatal(err)
			}
			if _, err := s.GetAndSetSandboxBlockIndex(!readOnly); err == nil {
				t.Fatal("wrong-mode reuse")
			}
			reused, err := s.GetAndSetSandboxBlockIndex(readOnly)
			if err != nil || reused != index {
				t.Fatal("slot was not reused", reused, err)
			}
		})
	}
}

func TestFCReadOnlyRejectsWrongSlotBeforeHostEffects(t *testing.T) {
	fc := firecracker{config: HypervisorConfig{BlockDeviceReadOnlySlots: 1}, jailerRoot: t.TempDir()}
	for _, drive := range []config.BlockDrive{{Index: 0, ReadOnly: true}, {Index: 7, ReadOnly: false}, {Index: -1}, {Index: 8}, {Index: 9, ReadOnly: true}} {
		if _, err := fc.hotplugBlockDevice(context.Background(), drive, AddDevice); err == nil {
			t.Fatal("wrong-mode slot accepted", drive)
		}
	}
	entries, err := os.ReadDir(fc.jailerRoot)
	if err != nil || len(entries) != 0 {
		t.Fatal("rejected attachment touched host paths", entries, err)
	}
	for _, slots := range []uint32{8, 9, ^uint32(0)} {
		if err = fc.setConfig(&HypervisorConfig{BlockDeviceReadOnlySlots: slots}); err == nil {
			t.Fatal("invalid pool accepted", slots)
		}
	}
}
