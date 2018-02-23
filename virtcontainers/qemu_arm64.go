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
	{"root", "/dev/vda1"},
	{"console", "ttyAMA0"},
	{"iommu.passthrough", "0"},
}

var supportedQemuMachines = []govmmQemu.Machine{
	{
		Type:    QemuVirt,
		Options: defaultQemuMachineOptions,
	},
}

// returns the maximum number of vCPUs supported
func maxQemuVCPUs() uint32 {
	return uint32(runtime.NumCPU())
}

func newQemuArch(machineType string) qemuArch {
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	return &qemuArm64{
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
