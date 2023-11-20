// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"strings"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
)

const (
	cpuFlagsTag        = genericCPUFlagsTag
	archCPUVendorField = genericCPUVendorField
	// On s390x the cpu model is indicated by the field machine.
	// Example:
	// processor 0: version = FF,  identification = 3FEC87,  machine = 2964
	archCPUModelField = "machine"
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
	if hypervisorType == "remote" {
		return nil
	}
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

// getS390xCPUDetails returns the cpu information
func getS390xCPUDetails() (vendor, model string, err error) {
	prefixModel := "processor"
	cpuinfo, err := getCPUInfo(procCPUInfo)
	if err != nil {
		return "", "", err
	}

	lines := strings.Split(cpuinfo, "\n")

	for _, line := range lines {
		if archCPUVendorField != "" {
			if strings.HasPrefix(line, archCPUVendorField) {
				fields := strings.Split(line, ":")
				if len(fields) > 1 {
					vendor = strings.TrimSpace(fields[1])
				}
			}
		}
		if archCPUModelField != "" {
			if strings.HasPrefix(line, prefixModel) {
				fields := strings.Split(strings.TrimSpace(line), ",")
				cpuModel := strings.Split(fields[2], "=")
				model = strings.TrimSpace(cpuModel[1])
			}
		}
	}

	if vendor == "" {
		return "", "", fmt.Errorf("cannot find vendor field in file %v", procCPUInfo)
	}

	if model == "" {
		return "", "", fmt.Errorf("Error in parsing cpu model from %v", procCPUInfo)
	}

	return vendor, model, nil
}

func getCPUDetails() (vendor, model string, err error) {
	if vendor, model, err := genericGetCPUDetails(); err == nil {
		return vendor, model, nil
	}
	return getS390xCPUDetails()
}
