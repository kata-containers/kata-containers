// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"github.com/sirupsen/logrus"
	"io/ioutil"
	"strings"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

const (
	cpuFlagsTag               = genericCPUFlagsTag
	archCPUVendorField        = genericCPUVendorField
	archCPUModelField         = genericCPUModelField
	archGenuineIntel          = "GenuineIntel"
	archAuthenticAMD          = "AuthenticAMD"
	msgKernelVM               = "Kernel-based Virtual Machine"
	msgKernelVirtio           = "Host kernel accelerator for virtio"
	msgKernelVirtioNet        = "Host kernel accelerator for virtio network"
	msgKernelVirtioVhostVsock = "Host Support for Linux VM Sockets"
)

// CPU types
const (
	cpuTypeIntel   = 0
	cpuTypeAMD     = 1
	cpuTypeUnknown = -1
)

// cpuType save the CPU type
var cpuType int

// archRequiredCPUFlags maps a CPU flag value to search for and a
// human-readable description of that value.
var archRequiredCPUFlags map[string]string

// archRequiredCPUAttribs maps a CPU (non-CPU flag) attribute value to search for
// and a human-readable description of that value.
var archRequiredCPUAttribs map[string]string

// archRequiredKernelModules maps a required module name to a human-readable
// description of the modules functionality and an optional list of
// required module parameters.
var archRequiredKernelModules map[string]kernelModule

func setCPUtype() error {
	cpuType = getCPUtype()

	if cpuType == cpuTypeUnknown {
		return fmt.Errorf("Unknow CPU Type")
	} else if cpuType == cpuTypeIntel {
		var kvmIntelParams map[string]string
		onVMM, err := vc.RunningOnVMM(procCPUInfo)
		if err != nil && !onVMM {
			kvmIntelParams = map[string]string{
				// "VMX Unrestricted mode support". This is used
				// as a heuristic to determine if the system is
				// "new enough" to run a Kata Container
				// (atleast a Westmere).
				"unrestricted_guest": "Y",
			}
		}
		archRequiredCPUFlags = map[string]string{
			"vmx":    "Virtualization support",
			"lm":     "64Bit CPU",
			"sse4_1": "SSE4.1",
		}
		archRequiredCPUAttribs = map[string]string{
			archGenuineIntel: "Intel Architecture CPU",
		}
		archRequiredKernelModules = map[string]kernelModule{
			"kvm": {
				desc:     msgKernelVM,
				required: true,
			},
			"kvm_intel": {
				desc:       "Intel KVM",
				parameters: kvmIntelParams,
				required:   true,
			},
			"vhost": {
				desc:     msgKernelVirtio,
				required: true,
			},
			"vhost_net": {
				desc:     msgKernelVirtioNet,
				required: true,
			},
			"vhost_vsock": {
				desc:     msgKernelVirtioVhostVsock,
				required: false,
			},
		}
	} else if cpuType == cpuTypeAMD {
		archRequiredCPUFlags = map[string]string{
			"svm":    "Virtualization support",
			"lm":     "64Bit CPU",
			"sse4_1": "SSE4.1",
		}
		archRequiredCPUAttribs = map[string]string{
			archAuthenticAMD: "AMD Architecture CPU",
		}
		archRequiredKernelModules = map[string]kernelModule{
			"kvm": {
				desc:     msgKernelVM,
				required: true,
			},
			"kvm_amd": {
				desc:     "AMD KVM",
				required: true,
			},
			"vhost": {
				desc:     msgKernelVirtio,
				required: true,
			},
			"vhost_net": {
				desc:     msgKernelVirtioNet,
				required: true,
			},
			"vhost_vsock": {
				desc:     msgKernelVirtioVhostVsock,
				required: false,
			},
		}
	}

	return nil
}

func getCPUtype() int {
	content, err := ioutil.ReadFile("/proc/cpuinfo")
	if err != nil {
		kataLog.WithError(err).Error("failed to read file")
		return cpuTypeUnknown
	}
	str := string(content)
	if strings.Contains(str, archGenuineIntel) {
		return cpuTypeIntel
	} else if strings.Contains(str, archAuthenticAMD) {
		return cpuTypeAMD
	} else {
		return cpuTypeUnknown
	}
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

func getCPUDetails() (vendor, model string, err error) {
	return genericGetCPUDetails()
}
