// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package oci

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"

	ctrAnnotations "github.com/containerd/containerd/pkg/cri/annotations"
	podmanAnnotations "github.com/containers/podman/v4/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"golang.org/x/sys/unix"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	dockerAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations/dockershim"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
)

const (
	containerID = "virtc-oci-test"
	fileMode    = os.FileMode(0640)
	dirMode     = os.FileMode(0750)
)

var (
	tempRoot       = ""
	tempBundlePath = ""
)

func createConfig(fileName string, fileData string) (string, error) {
	configPath := path.Join(tempBundlePath, fileName)

	err := os.WriteFile(configPath, []byte(fileData), fileMode)
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
	config.GetHostPathFunc = func(devInfo config.DeviceInfo, vhostUserStoreEnabled bool,
		vhostUserStorePath string) (string, error) {
		return devInfo.ContainerPath, nil
	}

	defer func() {
		config.GetHostPathFunc = savedFunc
	}()

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
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
			vcAnnotations.ContainerTypeKey: string(vc.SingleContainer),
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

		NetworkConfig: expectedNetworkConfig,

		Containers: []vc.ContainerConfig{expectedContainerConfig},

		Annotations: map[string]string{
			vcAnnotations.BundlePathKey: tempBundlePath,
		},

		SystemdCgroup: true,
	}

	sandboxConfig, err := SandboxConfig(spec, runtimeConfig, tempBundlePath, containerID, false, true)
	assert.NoError(err)

	assert.Exactly(sandboxConfig, expectedSandboxConfig)
	assert.NoError(os.Remove(configPath))
}

func TestContainerType(t *testing.T) {
	assert := assert.New(t)
	tests := []struct {
		description     string
		annotationKey   string
		annotationValue string
		expectedType    vc.ContainerType
		expectedErr     bool
	}{
		{
			description:     "no annotation, expect single container",
			annotationKey:   "",
			annotationValue: "",
			expectedType:    vc.SingleContainer,
			expectedErr:     false,
		},
		{
			description:     "unexpected annotation, expect error",
			annotationKey:   ctrAnnotations.ContainerType,
			annotationValue: "foo",
			expectedType:    vc.UnknownContainerType,
			expectedErr:     true,
		},
		{
			description:     "containerd sandbox",
			annotationKey:   ctrAnnotations.ContainerType,
			annotationValue: string(ctrAnnotations.ContainerTypeSandbox),
			expectedType:    vc.PodSandbox,
			expectedErr:     false,
		},
		{
			description:     "containerd container",
			annotationKey:   ctrAnnotations.ContainerType,
			annotationValue: string(ctrAnnotations.ContainerTypeContainer),
			expectedType:    vc.PodContainer,
			expectedErr:     false,
		},
		{
			description:     "crio unexpected annotation, expect error",
			annotationKey:   podmanAnnotations.ContainerType,
			annotationValue: "foo",
			expectedType:    vc.UnknownContainerType,
			expectedErr:     true,
		},
		{
			description:     "crio sandbox",
			annotationKey:   podmanAnnotations.ContainerType,
			annotationValue: string(podmanAnnotations.ContainerTypeSandbox),
			expectedType:    vc.PodSandbox,
			expectedErr:     false,
		},
		{
			description:     "crio container",
			annotationKey:   podmanAnnotations.ContainerType,
			annotationValue: string(podmanAnnotations.ContainerTypeContainer),
			expectedType:    vc.PodContainer,
			expectedErr:     false,
		},
		{
			description:     "dockershim unexpected annotation, expect error",
			annotationKey:   dockerAnnotations.ContainerTypeLabelKey,
			annotationValue: "foo",
			expectedType:    vc.UnknownContainerType,
			expectedErr:     true,
		},
		{
			description:     "dockershim sandbox",
			annotationKey:   dockerAnnotations.ContainerTypeLabelKey,
			annotationValue: string(dockerAnnotations.ContainerTypeLabelSandbox),
			expectedType:    vc.PodSandbox,
			expectedErr:     false,
		},
		{
			description:     "dockershim container",
			annotationKey:   dockerAnnotations.ContainerTypeLabelKey,
			annotationValue: string(dockerAnnotations.ContainerTypeLabelContainer),
			expectedType:    vc.PodContainer,
			expectedErr:     false,
		},
	}
	for _, tt := range tests {
		ociSpec := specs.Spec{
			Annotations: map[string]string{
				tt.annotationKey: tt.annotationValue,
			},
		}
		containerType, err := ContainerType(ociSpec)
		if tt.expectedErr {
			assert.Error(err)
		} else {
			assert.NoError(err)
		}
		assert.Equal(tt.expectedType, containerType, "test fail: %v", tt.description)
	}
}

func TestSandboxIDSuccessful(t *testing.T) {
	var ociSpec specs.Spec
	testSandboxID := "testSandboxID"
	assert := assert.New(t)

	ociSpec.Annotations = map[string]string{
		podmanAnnotations.SandboxID: testSandboxID,
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

	dir := t.TempDir()

	shmPath := filepath.Join(dir, "shm")
	err := os.Mkdir(shmPath, 0700)
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
	tempRoot, err = os.MkdirTemp("", "virtc-")
	if err != nil {
		panic(err)
	}

	tempBundlePath = filepath.Join(tempRoot, "ocibundle")

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

	tmpdir := t.TempDir()

	// Create a pretend asset file
	// (required since the existence of binary asset annotations is verified).
	fakeAssetFile := filepath.Join(tmpdir, "fake-binary")

	err := os.WriteFile(fakeAssetFile, []byte(""), fileMode)
	assert.NoError(err)

	expectedAnnotations := map[string]string{
		vcAnnotations.FirmwarePath: fakeAssetFile,
		vcAnnotations.FirmwareHash: "ffff",

		vcAnnotations.HypervisorPath: fakeAssetFile,
		vcAnnotations.HypervisorHash: "bbbbb",

		vcAnnotations.ImagePath: fakeAssetFile,
		vcAnnotations.ImageHash: "52ss2550983",

		vcAnnotations.InitrdPath: fakeAssetFile,
		vcAnnotations.InitrdHash: "aaaa",

		vcAnnotations.JailerPath: fakeAssetFile,
		vcAnnotations.JailerHash: "dddd",

		vcAnnotations.KernelPath: fakeAssetFile,
		vcAnnotations.KernelHash: "3l2353we871g",
	}

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: expectedAnnotations,
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
	}

	// Try annotations without enabling them first
	err = addAnnotations(ocispec, &config, runtimeConfig)

	assert.Error(err)
	assert.Exactly(map[string]string{}, config.Annotations)

	// Check if annotation not enabled correctly
	runtimeConfig.HypervisorConfig.EnableAnnotations = []string{"nonexistent"}
	err = addAnnotations(ocispec, &config, runtimeConfig)

	assert.Error(err)

	// Ensure it fails if all annotations enabled but path lists are not set
	runtimeConfig.HypervisorConfig.EnableAnnotations = []string{".*"}
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	tmpdirGlob := tmpdir + "/*"

	// Check that it works if all path lists are enabled
	runtimeConfig.HypervisorConfig.HypervisorPathList = []string{tmpdirGlob}
	runtimeConfig.HypervisorConfig.JailerPathList = []string{tmpdirGlob}

	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.NoError(err)
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
		ContainerPipeSize: 1024,
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
	}

	ocispec.Annotations[vcAnnotations.KernelModules] = strings.Join(expectedAgentConfig.KernelModules, KernelModulesSeparator)
	ocispec.Annotations[vcAnnotations.AgentContainerPipeSize] = "1024"
	addAnnotations(ocispec, &config, runtimeConfig)
	assert.Exactly(expectedAgentConfig, config.AgentConfig)
}

func TestContainerPipeSizeAnnotation(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
		AgentConfig: vc.KataAgentConfig{},
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	expectedAgentConfig := vc.KataAgentConfig{
		ContainerPipeSize: 0,
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
	}

	ocispec.Annotations[vcAnnotations.AgentContainerPipeSize] = "foo"
	err := addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)
	assert.Exactly(expectedAgentConfig, config.AgentConfig)
}

func TestAddHypervisorAnnotations(t *testing.T) {
	assert := assert.New(t)

	sbConfig := vc.SandboxConfig{
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

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
	}
	runtimeConfig.HypervisorConfig.EnableAnnotations = []string{".*"}
	runtimeConfig.HypervisorConfig.FileBackedMemRootList = []string{"/dev/shm*"}
	runtimeConfig.HypervisorConfig.VirtioFSDaemonList = []string{"/bin/*ls*"}

	ocispec.Annotations[vcAnnotations.KernelParams] = "vsyscall=emulate iommu=on"
	addHypervisorConfigOverrides(ocispec, &sbConfig, runtimeConfig)
	assert.Exactly(expectedHyperConfig, sbConfig.HypervisorConfig)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMemory] = "1024"
	ocispec.Annotations[vcAnnotations.MemSlots] = "20"
	ocispec.Annotations[vcAnnotations.MemOffset] = "512"
	ocispec.Annotations[vcAnnotations.VirtioMem] = "true"
	ocispec.Annotations[vcAnnotations.MemPrealloc] = "true"
	ocispec.Annotations[vcAnnotations.FileBackedMemRootDir] = "/dev/shm"
	ocispec.Annotations[vcAnnotations.HugePages] = "true"
	ocispec.Annotations[vcAnnotations.IOMMU] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceDriver] = "virtio-scsi"
	ocispec.Annotations[vcAnnotations.BlockDeviceAIO] = "io_uring"
	ocispec.Annotations[vcAnnotations.DisableBlockDeviceUse] = "true"
	ocispec.Annotations[vcAnnotations.EnableIOThreads] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheSet] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheDirect] = "true"
	ocispec.Annotations[vcAnnotations.BlockDeviceCacheNoflush] = "true"
	ocispec.Annotations[vcAnnotations.SharedFS] = "virtio-fs"
	ocispec.Annotations[vcAnnotations.VirtioFSDaemon] = "/bin/false"
	ocispec.Annotations[vcAnnotations.VirtioFSCache] = "auto"
	ocispec.Annotations[vcAnnotations.VirtioFSExtraArgs] = "[ \"arg0\", \"arg1\" ]"
	ocispec.Annotations[vcAnnotations.Msize9p] = "512"
	ocispec.Annotations[vcAnnotations.MachineType] = "q35"
	ocispec.Annotations[vcAnnotations.MachineAccelerators] = "nofw"
	ocispec.Annotations[vcAnnotations.CPUFeatures] = "pmu=off"
	ocispec.Annotations[vcAnnotations.DisableVhostNet] = "true"
	ocispec.Annotations[vcAnnotations.GuestHookPath] = "/usr/bin/"
	ocispec.Annotations[vcAnnotations.DisableImageNvdimm] = "true"
	ocispec.Annotations[vcAnnotations.ColdPlugVFIO] = config.BridgePort
	ocispec.Annotations[vcAnnotations.HotPlugVFIO] = config.NoPort
	ocispec.Annotations[vcAnnotations.PCIeRootPort] = "1"
	ocispec.Annotations[vcAnnotations.PCIeSwitchPort] = "1"
	ocispec.Annotations[vcAnnotations.IOMMUPlatform] = "true"
	ocispec.Annotations[vcAnnotations.SGXEPC] = "64Mi"
	ocispec.Annotations[vcAnnotations.UseLegacySerial] = "true"
	// 10Mbit
	ocispec.Annotations[vcAnnotations.RxRateLimiterMaxRate] = "10000000"
	ocispec.Annotations[vcAnnotations.TxRateLimiterMaxRate] = "10000000"

	err := addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.NoError(err)

	assert.Equal(sbConfig.HypervisorConfig.NumVCPUsF, float32(1))
	assert.Equal(sbConfig.HypervisorConfig.DefaultMaxVCPUs, uint32(1))
	assert.Equal(sbConfig.HypervisorConfig.MemorySize, uint32(1024))
	assert.Equal(sbConfig.HypervisorConfig.MemSlots, uint32(20))
	assert.Equal(sbConfig.HypervisorConfig.MemOffset, uint64(512))
	assert.Equal(sbConfig.HypervisorConfig.VirtioMem, true)
	assert.Equal(sbConfig.HypervisorConfig.MemPrealloc, true)
	assert.Equal(sbConfig.HypervisorConfig.FileBackedMemRootDir, "/dev/shm")
	assert.Equal(sbConfig.HypervisorConfig.HugePages, true)
	assert.Equal(sbConfig.HypervisorConfig.IOMMU, true)
	assert.Equal(sbConfig.HypervisorConfig.BlockDeviceDriver, "virtio-scsi")
	assert.Equal(sbConfig.HypervisorConfig.BlockDeviceAIO, "io_uring")
	assert.Equal(sbConfig.HypervisorConfig.DisableBlockDeviceUse, true)
	assert.Equal(sbConfig.HypervisorConfig.EnableIOThreads, true)
	assert.Equal(sbConfig.HypervisorConfig.BlockDeviceCacheSet, true)
	assert.Equal(sbConfig.HypervisorConfig.BlockDeviceCacheDirect, true)
	assert.Equal(sbConfig.HypervisorConfig.BlockDeviceCacheNoflush, true)
	assert.Equal(sbConfig.HypervisorConfig.SharedFS, "virtio-fs")
	assert.Equal(sbConfig.HypervisorConfig.VirtioFSDaemon, "/bin/false")
	assert.Equal(sbConfig.HypervisorConfig.VirtioFSCache, "auto")
	assert.ElementsMatch(sbConfig.HypervisorConfig.VirtioFSExtraArgs, [2]string{"arg0", "arg1"})
	assert.Equal(sbConfig.HypervisorConfig.Msize9p, uint32(512))
	assert.Equal(sbConfig.HypervisorConfig.HypervisorMachineType, "q35")
	assert.Equal(sbConfig.HypervisorConfig.MachineAccelerators, "nofw")
	assert.Equal(sbConfig.HypervisorConfig.CPUFeatures, "pmu=off")
	assert.Equal(sbConfig.HypervisorConfig.DisableVhostNet, true)
	assert.Equal(sbConfig.HypervisorConfig.GuestHookPath, "/usr/bin/")
	assert.Equal(sbConfig.HypervisorConfig.DisableImageNvdimm, true)
	assert.Equal(string(sbConfig.HypervisorConfig.ColdPlugVFIO), string(config.BridgePort))
	assert.Equal(string(sbConfig.HypervisorConfig.HotPlugVFIO), string(config.NoPort))
	assert.Equal(sbConfig.HypervisorConfig.PCIeRootPort, uint32(1))
	assert.Equal(sbConfig.HypervisorConfig.PCIeSwitchPort, uint32(1))
	assert.Equal(sbConfig.HypervisorConfig.IOMMUPlatform, true)
	assert.Equal(sbConfig.HypervisorConfig.SGXEPCSize, int64(67108864))
	assert.Equal(sbConfig.HypervisorConfig.LegacySerial, true)
	assert.Equal(sbConfig.HypervisorConfig.RxRateLimiterMaxRate, uint64(10000000))
	assert.Equal(sbConfig.HypervisorConfig.TxRateLimiterMaxRate, uint64(10000000))

	// In case an absurd large value is provided, the config value if not over-ridden
	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "655536"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "-1"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "-1"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMemory] = fmt.Sprintf("%d", vc.MinHypervisorMemory+1)
	assert.Error(err)
}

func TestAddRemoteHypervisorAnnotations(t *testing.T) {
	// Remote hypervisor uses DefaultVCPUs, DefaultMemory etc as annotations to pick the size of the separate VM to create,
	// so doesn't need to be bound by the host's capacity limits.
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	sbConfig := vc.SandboxConfig{
		Annotations:    make(map[string]string),
		HypervisorType: vc.RemoteHypervisor,
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.RemoteHypervisor,
	}

	err := addAnnotations(ocispec, &config, runtimeConfig)
	assert.NoError(err)
	assert.Exactly(vc.HypervisorConfig{}, config.HypervisorConfig)

	// Enable annotations
	runtimeConfig.HypervisorConfig.EnableAnnotations = []string{".*"}

	// When DefaultVCPUs is more than the number of cpus on the host, remote hypervisor annotations don't throw an error
	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "2000"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.NoError(err)

	// When DefaultMaxVCPUs is more than the number of cpus on the host, remote hypervisor annotations don't throw an error
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "2000"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.NoError(err)

	// When memory is smaller than the minimum Hypervisor memory, remote hypervisor annotations don't throw an error
	ocispec.Annotations[vcAnnotations.DefaultMemory] = "1"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.NoError(err)

	// When initdata specified, remote hypervisor annotations do have the annotation added.
	ocispec.Annotations[vcAnnotations.Initdata] = "initdata"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.NoError(err)
	assert.Equal(sbConfig.HypervisorConfig.Initdata, "initdata")

	// When GPU annotations are specified, remote hypervisor annotations have the annotation added
	ocispec.Annotations[vcAnnotations.DefaultGPUs] = "-1"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultGPUs] = "1"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.NoError(err)
	assert.Equal(sbConfig.HypervisorConfig.DefaultGPUs, uint32(1))

	// When GPU annotations are specified, remote hypervisor annotations have the annotation added
	ocispec.Annotations[vcAnnotations.DefaultGPUModel] = "tesla"
	err = addAnnotations(ocispec, &sbConfig, runtimeConfig)
	assert.NoError(err)
	assert.Equal(sbConfig.HypervisorConfig.DefaultGPUModel, "tesla")

}

func TestAddProtectedHypervisorAnnotations(t *testing.T) {
	assert := assert.New(t)

	config := vc.SandboxConfig{
		Annotations: make(map[string]string),
	}

	ocispec := specs.Spec{
		Annotations: make(map[string]string),
	}

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
	}
	ocispec.Annotations[vcAnnotations.KernelParams] = "vsyscall=emulate iommu=on"
	err := addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)
	assert.Exactly(vc.HypervisorConfig{}, config.HypervisorConfig)

	// Enable annotations
	runtimeConfig.HypervisorConfig.EnableAnnotations = []string{".*"}

	ocispec.Annotations[vcAnnotations.FileBackedMemRootDir] = "/dev/shm"
	ocispec.Annotations[vcAnnotations.VirtioFSDaemon] = "/bin/false"
	ocispec.Annotations[vcAnnotations.EntropySource] = "/dev/urandom"

	config.HypervisorConfig.FileBackedMemRootDir = "do-not-touch"
	config.HypervisorConfig.VirtioFSDaemon = "dangerous-daemon"
	config.HypervisorConfig.EntropySource = "truly-random"

	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)
	assert.Equal(config.HypervisorConfig.FileBackedMemRootDir, "do-not-touch")
	assert.Equal(config.HypervisorConfig.VirtioFSDaemon, "dangerous-daemon")
	assert.Equal(config.HypervisorConfig.EntropySource, "truly-random")

	// Now enable them and check again
	runtimeConfig.HypervisorConfig.FileBackedMemRootList = []string{"/dev/*m"}
	runtimeConfig.HypervisorConfig.VirtioFSDaemonList = []string{"/bin/*ls*"}
	runtimeConfig.HypervisorConfig.EntropySourceList = []string{"/dev/*random*"}
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.NoError(err)
	assert.Equal(config.HypervisorConfig.FileBackedMemRootDir, "/dev/shm")
	assert.Equal(config.HypervisorConfig.VirtioFSDaemon, "/bin/false")
	assert.Equal(config.HypervisorConfig.EntropySource, "/dev/urandom")

	// In case an absurd large value is provided, the config value if not over-ridden
	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "655536"
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "-1"
	err = addAnnotations(ocispec, &config, runtimeConfig)
	assert.Error(err)

	ocispec.Annotations[vcAnnotations.DefaultVCPUs] = "1"
	ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs] = "-1"
	err = addAnnotations(ocispec, &config, runtimeConfig)
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

	runtimeConfig := RuntimeConfig{
		HypervisorType: vc.QemuHypervisor,
	}

	ocispec.Annotations[vcAnnotations.DisableGuestSeccomp] = "true"
	ocispec.Annotations[vcAnnotations.SandboxCgroupOnly] = "true"
	ocispec.Annotations[vcAnnotations.DisableNewNetNs] = "true"
	ocispec.Annotations[vcAnnotations.InterNetworkModel] = "macvtap"
	ocispec.Annotations[vcAnnotations.CreateContainerTimeout] = "100"
	ocispec.Annotations[vcAnnotations.Initdata] = "initdata"

	addAnnotations(ocispec, &config, runtimeConfig)
	assert.Equal(config.DisableGuestSeccomp, true)
	assert.Equal(config.SandboxCgroupOnly, true)
	assert.Equal(config.NetworkConfig.DisableNewNetwork, true)
	assert.Equal(config.NetworkConfig.InterworkingModel, vc.NetXConnectMacVtapModel)
	assert.Equal(config.CreateContainerTimeout, uint64(100))
	assert.Equal(config.HypervisorConfig.Initdata, "initdata")
}

func TestRegexpContains(t *testing.T) {
	assert := assert.New(t)

	//nolint: govet
	type testData struct {
		regexps  []string
		toMatch  string
		expected bool
	}

	data := []testData{
		{[]string{}, "", false},
		{[]string{}, "nonempty", false},
		{[]string{"simple"}, "simple", true},
		{[]string{"simple"}, "some_simple_text", true},
		{[]string{"simple"}, "simp", false},
		{[]string{"one", "two"}, "one", true},
		{[]string{"one", "two"}, "two", true},
		{[]string{"o*"}, "oooo", true},
		{[]string{"o*"}, "oooa", true},
		{[]string{"^o*$"}, "oooa", false},
	}

	for _, d := range data {
		matched := regexpContains(d.regexps, d.toMatch)
		assert.Equal(d.expected, matched, "%+v", d)
	}
}

func TestCheckPathIsInGlobs(t *testing.T) {
	assert := assert.New(t)

	//nolint: govet
	type testData struct {
		globs    []string
		toMatch  string
		expected bool
	}

	data := []testData{
		{[]string{}, "", false},
		{[]string{}, "nonempty", false},
		{[]string{"simple"}, "simple", false},
		{[]string{"simple"}, "some_simple_text", false},
		{[]string{"/bin/ls"}, "/bin/ls", true},
		{[]string{"/bin/ls", "/bin/false"}, "/bin/ls", true},
		{[]string{"/bin/ls", "/bin/false"}, "/bin/false", true},
		{[]string{"/bin/ls", "/bin/false"}, "/bin/bar", false},
		{[]string{"/bin/*ls*"}, "/bin/ls", true},
		{[]string{"/bin/*ls*"}, "/bin/false", true},
		{[]string{"bin/ls"}, "/bin/ls", false},
		{[]string{"./bin/ls"}, "/bin/ls", false},
		{[]string{"*/bin/ls"}, "/bin/ls", false},
	}

	for _, d := range data {
		matched := checkPathIsInGlobs(d.globs, d.toMatch)
		assert.Equal(d.expected, matched, "%+v", d)
	}
}

func TestIsCRIOContainerManager(t *testing.T) {
	assert := assert.New(t)

	testCases := []struct {
		annotations map[string]string
		result      bool
	}{
		{
			annotations: map[string]string{podmanAnnotations.ContainerType: "abc"},
			result:      false,
		},
		{
			annotations: map[string]string{podmanAnnotations.ContainerType: podmanAnnotations.ContainerTypeSandbox},
			result:      true,
		},
		{
			annotations: map[string]string{podmanAnnotations.ContainerType: podmanAnnotations.ContainerTypeContainer},
			result:      true,
		},
	}

	for i := range testCases {
		tc := testCases[i]
		ocispec := specs.Spec{
			Annotations: tc.annotations,
		}
		result := IsCRIOContainerManager(&ocispec)
		assert.Equal(tc.result, result, "test case %d", (i + 1))
	}
}

func TestParseAnnotationUintConfiguration(t *testing.T) {
	assert := assert.New(t)

	const key = "my_key"

	validErr := fmt.Errorf("invalid value range: must between [10-1000]")
	validFunc := func(v uint64) error {
		if v < 10 || v > 1000 {
			return validErr
		}
		return nil
	}

	// nolint: govet
	testCases := []struct {
		annotations map[string]string
		expected    uint64
		err         error
		validFunc   func(uint64) error
	}{
		{
			annotations: map[string]string{key: ""},
			expected:    0,
			err:         fmt.Errorf(errAnnotationPositiveNumericKey, key),
			validFunc:   nil,
		},
		{
			annotations: map[string]string{key: "a"},
			expected:    0,
			err:         fmt.Errorf(errAnnotationPositiveNumericKey, key),
			validFunc:   nil,
		},
		{
			annotations: map[string]string{key: "16"},
			expected:    16,
			err:         nil,
			validFunc:   nil,
		},
		{
			annotations: map[string]string{key: "16"},
			expected:    16,
			err:         nil,
			validFunc:   validFunc,
		},
		{
			annotations: map[string]string{key: "8"},
			expected:    0,
			err:         validErr,
			validFunc:   validFunc,
		},
		{
			annotations: map[string]string{key: "0"},
			expected:    0,
			err:         nil,
			validFunc:   nil,
		},
		{
			annotations: map[string]string{key: "-1"},
			expected:    0,
			err:         fmt.Errorf(errAnnotationPositiveNumericKey, key),
			validFunc:   nil,
		},
	}

	for i := range testCases {
		tc := testCases[i]
		ocispec := specs.Spec{
			Annotations: tc.annotations,
		}
		var val uint64 = 0

		err := newAnnotationConfiguration(ocispec, key).setUintWithCheck(func(v uint64) error {
			if tc.validFunc != nil {
				if err := tc.validFunc(v); err != nil {
					return err
				}
			}
			val = v
			return nil
		})

		assert.Equal(tc.err, err, "test case %d check error", (i + 1))
		if tc.err == nil {
			assert.Equal(tc.expected, val, "test case %d check parsed result", (i + 1))
		}
	}
}

func TestParseAnnotationBoolConfiguration(t *testing.T) {
	assert := assert.New(t)

	const (
		u32Key  = "u32_key"
		u64Key  = "u64_key"
		boolKey = "bool_key"
	)

	// nolint: govet
	testCases := []struct {
		annotationKey       string
		annotationValueList []string
		expected            bool
		err                 error
	}{
		{
			annotationKey:       boolKey,
			annotationValueList: []string{"1", "t", "T", "true", "TRUE", "True"},
			expected:            true,
			err:                 nil,
		},
		{
			annotationKey:       boolKey,
			annotationValueList: []string{"0", "f", "F", "false", "FALSE", "False"},
			expected:            false,
			err:                 nil,
		},
		{
			annotationKey:       boolKey,
			annotationValueList: []string{"a", "FalSE", "Fal", "TRue", "TRU", "falsE"},
			expected:            false,
			err:                 fmt.Errorf(errAnnotationBoolKey, boolKey),
		},
	}

	for i := range testCases {
		tc := testCases[i]
		for _, annotaionValue := range tc.annotationValueList {
			ocispec := specs.Spec{
				Annotations: map[string]string{tc.annotationKey: annotaionValue},
			}
			var val bool = false

			err := newAnnotationConfiguration(ocispec, tc.annotationKey).setBool(func(v bool) {
				val = v
			})

			assert.Equal(tc.err, err, "test case %d check error", (i + 1))
			if tc.err == nil {
				assert.Equal(tc.expected, val, "test case %d check parsed result", (i + 1))
			}
		}
	}
}

func getCtrResourceSpec(memory, quota int64, period uint64) *specs.Spec {
	return &specs.Spec{
		Linux: &specs.Linux{
			Resources: &specs.LinuxResources{
				CPU: &specs.LinuxCPU{
					Quota:  &quota,
					Period: &period,
				},
				Memory: &specs.LinuxMemory{
					Limit: &memory,
				},
			},
		},
	}

}

func makeSizingAnnotations(memory, quota, period string) *specs.Spec {
	spec := specs.Spec{
		Annotations: make(map[string]string),
	}
	spec.Annotations[ctrAnnotations.SandboxCPUPeriod] = period
	spec.Annotations[ctrAnnotations.SandboxCPUQuota] = quota
	spec.Annotations[ctrAnnotations.SandboxMem] = memory

	return &spec
}

func TestCalculateContainerSizing(t *testing.T) {
	assert := assert.New(t)

	testCases := []struct {
		spec        *specs.Spec
		expectedCPU float32
		expectedMem uint32
	}{
		{
			spec:        nil,
			expectedCPU: 0,
			expectedMem: 0,
		},
		{
			spec:        &specs.Spec{},
			expectedCPU: 0,
			expectedMem: 0,
		},
		{
			spec: &specs.Spec{
				Linux: &specs.Linux{
					Resources: &specs.LinuxResources{
						CPU:    &specs.LinuxCPU{},
						Memory: &specs.LinuxMemory{},
					},
				},
			},
			expectedCPU: 0,
			expectedMem: 0,
		},
		{
			spec:        getCtrResourceSpec(1024*1024, 200, 100),
			expectedCPU: 2,
			expectedMem: 1,
		},
		{
			spec:        getCtrResourceSpec(1024*1024*1024, 200, 1),
			expectedCPU: 200,
			expectedMem: 1024,
		},
		{
			spec:        getCtrResourceSpec(-1*1024*1024*1024, 200, 1),
			expectedCPU: 200,
			expectedMem: 0,
		},
		{
			spec:        getCtrResourceSpec(0, 10, 0),
			expectedCPU: 0,
			expectedMem: 0,
		},
		{
			spec:        getCtrResourceSpec(-1, 10, 1),
			expectedCPU: 10,
			expectedMem: 0,
		},
	}

	for _, tt := range testCases {

		cpu, mem := CalculateContainerSizing(tt.spec)
		assert.Equal(tt.expectedCPU, cpu, "unexpected CPU")
		assert.Equal(tt.expectedMem, mem, "unexpected memory")
	}
}

func TestCalculateSandboxSizing(t *testing.T) {
	assert := assert.New(t)

	testCases := []struct {
		spec        *specs.Spec
		expectedCPU float32
		expectedMem uint32
	}{
		{
			spec:        nil,
			expectedCPU: 0,
			expectedMem: 0,
		},
		{
			spec:        &specs.Spec{},
			expectedCPU: 0,
			expectedMem: 0,
		},
		{
			spec:        makeSizingAnnotations("1048576", "200", "100"),
			expectedCPU: 2,
			expectedMem: 1,
		},
		{
			spec:        makeSizingAnnotations("1024", "200", "1"),
			expectedCPU: 200,
			expectedMem: 0,
		},
		{
			spec:        makeSizingAnnotations("foobar", "200", "spaghetti"),
			expectedCPU: 0,
			expectedMem: 0,
		},
		{
			spec:        makeSizingAnnotations("-1048576", "-100", "1"),
			expectedCPU: 0,
			expectedMem: 0,
		},
		{
			spec:        makeSizingAnnotations("-1", "100", "1"),
			expectedCPU: 100,
			expectedMem: 0,
		},
		{
			spec:        makeSizingAnnotations("4294967296", "400", "100"),
			expectedCPU: 4,
			expectedMem: 4096,
		},
	}

	for _, tt := range testCases {

		cpu, mem := CalculateSandboxSizing(tt.spec)
		assert.Equal(tt.expectedCPU, cpu, "unexpected CPU")
		assert.Equal(tt.expectedMem, mem, "unexpected memory")
	}
}

func TestNewMount(t *testing.T) {
	assert := assert.New(t)

	testCases := []struct {
		out vc.Mount
		in  specs.Mount
	}{
		{
			in: specs.Mount{
				Source:      "proc",
				Destination: "/proc",
				Type:        "proc",
				Options:     nil,
			},
			out: vc.Mount{
				Source:      "proc",
				Destination: "/proc",
				Type:        "proc",
				Options:     nil,
			},
		},
		{
			in: specs.Mount{
				Source:      "proc",
				Destination: "/proc",
				Type:        "proc",
				Options:     []string{"ro"},
			},
			out: vc.Mount{
				Source:      "proc",
				Destination: "/proc",
				Type:        "proc",
				Options:     []string{"ro"},
				ReadOnly:    true,
			},
		},
		{
			in: specs.Mount{
				Source:      "/abc",
				Destination: "/def",
				Type:        "none",
				Options:     []string{"bind"},
			},
			out: vc.Mount{
				Source:      "/abc",
				Destination: "/def",
				Type:        "bind",
				Options:     []string{"bind"},
			},
		}, {
			in: specs.Mount{
				Source:      "/abc",
				Destination: "/def",
				Type:        "none",
				Options:     []string{"rbind"},
			},
			out: vc.Mount{
				Source:      "/abc",
				Destination: "/def",
				Type:        "bind",
				Options:     []string{"rbind"},
			},
		},
	}

	for _, tt := range testCases {
		actualMount := newMount(tt.in)

		assert.Equal(tt.out.Source, actualMount.Source, "unexpected mount source")
		assert.Equal(tt.out.Destination, actualMount.Destination, "unexpected mount destination")
		assert.Equal(tt.out.Type, actualMount.Type, "unexpected mount type")
		assert.Equal(tt.out.Options, actualMount.Options, "unexpected mount options")
		assert.Equal(tt.out.ReadOnly, actualMount.ReadOnly, "unexpected mount ReadOnly")
	}
}
