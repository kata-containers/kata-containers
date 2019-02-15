// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"

	"github.com/sirupsen/logrus"
)

const (
	cpuFlagsTag        = "Features"
	archCPUVendorField = "CPU implementer"
	archCPUModelField  = "CPU architecture"
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

func setCPUtype() error {
	return nil
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

// The CPU Vendor here for Arm means the CPU core
// IP Implementer.
// normalizeArmVendor maps 'CPU implementer' in /proc/cpuinfo
// to human-readable description of that value.
func normalizeArmVendor(vendor string) string {

	switch vendor {
	case "0x41":
		vendor = "ARM Limited"
	default:
		vendor = "3rd Party Limited"
	}

	return vendor
}

// The CPU Model here for Arm means the Instruction set, that is
// the variant number of Arm processor.
// normalizeArmModel maps 'CPU architecture' in /proc/cpuinfo
// to human-readable description of that value.
func normalizeArmModel(model string) string {
	switch model {
	case "8":
		model = "v8"
	case "7", "7M", "?(12)", "?(13)", "?(14)", "?(15)", "?(16)", "?(17)":
		model = "v7"
	case "6", "6TEJ":
		model = "v6"
	case "5", "5T", "5TE", "5TEJ":
		model = "v5"
	case "4", "4T":
		model = "v4"
	case "3":
		model = "v3"
	default:
		model = "unknown"
	}

	return model
}

func getCPUDetails() (string, string, error) {
	vendor, model, err := genericGetCPUDetails()
	if err == nil {
		vendor = normalizeArmVendor(vendor)
		model = normalizeArmModel(model)
	}

	return vendor, model, err
}
