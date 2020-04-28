// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"fmt"
	"golang.org/x/sys/unix"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
)

// FileExists test is a file exiting or not
func FileExists(path string) bool {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return false
	}

	return true
}

// ResolvePath returns the fully resolved and expanded value of the
// specified path.
func ResolvePath(path string) (string, error) {
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

// IsBlockDevice returns true if the give path is a block device
func IsBlockDevice(filePath string) bool {
	var stat unix.Stat_t

	if filePath == "" {
		return false
	}

	devicePath, err := ResolvePath(filePath)
	if err != nil {
		return false
	}

	if err := unix.Stat(devicePath, &stat); err != nil {
		return false
	}

	if stat.Mode&unix.S_IFBLK == unix.S_IFBLK {
		return true
	}
	return false
}

// fileSize returns the number of bytes in the specified file
func fileSize(file string) (int64, error) {
	st := syscall.Stat_t{}

	err := syscall.Stat(file, &st)
	if err != nil {
		return 0, err
	}

	return st.Size, nil
}

// WriteFile write data into specified file
func WriteFile(filePath string, data string, fileMode os.FileMode) error {
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

// GetFileContents return the file contents as a string.
func GetFileContents(file string) (string, error) {
	bytes, err := ioutil.ReadFile(file)
	if err != nil {
		return "", err
	}

	return string(bytes), nil
}

// RunCommandFull returns the commands space-trimmed standard output and
// error on success. Note that if the command fails, the requested output will
// still be returned, along with an error.
func RunCommandFull(args []string, includeStderr bool) (string, error) {
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

// RunCommand returns the commands space-trimmed standard output on success
func RunCommand(args []string) (string, error) {
	return RunCommandFull(args, false)
}
