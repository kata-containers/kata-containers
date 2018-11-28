// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/kata-containers/runtime/pkg/katautils"
)

const (
	unknown = "<<unknown>>"
)

// variables to allow tests to modify the values
var (
	procVersion = "/proc/version"
	osRelease   = "/etc/os-release"

	// Clear Linux has a different path (for stateless support)
	osReleaseClr = "/usr/lib/os-release"
)

func getKernelVersion() (string, error) {
	contents, err := katautils.GetFileContents(procVersion)
	if err != nil {
		return "", err
	}

	fields := strings.Fields(contents)

	if len(fields) < 3 {
		return "", fmt.Errorf("unexpected contents in %v", procVersion)
	}

	version := fields[2]

	return version, nil
}

// getDistroDetails returns the distributions name and version string.
// If it is not possible to determine both values an error is
// returned.
func getDistroDetails() (name, version string, err error) {
	files := []string{osRelease, osReleaseClr}

	for _, file := range files {
		contents, err := katautils.GetFileContents(file)
		if err != nil {
			if os.IsNotExist(err) {
				continue
			}

			return "", "", err
		}

		lines := strings.Split(contents, "\n")

		for _, line := range lines {
			if strings.HasPrefix(line, "NAME=") {
				fields := strings.Split(line, "=")
				name = strings.Trim(fields[1], `"`)
			} else if strings.HasPrefix(line, "VERSION_ID=") {
				fields := strings.Split(line, "=")
				version = strings.Trim(fields[1], `"`)
			}
		}

		if name != "" && version != "" {
			return name, version, nil
		}
	}

	return "", "", fmt.Errorf("failed to find expected fields in one of %v", files)
}

// genericGetCPUDetails returns the vendor and model of the CPU.
// If it is not possible to determine both values an error is
// returned.
func genericGetCPUDetails() (vendor, model string, err error) {
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

	// model name is optional
	if archCPUModelField != "" && model == "" {
		return "", "", fmt.Errorf("cannot find model field in file %v", procCPUInfo)
	}

	return vendor, model, nil
}
