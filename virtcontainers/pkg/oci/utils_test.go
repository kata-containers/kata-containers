// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package oci

import (
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"

	"github.com/cri-o/cri-o/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"golang.org/x/sys/unix"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

const (
	containerID = "virtc-oci-test"
	fileMode    = os.FileMode(0640)
	dirMode     = os.FileMode(0750)
)

var (
	tempRoot       = ""
	tempBundlePath = ""
	consolePath    = ""
)

func createConfig(fileName string, fileData string) (string, error) {
	configPath := path.Join(tempBundlePath, fileName)

	err := ioutil.WriteFile(configPath, []byte(fileData), fileMode)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Unable to create config file %s %v\n", configPath, err)
		return "", err
	}

	return configPath, nil
}

func TestMinimalSandboxConfig(t *testing.T) {
	assert := assert.New(t)
	configPath, err := createConfig("config.json", minimalConfig)
	assert.NoError(err)

	savedFunc := config.GetHostPathFunc

	// Simply assign container path to host path for device.
	config.GetHostPathFunc = func(devInfo config.DeviceInfo) (string, error) {
		return devInfo.ContainerPath, nil
	}

	defer func() {
		config.GetHostPathFunc = savedFunc
	}()

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
		AgentType:      vc.KataContainersAgent,
		ProxyType:      vc.KataProxyType,
		ShimType:       vc.KataShimType,
		Console:        consolePath,
	}

	capList := []string{"CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"}

	expectedCmd := types.Cmd{
		Args: []string{"sh"},
		Envs: []types.EnvVar{
			{
				Var:   "PATH",
				Value: "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
			},
			{
				Var:   "TERM",
				Value: "xterm",
			},
		},
		WorkDir:             "/",
		User:                "0",
		PrimaryGroup:        "0",
		SupplementaryGroups: []string{"10", "29"},
		Interactive:         true,
		Console:             consolePath,
		NoNewPrivileges:     true,
		Capabilities: &specs.LinuxCapabilities{
			Bounding:    capList,
			Effective:   capList,
			Inheritable: capList,
			Permitted:   capList,
			Ambient:     capList,
		},
	}

	expectedMounts := []vc.Mount{
		{
			Source:      "proc",
			Destination: "/proc",
			Type:        "proc",
			Options:     nil,
			HostPath:    "",
		},
		{
			Source:      "tmpfs",
			Destination: "/dev",
			Type:        "tmpfs",
			Options:     []string{"nosuid", "strictatime", "mode=755", "size=65536k"},
			HostPath:    "",
		},
		{
			Source:      "devpts",
			Destination: "/dev/pts",
			Type:        "devpts",
			Options:     []string{"nosuid", "noexec", "newinstance", "ptmxmode=0666", "mode=0620", "gid=5"},
			HostPath:    "",
		},
	}

	spec, err := compatoci.ParseConfigJSON(tempBundlePath)
	assert.NoError(err)

	devInfo := config.DeviceInfo{
		ContainerPath: "/dev/vfio/17",
		Major:         242,
		Minor:         0,
		DevType:       "c",
		UID:           0,
		GID:           0,
	}

	expectedDeviceInfo := []config.DeviceInfo{
		devInfo,
	}

	expectedContainerConfig := vc.ContainerConfig{
		ID:             containerID,
		RootFs:         vc.RootFs{Target: path.Join(tempBundlePath, "rootfs"), Mounted: true},
		ReadonlyRootfs: true,
		Cmd:            expectedCmd,
		Annotations: map[string]string{
			vcAnnotations.BundlePathKey:    tempBundlePath,
			vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		},
		Mounts:      expectedMounts,
		DeviceInfos: expectedDeviceInfo,
		Resources: specs.LinuxResources{Devices: []specs.LinuxDeviceCgroup{
			{Allow: false, Type: "", Major: (*int64)(nil), Minor: (*int64)(nil), Access: "rwm"},
		}},
		CustomSpec: &spec,
	}

	expectedNetworkConfig := vc.NetworkConfig{}

	expectedSandboxConfig := vc.SandboxConfig{
		ID:       containerID,
		Hostname: "testHostname",

		HypervisorType: vc.QemuHypervisor,
		AgentType:      vc.KataContainersAgent,
		ProxyType:      vc.KataProxyType,
		ShimType:       vc.KataShimType,

		NetworkConfig: expectedNetworkConfig,

		Containers: []vc.ContainerConfig{expectedContainerConfig},

		Annotations: map[string]string{
			vcAnnotations.BundlePathKey: tempBundlePath,
		},

		SystemdCgroup: true,
	}

	sandboxConfig, err := SandboxConfig(spec, runtimeConfig, tempBundlePath, containerID, consolePath, false, true)
	assert.NoError(err)

	assert.Exactly(sandboxConfig, expectedSandboxConfig)
	assert.NoError(os.Remove(configPath))
}

func testStatusToOCIStateSuccessful(t *testing.T, cStatus vc.ContainerStatus, expected specs.State) {
	ociState := StatusToOCIState(cStatus)
	assert.Exactly(t, ociState, expected)
}

func TestStatusToOCIStateSuccessfulWithReadyState(t *testing.T) {

	testContID := "testContID"
	testPID := 12345
	testRootFs := "testRootFs"

	state := types.ContainerState{
		State: types.StateReady,
	}

	containerAnnotations := map[string]string{
		vcAnnotations.BundlePathKey: tempBundlePath,
	}

	cStatus := vc.ContainerStatus{
		ID:          testContID,
		State:       state,
		PID:         testPID,
		RootFs:      testRootFs,
		Annotations: containerAnnotations,
	}

	expected := specs.State{
		Version:     specs.Version,
		ID:          testContID,
		Status:      "created",
		Pid:         testPID,
		Bundle:      tempBundlePath,
		Annotations: containerAnnotations,
	}

	testStatusToOCIStateSuccessful(t, cStatus, expected)

}

func TestStatusToOCIStateSuccessfulWithRunningState(t *testing.T) {

	testContID := "testContID"
	testPID := 12345
	testRootFs := "testRootFs"

	state := types.ContainerState{
		State: types.StateRunning,
	}

	containerAnnotations := map[string]string{
		vcAnnotations.BundlePathKey: tempBundlePath,
	}

	cStatus := vc.ContainerStatus{
		ID:          testContID,
		State:       state,
		PID:         testPID,
		RootFs:      testRootFs,
		Annotations: containerAnnotations,
	}

	expected := specs.State{
		Version:     specs.Version,
		ID:          testContID,
		Status:      "running",
		Pid:         testPID,
		Bundle:      tempBundlePath,
		Annotations: containerAnnotations,
	}

	testStatusToOCIStateSuccessful(t, cStatus, expected)

}

func TestStatusToOCIStateSuccessfulWithStoppedState(t *testing.T) {
	testContID := "testContID"
	testPID := 12345
	testRootFs := "testRootFs"

	state := types.ContainerState{
		State: types.StateStopped,
	}

	containerAnnotations := map[string]string{
		vcAnnotations.BundlePathKey: tempBundlePath,
	}

	cStatus := vc.ContainerStatus{
		ID:          testContID,
		State:       state,
		PID:         testPID,
		RootFs:      testRootFs,
		Annotations: containerAnnotations,
	}

	expected := specs.State{
		Version:     specs.Version,
		ID:          testContID,
		Status:      "stopped",
		Pid:         testPID,
		Bundle:      tempBundlePath,
		Annotations: containerAnnotations,
	}

	testStatusToOCIStateSuccessful(t, cStatus, expected)

}

func TestStatusToOCIStateSuccessfulWithNoState(t *testing.T) {
	testContID := "testContID"
	testPID := 12345
	testRootFs := "testRootFs"

	containerAnnotations := map[string]string{
		vcAnnotations.BundlePathKey: tempBundlePath,
	}

	cStatus := vc.ContainerStatus{
		ID:          testContID,
		PID:         testPID,
		RootFs:      testRootFs,
		Annotations: containerAnnotations,
	}

	expected := specs.State{
		Version:     specs.Version,
		ID:          testContID,
		Status:      "",
		Pid:         testPID,
		Bundle:      tempBundlePath,
		Annotations: containerAnnotations,
	}

	testStatusToOCIStateSuccessful(t, cStatus, expected)

}

func TestStateToOCIState(t *testing.T) {
	var state types.StateString
	assert := assert.New(t)

	assert.Empty(StateToOCIState(state))

	state = types.StateReady
	assert.Equal(StateToOCIState(state), "created")

	state = types.StateRunning
	assert.Equal(StateToOCIState(state), "running")

	state = types.StateStopped
	assert.Equal(StateToOCIState(state), "stopped")

	state = types.StatePaused
	assert.Equal(StateToOCIState(state), "paused")
}

func TestEnvVars(t *testing.T) {
	assert := assert.New(t)
	envVars := []string{"foo=bar", "TERM=xterm", "HOME=/home/foo", "TERM=\"bar\"", "foo=\"\""}
	expectecVcEnvVars := []types.EnvVar{
		{
			Var:   "foo",
			Value: "bar",
		},
		{
			Var:   "TERM",
			Value: "xterm",
		},
		{
			Var:   "HOME",
			Value: "/home/foo",
		},
		{
			Var:   "TERM",
			Value: "\"bar\"",
		},
		{
			Var:   "foo",
			Value: "\"\"",
		},
	}

	vcEnvVars, err := EnvVars(envVars)
	assert.NoError(err)
	assert.Exactly(vcEnvVars, expectecVcEnvVars)
}

func TestMalformedEnvVars(t *testing.T) {
	assert := assert.New(t)
	envVars := []string{"foo"}
	_, err := EnvVars(envVars)
	assert.Error(err)

	envVars = []string{"=foo"}
	_, err = EnvVars(envVars)
	assert.Error(err)

	envVars = []string{"=foo="}
	_, err = EnvVars(envVars)
	assert.Error(err)
}

func testGetContainerTypeSuccessful(t *testing.T, annotations map[string]string, expected vc.ContainerType) {
	assert := assert.New(t)
	containerType, err := GetContainerType(annotations)
	assert.NoError(err)
	assert.Equal(containerType, expected)
}

func TestGetContainerTypePodSandbox(t *testing.T) {
	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
	}

	testGetContainerTypeSuccessful(t, annotations, vc.PodSandbox)
}

func TestGetContainerTypePodContainer(t *testing.T) {
	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
	}

	testGetContainerTypeSuccessful(t, annotations, vc.PodContainer)
}

func TestGetContainerTypeFailure(t *testing.T) {
	expected := vc.UnknownContainerType
	assert := assert.New(t)

	containerType, err := GetContainerType(map[string]string{})
	assert.Error(err)
	assert.Equal(containerType, expected)
}

func testContainerTypeSuccessful(t *testing.T, ociSpec specs.Spec, expected vc.ContainerType) {
	containerType, err := ContainerType(ociSpec)
	assert := assert.New(t)

	assert.NoError(err)
	assert.Equal(containerType, expected)
}

func TestContainerTypePodSandbox(t *testing.T) {
	var ociSpec specs.Spec

	ociSpec.Annotations = map[string]string{
		annotations.ContainerType: annotations.ContainerTypeSandbox,
	}

	testContainerTypeSuccessful(t, ociSpec, vc.PodSandbox)
}

func TestContainerTypePodContainer(t *testing.T) {
	var ociSpec specs.Spec

	ociSpec.Annotations = map[string]string{
		annotations.ContainerType: annotations.ContainerTypeContainer,
	}

	testContainerTypeSuccessful(t, ociSpec, vc.PodContainer)
}

func TestContainerTypePodSandboxEmptyAnnotation(t *testing.T) {
	testContainerTypeSuccessful(t, specs.Spec{}, vc.PodSandbox)
}

func TestContainerTypeFailure(t *testing.T) {
	var ociSpec specs.Spec
	expected := vc.UnknownContainerType
	unknownType := "unknown_type"
	assert := assert.New(t)

	ociSpec.Annotations = map[string]string{
		annotations.ContainerType: unknownType,
	}

	containerType, err := ContainerType(ociSpec)
	assert.Error(err)
	assert.Equal(containerType, expected)
}

func TestSandboxIDSuccessful(t *testing.T) {
	var ociSpec specs.Spec
	testSandboxID := "testSandboxID"
	assert := assert.New(t)

	ociSpec.Annotations = map[string]string{
		annotations.SandboxID: testSandboxID,
	}

	sandboxID, err := SandboxID(ociSpec)
	assert.NoError(err)
	assert.Equal(sandboxID, testSandboxID)
}

func TestSandboxIDFailure(t *testing.T) {
	var ociSpec specs.Spec
	assert := assert.New(t)

	sandboxID, err := SandboxID(ociSpec)
	assert.Error(err)
	assert.Empty(sandboxID)
}

func TestAddKernelParamValid(t *testing.T) {
	var config RuntimeConfig
	assert := assert.New(t)

	expected := []vc.Param{
		{
			Key:   "foo",
			Value: "bar",
		},
	}

	err := config.AddKernelParam(expected[0])
	assert.NoError(err)
	assert.Exactly(config.HypervisorConfig.KernelParams, expected)
}

func TestAddKernelParamInvalid(t *testing.T) {
	var config RuntimeConfig

	invalid := []vc.Param{
		{
			Key:   "",
			Value: "bar",
		},
	}

	err := config.AddKernelParam(invalid[0])
	assert.Error(t, err)
}

func TestDeviceTypeFailure(t *testing.T) {
	var ociSpec specs.Spec

	invalidDeviceType := "f"
	ociSpec.Linux = &specs.Linux{}
	ociSpec.Linux.Devices = []specs.LinuxDevice{
		{
			Path: "/dev/vfio",
			Type: invalidDeviceType,
		},
	}

	_, err := containerDeviceInfos(ociSpec)
	assert.NotNil(t, err, "This test should fail as device type [%s] is invalid ", invalidDeviceType)
}

func TestContains(t *testing.T) {
	s := []string{"char", "block", "pipe"}

	assert.True(t, contains(s, "char"))
	assert.True(t, contains(s, "pipe"))
	assert.False(t, contains(s, "chara"))
	assert.False(t, contains(s, "socket"))
}

func TestDevicePathEmpty(t *testing.T) {
	var ociSpec specs.Spec

	ociSpec.Linux = &specs.Linux{}
	ociSpec.Linux.Devices = []specs.LinuxDevice{
		{
			Type:  "c",
			Major: 252,
			Minor: 1,
		},
	}

	_, err := containerDeviceInfos(ociSpec)
	assert.NotNil(t, err, "This test should fail as path cannot be empty for device")
}

func TestGetShmSize(t *testing.T) {
	containerConfig := vc.ContainerConfig{
		Mounts: []vc.Mount{},
	}

	shmSize, err := getShmSize(containerConfig)
	assert.Nil(t, err)
	assert.Equal(t, shmSize, uint64(0))

	m := vc.Mount{
		Source:      "/dev/shm",
		Destination: "/dev/shm",
		Type:        "tmpfs",
		Options:     nil,
	}

	containerConfig.Mounts = append(containerConfig.Mounts, m)
	shmSize, err = getShmSize(containerConfig)
	assert.Nil(t, err)
	assert.Equal(t, shmSize, uint64(vc.DefaultShmSize))

	containerConfig.Mounts[0].Source = "/var/run/shared/shm"
	containerConfig.Mounts[0].Type = "bind"
	_, err = getShmSize(containerConfig)
	assert.NotNil(t, err)
}

func TestGetShmSizeBindMounted(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip("Test disabled as requires root privileges")
	}

	dir, err := ioutil.TempDir("", "")
	assert.Nil(t, err)
	defer os.RemoveAll(dir)

	shmPath := filepath.Join(dir, "shm")
	err = os.Mkdir(shmPath, 0700)
	assert.Nil(t, err)

	size := 8192
	if runtime.GOARCH == "ppc64le" {
		// PAGE_SIZE on ppc64le is 65536
		size = 65536
	}

	shmOptions := "mode=1777,size=" + strconv.Itoa(size)
	err = unix.Mount("shm", shmPath, "tmpfs", unix.MS_NOEXEC|unix.MS_NOSUID|unix.MS_NODEV, shmOptions)
	assert.Nil(t, err)

	defer unix.Unmount(shmPath, 0)

	containerConfig := vc.ContainerConfig{
		Mounts: []vc.Mount{
			{
				Source:      shmPath,
				Destination: "/dev/shm",
				Type:        "bind",
				Options:     nil,
			},
		},
	}

	shmSize, err := getShmSize(containerConfig)
	assert.Nil(t, err)
	assert.Equal(t, shmSize, uint64(size))
}

func TestMain(m *testing.M) {
	var err error
	tempRoot, err = ioutil.TempDir("", "virtc-")
	if err != nil {
		panic(err)
	}

	tempBundlePath = filepath.Join(tempRoot, "ocibundle")
	consolePath = filepath.Join(tempRoot, "console")

	/* Create temp bundle directory if necessary */
	err = os.MkdirAll(tempBundlePath, dirMode)
	if err != nil {
		fmt.Printf("Unable to create %s %v\n", tempBundlePath, err)
		os.Exit(1)
	}

	ret := m.Run()

	os.RemoveAll(tempRoot)

	os.Exit(ret)
}

func TestAddAssetAnnotations(t *testing.T) {
	assert := assert.New(t)

	expectedAnnotations := map[string]string{
		vcAnnotations.KernelPath:    "/abc/rgb/kernel",
		vcAnnotations.ImagePath:     "/abc/rgb/image",
		vcAnnotations.InitrdPath:    "/abc/rgb/initrd",
		vcAnnotations.KernelHash:    "3l2353we871g",
		vcAnnotations.ImageHash:     "52ss2550983",
		vcAnnotations.AssetHashType: "sha",
	}

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: expectedAnnotations,
	}

	addAnnotations(ocispec, &config)
	assert.Exactly(expectedAnnotations, config.Annotations)
}

func TestAddAgentAnnotations(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
		AgentConfig: vc.KataAgentConfig{},
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	expectedAgentConfig := vc.KataAgentConfig{
		KernelModules: []string{
			"e1000e InterruptThrottleRate=3000,3000,3000 EEE=1",
			"i915 enable_ppgtt=0",
		},
	}

	ocispec.Annotations[vcAnnotations.KernelModules] = strings.Join(expectedAgentConfig.KernelModules, KernelModulesSeparator)
	addAnnotations(ocispec, &config)
	assert.Exactly(expectedAgentConfig, config.AgentConfig)
}

func TestAddHypervisorAnnotations(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	expectedHyperConfig := vc.HypervisorConfig{
		KernelParams: []vc.Param{
			{
				Key:   "vsyscall",
				Value: "emulate",
			},
			{
				Key:   "iommu",
				Value: "on",
			},
		},
	}

	ocispec.Annotations[vcAnnotations.KernelParams] = "vsyscall=emulate iommu=on"
	addHypervisorConfigOverrides(ocispec, &config)
	assert.Exactly(expectedHyperConfig, config.HypervisorConfig)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMemory] = "1024"
	ocispec.Annotations[vcAnnotations.MemSlots] = "20"
	ocispec.Annotations[vcAnnotations.MemOffset] = "512"
	ocispec.Annotations[vcAnnotations.MemPrealloc] = "true"
	ocispec.Annotations[vcAnnotations.EnableSwap] = "true"
	ocispec.Annotations[vcAnnotations.FileBackedMemRootDir] = "/dev/shm"
	ocispec.Annotations[vcAnnotations.HugePages] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceDriver] = "virtio-scsi"
	ocispec.Annotations[vcAnnotations.DisableBlockDeviceUse] = "true"
	ocispec.Annotations[vcAnnotations.EnableIOThreads] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheSet] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheDirect] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheNoflush] = "true"
	ocispec.Annotations[vcAnnotations.SharedFS] = "virtio-fs"
	ocispec.Annotations[vcAnnotations.VirtioFSDaemon] = "/home/virtiofsd"
	ocispec.Annotations[vcAnnotations.VirtioFSCache] = "/home/cache"
	ocispec.Annotations[vcAnnotations.Msize9p] = "512"
	ocispec.Annotations[vcAnnotations.MachineType] = "q35"
	ocispec.Annotations[vcAnnotations.MachineAccelerators] = "nofw"
	ocispec.Annotations[vcAnnotations.DisableVhostNet] = "true"
	ocispec.Annotations[vcAnnotations.GuestHookPath] = "/usr/bin/"
	ocispec.Annotations[vcAnnotations.UseVSock] = "true"
	ocispec.Annotations[vcAnnotations.DisableImageNvdimm] = "true"
	ocispec.Annotations[vcAnnotations.HotplugVFIOOnRootBus] = "true"
	ocispec.Annotations[vcAnnotations.EntropySource] = "/dev/urandom"

	addAnnotations(ocispec, &config)
	assert.Equal(config.HypervisorConfig.NumVCPUs, uint32(1))
	assert.Equal(config.HypervisorConfig.DefaultMaxVCPUs, uint32(1))
	assert.Equal(config.HypervisorConfig.MemorySize, uint32(1024))
	assert.Equal(config.HypervisorConfig.MemSlots, uint32(20))
	assert.Equal(config.HypervisorConfig.MemOffset, uint32(512))
	assert.Equal(config.HypervisorConfig.MemPrealloc, true)
	assert.Equal(config.HypervisorConfig.Mlock, false)
	assert.Equal(config.HypervisorConfig.FileBackedMemRootDir, "/dev/shm")
	assert.Equal(config.HypervisorConfig.HugePages, true)
	assert.Equal(config.HypervisorConfig.BlockDeviceDriver, "virtio-scsi")
	assert.Equal(config.HypervisorConfig.DisableBlockDeviceUse, true)
	assert.Equal(config.HypervisorConfig.EnableIOThreads, true)
	assert.Equal(config.HypervisorConfig.BlockDeviceCacheSet, true)
	assert.Equal(config.HypervisorConfig.BlockDeviceCacheDirect, true)
	assert.Equal(config.HypervisorConfig.BlockDeviceCacheNoflush, true)
	assert.Equal(config.HypervisorConfig.SharedFS, "virtio-fs")
	assert.Equal(config.HypervisorConfig.VirtioFSDaemon, "/home/virtiofsd")
	assert.Equal(config.HypervisorConfig.VirtioFSCache, "/home/cache")
	assert.Equal(config.HypervisorConfig.Msize9p, uint32(512))
	assert.Equal(config.HypervisorConfig.HypervisorMachineType, "q35")
	assert.Equal(config.HypervisorConfig.MachineAccelerators, "nofw")
	assert.Equal(config.HypervisorConfig.DisableVhostNet, true)
	assert.Equal(config.HypervisorConfig.GuestHookPath, "/usr/bin/")
	assert.Equal(config.HypervisorConfig.UseVSock, true)
	assert.Equal(config.HypervisorConfig.DisableImageNvdimm, true)
	assert.Equal(config.HypervisorConfig.HotplugVFIOOnRootBus, true)
	assert.Equal(config.HypervisorConfig.EntropySource, "/dev/urandom")

	// In case an absurd large value is provided, the config value if not over-ridden
	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "655536"
	err := addAnnotations(ocispec, &config)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "-1"
	err = addAnnotations(ocispec, &config)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "-1"
	err = addAnnotations(ocispec, &config)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMemory] = fmt.Sprintf("%d", vc.MinHypervisorMemory+1)
	assert.Error(err)
}

func TestAddRuntimeAnnotations(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	ocispec.Annotations[vcAnnotations.DisableGuestSeccomp] = "true"
	ocispec.Annotations[vcAnnotations.SandboxCgroupOnly] = "true"
	ocispec.Annotations[vcAnnotations.DisableNewNetNs] = "true"
	ocispec.Annotations[vcAnnotations.InterNetworkModel] = "macvtap"

	addAnnotations(ocispec, &config)
	assert.Equal(config.DisableGuestSeccomp, true)
	assert.Equal(config.SandboxCgroupOnly, true)
	assert.Equal(config.NetworkConfig.DisableNewNetNs, true)
	assert.Equal(config.NetworkConfig.InterworkingModel, vc.NetXConnectMacVtapModel)
}
