// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
)

const testSandboxID = "7f49d00d-1995-4156-8c79-5f5ab24ce138"
const testContainerID = "containerID"
const testKernel = "kernel"
const testInitrd = "initrd"
const testImage = "image"
const testHypervisor = "hypervisor"
const testJailer = "jailer"
const testFirmware = "firmware"
const testVirtiofsd = "virtiofsd"
const testBundle = "bundle"

const testDisabledAsNonRoot = "Test disabled as requires root privileges"

// package variables set in TestMain
var testDir = ""
var sandboxDirState = ""
var testQemuKernelPath = ""
var testQemuInitrdPath = ""
var testQemuImagePath = ""
var testQemuPath = ""
var testClhKernelPath = ""
var testClhImagePath = ""
var testClhInitrdPath = ""
var testClhPath = ""
var testStratovirtKernelPath = ""
var testStratovirtImagePath = ""
var testStratovirtInitrdPath = ""
var testStratovirtPath = ""
var testVirtiofsdPath = ""

var testHyperstartCtlSocket = ""
var testHyperstartTtySocket = ""

func setup() {
	os.Mkdir(filepath.Join(testDir, testBundle), DirMode)

	for _, filename := range []string{testQemuKernelPath, testQemuInitrdPath, testQemuImagePath, testQemuPath} {
		_, err := os.Create(filename)
		if err != nil {
			fmt.Printf("Could not recreate %s:%v", filename, err)
			os.Exit(1)
		}
	}
}

func setupClh() {
	os.Mkdir(filepath.Join(testDir, testBundle), DirMode)

	for _, filename := range []string{testClhKernelPath, testClhImagePath, testClhPath, testVirtiofsdPath} {
		_, err := os.Create(filename)
		if err != nil {
			fmt.Printf("Could not recreate %s:%v", filename, err)
			os.Exit(1)
		}
	}
}

func setupStratovirt() {
	os.Mkdir(filepath.Join(testDir, testBundle), DirMode)

	for _, filename := range []string{testStratovirtKernelPath, testStratovirtInitrdPath, testStratovirtPath, testVirtiofsdPath} {
		_, err := os.Create(filename)
		if err != nil {
			fmt.Printf("Could not recreate %s:%v", filename, err)
			os.Exit(1)
		}
	}
}

// TestMain is the common main function used by ALL the test functions
// for this package.
func TestMain(m *testing.M) {
	var err error

	flag.Parse()

	logger := logrus.NewEntry(logrus.New())
	logger.Logger.Level = logrus.ErrorLevel
	for _, arg := range flag.Args() {
		if arg == "debug-logs" {
			logger.Logger.Level = logrus.DebugLevel
		}
	}
	SetLogger(context.Background(), logger)

	testDir, err = os.MkdirTemp("", "vc-tmp-")
	if err != nil {
		panic(err)
	}

	fs.EnableMockTesting(filepath.Join(testDir, "mockfs"))

	fmt.Printf("INFO: Creating virtcontainers test directory %s\n", testDir)
	err = os.MkdirAll(testDir, DirMode)
	if err != nil {
		fmt.Println("Could not create test directories:", err)
		os.Exit(1)
	}

	utils.StartCmd = func(c *exec.Cmd) error {
		//StartVM will Check if the hypervisor is alive and
		// checks for the PID is running, lets fake it using our
		// own PID
		c.Process = &os.Process{Pid: os.Getpid()}
		return nil
	}

	testQemuKernelPath = filepath.Join(testDir, testKernel)
	testQemuInitrdPath = filepath.Join(testDir, testInitrd)
	testQemuImagePath = filepath.Join(testDir, testImage)
	testQemuPath = filepath.Join(testDir, testHypervisor)

	setup()

	testVirtiofsdPath = filepath.Join(testDir, testBundle, testVirtiofsd)
	testClhKernelPath = filepath.Join(testDir, testBundle, testKernel)
	testClhImagePath = filepath.Join(testDir, testBundle, testImage)
	testClhInitrdPath = filepath.Join(testDir, testBundle, testInitrd)
	testClhPath = filepath.Join(testDir, testBundle, testHypervisor)

	setupClh()

	testStratovirtKernelPath = filepath.Join(testDir, testBundle, testKernel)
	testStratovirtImagePath = filepath.Join(testDir, testBundle, testInitrd)
	testStratovirtInitrdPath = filepath.Join(testDir, testBundle, testInitrd)
	testStratovirtPath = filepath.Join(testDir, testBundle, testHypervisor)

	setupStratovirt()

	// set now that configStoragePath has been overridden.
	sandboxDirState = filepath.Join(fs.MockRunStoragePath(), testSandboxID)

	testHyperstartCtlSocket = filepath.Join(testDir, "test_hyper.sock")
	testHyperstartTtySocket = filepath.Join(testDir, "test_tty.sock")

	ret := m.Run()

	os.RemoveAll(testDir)

	os.Exit(ret)
}
