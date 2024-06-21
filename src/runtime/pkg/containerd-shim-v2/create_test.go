// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"os"
	"path"
	"testing"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/containerd/containerd/namespaces"
	"github.com/containerd/containerd/protobuf"
	crioption "github.com/containerd/cri-containerd/pkg/api/runtimeoptions/v1"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/vcmock"
)

func TestCreateSandboxSuccess(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		MockContainers: []*vcmock.Container{
			{MockID: testContainerID},
		},
	}

	testingImpl.CreateSandboxFunc = func(ctx context.Context, sandboxConfig vc.SandboxConfig, hookFunc func(context.Context) error) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
	}()

	tmpdir, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, true)
	assert.NoError(err)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	// Force sandbox-type container
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeSandbox

	// Set a limit to ensure processCgroupsPath() considers the
	// cgroup part of the spec
	limit := int64(1024 * 1024)
	spec.Linux.Resources.Memory = &specs.LinuxMemory{
		Limit: &limit,
	}

	// Rewrite the file
	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	s := &service{
		id:         testSandboxID,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
		ctx:        context.Background(),
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testSandboxID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.NoError(err)
}

func TestCreateSandboxFail(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)

	tmpdir, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, true)
	assert.NoError(err)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	s := &service{
		id:         testSandboxID,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
		ctx:        context.Background(),
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testSandboxID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))
}

func TestCreateSandboxConfigFail(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

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

	// specify an invalid spec
	spec.Linux.Resources.CPU = &specs.LinuxCPU{
		Quota: &quota,
	}

	s := &service{
		id:         testSandboxID,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
		ctx:        context.Background(),
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testSandboxID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))
}

func TestCreateContainerSuccess(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		CreateContainerFunc: func(containerConfig vc.ContainerConfig) (vc.VCContainer, error) {
			return &vcmock.Container{}, nil
		},
	}

	tmpdir, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, true)
	assert.NoError(err)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	// set expected container type and sandboxID
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	// rewrite file
	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	s := &service{
		id:         testContainerID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
		ctx:        context.Background(),
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testContainerID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.NoError(err)
}

func TestCreateContainerFail(t *testing.T) {
	assert := assert.New(t)

	tmpdir, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, true)
	assert.NoError(err)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = testContainerTypeContainer
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	// doesn't create sandbox first
	s := &service{
		id:         testContainerID,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
		ctx:        context.Background(),
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testContainerID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}

func TestCreateContainerConfigFail(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.CreateContainerFunc = func(conf vc.ContainerConfig) (vc.VCContainer, error) {
		return &vcmock.Container{}, nil
	}

	defer func() {
		sandbox.CreateContainerFunc = nil
	}()

	tmpdir, bundlePath, ociConfigFile := ktu.SetupOCIConfigFile(t)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, true)
	assert.NoError(err)

	spec, err := compatoci.ParseConfigJSON(bundlePath)
	assert.NoError(err)

	// set the error containerType
	spec.Annotations = make(map[string]string)
	spec.Annotations[testContainerTypeAnnotation] = "errorType"
	spec.Annotations[testSandboxIDAnnotation] = testSandboxID

	err = ktu.WriteOCIConfigFile(spec, ociConfigFile)
	assert.NoError(err)

	s := &service{
		id:         testContainerID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
		config:     &runtimeConfig,
		ctx:        context.Background(),
	}

	req := &taskAPI.CreateTaskRequest{
		ID:       testContainerID,
		Bundle:   bundlePath,
		Terminal: true,
	}

	ctx := namespaces.WithNamespace(context.Background(), "UnitTest")
	_, err = s.Create(ctx, req)
	assert.Error(err)
}

func createAllRuntimeConfigFiles(dir, hypervisor string) (runtimeConfig string, err error) {
	var hotPlugVFIO config.PCIePort
	var coldPlugVFIO config.PCIePort
	if dir == "" {
		return "", fmt.Errorf("BUG: need directory")
	}

	if hypervisor == "" {
		return "", fmt.Errorf("BUG: need hypervisor")
	}

	hypervisorPath := path.Join(dir, "hypervisor")
	kernelPath := path.Join(dir, "kernel")
	kernelParams := "foo=bar xyz"
	imagePath := path.Join(dir, "image")
	rootfsType := "ext4"
	logDir := path.Join(dir, "logs")
	logPath := path.Join(logDir, "runtime.log")
	machineType := "machineType"
	disableBlockDevice := true
	blockDeviceDriver := "virtio-scsi"
	enableIOThreads := true
	disableNewNetNs := false
	sharedFS := "virtio-9p"
	virtioFSdaemon := path.Join(dir, "virtiofsd")
	hotPlugVFIO = config.BridgePort
	coldPlugVFIO = config.NoPort
	pcieRootPort := uint32(0)
	pcieSwitchPort := uint32(0)

	configFileOptions := ktu.RuntimeConfigOptions{
		Hypervisor:        "qemu",
		HypervisorPath:    hypervisorPath,
		KernelPath:        kernelPath,
		ImagePath:         imagePath,
		RootfsType:        rootfsType,
		KernelParams:      kernelParams,
		MachineType:       machineType,
		LogPath:           logPath,
		DisableBlock:      disableBlockDevice,
		BlockDeviceDriver: blockDeviceDriver,
		EnableIOThreads:   enableIOThreads,
		DisableNewNetNs:   disableNewNetNs,
		SharedFS:          sharedFS,
		VirtioFSDaemon:    virtioFSdaemon,
		HotPlugVFIO:       hotPlugVFIO,
		ColdPlugVFIO:      coldPlugVFIO,
		PCIeRootPort:      pcieRootPort,
		PCIeSwitchPort:    pcieSwitchPort,
	}

	runtimeConfigFileData := ktu.MakeRuntimeConfigFileData(configFileOptions)

	configPath := path.Join(dir, "runtime.toml")
	err = os.WriteFile(configPath, []byte(runtimeConfigFileData), os.FileMode(0640))
	if err != nil {
		return "", err
	}

	files := []string{hypervisorPath, kernelPath, imagePath}

	for _, file := range files {
		// create the resource (which must be >0 bytes)
		err := os.WriteFile(file, []byte("foo"), os.FileMode(0640))
		if err != nil {
			return "", err
		}
	}

	return configPath, nil
}

func TestCreateLoadRuntimeConfig(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	config, err := createAllRuntimeConfigFiles(tmpdir, "qemu")
	assert.NoError(err)

	s := &service{
		id:  testSandboxID,
		ctx: context.Background(),
	}
	r := &taskAPI.CreateTaskRequest{}
	anno := make(map[string]string)

	// set all to fake path
	fakeConfig := "foobar"
	anno[vcAnnotations.SandboxConfigPathKey] = fakeConfig
	option := &crioption.Options{ConfigPath: fakeConfig}
	r.Options, err = protobuf.MarshalAnyToProto(option)
	assert.NoError(err)
	err = os.Setenv("KATA_CONF_FILE", fakeConfig)
	assert.NoError(err)
	defer os.Setenv("KATA_CONF_FILE", "")

	// fake config should fail
	_, err = loadRuntimeConfig(s, r, anno)
	assert.Error(err)

	// 1. podsandbox annotation
	anno[vcAnnotations.SandboxConfigPathKey] = config
	_, err = loadRuntimeConfig(s, r, anno)
	assert.NoError(err)
	anno[vcAnnotations.SandboxConfigPathKey] = ""

	// 2. shimv2 create task option
	option.ConfigPath = config
	r.Options, err = protobuf.MarshalAnyToProto(option)
	assert.NoError(err)
	_, err = loadRuntimeConfig(s, r, anno)
	assert.NoError(err)
	option.ConfigPath = ""
	r.Options, err = protobuf.MarshalAnyToProto(option)
	assert.NoError(err)

	// 3. environment
	err = os.Setenv("KATA_CONF_FILE", config)
	assert.NoError(err)
	_, err = loadRuntimeConfig(s, r, anno)
	assert.NoError(err)
}
