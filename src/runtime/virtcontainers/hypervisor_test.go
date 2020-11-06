// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func testSetHypervisorType(t *testing.T, value string, expected HypervisorType) {
	var hypervisorType HypervisorType
	assert := assert.New(t)

	err := (&hypervisorType).Set(value)
	assert.NoError(err)
	assert.Equal(hypervisorType, expected)
}

func TestSetQemuHypervisorType(t *testing.T) {
	testSetHypervisorType(t, "qemu", QemuHypervisor)
}

func TestSetMockHypervisorType(t *testing.T) {
	testSetHypervisorType(t, "mock", MockHypervisor)
}

func TestSetUnknownHypervisorType(t *testing.T) {
	var hypervisorType HypervisorType
	assert := assert.New(t)

	err := (&hypervisorType).Set("unknown")
	assert.Error(err)
	assert.NotEqual(hypervisorType, QemuHypervisor)
	assert.NotEqual(hypervisorType, MockHypervisor)
}

func testStringFromHypervisorType(t *testing.T, hypervisorType HypervisorType, expected string) {
	hypervisorTypeStr := (&hypervisorType).String()
	assert := assert.New(t)
	assert.Equal(hypervisorTypeStr, expected)
}

func TestStringFromQemuHypervisorType(t *testing.T) {
	hypervisorType := QemuHypervisor
	testStringFromHypervisorType(t, hypervisorType, "qemu")
}

func TestStringFromMockHypervisorType(t *testing.T) {
	hypervisorType := MockHypervisor
	testStringFromHypervisorType(t, hypervisorType, "mock")
}

func TestStringFromUnknownHypervisorType(t *testing.T) {
	var hypervisorType HypervisorType
	testStringFromHypervisorType(t, hypervisorType, "")
}

func testNewHypervisorFromHypervisorType(t *testing.T, hypervisorType HypervisorType, expected hypervisor) {
	assert := assert.New(t)
	hy, err := newHypervisor(hypervisorType)
	assert.NoError(err)
	assert.Exactly(hy, expected)
}

func TestNewHypervisorFromMockHypervisorType(t *testing.T) {
	hypervisorType := MockHypervisor
	expectedHypervisor := &mockHypervisor{}
	testNewHypervisorFromHypervisorType(t, hypervisorType, expectedHypervisor)
}

func TestNewHypervisorFromUnknownHypervisorType(t *testing.T) {
	var hypervisorType HypervisorType
	assert := assert.New(t)

	hy, err := newHypervisor(hypervisorType)
	assert.Error(err)
	assert.Nil(hy)
}

func testHypervisorConfigValid(t *testing.T, hypervisorConfig *HypervisorConfig, success bool) {
	err := hypervisorConfig.valid()
	assert := assert.New(t)
	assert.False(success && err != nil)
	assert.False(!success && err == nil)
}

func TestHypervisorConfigNoKernelPath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     "",
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigNoImagePath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      "",
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigNoHypervisorPath(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: "",
	}

	testHypervisorConfigValid(t, hypervisorConfig, true)
}

func TestHypervisorConfigIsValid(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: fmt.Sprintf("%s/%s", testDir, testHypervisor),
	}

	testHypervisorConfigValid(t, hypervisorConfig, true)
}

func TestHypervisorConfigValidTemplateConfig(t *testing.T) {
	hypervisorConfig := &HypervisorConfig{
		KernelPath:       fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:        fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath:   fmt.Sprintf("%s/%s", testDir, testHypervisor),
		BootToBeTemplate: true,
		BootFromTemplate: true,
	}
	testHypervisorConfigValid(t, hypervisorConfig, false)

	hypervisorConfig.BootToBeTemplate = false
	testHypervisorConfigValid(t, hypervisorConfig, false)
	hypervisorConfig.MemoryPath = "foobar"
	testHypervisorConfigValid(t, hypervisorConfig, false)
	hypervisorConfig.DevicesStatePath = "foobar"
	testHypervisorConfigValid(t, hypervisorConfig, true)

	hypervisorConfig.BootFromTemplate = false
	hypervisorConfig.BootToBeTemplate = true
	testHypervisorConfigValid(t, hypervisorConfig, true)
	hypervisorConfig.MemoryPath = ""
	testHypervisorConfigValid(t, hypervisorConfig, false)
}

func TestHypervisorConfigDefaults(t *testing.T) {
	assert := assert.New(t)
	hypervisorConfig := &HypervisorConfig{
		KernelPath:     fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:      fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath: "",
	}
	testHypervisorConfigValid(t, hypervisorConfig, true)

	hypervisorConfigDefaultsExpected := &HypervisorConfig{
		KernelPath:        fmt.Sprintf("%s/%s", testDir, testKernel),
		ImagePath:         fmt.Sprintf("%s/%s", testDir, testImage),
		HypervisorPath:    "",
		NumVCPUs:          defaultVCPUs,
		MemorySize:        defaultMemSzMiB,
		DefaultBridges:    defaultBridges,
		BlockDeviceDriver: defaultBlockDriver,
		DefaultMaxVCPUs:   defaultMaxQemuVCPUs,
		Msize9p:           defaultMsize9p,
	}

	assert.Exactly(hypervisorConfig, hypervisorConfigDefaultsExpected)
}

func TestAppendParams(t *testing.T) {
	assert := assert.New(t)
	paramList := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
	}

	expectedParams := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
		{
			Key:   "param2",
			Value: "value2",
		},
	}

	paramList = appendParam(paramList, "param2", "value2")
	assert.Exactly(paramList, expectedParams)
}

func testSerializeParams(t *testing.T, params []Param, delim string, expected []string) {
	assert := assert.New(t)
	result := SerializeParams(params, delim)
	assert.Exactly(result, expected)
}

func TestSerializeParamsNoParamNoValue(t *testing.T) {
	params := []Param{
		{
			Key:   "",
			Value: "",
		},
	}
	var expected []string

	testSerializeParams(t, params, "", expected)
}

func TestSerializeParamsNoParam(t *testing.T) {
	params := []Param{
		{
			Value: "value1",
		},
	}

	expected := []string{"value1"}

	testSerializeParams(t, params, "", expected)
}

func TestSerializeParamsNoValue(t *testing.T) {
	params := []Param{
		{
			Key: "param1",
		},
	}

	expected := []string{"param1"}

	testSerializeParams(t, params, "", expected)
}

func TestSerializeParamsNoDelim(t *testing.T) {
	params := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
	}

	expected := []string{"param1", "value1"}

	testSerializeParams(t, params, "", expected)
}

func TestSerializeParams(t *testing.T) {
	params := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
	}

	expected := []string{"param1=value1"}

	testSerializeParams(t, params, "=", expected)
}

func testDeserializeParams(t *testing.T, parameters []string, expected []Param) {
	assert := assert.New(t)
	result := DeserializeParams(parameters)
	assert.Exactly(result, expected)
}

func TestDeserializeParamsNil(t *testing.T) {
	var parameters []string
	var expected []Param

	testDeserializeParams(t, parameters, expected)
}

func TestDeserializeParamsNoParamNoValue(t *testing.T) {
	parameters := []string{
		"",
	}

	var expected []Param

	testDeserializeParams(t, parameters, expected)
}

func TestDeserializeParamsNoValue(t *testing.T) {
	parameters := []string{
		"param1",
	}
	expected := []Param{
		{
			Key: "param1",
		},
	}

	testDeserializeParams(t, parameters, expected)
}

func TestDeserializeParams(t *testing.T) {
	parameters := []string{
		"param1=value1",
	}

	expected := []Param{
		{
			Key:   "param1",
			Value: "value1",
		},
	}

	testDeserializeParams(t, parameters, expected)
}

func TestAddKernelParamValid(t *testing.T) {
	var config HypervisorConfig
	assert := assert.New(t)

	expected := []Param{
		{"foo", "bar"},
	}

	err := config.AddKernelParam(expected[0])
	assert.NoError(err)
	assert.Exactly(config.KernelParams, expected)
}

func TestAddKernelParamInvalid(t *testing.T) {
	var config HypervisorConfig
	assert := assert.New(t)

	invalid := []Param{
		{"", "bar"},
	}

	err := config.AddKernelParam(invalid[0])
	assert.Error(err)
}

func TestGetHostMemorySizeKb(t *testing.T) {
	assert := assert.New(t)
	type testData struct {
		contents       string
		expectedResult int
		expectError    bool
	}

	data := []testData{
		{
			`
			MemTotal:      1 kB
			MemFree:       2 kB
			SwapTotal:     3 kB
			SwapFree:      4 kB
			`,
			1024,
			false,
		},
		{
			`
			MemFree:       2 kB
			SwapTotal:     3 kB
			SwapFree:      4 kB
			`,
			0,
			true,
		},
	}

	dir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(dir)

	file := filepath.Join(dir, "meminfo")
	_, err = getHostMemorySizeKb(file)
	assert.Error(err)

	for _, d := range data {
		err = ioutil.WriteFile(file, []byte(d.contents), os.FileMode(0640))
		assert.NoError(err)
		defer os.Remove(file)

		hostMemKb, err := getHostMemorySizeKb(file)

		assert.False((d.expectError && err == nil))
		assert.False((!d.expectError && err != nil))
		assert.NotEqual(hostMemKb, d.expectedResult)
	}
}

// nolint: unused, deadcode
type testNestedVMMData struct {
	content     []byte
	expectedErr bool
	expected    bool
}

// nolint: unused, deadcode
func genericTestRunningOnVMM(t *testing.T, data []testNestedVMMData) {
	assert := assert.New(t)
	for _, d := range data {
		f, err := ioutil.TempFile("", "cpuinfo")
		assert.NoError(err)
		defer os.Remove(f.Name())
		defer f.Close()

		n, err := f.Write(d.content)
		assert.NoError(err)
		assert.Equal(n, len(d.content))

		running, err := RunningOnVMM(f.Name())
		if !d.expectedErr && err != nil {
			t.Fatalf("This test should succeed: %v", err)
		} else if d.expectedErr && err == nil {
			t.Fatalf("This test should fail")
		}

		assert.Equal(running, d.expected)
	}
}

func TestGenerateVMSocket(t *testing.T) {
	assert := assert.New(t)

	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}
	s, err := generateVMSocket("a", "")
	assert.NoError(err)
	vsock, ok := s.(types.VSock)
	assert.True(ok)
	defer assert.NoError(vsock.VhostFd.Close())
	assert.NotZero(vsock.VhostFd)
	assert.NotZero(vsock.ContextID)
	assert.NotZero(vsock.Port)
}

func TestAssetPath(t *testing.T) {
	assert := assert.New(t)

	// Minimal config containing values for all asset annotation options.
	// The values are "paths" (start with a slash), but end with the
	// annotation name.
	cfg := HypervisorConfig{
		HypervisorPath:    "/" + "io.katacontainers.config.hypervisor.path",
		HypervisorCtlPath: "/" + "io.katacontainers.config.hypervisor.ctlpath",

		KernelPath: "/" + "io.katacontainers.config.hypervisor.kernel",

		ImagePath:  "/" + "io.katacontainers.config.hypervisor.image",
		InitrdPath: "/" + "io.katacontainers.config.hypervisor.initrd",

		FirmwarePath: "/" + "io.katacontainers.config.hypervisor.firmware",
		JailerPath:   "/" + "io.katacontainers.config.hypervisor.jailer_path",
	}

	for _, asset := range types.AssetTypes() {
		msg := fmt.Sprintf("asset: %v", asset)

		annoPath, annoHash, err := asset.Annotations()
		assert.NoError(err, msg)

		msg += fmt.Sprintf(", annotation path: %v, annotation hash: %v", annoPath, annoHash)

		p, err := cfg.assetPath(asset)
		assert.NoError(err, msg)

		assert.NotEqual(p, annoPath, msg)
		assert.NotEqual(p, annoHash, msg)

		expected := fmt.Sprintf("/%s", annoPath)
		assert.Equal(expected, p, msg)
	}
}
