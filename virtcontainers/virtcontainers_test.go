// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"flag"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/sirupsen/logrus"
)

const testSandboxID = "7f49d00d-1995-4156-8c79-5f5ab24ce138"
const testContainerID = "containerID"
const testKernel = "kernel"
const testInitrd = "initrd"
const testImage = "image"
const testHypervisor = "hypervisor"
const testBundle = "bundle"

const testDisabledAsNonRoot = "Test disabled as requires root privileges"

// package variables set in TestMain
var testDir = ""
var sandboxDirConfig = ""
var sandboxFileConfig = ""
var sandboxDirState = ""
var sandboxDirLock = ""
var sandboxFileState = ""
var sandboxFileLock = ""
var testQemuKernelPath = ""
var testQemuInitrdPath = ""
var testQemuImagePath = ""
var testQemuPath = ""
var testHyperstartCtlSocket = ""
var testHyperstartTtySocket = ""

// cleanUp Removes any stale sandbox/container state that can affect
// the next test to run.
func cleanUp() {
	globalSandboxList.removeSandbox(testSandboxID)
	for _, dir := range []string{testDir, defaultSharedDir} {
		os.RemoveAll(dir)
		os.MkdirAll(dir, dirMode)
	}

	os.Mkdir(filepath.Join(testDir, testBundle), dirMode)

	_, err := os.Create(filepath.Join(testDir, testImage))
	if err != nil {
		fmt.Println("Could not recreate test image:", err)
		os.Exit(1)
	}
}

// TestMain is the common main function used by ALL the test functions
// for this package.
func TestMain(m *testing.M) {
	var err error

	flag.Parse()

	logger := logrus.New()
	logger.Level = logrus.ErrorLevel
	for _, arg := range flag.Args() {
		if arg == "debug-logs" {
			logger.Level = logrus.DebugLevel
		}
	}
	SetLogger(logger)

	testDir, err = ioutil.TempDir("", "vc-tmp-")
	if err != nil {
		panic(err)
	}

	err = os.MkdirAll(testDir, dirMode)
	if err != nil {
		fmt.Println("Could not create test directories:", err)
		os.Exit(1)
	}

	_, err = os.Create(filepath.Join(testDir, testKernel))
	if err != nil {
		fmt.Println("Could not create test kernel:", err)
		os.RemoveAll(testDir)
		os.Exit(1)
	}

	_, err = os.Create(filepath.Join(testDir, testImage))
	if err != nil {
		fmt.Println("Could not create test image:", err)
		os.RemoveAll(testDir)
		os.Exit(1)
	}

	_, err = os.Create(filepath.Join(testDir, testHypervisor))
	if err != nil {
		fmt.Println("Could not create test hypervisor:", err)
		os.RemoveAll(testDir)
		os.Exit(1)
	}

	err = os.Mkdir(filepath.Join(testDir, testBundle), dirMode)
	if err != nil {
		fmt.Println("Could not create test bundle directory:", err)
		os.RemoveAll(testDir)
		os.Exit(1)
	}

	// allow the tests to run without affecting the host system.
	configStoragePath = filepath.Join(testDir, storagePathSuffix, "config")
	runStoragePath = filepath.Join(testDir, storagePathSuffix, "run")

	// set now that configStoragePath has been overridden.
	sandboxDirConfig = filepath.Join(configStoragePath, testSandboxID)
	sandboxFileConfig = filepath.Join(configStoragePath, testSandboxID, configFile)
	sandboxDirState = filepath.Join(runStoragePath, testSandboxID)
	sandboxDirLock = filepath.Join(runStoragePath, testSandboxID)
	sandboxFileState = filepath.Join(runStoragePath, testSandboxID, stateFile)
	sandboxFileLock = filepath.Join(runStoragePath, testSandboxID, lockFileName)

	testQemuKernelPath = filepath.Join(testDir, testKernel)
	testQemuInitrdPath = filepath.Join(testDir, testInitrd)
	testQemuImagePath = filepath.Join(testDir, testImage)
	testQemuPath = filepath.Join(testDir, testHypervisor)

	testHyperstartCtlSocket = filepath.Join(testDir, "test_hyper.sock")
	testHyperstartTtySocket = filepath.Join(testDir, "test_tty.sock")

	ret := m.Run()

	os.RemoveAll(testDir)

	os.Exit(ret)
}
