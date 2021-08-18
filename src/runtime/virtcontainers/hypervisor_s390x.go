// Copyright (c) IBM Corp. 2021
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"
)

const (
	// This is valid in other architectures, but varcheck will complain
	// when setting it in common code as it will be regarded unused
	procKernelCmdline = "/proc/cmdline"

	// Secure Execution
	// https://www.kernel.org/doc/html/latest/virt/kvm/s390-pv.html
	seCPUFacilityBit = 158
	seCmdlineParam   = "prot_virt"
)

var seCmdlineValues = []string{
	"1", "on", "y", "yes",
}

// CPUFacilities retrieves active CPU facilities according to "Facility Indications", Principles of Operation.
// Takes cpuinfo path (such as /proc/cpuinfo), returns map of all active facility bits.
func CPUFacilities(cpuInfoPath string) (map[int]bool, error) {
	facilitiesField := "facilities"

	f, err := os.Open(cpuInfoPath)
	if err != nil {
		return map[int]bool{}, err
	}
	defer f.Close()

	facilities := make(map[int]bool)
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		// Expected format: ["facilities", ":", ...] or ["facilities:", ...]
		fields := strings.Fields(scanner.Text())
		if len(fields) < 2 {
			continue
		}

		if !strings.HasPrefix(fields[0], facilitiesField) {
			continue
		}

		start := 1
		if fields[1] == ":" {
			start = 2
		}
		for _, field := range fields[start:] {
			bit, err := strconv.Atoi(field)
			if err != nil {
				return map[int]bool{}, err
			}
			facilities[bit] = true
		}

		return facilities, nil
	}

	if err := scanner.Err(); err != nil {
		return map[int]bool{}, err
	}

	return map[int]bool{}, fmt.Errorf("Couldn't find %q from %q output", facilitiesField, cpuInfoPath)
}

// availableGuestProtection returns seProtection (Secure Execution) if available.
// Checks that Secure Execution is available (CPU facilities) and enabled (kernel command line).
func availableGuestProtection() (guestProtection, error) {
	facilities, err := CPUFacilities(procCPUInfo)
	if err != nil {
		return noneProtection, err
	}
	if !facilities[seCPUFacilityBit] {
		return noneProtection, fmt.Errorf("This CPU does not support Secure Execution")
	}

	seCmdlinePresent, err := CheckCmdline(procKernelCmdline, seCmdlineParam, seCmdlineValues)
	if err != nil {
		return noneProtection, err
	}
	if !seCmdlinePresent {
		return noneProtection, fmt.Errorf("Protected Virtualization is not enabled on kernel command line! "+
			"Need %s=%s (or %s) to enable Secure Execution",
			seCmdlineParam, seCmdlineValues[0], strings.Join(seCmdlineValues[1:], ", "))
	}

	return seProtection, nil
}
