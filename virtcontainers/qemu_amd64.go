//
// Copyright (c) 2018 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

import (
	"fmt"
	"os"

	govmmQemu "github.com/intel/govmm/qemu"
)

type qemuAmd64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-x86_64"

const defaultQemuMachineType = QemuPC

const defaultQemuMachineOptions = "accel=kvm,kernel_irqchip,nvdimm"

const defaultPCBridgeBus = "pci.0"

var qemuPaths = map[string]string{
	QemuPCLite: "/usr/bin/qemu-lite-system-x86_64",
	QemuPC:     defaultQemuPath,
	QemuQ35:    defaultQemuPath,
}

var kernelParams = []Param{
	{"root", "/dev/pmem0p1"},
	{"rootflags", "dax,data=ordered,errors=remount-ro rw"},
	{"rootfstype", "ext4"},
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
}

// returns the maximum number of vCPUs supported
func maxQemuVCPUs() uint32 {
	return uint32(240)
}

func newQemuArch(machineType string) qemuArch {
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	return &qemuAmd64{
		qemuArchBase{
			machineType:           machineType,
			qemuPaths:             qemuPaths,
			supportedQemuMachines: supportedQemuMachines,
			kernelParamsNonDebug:  kernelParamsNonDebug,
			kernelParamsDebug:     kernelParamsDebug,
			kernelParams:          kernelParams,
		},
	}
}

func (q *qemuAmd64) capabilities() capabilities {
	var caps capabilities

	// Only pc machine type supports hotplugging drives
	if q.machineType == QemuPC {
		caps.setBlockDeviceHotplugSupport()
	}

	return caps
}

func (q *qemuAmd64) bridges(number uint32) []Bridge {
	var bridges []Bridge
	var bt bridgeType

	switch q.machineType {
	case QemuQ35:
		// currently only pci bridges are supported
		// qemu-2.10 will introduce pcie bridges
		fallthrough
	case QemuPC:
		bt = pciBridge
	default:
		return nil
	}

	for i := uint32(0); i < number; i++ {
		bridges = append(bridges, Bridge{
			Type:    bt,
			ID:      fmt.Sprintf("%s-bridge-%d", bt, i),
			Address: make(map[uint32]string),
		})
	}

	return bridges
}

func (q *qemuAmd64) cpuModel() string {
	cpuModel := defaultCPUModel
	if q.nestedRun {
		cpuModel += ",pmu=off"
	}
	return cpuModel
}

func (q *qemuAmd64) memoryTopology(memoryMb, hostMemoryMb uint64) govmmQemu.Memory {
	// NVDIMM device needs memory space 1024MB
	// See https://github.com/clearcontainers/runtime/issues/380
	memoryOffset := 1024

	// add 1G memory space for nvdimm device (vm guest image)
	memMax := fmt.Sprintf("%dM", hostMemoryMb+uint64(memoryOffset))

	mem := fmt.Sprintf("%dM", memoryMb)

	memory := govmmQemu.Memory{
		Size:   mem,
		Slots:  defaultMemSlots,
		MaxMem: memMax,
	}

	return memory
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
func (q *qemuAmd64) appendBridges(devices []govmmQemu.Device, bridges []Bridge) []govmmQemu.Device {
	bus := defaultPCBridgeBus
	if q.machineType == QemuQ35 {
		bus = defaultBridgeBus
	}

	for idx, b := range bridges {
		t := govmmQemu.PCIBridge
		if b.Type == pcieBridge {
			t = govmmQemu.PCIEBridge
		}

		devices = append(devices,
			govmmQemu.BridgeDevice{
				Type: t,
				Bus:  bus,
				ID:   b.ID,
				// Each bridge is required to be assigned a unique chassis id > 0
				Chassis: (idx + 1),
				SHPC:    true,
			},
		)
	}

	return devices
}
