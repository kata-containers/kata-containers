// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os"
	"time"

	"github.com/kata-containers/runtime/virtcontainers/types"

	govmmQemu "github.com/intel/govmm/qemu"
)

type qemuAmd64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase

	vmFactory bool
}

const defaultQemuPath = "/usr/bin/qemu-system-x86_64"

const defaultQemuMachineType = QemuPC

const defaultQemuMachineOptions = "accel=kvm,kernel_irqchip,nvdimm"

const qmpMigrationWaitTimeout = 5 * time.Second

var qemuPaths = map[string]string{
	QemuPCLite: "/usr/bin/qemu-lite-system-x86_64",
	QemuPC:     defaultQemuPath,
	QemuQ35:    defaultQemuPath,
}

var kernelRootParams = commonNvdimmKernelRootParams

var kernelParams = []Param{
	{"tsc", "reliable"},
	{"no_timer_check", ""},
	{"rcupdate.rcu_expedited", "1"},
	{"i8042.direct", "1"},
	{"i8042.dumbkbd", "1"},
	{"i8042.nopnp", "1"},
	{"i8042.noaux", "1"},
	{"noreplace-smp", ""},
	{"reboot", "k"},
	{"console", "hvc0"},
	{"console", "hvc1"},
	{"iommu", "off"},
	{"cryptomgr.notests", ""},
	{"net.ifnames", "0"},
	{"pci", "lastbus=0"},
}

var supportedQemuMachines = []govmmQemu.Machine{
	{
		Type:    QemuPCLite,
		Options: defaultQemuMachineOptions,
	},
	{
		Type:    QemuPC,
		Options: defaultQemuMachineOptions,
	},
	{
		Type:    QemuQ35,
		Options: defaultQemuMachineOptions,
	},
	{
		Type:    QemuVirt,
		Options: defaultQemuMachineOptions,
	},
}

// MaxQemuVCPUs returns the maximum number of vCPUs supported
func MaxQemuVCPUs() uint32 {
	return uint32(240)
}

func newQemuArch(config HypervisorConfig) qemuArch {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	factory := false
	if config.BootToBeTemplate || config.BootFromTemplate {
		factory = true
	}

	q := &qemuAmd64{
		qemuArchBase: qemuArchBase{
			machineType:           machineType,
			memoryOffset:          config.MemOffset,
			qemuPaths:             qemuPaths,
			supportedQemuMachines: supportedQemuMachines,
			kernelParamsNonDebug:  kernelParamsNonDebug,
			kernelParamsDebug:     kernelParamsDebug,
			kernelParams:          kernelParams,
		},
		vmFactory: factory,
	}

	q.handleImagePath(config)

	return q
}

func (q *qemuAmd64) capabilities() types.Capabilities {
	var caps types.Capabilities

	if q.machineType == QemuPC ||
		q.machineType == QemuQ35 ||
		q.machineType == QemuVirt {
		caps.SetBlockDeviceHotplugSupport()
	}

	caps.SetMultiQueueSupport()

	return caps
}

func (q *qemuAmd64) bridges(number uint32) []types.PCIBridge {
	return genericBridges(number, q.machineType)
}

func (q *qemuAmd64) cpuModel() string {
	cpuModel := defaultCPUModel
	if q.nestedRun {
		cpuModel += ",pmu=off"
	}

	// VMX is not migratable yet.
	// issue: https://github.com/kata-containers/runtime/issues/1750
	if q.vmFactory {
		virtLog.WithField("subsystem", "qemuAmd64").Warn("VMX is not migratable yet: turning it off")
		cpuModel += ",vmx=off"
	}

	return cpuModel
}

func (q *qemuAmd64) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {
	return genericMemoryTopology(memoryMb, hostMemoryMb, slots, q.memoryOffset)
}

func (q *qemuAmd64) appendImage(devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	imageFile, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer func() { _ = imageFile.Close() }()

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

// appendBridges appends to devices the given bridges
func (q *qemuAmd64) appendBridges(devices []govmmQemu.Device, bridges []types.PCIBridge) []govmmQemu.Device {
	return genericAppendBridges(devices, bridges, q.machineType)
}
