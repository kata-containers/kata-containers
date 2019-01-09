// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"strings"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

const (
	testConsole                 = "/dev/pts/999"
	testContainerTypeAnnotation = "io.kubernetes.cri-o.ContainerType"
	testSandboxIDAnnotation     = "io.kubernetes.cri-o.SandboxID"
	testContainerTypeContainer  = "container"
)

var (
	testBundleDir = ""

	// testingImpl is a concrete mock RVC implementation used for testing
	testingImpl = &vcmock.VCMock{}
)

// readOCIConfig returns an OCI spec.
func readOCIConfigFile(configPath string) (oci.CompatOCISpec, error) {
	if configPath == "" {
		return oci.CompatOCISpec{}, errors.New("BUG: need config file path")
	}

	data, err := ioutil.ReadFile(configPath)
	if err != nil {
		return oci.CompatOCISpec{}, err
	}

	var ociSpec oci.CompatOCISpec
	if err := json.Unmarshal(data, &ociSpec); err != nil {
		return oci.CompatOCISpec{}, err
	}
	caps, err := oci.ContainerCapabilities(ociSpec)
	if err != nil {
		return oci.CompatOCISpec{}, err
	}
	ociSpec.Process.Capabilities = caps
	return ociSpec, nil
}

func writeOCIConfigFile(spec oci.CompatOCISpec, configPath string) error {
	if configPath == "" {
		return errors.New("BUG: need config file path")
	}

	bytes, err := json.MarshalIndent(spec, "", "\t")
	if err != nil {
		return err
	}

	return ioutil.WriteFile(configPath, bytes, testFileMode)
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
		if !FileExists(dir) {
			return fmt.Errorf("BUG: directory %v should exist", dir)
		}
	}

	output, err := RunCommandFull([]string{"cp", "-a", from, to}, true)
	if err != nil {
		return fmt.Errorf("failed to copy test OCI bundle from %v to %v: %v (output: %v)", from, to, err, output)
	}

	return nil
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
		AgentType:        vc.KataContainersAgent,
		ProxyType:        vc.CCProxyType,
		ShimType:         vc.CCShimType,
		Console:          consolePath,
	}, nil
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

// return the value of the *last* param with the specified key
func findLastParam(key string, params []vc.Param) (string, error) {
	if key == "" {
		return "", errors.New("ERROR: need non-nil key")
	}

	l := len(params)
	if l == 0 {
		return "", errors.New("ERROR: no params")
	}

	for i := l - 1; i >= 0; i-- {
		p := params[i]

		if key == p.Key {
			return p.Value, nil
		}
	}

	return "", fmt.Errorf("no param called %q found", name)
}

func TestSetEphemeralStorageType(t *testing.T) {
	assert := assert.New(t)

	ociSpec := oci.CompatOCISpec{}
	var ociMounts []specs.Mount
	mount := specs.Mount{
		Source: "/var/lib/kubelet/pods/366c3a77-4869-11e8-b479-507b9ddd5ce4/volumes/kubernetes.io~empty-dir/cache-volume",
	}

	ociMounts = append(ociMounts, mount)
	ociSpec.Mounts = ociMounts
	ociSpec = SetEphemeralStorageType(ociSpec)

	mountType := ociSpec.Mounts[0].Type
	assert.Equal(mountType, "ephemeral",
		"Unexpected mount type, got %s expected ephemeral", mountType)
}

func TestSetKernelParams(t *testing.T) {
	assert := assert.New(t)

	config := oci.RuntimeConfig{}

	assert.Empty(config.HypervisorConfig.KernelParams)

	err := SetKernelParams(&config)
	assert.NoError(err)

	if needSystemd(config.HypervisorConfig) {
		assert.NotEmpty(config.HypervisorConfig.KernelParams)
	}
}

func TestSetKernelParamsUserOptionTakesPriority(t *testing.T) {
	assert := assert.New(t)

	initName := "init"
	initValue := "/sbin/myinit"

	ipName := "ip"
	ipValue := "127.0.0.1"

	params := []vc.Param{
		{Key: initName, Value: initValue},
		{Key: ipName, Value: ipValue},
	}

	hypervisorConfig := vc.HypervisorConfig{
		KernelParams: params,
	}

	// Config containing user-specified kernel parameters
	config := oci.RuntimeConfig{
		HypervisorConfig: hypervisorConfig,
	}

	assert.NotEmpty(config.HypervisorConfig.KernelParams)

	err := SetKernelParams(&config)
	assert.NoError(err)

	kernelParams := config.HypervisorConfig.KernelParams

	init, err := findLastParam(initName, kernelParams)
	assert.NoError(err)
	assert.Equal(initValue, init)

	ip, err := findLastParam(ipName, kernelParams)
	assert.NoError(err)
	assert.Equal(ipValue, ip)

}

func TestCreateSandboxConfigFail(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	quota := int64(0)
	limit := int64(0)

	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	spec.Linux.Resources.CPU = &specs.LinuxCPU{
		// specify an invalid value
		Quota: &quota,
	}

	_, _, err = CreateSandbox(context.Background(), testingImpl, spec, runtimeConfig, testContainerID, bundlePath, testConsole, true, true, false)
	assert.Error(err)
}

func TestCreateSandboxFail(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	_, _, err = CreateSandbox(context.Background(), testingImpl, spec, runtimeConfig, testContainerID, bundlePath, testConsole, true, true, false)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))
}

func TestCreateContainerContainerConfigFail(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// Set invalid container type
	containerType := "你好，世界"
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = containerType

	// rewrite file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for _, disableOutput := range []bool{true, false} {
		_, err = CreateContainer(context.Background(), testingImpl, nil, spec, testContainerID, bundlePath, testConsole, disableOutput, false)
		assert.Error(err)
		assert.False(vcmock.IsMockError(err))
		assert.True(strings.Contains(err.Error(), containerType))
		os.RemoveAll(path)
	}
}

func TestCreateContainerFail(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// set expected container type and sandboxID
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	// rewrite file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for _, disableOutput := range []bool{true, false} {
		_, err = CreateContainer(context.Background(), testingImpl, nil, spec, testContainerID, bundlePath, testConsole, disableOutput, false)
		assert.Error(err)
		assert.True(vcmock.IsMockError(err))
		os.RemoveAll(path)
	}
}

func TestCreateContainer(t *testing.T) {
	assert := assert.New(t)

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	testingImpl.CreateContainerFunc = func(ctx context.Context, sandboxID string, containerConfig vc.ContainerConfig) (vc.VCSandbox, vc.VCContainer, error) {
		return &vcmock.Sandbox{}, &vcmock.Container{}, nil
	}

	defer func() {
		testingImpl.CreateContainerFunc = nil
	}()

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	ociConfigFile := filepath.Join(bundlePath, "config.json")
	assert.True(FileExists(ociConfigFile))

	spec, err := readOCIConfigFile(ociConfigFile)
	assert.NoError(err)

	// set expected container type and sandboxID
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	// rewrite file
	err = writeOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	for _, disableOutput := range []bool{true, false} {
		_, err = CreateContainer(context.Background(), testingImpl, nil, spec, testContainerID, bundlePath, testConsole, disableOutput, false)
		assert.NoError(err)
		os.RemoveAll(path)
	}
}
