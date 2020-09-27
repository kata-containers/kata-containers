// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"time"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
)

type qemuPPC64le struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-ppc64"

const defaultQemuMachineType = QemuPseries

const defaultQemuMachineOptions = "accel=kvm,usb=off"

const qmpMigrationWaitTimeout = 5 * time.Second

var kernelParams = []Param{
	{"rcupdate.rcu_expedited", "1"},
	{"reboot", "k"},
	{"console", "hvc0"},
	{"console", "hvc1"},
	{"cryptomgr.notests", ""},
	{"net.ifnames", "0"},
}

var supportedQemuMachine = govmmQemu.Machine{
	Type:    QemuPseries,
	Options: defaultQemuMachineOptions,
}

// Logger returns a logrus logger appropriate for logging qemu messages
func (q *qemuPPC64le) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "qemu")
}

// MaxQemuVCPUs returns the maximum number of vCPUs supported
func MaxQemuVCPUs() uint32 {
	return uint32(128)
}

func newQemuArch(config HypervisorConfig) (qemuArch, error) {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	if machineType != defaultQemuMachineType {
		return nil, fmt.Errorf("unrecognised machinetype: %v", machineType)
	}

	q := &qemuPPC64le{
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

	q.memoryOffset = config.MemOffset

	return q, nil
}

func (q *qemuPPC64le) capabilities() types.Capabilities {
	var caps types.Capabilities

	// pseries machine type supports hotplugging drives
	if q.qemuMachine.Type == QemuPseries {
		caps.SetBlockDeviceHotplugSupport()
	}

	caps.SetMultiQueueSupport()
	caps.SetFsSharingSupport()

	return caps
}

func (q *qemuPPC64le) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
}

func (q *qemuPPC64le) cpuModel() string {
	return defaultCPUModel
}

func (q *qemuPPC64le) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {

	q.Logger().Debug("Aligning maxmem to multiples of 256MB. Assumption: Kernel Version >= 4.11")
	hostMemoryMb -= (hostMemoryMb % 256)
	return genericMemoryTopology(memoryMb, hostMemoryMb, slots, q.memoryOffset)
}

// appendBridges appends to devices the given bridges
func (q *qemuPPC64le) appendBridges(devices []govmmQemu.Device) []govmmQemu.Device {
	return genericAppendBridges(devices, q.Bridges, q.qemuMachine.Type)
}

func (q *qemuPPC64le) appendIOMMU(devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	return devices, fmt.Errorf("PPC64le does not support appending a vIOMMU")
}
