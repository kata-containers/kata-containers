// Copyright (c) 2026 Loongson Technology Corporation Limited.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"time"

	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
)

type qemuLoongArch64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const (
	defaultQemuPath           = "/usr/bin/qemu-system-loongarch64"
	defaultQemuMachineType    = QemuVirt
	qmpMigrationWaitTimeout   = 5 * time.Second
	defaultQemuMachineOptions = "accel=kvm,usb=off"
)

var kernelParams = []Param{
	{"rcupdate.rcu_expedited", "1"},
	{"reboot", "k"},
	{"cryptomgr.notests", ""},
	{"net.ifnames", "0"},
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

	q := &qemuLoongArch64{
		qemuArchBase{
			qemuMachine:          supportedQemuMachine,
			qemuExePath:          defaultQemuPath,
			memoryOffset:         config.MemOffset,
			kernelParamsNonDebug: kernelParamsNonDebug,
			kernelParamsDebug:    kernelParamsDebug,
			kernelParams:         kernelParams,
			protection:           noneProtection,
		},
	}

	q.handleImagePath(config)

	return q, nil
}

func (q *qemuLoongArch64) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
}

func (q *qemuLoongArch64) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {
        return genericMemoryTopology(memoryMb, hostMemoryMb, slots, q.memoryOffset)
}

func (q *qemuLoongArch64) cpuModel() string {
	cpuModel := "la464"
	return cpuModel
}

func (q *qemuLoongArch64) appendImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	if !q.disableNvdimm {
		return q.appendNvdimmImage(devices, path)
	}
	return q.appendBlockImage(ctx, devices, path)
}
