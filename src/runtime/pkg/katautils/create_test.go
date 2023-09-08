// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
// Copyright (c) 2021 Adobe Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
	"syscall"
	"testing"

	config "github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

const (
	testContainerTypeAnnotation = "io.kubernetes.cri-o.ContainerType"
	testSandboxIDAnnotation     = "io.kubernetes.cri-o.SandboxID"
	testContainerTypeContainer  = "container"
)

var (
	// testingImpl is a concrete mock RVC implementation used for testing
	testingImpl = &vcmock.VCMock{}
	// mock sandbox
	mockSandbox = &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	tc ktu.TestConstraint
)

func init() {
	tc = ktu.NewTestConstraint(false)
}

// newTestRuntimeConfig creates a new RuntimeConfig
func newTestRuntimeConfig(dir string, create bool) (oci.RuntimeConfig, error) {
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
		HypervisorMachineType: "q35",
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

	return "", fmt.Errorf("no param called %q found", NAME)
}

func TestSetEphemeralStorageType(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)

	dir := t.TempDir()

	ephePath := filepath.Join(dir, vc.K8sEmptyDir, "tmp-volume")
	err := os.MkdirAll(ephePath, testDirMode)
	assert.Nil(err)

	err = syscall.Mount("tmpfs", ephePath, "tmpfs", 0, "")
	assert.Nil(err)
	defer syscall.Unmount(ephePath, 0)

	ociSpec := specs.Spec{}
	var ociMounts []specs.Mount
	mount := specs.Mount{
		Source: ephePath,
	}

	ociMounts = append(ociMounts, mount)
	ociSpec.Mounts = ociMounts
	ociSpec = SetEphemeralStorageType(ociSpec, false)

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

	config.HypervisorConfig.BlockDeviceDriver = "virtio-scsi"
	err = SetKernelParams(&config)
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

	tmpdir, bundlePath, _ := ktu.SetupOCIConfigFile(t)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, true)
	assert.NoError(err)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
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

	rootFs := vc.RootFs{Mounted: true}

	_, _, err = CreateSandbox(context.Background(), testingImpl, spec, runtimeConfig, rootFs, testContainerID, bundlePath, true, true)
	assert.Error(err)
}

func TestCreateSandboxFail(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)

	tmpdir, bundlePath, _ := ktu.SetupOCIConfigFile(t)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, true)
	assert.NoError(err)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	rootFs := vc.RootFs{Mounted: true}

	_, _, err = CreateSandbox(context.Background(), testingImpl, spec, runtimeConfig, rootFs, testContainerID, bundlePath, true, true)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))
}

func TestCreateSandboxAnnotations(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)

	tmpdir, bundlePath, _ := ktu.SetupOCIConfigFile(t)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, true)
	assert.NoError(err)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	rootFs := vc.RootFs{Mounted: true}

	testingImpl.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig, hookFunc func(context.Context) error) (vc.VCSandbox, error) {
		return &vcmock.Sandbox{
			MockID: testSandboxID,
			MockContainers: []*vcmock.Container{
				{MockID: testContainerID},
			},
			MockAnnotations: sandboxConfig.Annotations,
		}, nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
	}()

	sandbox, _, err := CreateSandbox(context.Background(), testingImpl, spec, runtimeConfig, rootFs, testContainerID, bundlePath, true, true)
	assert.NoError(err)

	netNsPath, err := sandbox.Annotations("nerdctl/network-namespace")
	assert.NoError(err)
	assert.Equal(path.Dir(netNsPath), "/var/run/netns")
}

func TestCheckForFips(t *testing.T) {
	assert := assert.New(t)

	val := procFIPS
	procFIPS = filepath.Join(t.TempDir(), "fips-enabled")
	defer func() {
		procFIPS = val
	}()

	err := os.WriteFile(procFIPS, []byte("1"), 0644)
	assert.NoError(err)

	hconfig := vc.HypervisorConfig{
		KernelParams: []vc.Param{
			{Key: "init", Value: "/sys/init"},
		},
	}
	config := vc.SandboxConfig{
		HypervisorConfig: hconfig,
	}
	assert.NoError(checkForFIPS(&config))

	params := config.HypervisorConfig.KernelParams
	assert.Equal(len(params), 2)
	assert.Equal(params[1].Key, "fips")
	assert.Equal(params[1].Value, "1")

	config.HypervisorConfig = hconfig
	err = os.WriteFile(procFIPS, []byte("unexpected contents"), 0644)
	assert.NoError(err)
	assert.NoError(checkForFIPS(&config))
	assert.Equal(config.HypervisorConfig, hconfig)

	assert.NoError(os.Remove(procFIPS))
	assert.NoError(checkForFIPS(&config))
	assert.Equal(config.HypervisorConfig, hconfig)
}

func TestCreateContainerContainerConfigFail(t *testing.T) {
	assert := assert.New(t)

	_, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	// Set invalid container type
	containerType := "你好，世界"
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = containerType

	// rewrite file
	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	rootFs := vc.RootFs{Mounted: true}

	for _, disableOutput := range []bool{true, false} {
		_, err = CreateContainer(context.Background(), mockSandbox, spec, rootFs, testContainerID, bundlePath, disableOutput, false)
		assert.Error(err)
		assert.False(vcmock.IsMockError(err))
		assert.True(strings.Contains(err.Error(), containerType))
	}
}

func TestCreateContainerFail(t *testing.T) {
	assert := assert.New(t)

	_, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	// set expected container type and sandboxID
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	// rewrite file
	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	rootFs := vc.RootFs{Mounted: true}

	for _, disableOutput := range []bool{true, false} {
		_, err = CreateContainer(context.Background(), mockSandbox, spec, rootFs, testContainerID, bundlePath, disableOutput, false)
		assert.Error(err)
		assert.True(vcmock.IsMockError(err))
	}
}

func TestCreateContainer(t *testing.T) {
	assert := assert.New(t)

	mockSandbox.CreateContainerFunc = func(containerConfig vc.ContainerConfig) (vc.VCContainer, error) {
		return &vcmock.Container{}, nil
	}

	defer func() {
		mockSandbox.CreateContainerFunc = nil
	}()

	_, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	// set expected container type and sandboxID
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	// rewrite file
	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	rootFs := vc.RootFs{Mounted: true}

	for _, disableOutput := range []bool{true, false} {
		_, err = CreateContainer(context.Background(), mockSandbox, spec, rootFs, testContainerID, bundlePath, disableOutput, false)
		assert.NoError(err)
	}
}

func TestVfioChecksClh(t *testing.T) {
	assert := assert.New(t)

	// Check valid CLH vfio configs
	f := func(coldPlug, hotPlug config.PCIePort) error {
		return checkPCIeConfig(coldPlug, hotPlug, defaultMachineType, virtcontainers.ClhHypervisor)
	}
	assert.NoError(f(config.NoPort, config.NoPort))
	assert.NoError(f(config.NoPort, config.RootPort))
	assert.Error(f(config.RootPort, config.RootPort))
	assert.Error(f(config.RootPort, config.NoPort))
	assert.Error(f(config.NoPort, config.SwitchPort))
}

func TestVfioCheckQemu(t *testing.T) {
	assert := assert.New(t)

	// Check valid Qemu vfio configs
	f := func(coldPlug, hotPlug config.PCIePort) error {
		return checkPCIeConfig(coldPlug, hotPlug, defaultMachineType, virtcontainers.QemuHypervisor)
	}

	assert.NoError(f(config.NoPort, config.NoPort))
	assert.NoError(f(config.RootPort, config.NoPort))
	assert.NoError(f(config.NoPort, config.RootPort))
	assert.Error(f(config.RootPort, config.RootPort))
	assert.Error(f(config.SwitchPort, config.RootPort))
}
