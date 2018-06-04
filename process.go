// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package tests

import (
	"errors"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
	"regexp"
)

const procPath = "/proc"

var errFound = errors.New("found")

// processRunning looks for a process in /proc that matches with the regexps
func processRunning(regexps []string) bool {
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

		for _, r := range regexps {
			matcher := regexp.MustCompile(r)
			if matcher.MatchString(string(content)) {
				return errFound
			}
		}

		return nil
	})

	return err == errFound
}

// HypervisorRunning returns true if the hypervisor is still running, otherwise false
func HypervisorRunning(containerID string) bool {
	hypervisorPath := KataConfig.Hypervisor[DefaultHypervisor].Path
	if hypervisorPath == "" {
		log.Fatal("Could not determine if hypervisor is running: hypervisor path is empty")
		return false
	}
	hypervisorRegexps := []string{hypervisorPath + ".*-name.*" + containerID + ".*-qmp.*unix:.*/" + containerID + "/.*"}
	return processRunning(hypervisorRegexps)
}
