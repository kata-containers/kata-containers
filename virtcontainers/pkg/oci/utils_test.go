// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package oci

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"reflect"
	"runtime"
	"strconv"
	"testing"

	"github.com/cri-o/cri-o/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
	"golang.org/x/sys/unix"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

const (
	tempBundlePath = "/tmp/virtc/ocibundle/"
	containerID    = "virtc-oci-test"
	consolePath    = "/tmp/virtc/console"
	fileMode       = os.FileMode(0640)
	dirMode        = os.FileMode(0750)

	capabilitiesSpecArray = `
		{
		    "ociVersion": "1.0.0-rc2-dev",
		    "process": {
		        "capabilities": [
		            "CAP_CHOWN",
		            "CAP_DAC_OVERRIDE",
		            "CAP_FSETID"
		        ]
		    }
		}`

	capabilitiesSpecStruct = `
		{
		    "ociVersion": "1.0.0-rc5",
		    "process": {
		        "capabilities": {
		            "bounding": [
		                "CAP_CHOWN",
		                "CAP_DAC_OVERRIDE",
		                "CAP_FSETID"
		            ],
		            "effective": [
		                "CAP_CHOWN",
		                "CAP_DAC_OVERRIDE",
		                "CAP_FSETID"
		            ],
		            "inheritable": [
		                "CAP_CHOWN",
		                "CAP_DAC_OVERRIDE",
		                "CAP_FSETID"
		            ],
		            "permitted": [
		                "CAP_CHOWN",
		                "CAP_DAC_OVERRIDE",
		                "CAP_FSETID"
		            ]
		        }
		    }
		}`
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
	configPath, err := createConfig("config.json", minimalConfig)
	if err != nil {
		t.Fatal(err)
	}

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
		Capabilities: types.LinuxCapabilities{
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

	var minimalOCISpec CompatOCISpec

	//Marshal and unmarshall json to compare  sandboxConfig and expectedSandboxConfig
	if err := json.Unmarshal([]byte(minimalConfig), &minimalOCISpec); err != nil {
		t.Fatal(err)
	}
	if minimalOCISpec.Process != nil {
		caps, err := ContainerCapabilities(minimalOCISpec)
		if err != nil {
			t.Fatal(err)
		}
		minimalOCISpec.Process.Capabilities = caps
	}
	ociSpecJSON, err := json.Marshal(minimalOCISpec)
	if err != nil {
		t.Fatal(err)
	}

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
			vcAnnotations.ConfigJSONKey:    string(ociSpecJSON),
			vcAnnotations.BundlePathKey:    tempBundlePath,
			vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		},
		Mounts:      expectedMounts,
		DeviceInfos: expectedDeviceInfo,
		Resources: specs.LinuxResources{Devices: []specs.LinuxDeviceCgroup{
			{Allow: false, Type: "", Major: (*int64)(nil), Minor: (*int64)(nil), Access: "rwm"},
		}},
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
			vcAnnotations.ConfigJSONKey: string(ociSpecJSON),
			vcAnnotations.BundlePathKey: tempBundlePath,
		},

		SystemdCgroup: true,
	}

	ociSpec, err := ParseConfigJSON(tempBundlePath)
	if err != nil {
		t.Fatalf("Could not parse config.json: %v", err)
	}

	sandboxConfig, err := SandboxConfig(ociSpec, runtimeConfig, tempBundlePath, containerID, consolePath, false, true)
	if err != nil {
		t.Fatalf("Could not create Sandbox configuration %v", err)
	}

	if reflect.DeepEqual(sandboxConfig, expectedSandboxConfig) == false {
		t.Fatalf("Got %v\n expecting %v", sandboxConfig, expectedSandboxConfig)
	}

	if err := os.Remove(configPath); err != nil {
		t.Fatal(err)
	}
}

func testStatusToOCIStateSuccessful(t *testing.T, cStatus vc.ContainerStatus, expected specs.State) {
	ociState := StatusToOCIState(cStatus)

	if reflect.DeepEqual(ociState, expected) == false {
		t.Fatalf("Got %v\n expecting %v", ociState, expected)
	}
}

func TestStatusToOCIStateSuccessfulWithReadyState(t *testing.T) {

	testContID := "testContID"
	testPID := 12345
	testRootFs := "testRootFs"

	state := types.ContainerState{
		State: types.StateReady,
	}

	containerAnnotations := map[string]string{
		vcAnnotations.ConfigJSONKey: minimalConfig,
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
		vcAnnotations.ConfigJSONKey: minimalConfig,
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
		vcAnnotations.ConfigJSONKey: minimalConfig,
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
		vcAnnotations.ConfigJSONKey: minimalConfig,
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

	if ociState := StateToOCIState(state); ociState != "" {
		t.Fatalf("Expecting \"created\" state, got \"%s\"", ociState)
	}

	state = types.StateReady
	if ociState := StateToOCIState(state); ociState != "created" {
		t.Fatalf("Expecting \"created\" state, got \"%s\"", ociState)
	}

	state = types.StateRunning
	if ociState := StateToOCIState(state); ociState != "running" {
		t.Fatalf("Expecting \"created\" state, got \"%s\"", ociState)
	}

	state = types.StateStopped
	if ociState := StateToOCIState(state); ociState != "stopped" {
		t.Fatalf("Expecting \"created\" state, got \"%s\"", ociState)
	}

	state = types.StatePaused
	if ociState := StateToOCIState(state); ociState != "paused" {
		t.Fatalf("Expecting \"paused\" state, got \"%s\"", ociState)
	}
}

func TestEnvVars(t *testing.T) {
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
	if err != nil {
		t.Fatalf("Could not create environment variable slice %v", err)
	}

	if reflect.DeepEqual(vcEnvVars, expectecVcEnvVars) == false {
		t.Fatalf("Got %v\n expecting %v", vcEnvVars, expectecVcEnvVars)
	}
}

func TestMalformedEnvVars(t *testing.T) {
	envVars := []string{"foo"}
	r, err := EnvVars(envVars)
	if err == nil {
		t.Fatalf("EnvVars() succeeded unexpectedly: [%s] variable=%s value=%s", envVars[0], r[0].Var, r[0].Value)
	}

	envVars = []string{"=foo"}
	r, err = EnvVars(envVars)
	if err == nil {
		t.Fatalf("EnvVars() succeeded unexpectedly: [%s] variable=%s value=%s", envVars[0], r[0].Var, r[0].Value)
	}

	envVars = []string{"=foo="}
	r, err = EnvVars(envVars)
	if err == nil {
		t.Fatalf("EnvVars() succeeded unexpectedly: [%s] variable=%s value=%s", envVars[0], r[0].Var, r[0].Value)
	}
}

func TestGetConfigPath(t *testing.T) {
	expected := filepath.Join(tempBundlePath, "config.json")

	configPath := getConfigPath(tempBundlePath)

	if configPath != expected {
		t.Fatalf("Got %s, Expecting %s", configPath, expected)
	}
}

func testGetContainerTypeSuccessful(t *testing.T, annotations map[string]string, expected vc.ContainerType) {
	containerType, err := GetContainerType(annotations)
	if err != nil {
		t.Fatal(err)
	}

	if containerType != expected {
		t.Fatalf("Got %s, Expecting %s", containerType, expected)
	}
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

	containerType, err := GetContainerType(map[string]string{})
	if err == nil {
		t.Fatalf("This test should fail because annotations is empty")
	}

	if containerType != expected {
		t.Fatalf("Got %s, Expecting %s", containerType, expected)
	}
}

func testContainerTypeSuccessful(t *testing.T, ociSpec CompatOCISpec, expected vc.ContainerType) {
	containerType, err := ociSpec.ContainerType()
	if err != nil {
		t.Fatal(err)
	}

	if containerType != expected {
		t.Fatalf("Got %s, Expecting %s", containerType, expected)
	}
}

func TestContainerTypePodSandbox(t *testing.T) {
	var ociSpec CompatOCISpec

	ociSpec.Annotations = map[string]string{
		annotations.ContainerType: annotations.ContainerTypeSandbox,
	}

	testContainerTypeSuccessful(t, ociSpec, vc.PodSandbox)
}

func TestContainerTypePodContainer(t *testing.T) {
	var ociSpec CompatOCISpec

	ociSpec.Annotations = map[string]string{
		annotations.ContainerType: annotations.ContainerTypeContainer,
	}

	testContainerTypeSuccessful(t, ociSpec, vc.PodContainer)
}

func TestContainerTypePodSandboxEmptyAnnotation(t *testing.T) {
	testContainerTypeSuccessful(t, CompatOCISpec{}, vc.PodSandbox)
}

func TestContainerTypeFailure(t *testing.T) {
	var ociSpec CompatOCISpec
	expected := vc.UnknownContainerType
	unknownType := "unknown_type"

	ociSpec.Annotations = map[string]string{
		annotations.ContainerType: unknownType,
	}

	containerType, err := ociSpec.ContainerType()
	if err == nil {
		t.Fatalf("This test should fail because the container type is %s", unknownType)
	}

	if containerType != expected {
		t.Fatalf("Got %s, Expecting %s", containerType, expected)
	}
}

func TestSandboxIDSuccessful(t *testing.T) {
	var ociSpec CompatOCISpec
	testSandboxID := "testSandboxID"

	ociSpec.Annotations = map[string]string{
		annotations.SandboxID: testSandboxID,
	}

	sandboxID, err := ociSpec.SandboxID()
	if err != nil {
		t.Fatal(err)
	}

	if sandboxID != testSandboxID {
		t.Fatalf("Got %s, Expecting %s", sandboxID, testSandboxID)
	}
}

func TestSandboxIDFailure(t *testing.T) {
	var ociSpec CompatOCISpec

	sandboxID, err := ociSpec.SandboxID()
	if err == nil {
		t.Fatalf("This test should fail because annotations is empty")
	}

	if sandboxID != "" {
		t.Fatalf("Got %s, Expecting empty sandbox ID", sandboxID)
	}
}

func TestAddKernelParamValid(t *testing.T) {
	var config RuntimeConfig

	expected := []vc.Param{
		{
			Key:   "foo",
			Value: "bar",
		},
	}

	err := config.AddKernelParam(expected[0])
	if err != nil || reflect.DeepEqual(config.HypervisorConfig.KernelParams, expected) == false {
		t.Fatal()
	}
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
	if err == nil {
		t.Fatal()
	}
}

func TestDeviceTypeFailure(t *testing.T) {
	var ociSpec CompatOCISpec

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
	var ociSpec CompatOCISpec

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

func TestContainerCapabilities(t *testing.T) {
	var ociSpec CompatOCISpec

	ociSpec.Process = &CompatOCIProcess{}
	ociSpec.Process.Capabilities = map[string]interface{}{
		"bounding":    []interface{}{"CAP_KILL"},
		"effective":   []interface{}{"CAP_KILL", "CAP_LEASE"},
		"permitted":   []interface{}{"CAP_SETUID"},
		"inheritable": []interface{}{"CAP_KILL", "CAP_LEASE", "CAP_SYS_ADMIN"},
		"ambient":     []interface{}{""},
	}

	c, err := ContainerCapabilities(ociSpec)
	assert.Nil(t, err)
	assert.Equal(t, c.Bounding, []string{"CAP_KILL"})
	assert.Equal(t, c.Effective, []string{"CAP_KILL", "CAP_LEASE"})
	assert.Equal(t, c.Permitted, []string{"CAP_SETUID"})
	assert.Equal(t, c.Inheritable, []string{"CAP_KILL", "CAP_LEASE", "CAP_SYS_ADMIN"})
	assert.Equal(t, c.Ambient, []string{""})

	ociSpec.Process.Capabilities = []interface{}{"CAP_LEASE", "CAP_SETUID"}

	c, err = ContainerCapabilities(ociSpec)
	assert.Nil(t, err)
	assert.Equal(t, c.Bounding, []string{"CAP_LEASE", "CAP_SETUID"})
	assert.Equal(t, c.Effective, []string{"CAP_LEASE", "CAP_SETUID"})
	assert.Equal(t, c.Permitted, []string{"CAP_LEASE", "CAP_SETUID"})
	assert.Equal(t, c.Inheritable, []string{"CAP_LEASE", "CAP_SETUID"})
	assert.Equal(t, c.Ambient, []string{"CAP_LEASE", "CAP_SETUID"})

	ociSpec.Process.Capabilities = nil

	c, err = ContainerCapabilities(ociSpec)
	assert.Nil(t, err)
	assert.Equal(t, c.Bounding, []string(nil))
	assert.Equal(t, c.Effective, []string(nil))
	assert.Equal(t, c.Permitted, []string(nil))
	assert.Equal(t, c.Inheritable, []string(nil))
	assert.Equal(t, c.Ambient, []string(nil))
}

// use specs.Spec to decode the spec, the content of capabilities is [] string
func TestCompatOCISpecWithArray(t *testing.T) {
	compatOCISpec := CompatOCISpec{}
	err := json.Unmarshal([]byte(capabilitiesSpecArray), &compatOCISpec)
	assert.Nil(t, err, "use CompatOCISpec to decode capabilitiesSpecArray failed")

	ociSpecJSON, err := json.Marshal(compatOCISpec)
	assert.Nil(t, err, "encode compatOCISpec failed")

	// use specs.Spec to decode the spec, specs.Spec' capabilities is struct,
	// but the content of spec' capabilities is [] string
	ociSpec := specs.Spec{}
	err = json.Unmarshal(ociSpecJSON, &ociSpec)
	assert.NotNil(t, err, "This test should fail")

	caps, err := ContainerCapabilities(compatOCISpec)
	assert.Nil(t, err, "decode capabilities failed")
	compatOCISpec.Process.Capabilities = caps

	ociSpecJSON, err = json.Marshal(compatOCISpec)
	assert.Nil(t, err, "encode compatOCISpec failed")

	// capabilities has been chaged to struct
	err = json.Unmarshal(ociSpecJSON, &ociSpec)
	assert.Nil(t, err, "This test should fail")
}

// use specs.Spec to decode the spec, the content of capabilities is struct
func TestCompatOCISpecWithStruct(t *testing.T) {
	compatOCISpec := CompatOCISpec{}
	err := json.Unmarshal([]byte(capabilitiesSpecStruct), &compatOCISpec)
	assert.Nil(t, err, "use CompatOCISpec to decode capabilitiesSpecStruct failed")

	ociSpecJSON, err := json.Marshal(compatOCISpec)
	assert.Nil(t, err, "encode compatOCISpec failed")

	ociSpec := specs.Spec{}
	err = json.Unmarshal(ociSpecJSON, &ociSpec)
	assert.Nil(t, err, "This test should not fail")
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
	/* Create temp bundle directory if necessary */
	err := os.MkdirAll(tempBundlePath, dirMode)
	if err != nil {
		fmt.Printf("Unable to create %s %v\n", tempBundlePath, err)
		os.Exit(1)
	}

	defer os.RemoveAll(tempBundlePath)

	os.Exit(m.Run())
}
