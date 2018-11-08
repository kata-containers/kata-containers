// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"os/exec"
	"strings"

	"github.com/kata-containers/runtime/pkg/katautils"
)

const (
	unknown     = "<<unknown>>"
	k8sEmptyDir = "kubernetes.io~empty-dir"
)

// variables to allow tests to modify the values
var (
	procVersion = "/proc/version"
	osRelease   = "/etc/os-release"

	// Clear Linux has a different path (for stateless support)
	osReleaseClr = "/usr/lib/os-release"
)

func fileExists(path string) bool {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return false
	}

	return true
}

// IsEphemeralStorage returns true if the given path
// to the storage belongs to kubernetes ephemeral storage
//
// This method depends on a specific path used by k8s
// to detect if it's of type ephemeral. As of now,
// this is a very k8s specific solution that works
// but in future there should be a better way for this
// method to determine if the path is for ephemeral
// volume type
func IsEphemeralStorage(path string) bool {
	splitSourceSlice := strings.Split(path, "/")
	if len(splitSourceSlice) > 1 {
		storageType := splitSourceSlice[len(splitSourceSlice)-2]
		if storageType == k8sEmptyDir {
			return true
		}
	}
	return false
}

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

	if vendor == "" {
		return "", "", fmt.Errorf("cannot find vendor field in file %v", procCPUInfo)
	}

	// model name is optional
	if archCPUModelField != "" && model == "" {
		return "", "", fmt.Errorf("cannot find model field in file %v", procCPUInfo)
	}

	return vendor, model, nil
}

// runCommandFull returns the commands space-trimmed standard output and
// error on success. Note that if the command fails, the requested output will
// still be returned, along with an error.
func runCommandFull(args []string, includeStderr bool) (string, error) {
	cmd := exec.Command(args[0], args[1:]...)
	var err error
	var bytes []byte

	if includeStderr {
		bytes, err = cmd.CombinedOutput()
	} else {
		bytes, err = cmd.Output()
	}

	trimmed := strings.TrimSpace(string(bytes))

	return trimmed, err
}

// runCommand returns the commands space-trimmed standard output on success
func runCommand(args []string) (string, error) {
	return runCommandFull(args, false)
}
