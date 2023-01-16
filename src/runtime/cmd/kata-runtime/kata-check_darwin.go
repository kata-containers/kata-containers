// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
)

// variables rather than consts to allow tests to modify them
var (
	archCPUVendorField = ""
	archCPUModelField  = ""

	successMessageCapable = "System is capable of running " + katautils.PROJECT
	successMessageCreate  = "System can currently create " + katautils.PROJECT
	failMessage           = "System is not capable of running " + katautils.PROJECT

	procCPUInfo = "N/A"

	// If set, do not perform any network checks
	noNetworkEnvVar = "KATA_CHECK_NO_NETWORK"
)

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
	return nil
}

func hostIsVMContainerCapable(details vmContainerCapableDetails) error {
	return nil
}

func archHostCanCreateVMContainer(hypervisorType vc.HypervisorType) error {
	return nil
}

func getCPUInfo(cpuInfoFile string) (string, error) {
	return "", nil
}

func getCPUDetails() (vendor, model string, err error) {
	return "", "", nil
}
