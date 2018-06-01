// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "github.com/sirupsen/logrus"

const (
	cpuFlagsTag        = "Features"
	archCPUVendorField = "CPU implementer"
	archCPUModelField  = "CPU variant"
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
		desc: "Kernel-based Virtual Machine",
	},
	"vhost": {
		desc: "Host kernel accelerator for virtio",
	},
	"vhost_net": {
		desc: "Host kernel accelerator for virtio network",
	},
}

// kvmIsUsable determines if it will be possible to create a full virtual machine
// by creating a minimal VM and then deleting it.
func kvmIsUsable() error {
	return genericKvmIsUsable()
}

func archHostCanCreateVMContainer() error {
	return kvmIsUsable()
}

// hostIsVMContainerCapable checks to see if the host is theoretically capable
// of creating a VM container.
func hostIsVMContainerCapable(details vmContainerCapableDetails) error {
	return genericHostIsVMContainerCapable(details)
}

func archKernelParamHandler(onVMM bool, fields logrus.Fields, msg string) bool {
	return genericArchKernelParamHandler(onVMM, fields, msg)
}
