// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
)

const (
	k8sEmptyDir = "kubernetes.io~empty-dir"
)

// FileExists test is a file exiting or not
func FileExists(path string) bool {
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
