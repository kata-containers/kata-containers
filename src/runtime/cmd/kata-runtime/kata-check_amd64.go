// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"strings"

	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
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
	cpuFlagVMX                = "vmx"
	cpuFlagLM                 = "lm"
	cpuFlagSVM                = "svm"
	cpuFlagSSE4_1             = "sse4_1"
	kernelModvhost            = "vhost"
	kernelModvhostnet         = "vhost_net"
	kernelModvhostvsock       = "vhost_vsock"
	kernelModkvm              = "kvm"
	kernelModkvmintel         = "kvm_intel"
	kernelModkvmamd           = "kvm_amd"
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

func setCPUtype(hypervisorType vc.HypervisorType) error {
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

		switch hypervisorType {
		case vc.StratovirtHypervisor:
			fallthrough
		case vc.FirecrackerHypervisor:
			fallthrough
		case vc.ClhHypervisor:
			fallthrough
		case vc.DragonballHypervisor:
			fallthrough
		case vc.QemuHypervisor:
			archRequiredCPUFlags = map[string]string{
				cpuFlagVMX:    "Virtualization support",
				cpuFlagLM:     "64Bit CPU",
				cpuFlagSSE4_1: "SSE4.1",
			}
			archRequiredCPUAttribs = map[string]string{
				archGenuineIntel: "Intel Architecture CPU",
			}
			archRequiredKernelModules = map[string]kernelModule{
				kernelModkvm: {
					desc:     msgKernelVM,
					required: true,
				},
				kernelModkvmintel: {
					desc:       "Intel KVM",
					parameters: kvmIntelParams,
					required:   true,
				},
				kernelModvhost: {
					desc:     msgKernelVirtio,
					required: true,
				},
				kernelModvhostnet: {
					desc:     msgKernelVirtioNet,
					required: true,
				},
				kernelModvhostvsock: {
					desc:     msgKernelVirtioVhostVsock,
					required: false,
				},
			}
		case vc.MockHypervisor:
			archRequiredCPUFlags = map[string]string{
				cpuFlagVMX:    "Virtualization support",
				cpuFlagLM:     "64Bit CPU",
				cpuFlagSSE4_1: "SSE4.1",
			}
			archRequiredCPUAttribs = map[string]string{
				archGenuineIntel: "Intel Architecture CPU",
			}

		default:
			return fmt.Errorf("setCPUtype: Unknown hypervisor type %s", hypervisorType)
		}

	} else if cpuType == cpuTypeAMD {
		archRequiredCPUFlags = map[string]string{
			cpuFlagSVM:    "Virtualization support",
			cpuFlagLM:     "64Bit CPU",
			cpuFlagSSE4_1: "SSE4.1",
		}
		archRequiredCPUAttribs = map[string]string{
			archAuthenticAMD: "AMD Architecture CPU",
		}
		archRequiredKernelModules = map[string]kernelModule{
			kernelModkvm: {
				desc:     msgKernelVM,
				required: true,
			},
			kernelModkvmamd: {
				desc:     "AMD KVM",
				required: true,
			},
			kernelModvhost: {
				desc:     msgKernelVirtio,
				required: true,
			},
			kernelModvhostnet: {
				desc:     msgKernelVirtioNet,
				required: true,
			},
			kernelModvhostvsock: {
				desc:     msgKernelVirtioVhostVsock,
				required: false,
			},
		}
	}

	return nil
}

func getCPUtype() int {
	content, err := os.ReadFile("/proc/cpuinfo")
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

func archHostCanCreateVMContainer(hypervisorType vc.HypervisorType) error {
	switch hypervisorType {
	case vc.QemuHypervisor:
		fallthrough
	case vc.ClhHypervisor:
		fallthrough
	case vc.StratovirtHypervisor:
		fallthrough
	case vc.FirecrackerHypervisor:
		return kvmIsUsable()
	case vc.RemoteHypervisor:
		return nil
	case vc.MockHypervisor:
		return nil
	default:
		return fmt.Errorf("archHostCanCreateVMContainer: Unknown hypervisor type %s", hypervisorType)
	}
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
