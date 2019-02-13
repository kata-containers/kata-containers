// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/hex"
	"os"

	govmmQemu "github.com/intel/govmm/qemu"
	deviceConfig "github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/kata-containers/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
)

type qemuPPC64le struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-ppc64le"

const defaultQemuMachineType = QemuPseries

const defaultQemuMachineOptions = "accel=kvm,usb=off"

const defaultMemMaxPPC64le = 32256 // Restrict MemMax to 32Gb on PPC64le

var qemuPaths = map[string]string{
	QemuPseries: defaultQemuPath,
}

var kernelRootParams = []Param{}

var kernelParams = []Param{
	{"tsc", "reliable"},
	{"no_timer_check", ""},
	{"rcupdate.rcu_expedited", "1"},
	{"noreplace-smp", ""},
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

	return caps
}

func (q *qemuPPC64le) bridges(number uint32) []types.PCIBridge {
	return genericBridges(number, q.machineType)
}

func (q *qemuPPC64le) cpuModel() string {
	cpuModel := defaultCPUModel
	if q.nestedRun {
		cpuModel += ",pmu=off"
	}
	return cpuModel
}

func (q *qemuPPC64le) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {

	if qemuMajorVersion >= 2 && qemuMinorVersion >= 10 {
		q.Logger().Debug("Aligning maxmem to multiples of 256MB. Assumption: Kernel Version >= 4.11")
		hostMemoryMb -= (hostMemoryMb % 256)
	} else {
		q.Logger().Debug("Restricting maxmem to 32GB as Qemu Version < 2.10, Assumption: Kernel Version >= 4.11")
		hostMemoryMb = defaultMemMaxPPC64le
	}

	return genericMemoryTopology(memoryMb, hostMemoryMb, slots, q.memoryOffset)
}

func (q *qemuPPC64le) appendImage(devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return nil, err
	}

	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return nil, err
	}

	id := utils.MakeNameID("image", hex.EncodeToString(randBytes), maxDevIDSize)

	drive := deviceConfig.BlockDrive{
		File:   path,
		Format: "raw",
		ID:     id,
	}

	return q.appendBlockDevice(devices, drive), nil
}

// appendBridges appends to devices the given bridges
func (q *qemuPPC64le) appendBridges(devices []govmmQemu.Device, bridges []types.PCIBridge) []govmmQemu.Device {
	return genericAppendBridges(devices, bridges, q.machineType)
}
