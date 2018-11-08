// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os/exec"
	"strings"

	"github.com/kata-containers/runtime/pkg/katautils"
	"github.com/sirupsen/logrus"
)

const (
	cpuFlagsTag        = genericCPUFlagsTag
	archCPUVendorField = genericCPUVendorField
	archCPUModelField  = genericCPUModelField
)

var (
	ppc64CpuCmd     = "ppc64_cpu"
	smtStatusOption = "--smt"
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
	"kvm_hv": {
		desc: "Kernel-based Virtual Machine hardware virtualization",
	},
}

func setCPUtype() {
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

	text, err := katautils.GetFileContents(details.cpuInfoFile)
	if err != nil {
		return err
	}

	if strings.Contains(text, "POWER8") {
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

func getCPUDetails() (vendor, model string, err error) {
	return genericGetCPUDetails()
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
