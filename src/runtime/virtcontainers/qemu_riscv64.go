// Copyright (c) 2024 Institute of Software, CAS.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"time"

	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
)

type qemuRiscv64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-riscv64"

const defaultQemuMachineType = QemuVirt

const qmpMigrationWaitTimeout = 10 * time.Second

const defaultQemuMachineOptions = "accel=kvm,usb=off"

var kernelParams = []Param{
	{"numa", "off"},
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

	q := &qemuRiscv64{
		qemuArchBase{
			qemuMachine:          supportedQemuMachine,
			qemuExePath:          defaultQemuPath,
			memoryOffset:         config.MemOffset,
			kernelParamsNonDebug: kernelParamsNonDebug,
			kernelParamsDebug:    kernelParamsDebug,
			kernelParams:         kernelParams,
		},
	}

	q.handleImagePath(config)

	return q, nil
}

func (q *qemuRiscv64) appendIOMMU(devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	return devices, fmt.Errorf("riscv64 does not support appending a vIOMMU")
}
