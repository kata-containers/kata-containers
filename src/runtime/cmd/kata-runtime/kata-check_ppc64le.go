// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os/exec"
	"regexp"
	"strconv"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
)

const (
	cpuFlagsTag        = genericCPUFlagsTag
	archCPUVendorField = ""
	archCPUModelField  = "model"
)

var (
	ppc64CpuCmd     = "ppc64_cpu"
	smtStatusOption = "--smt"
	_               = genericCPUVendorField
	_               = genericCPUModelField
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
	"kvm_hv": {
		desc:     "Kernel-based Virtual Machine hardware virtualization",
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

	text, err := katautils.GetFileContents(details.cpuInfoFile)
	if err != nil {
		return err
	}

	ae := regexp.MustCompile("[0-9]+")
	re := regexp.MustCompile("POWER[0-9]")
	powerProcessor, err := strconv.Atoi(ae.FindString(re.FindString(text)))
	if err != nil {
		kataLog.WithError(err).Error("Failed to find Power Processor number from ", details.cpuInfoFile)
	}

	if powerProcessor <= 8 {
		if !isSMTOff() {
			return fmt.Errorf("SMT is not Off. %s", failMessage)
		}
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

// kvmIsUsable determines if it will be possible to create a full virtual machine
// by creating a minimal VM and then deleting it.
func kvmIsUsable() error {
	return genericKvmIsUsable()
}

func archKernelParamHandler(onVMM bool, fields logrus.Fields, msg string) bool {
	return genericArchKernelParamHandler(onVMM, fields, msg)
}

func getPPC64leCPUInfo(cpuInfoFile string) (string, error) {
	text, err := katautils.GetFileContents(cpuInfoFile)
	if err != nil {
		return "", err
	}

	if len(strings.TrimSpace(text)) == 0 {
		return "", fmt.Errorf("Cannot determine CPU details")
	}

	return text, nil
}

func getCPUDetails() (vendor, model string, err error) {

	if vendor, model, err := genericGetCPUDetails(); err == nil {
		return vendor, model, nil
	}

	cpuinfo, err := getPPC64leCPUInfo(procCPUInfo)
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
			if strings.HasPrefix(line, archCPUModelField) {
				fields := strings.Split(line, ":")
				if len(fields) > 1 {
					model = strings.TrimSpace(fields[1])
				}
			}
		}
	}

	if archCPUVendorField != "" && vendor == "" {
		return "", "", fmt.Errorf("cannot find vendor field in file %v", procCPUInfo)
	}

	if archCPUModelField != "" && model == "" {
		return "", "", fmt.Errorf("cannot find model field in file %v", procCPUInfo)
	}

	return vendor, model, nil
}

func isSMTOff() bool {

	// Check if the SMT is available and off

	cmd := exec.Command(ppc64CpuCmd, smtStatusOption)
	additionalEnv := "LANG=C"
	cmd.Env = append(cmd.Env, additionalEnv)
	out, err := cmd.Output()

	if err == nil && strings.TrimRight(string(out), "\n") == "SMT is off" {
		return true
	} else if err != nil {
		kataLog.Warn("ppc64_cpu isn't installed, can't detect SMT")
		return true
	}

	return false
}
