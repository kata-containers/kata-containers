//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"os"
	"runtime"
	"time"

	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

type qemuArm64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-aarch64"

const defaultQemuMachineType = QemuVirt

const qmpMigrationWaitTimeout = 10 * time.Second

const defaultQemuMachineOptions = "usb=off,accel=kvm,gic-version=host"

var kernelParams = []Param{
	{"iommu.passthrough", "0"},
}

var supportedQemuMachine = govmmQemu.Machine{
	Type:    QemuVirt,
	Options: defaultQemuMachineOptions,
}

func newQemuArch(config HypervisorConfig) (qemuArch, error) {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	if machineType != defaultQemuMachineType {
		return nil, fmt.Errorf("unrecognised machinetype: %v", machineType)
	}

	q := &qemuArm64{
		qemuArchBase{
			qemuMachine:          supportedQemuMachine,
			qemuExePath:          defaultQemuPath,
			memoryOffset:         config.MemOffset,
			kernelParamsNonDebug: kernelParamsNonDebug,
			kernelParamsDebug:    kernelParamsDebug,
			kernelParams:         kernelParams,
			disableNvdimm:        config.DisableImageNvdimm,
			dax:                  true,
			protection:           noneProtection,
			legacySerial:         config.LegacySerial,
		},
	}

	if err := q.handleImagePath(config); err != nil {
		return nil, err
	}

	return q, nil
}

func (q *qemuArm64) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
}

func (q *qemuArm64) appendImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	if !q.disableNvdimm {
		return q.appendNvdimmImage(devices, path)
	}
	return q.appendBlockImage(ctx, devices, path)
}

// There is no nvdimm/readonly feature in qemu 5.1 which is used by arm64 for now,
// so we temporarily add this specific implementation for arm64 here until
// the qemu used by arm64 is capable for that feature
func (q *qemuArm64) appendNvdimmImage(devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	imageFile, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer imageFile.Close()

	imageStat, err := imageFile.Stat()
	if err != nil {
		return nil, err
	}

	object := govmmQemu.Object{
		Driver:   govmmQemu.NVDIMM,
		Type:     govmmQemu.MemoryBackendFile,
		DeviceID: "nv0",
		ID:       "mem0",
		MemPath:  path,
		Size:     (uint64)(imageStat.Size()),
	}

	devices = append(devices, object)

	return devices, nil
}

func (q *qemuArm64) setIgnoreSharedMemoryMigrationCaps(_ context.Context, _ *govmmQemu.QMP) error {
	// x-ignore-shared not support in arm64 for now
	return nil
}

func (q *qemuArm64) appendIOMMU(devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	return devices, fmt.Errorf("Arm64 architecture does not support vIOMMU")
}

func (q *qemuArm64) append9PVolume(_ context.Context, devices []govmmQemu.Device, volume types.Volume) ([]govmmQemu.Device, error) {
	d, err := genericAppend9PVolume(devices, volume, q.nestedRun)
	if err != nil {
		return nil, err
	}

	d.Multidev = ""
	devices = append(devices, d)
	return devices, nil
}

func (q *qemuArm64) getPFlash() ([]string, error) {
	length := len(q.PFlash)
	if length == 0 {
		return nil, nil
	} else if length == 1 {
		return nil, fmt.Errorf("two pflash images needed for arm64")
	} else if length == 2 {
		return q.PFlash, nil
	} else {
		return nil, fmt.Errorf("too many pflash images for arm64")
	}
}

func (q *qemuArm64) enableProtection() error {
	q.protection, _ = availableGuestProtection()
	if q.protection != noneProtection {
		return fmt.Errorf("Protection %v is not supported on arm64", q.protection)
	}

	return nil
}

func (q *qemuArm64) appendProtectionDevice(devices []govmmQemu.Device, firmware, firmwareVolume string) ([]govmmQemu.Device, string, error) {
	err := q.enableProtection()
	if err != nil {
		hvLogger.WithField("arch", runtime.GOARCH).Error(err)
	}
	return devices, firmware, err
}
