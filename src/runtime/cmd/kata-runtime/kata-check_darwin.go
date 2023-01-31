// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os/exec"
	"strconv"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/pkg/errors"
	"golang.org/x/sys/unix"
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

func archHostCanCreateVMContainer(hypervisorType vc.HypervisorType) error {
	switch hypervisorType {
	case vc.VirtframeworkHypervisor:
		return virtFrameworkChecks()
	default:
		return fmt.Errorf("archHostCanCreateVMContainer: Unknown hypervisor type %s", hypervisorType)
	}
}

func virtFrameworkChecks() error {
	// Just check if they're on a macOS version that has the framework (11+).
	output, err := exec.Command("sw_vers", "--productversion").Output()
	if err != nil {
		return fmt.Errorf("failed to get MacOS version: %w", err)
	}

	version := strings.Split(string(output), ".")
	if len(version) == 0 {
		return errors.New("failed to parse OS version")
	}

	major, err := strconv.Atoi(version[0])
	if err != nil {
		return fmt.Errorf("failed to convert major number to integer: %w", err)
	}

	if major < 11 {
		return errors.New("host doesn't support the virtualization framework")
	}

	return nil
}

func hostIsVMContainerCapable(details vmContainerCapableDetails) error {
	// Check if we're already in a guest, no nested support in virt framework
	// as of now so the "host" here won't be capable.
	vmmPresent, err := unix.SysctlUint32("kern.hv_vmm_present")
	if err != nil {
		return err
	}
	if vmmPresent == 1 {
		return errors.New("unsupported: cannot run a nested guest")
	}

	// Check if virt was disabled, and that the host supports it in general.
	hvDisabled, err := unix.SysctlUint32("kern.hv_disabled")
	if err != nil {
		return err
	}
	if hvDisabled == 1 {
		return errors.New("unsupported: virtualization is disabled")
	}

	// Check if the host supports virt
	hvSupport, err := unix.SysctlUint32("kern.hv_support")
	if err != nil {
		return err
	}
	if hvSupport != 1 {
		return errors.New("unsupported: host doesn't support virtualization")
	}

	return nil
}

func getCPUInfo(cpuInfoFile string) (string, error) {
	return "", nil
}

func getCPUDetails() (vendor, model string, err error) {
	return "", "", nil
}
