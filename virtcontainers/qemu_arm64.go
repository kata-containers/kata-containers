// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"runtime"

	govmmQemu "github.com/intel/govmm/qemu"
)

type qemuArm64 struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-aarch64"

const defaultQemuMachineType = QemuVirt

const defaultQemuMachineOptions = "gic-version=host,usb=off,accel=kvm"

var qemuPaths = map[string]string{
	QemuVirt: defaultQemuPath,
}

var kernelParams = []Param{
	{"console", "ttyAMA0"},
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

// MaxQemuVCPUs returns the maximum number of vCPUs supported
func MaxQemuVCPUs() uint32 {
	return uint32(runtime.NumCPU())
}

func newQemuArch(config HypervisrConfig) qemuArch {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	q := &qemuArm64{
		qemuArchBase{
			machineType:           machineType,
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
