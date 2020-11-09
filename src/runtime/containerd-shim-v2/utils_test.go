// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	sysExec "os/exec"
	"path"
	"path/filepath"
	"strings"

	"github.com/opencontainers/runtime-spec/specs-go"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
)

const (
	// specConf is the name of the file holding the containers configuration
	specConf = "config.json"

	TestID = "container_test"

	testDirMode  = os.FileMode(0750)
	testFileMode = os.FileMode(0640)
	// testExeFileMode         = os.FileMode(0750)

	// small docker image used to create root filesystems from
	testDockerImage = "busybox"

	testSandboxID   = "777-77-77777777"
	testContainerID = "42"
	testBundle      = "bundle"
	testConsole     = "/dev/pts/888"

	testContainerTypeAnnotation = "io.kubernetes.cri.container-type"
	testSandboxIDAnnotation     = "io.kubernetes.cri.sandbox-id"
	testContainerTypeSandbox    = "sandbox"
	testContainerTypeContainer  = "container"
)

var (
	// package variables set by calling TestMain()
	testDir       = ""
	testBundleDir = ""
	tc            ktu.TestConstraint
	ctrEngine     = katautils.CtrEngine{}
)

// testingImpl is a concrete mock RVC implementation used for testing
var testingImpl = &vcmock.VCMock{}

func init() {
	fmt.Printf("INFO: running as actual user %v (effective %v), actual group %v (effective %v)\n",
		os.Getuid(), os.Geteuid(), os.Getgid(), os.Getegid())

	fmt.Printf("INFO: switching to fake virtcontainers implementation for testing\n")
	vci = testingImpl

	var err error

	fmt.Printf("INFO: creating test directory\n")
	testDir, err = ioutil.TempDir("", fmt.Sprintf("shimV2-"))
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create test directory: %v", err))
	}
	fmt.Printf("INFO: test directory is %v\n", testDir)

	var output string
	for _, name := range katautils.DockerLikeCtrEngines {
		fmt.Printf("INFO: checking for container engine: %s\n", name)

		output, err = ctrEngine.Init(name)
		if err == nil {
			break
		}
	}

	if ctrEngine.Name == "" {
		panic(fmt.Sprintf("ERROR: Docker-like container engine not accessible to current user: %v (error %v)",
			output, err))
	}

	// Do this now to avoid hitting the test timeout value due to
	// slow network response.
	fmt.Printf("INFO: ensuring required container image (%v) is available\n", testDockerImage)
	// Only hit the network if the image doesn't exist locally
	_, err = ctrEngine.Inspect(testDockerImage)
	if err == nil {
		fmt.Printf("INFO: container image %v already exists locally\n", testDockerImage)
	} else {
		fmt.Printf("INFO: pulling container image %v\n", testDockerImage)
		_, err = ctrEngine.Pull(testDockerImage)
		if err != nil {
			panic(err)
		}
	}

	testBundleDir = filepath.Join(testDir, testBundle)
	err = os.MkdirAll(testBundleDir, testDirMode)
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create bundle directory %v: %v", testBundleDir, err))
	}

	fmt.Printf("INFO: creating OCI bundle in %v for tests to use\n", testBundleDir)
	err = realMakeOCIBundle(testBundleDir)
	if err != nil {
		panic(fmt.Sprintf("ERROR: failed to create OCI bundle: %v", err))
	}

	tc = ktu.NewTestConstraint(false)
}

// createOCIConfig creates an OCI configuration (spec) file in
// the bundle directory specified (which must exist).
func createOCIConfig(bundleDir string) error {
	if bundleDir == "" {
		return errors.New("BUG: Need bundle directory")
	}

	if !katautils.FileExists(bundleDir) {
		return fmt.Errorf("BUG: Bundle directory %s does not exist", bundleDir)
	}

	var configCmd string

	// Search for a suitable version of runc to use to generate
	// the OCI config file.
	for _, cmd := range []string{"docker-runc", "runc"} {
		fullPath, err := sysExec.LookPath(cmd)
		if err == nil {
			configCmd = fullPath
			break
		}
	}

	if configCmd == "" {
		return errors.New("Cannot find command to generate OCI config file")
	}

	_, err := utils.RunCommand([]string{configCmd, "spec", "--bundle", bundleDir})
	if err != nil {
		return err
	}

	specFile := filepath.Join(bundleDir, specConf)
	if !katautils.FileExists(specFile) {
		return fmt.Errorf("generated OCI config file does not exist: %v", specFile)
	}

	return nil
}

func createEmptyFile(path string) (err error) {
	return ioutil.WriteFile(path, []byte(""), testFileMode)
}

// newTestHypervisorConfig creaets a new virtcontainers
// HypervisorConfig, ensuring that the required resources are also
// created.
//
// Note: no parameter validation in case caller wishes to create an invalid
// object.
func newTestHypervisorConfig(dir string, create bool) (vc.HypervisorConfig, error) {
	kernelPath := path.Join(dir, "kernel")
	imagePath := path.Join(dir, "image")
	hypervisorPath := path.Join(dir, "hypervisor")

	if create {
		for _, file := range []string{kernelPath, imagePath, hypervisorPath} {
			err := createEmptyFile(file)
			if err != nil {
				return vc.HypervisorConfig{}, err
			}
		}
	}

	return vc.HypervisorConfig{
		KernelPath:            kernelPath,
		ImagePath:             imagePath,
		HypervisorPath:        hypervisorPath,
		HypervisorMachineType: "pc-lite",
	}, nil
}

// newTestRuntimeConfig creates a new RuntimeConfig
func newTestRuntimeConfig(dir, consolePath string, create bool) (oci.RuntimeConfig, error) {
	if dir == "" {
		return oci.RuntimeConfig{}, errors.New("BUG: need directory")
	}

	hypervisorConfig, err := newTestHypervisorConfig(dir, create)
	if err != nil {
		return oci.RuntimeConfig{}, err
	}

	return oci.RuntimeConfig{
		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,
		Console:          consolePath,
	}, nil
}

// realMakeOCIBundle will create an OCI bundle (including the "config.json"
// config file) in the directory specified (which must already exist).
//
// XXX: Note that tests should *NOT* call this function - they should
// XXX: instead call makeOCIBundle().
func realMakeOCIBundle(bundleDir string) error {
	if bundleDir == "" {
		return errors.New("BUG: Need bundle directory")
	}

	if !katautils.FileExists(bundleDir) {
		return fmt.Errorf("BUG: Bundle directory %v does not exist", bundleDir)
	}

	err := createOCIConfig(bundleDir)
	if err != nil {
		return err
	}

	// Note the unusual parameter (a directory, not the config
	// file to parse!)
	spec, err := compatoci.ParseConfigJSON(bundleDir)
	if err != nil {
		return err
	}

	// Determine the rootfs directory name the OCI config refers to
	ociRootPath := spec.Root.Path

	rootfsDir := filepath.Join(bundleDir, ociRootPath)

	if strings.HasPrefix(ociRootPath, "/") {
		return fmt.Errorf("Cannot handle absolute rootfs as bundle must be unique to each test")
	}

	err = createRootfs(rootfsDir)
	if err != nil {
		return err
	}

	return nil
}

// Create an OCI bundle in the specified directory.
//
// Note that the directory will be created, but it's parent is expected to exist.
//
// This function works by copying the already-created test bundle. Ideally,
// the bundle would be recreated for each test, but createRootfs() uses
// docker which on some systems is too slow, resulting in the tests timing
// out.
func makeOCIBundle(bundleDir string) error {
	from := testBundleDir
	to := bundleDir

	// only the basename of bundleDir needs to exist as bundleDir
	// will get created by cp(1).
	base := filepath.Dir(bundleDir)

	for _, dir := range []string{from, base} {
		if !katautils.FileExists(dir) {
			return fmt.Errorf("BUG: directory %v should exist", dir)
		}
	}

	output, err := utils.RunCommandFull([]string{"cp", "-a", from, to}, true)
	if err != nil {
		return fmt.Errorf("failed to copy test OCI bundle from %v to %v: %v (output: %v)", from, to, err, output)
	}

	return nil
}

// createRootfs creates a minimal root filesystem below the specified
// directory.
func createRootfs(dir string) error {
	err := os.MkdirAll(dir, testDirMode)
	if err != nil {
		return err
	}

	container, err := ctrEngine.Create(testDockerImage)
	if err != nil {
		return err
	}

	err = ctrEngine.GetRootfs(container, dir)
	if err != nil {
		return err
	}

	// Clean up
	_, err = ctrEngine.Rm(container)
	if err != nil {
		return err
	}

	return nil
}

func writeOCIConfigFile(spec specs.Spec, configPath string) error {
	if configPath == "" {
		return errors.New("BUG: need config file path")
	}

	bytes, err := json.MarshalIndent(spec, "", "\t")
	if err != nil {
		return err
	}

	return ioutil.WriteFile(configPath, bytes, testFileMode)
}
