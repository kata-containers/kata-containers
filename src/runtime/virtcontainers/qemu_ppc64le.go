// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"time"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
)

type qemuPPC64le struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-ppc64le"

const defaultQemuMachineType = QemuPseries

const defaultQemuMachineOptions = "accel=kvm,usb=off,cap-cfpc=broken,cap-sbbc=broken,cap-ibs=broken,cap-large-decr=off"

const defaultMemMaxPPC64le = 32256 // Restrict MemMax to 32Gb on PPC64le

const qmpMigrationWaitTimeout = 5 * time.Second

var qemuPaths = map[string]string{
	QemuPseries: defaultQemuPath,
}

var kernelParams = []Param{
	{"rcupdate.rcu_expedited", "1"},
	{"reboot", "k"},
	{"console", "hvc0"},
	{"console", "hvc1"},
	{"cryptomgr.notests", ""},
	{"net.ifnames", "0"},
}

var supportedQemuMachines = []govmmQemu.Machine{
	{
		Type:    QemuPseries,
		Options: defaultQemuMachineOptions,
	},
}

// Logger returns a logrus logger appropriate for logging qemu messages
func (q *qemuPPC64le) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "qemu")
}

// MaxQemuVCPUs returns the maximum number of vCPUs supported
func MaxQemuVCPUs() uint32 {
	return uint32(128)
}

func newQemuArch(config HypervisorConfig) qemuArch {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	q := &qemuPPC64le{
		qemuArchBase{
			machineType:           machineType,
			memoryOffset:          config.MemOffset,
			qemuPaths:             qemuPaths,
			supportedQemuMachines: supportedQemuMachines,
			kernelParamsNonDebug:  kernelParamsNonDebug,
			kernelParamsDebug:     kernelParamsDebug,
			kernelParams:          kernelParams,
		},
	}

	q.handleImagePath(config)

	q.memoryOffset = config.MemOffset

	return q
}

func (q *qemuPPC64le) capabilities() types.Capabilities {
	var caps types.Capabilities

	// pseries machine type supports hotplugging drives
	if q.machineType == QemuPseries {
		caps.SetBlockDeviceHotplugSupport()
	}

	caps.SetMultiQueueSupport()
	caps.SetFsSharingSupport()

	return caps
}

func (q *qemuPPC64le) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.machineType)
}

func (q *qemuPPC64le) cpuModel() string {
	return defaultCPUModel
}

func (q *qemuPPC64le) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {

	if (qemuMajorVersion > 2) || (qemuMajorVersion == 2 && qemuMinorVersion >= 10) {
		q.Logger().Debug("Aligning maxmem to multiples of 256MB. Assumption: Kernel Version >= 4.11")
		hostMemoryMb -= (hostMemoryMb % 256)
	} else {
		q.Logger().Debug("Restricting maxmem to 32GB as Qemu Version < 2.10, Assumption: Kernel Version >= 4.11")
		hostMemoryMb = defaultMemMaxPPC64le
	}

	return genericMemoryTopology(memoryMb, hostMemoryMb, slots, q.memoryOffset)
}

// appendBridges appends to devices the given bridges
func (q *qemuPPC64le) appendBridges(devices []govmmQemu.Device) []govmmQemu.Device {
	return genericAppendBridges(devices, q.Bridges, q.machineType)
}
