// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"io/ioutil"
	"runtime"
	"strings"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
)

type qemuArm64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-aarch64"

const defaultQemuMachineType = QemuVirt

var defaultQemuMachineOptions = "usb=off,accel=kvm,gic-version=" + getGuestGICVersion()

var qemuPaths = map[string]string{
	QemuVirt: defaultQemuPath,
}

var kernelParams = []Param{
	{"console", "hvc0"},
	{"console", "hvc1"},
	{"iommu.passthrough", "0"},
}

var kernelRootParams = []Param{
	{"root", "/dev/vda1"},
}

var supportedQemuMachines = []govmmQemu.Machine{
	{
		Type:    QemuVirt,
		Options: defaultQemuMachineOptions,
	},
}

// Logger returns a logrus logger appropriate for logging qemu-aarch64 messages
func qemuArmLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "qemu-aarch64")
}

// On ARM platform, we have different GIC interrupt controllers. Different
// GIC supports different QEMU parameters for virtual GIC and max VCPUs
var hostGICVersion = getHostGICVersion()

// We will access this file on host to detect host GIC version
var gicProfile = "/proc/interrupts"

// Detect the host GIC version.
// Success: return the number of GIC version
// Failed: return 0
func getHostGICVersion() (version uint32) {
	bytes, err := ioutil.ReadFile(gicProfile)
	if err != nil {
		qemuArmLogger().WithField("GIC profile", gicProfile).WithError(err).Error("Failed to parse GIC profile")
		return 0
	}

	s := string(bytes)
	if strings.Contains(s, "GICv2") {
		return 2
	}

	if strings.Contains(s, "GICv3") {
		return 3
	}

	if strings.Contains(s, "GICv4") {
		return 4
	}

	return 0
}

// QEMU supports GICv2, GICv3 and host parameters for gic-version. The host
// parameter will let QEMU detect GIC version by itself. This parameter
// will work properly when host GIC version is GICv2 or GICv3. But the
// detection will failed when host GIC is gicv4 or higher. In this case,
// we have to detect the host GIC version manually and force QEMU to use
// GICv3 when host GIC is GICv4 or higher.
func getGuestGICVersion() (version string) {
	if hostGICVersion == 2 {
		return "2"
	}

	if hostGICVersion >= 3 {
		return "3"
	}

	// We can't parse valid host GIC version from GIC profile.
	// But we can use "host" to ask QEMU to detect valid GIC
	// through KVM API for a try.
	return "host"
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
	if hostGICVersion != 0 {
		return gicList[hostGICVersion]
	}
	return uint32(runtime.NumCPU())
}

func newQemuArch(config HypervisorConfig) qemuArch {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	q := &qemuArm64{
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

	if config.ImagePath != "" {
		q.kernelParams = append(q.kernelParams, kernelRootParams...)
		q.kernelParamsNonDebug = append(q.kernelParamsNonDebug, kernelParamsSystemdNonDebug...)
		q.kernelParamsDebug = append(q.kernelParamsDebug, kernelParamsSystemdDebug...)
	}

	return q
}

func (q *qemuArm64) bridges(number uint32) []types.PCIBridge {
	return genericBridges(number, q.machineType)
}

// appendBridges appends to devices the given bridges
func (q *qemuArm64) appendBridges(devices []govmmQemu.Device, bridges []types.PCIBridge) []govmmQemu.Device {
	return genericAppendBridges(devices, bridges, q.machineType)
}
