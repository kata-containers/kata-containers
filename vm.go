// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package tests

import (
	"errors"
	"io/ioutil"
	"os"
	"path/filepath"
	"regexp"
)

const procPath = "/proc"

var errFound = errors.New("found")

// IsVMRunning looks in /proc for a hypervisor process that contains
// the containerID in its command line
func IsVMRunning(containerID string) bool {
	err := filepath.Walk(procPath, func(path string, _ os.FileInfo, _ error) error {
		if path == "" {
			return filepath.SkipDir
		}

		info, err := os.Stat(path)
		if err != nil {
			return filepath.SkipDir
		}

		if !info.IsDir() {
			return filepath.SkipDir
		}

		content, err := ioutil.ReadFile(filepath.Join(path, "cmdline"))
		if err != nil {
			return filepath.SkipDir
		}

		hypervisorRegexs := []string{".*/qemu.*-name.*" + containerID + ".*-qmp.*unix:.*/" + containerID + "/.*"}

		for _, regex := range hypervisorRegexs {
			matcher := regexp.MustCompile(regex)
			if matcher.MatchString(string(content)) {
				return errFound
			}
		}

		return nil
	})

	return err == errFound
}
