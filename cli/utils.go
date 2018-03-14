// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

const unknown = "<<unknown>>"

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

func getFileContents(file string) (string, error) {
	bytes, err := ioutil.ReadFile(file)
	if err != nil {
		return "", err
	}

	return string(bytes), nil
}

func getKernelVersion() (string, error) {
	contents, err := getFileContents(procVersion)
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
		contents, err := getFileContents(file)
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

// getCPUDetails returns the vendor and model of the CPU.
// If it is not possible to determine both values an error is
// returned.
func getCPUDetails() (vendor, model string, err error) {
	cpuinfo, err := getCPUInfo(procCPUInfo)
	if err != nil {
		return "", "", err
	}

	lines := strings.Split(cpuinfo, "\n")

	for _, line := range lines {
		if strings.HasPrefix(line, "vendor_id") {
			fields := strings.Split(line, ":")
			if len(fields) > 1 {
				vendor = strings.TrimSpace(fields[1])
			}
		} else if strings.HasPrefix(line, "model name") {
			fields := strings.Split(line, ":")
			if len(fields) > 1 {
				model = strings.TrimSpace(fields[1])
			}
		}
	}

	if vendor != "" && model != "" {
		return vendor, model, nil
	}

	return "", "", fmt.Errorf("failed to find expected fields in file %v", procCPUInfo)
}

// resolvePath returns the fully resolved and expanded value of the
// specified path.
func resolvePath(path string) (string, error) {
	if path == "" {
		return "", fmt.Errorf("path must be specified")
	}

	absolute, err := filepath.Abs(path)
	if err != nil {
		return "", err
	}

	resolved, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		if os.IsNotExist(err) {
			// Make the error clearer than the default
			return "", fmt.Errorf("file %v does not exist", absolute)
		}

		return "", err
	}

	return resolved, nil
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

// writeFile write data into specified file
func writeFile(filePath string, data string, fileMode os.FileMode) error {
	// Normally dir should not be empty, one case is that cgroup subsystem
	// is not mounted, we will get empty dir, and we want it fail here.
	if filePath == "" {
		return fmt.Errorf("no such file for %s", filePath)
	}

	if err := ioutil.WriteFile(filePath, []byte(data), fileMode); err != nil {
		return fmt.Errorf("failed to write %v to %v: %v", data, filePath, err)
	}

	return nil
}

// isEmptyString return if string is empty
func isEmptyString(b []byte) bool {
	return len(bytes.Trim(b, "\n")) == 0
}
