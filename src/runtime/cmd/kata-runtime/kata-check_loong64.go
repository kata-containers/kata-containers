// Copyright (c) 2023 Loongson Technology Corporation Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
)

const (
	cpuFlagsTag        = "Features"
	archCPUVendorField = ""
	archCPUModelField  = "Model Name"
)

// archRequiredCPUFlags maps a CPU flag value to search for and a
// human-readable description of that value.
var archRequiredCPUFlags = map[string]string{}

// archRequiredCPUAttribs maps a CPU (non-CPU flag) attribute value to search for
// and a human-readable description of that value.
var archRequiredCPUAttribs = map[string]string{}

// archRequiredKernelModules maps a required module name to a human-readable
// description of the modules functionality and an optional list of
// required module parameters.
var archRequiredKernelModules = map[string]kernelModule{
	"kvm": {
		desc:     "Kernel-based Virtual Machine",
		required: true,
	},
	"vhost": {
		desc:     "Host kernel accelerator for virtio",
		required: true,
	},
	"vhost_net": {
		desc:     "Host kernel accelerator for virtio network",
		required: true,
	},
	"vhost_vsock": {
		desc:     "Host Support for Linux VM Sockets",
		required: false,
	},
}

func setCPUtype(hypervisorType vc.HypervisorType) error {
	return nil
}

// kvmIsUsable determines if it will be possible to create a full virtual machine
// by creating a minimal VM and then deleting it.
func kvmIsUsable() error {
	return genericKvmIsUsable()
}

func archHostCanCreateVMContainer(hypervisorType vc.HypervisorType) error {
	return kvmIsUsable()
}

// hostIsVMContainerCapable checks to see if the host is theoretically capable
// of creating a VM container.
func hostIsVMContainerCapable(details vmContainerCapableDetails) error {

	_, err := getCPUInfo(details.cpuInfoFile)
	if err != nil {
		return err
	}

	count, err := checkKernelModules(details.requiredKernelModules, archKernelParamHandler)
	if err != nil {
		return err
	}

	if count == 0 {
		return nil
	}

	return fmt.Errorf("ERROR: %s", failMessage)

}

func archKernelParamHandler(onVMM bool, fields logrus.Fields, msg string) bool {
	return genericArchKernelParamHandler(onVMM, fields, msg)
}

func getCPUDetails() (string, string, error) {
	return genericGetCPUDetails()
}
