// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"time"

	govmmQemu "github.com/kata-containers/govmm/qemu"
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

var defaultGICVersion = uint32(3)

var kernelParams = []Param{
	{"console", "hvc0"},
	{"console", "hvc1"},
	{"iommu.passthrough", "0"},
}

var supportedQemuMachine = govmmQemu.Machine{
	Type:    QemuVirt,
	Options: defaultQemuMachineOptions,
}

//In qemu, maximum number of vCPUs depends on the GIC version, or on how
//many redistributors we can fit into the memory map.
//related codes are under github.com/qemu/qemu/hw/arm/virt.c(Line 135 and 1306 in stable-2.11)
//for now, qemu only supports v2 and v3, we treat v4 as v3 based on
//backward compatibility.
var gicList = map[uint32]uint32{
	uint32(2): uint32(8),
	uint32(3): uint32(123),
	uint32(4): uint32(123),
}

// MaxQemuVCPUs returns the maximum number of vCPUs supported
func MaxQemuVCPUs() uint32 {
	return gicList[defaultGICVersion]
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
		},
	}

	q.handleImagePath(config)

	return q, nil
}

func (q *qemuArm64) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
}

// appendBridges appends to devices the given bridges
func (q *qemuArm64) appendBridges(devices []govmmQemu.Device) []govmmQemu.Device {
	return genericAppendBridges(devices, q.Bridges, q.qemuMachine.Type)
}

func (q *qemuArm64) appendImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	if !q.disableNvdimm {
		return q.appendNvdimmImage(devices, path)
	}
	return q.appendBlockImage(ctx, devices, path)
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
