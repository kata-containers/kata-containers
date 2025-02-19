// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os"
	"strings"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

func TestGetKernelRootParams(t *testing.T) {
	assert := assert.New(t)
	tests := []struct {
		rootfstype    string
		expected      []Param
		disableNvdimm bool
		dax           bool
		error         bool
	}{
		// EXT4
		{
			rootfstype: string(EXT4),
			expected: []Param{
				{"root", string(Nvdimm)},
				{"rootflags", "data=ordered,errors=remount-ro ro"},
				{"rootfstype", string(EXT4)},
			},
			disableNvdimm: false,
			dax:           false,
			error:         false,
		},
		{
			rootfstype: string(EXT4),
			expected: []Param{
				{"root", string(Nvdimm)},
				{"rootflags", "dax,data=ordered,errors=remount-ro ro"},
				{"rootfstype", string(EXT4)},
			},
			disableNvdimm: false,
			dax:           true,
			error:         false,
		},
		{
			rootfstype: string(EXT4),
			expected: []Param{
				{"root", string(VirtioBlk)},
				{"rootflags", "data=ordered,errors=remount-ro ro"},
				{"rootfstype", string(EXT4)},
			},
			disableNvdimm: true,
			dax:           false,
			error:         false,
		},

		// XFS
		{
			rootfstype: string(XFS),
			expected: []Param{
				{"root", string(Nvdimm)},
				{"rootflags", "data=ordered,errors=remount-ro ro"},
				{"rootfstype", string(XFS)},
			},
			disableNvdimm: false,
			dax:           false,
			error:         false,
		},
		{
			rootfstype: string(XFS),
			expected: []Param{
				{"root", string(Nvdimm)},
				{"rootflags", "dax,data=ordered,errors=remount-ro ro"},
				{"rootfstype", string(XFS)},
			},
			disableNvdimm: false,
			dax:           true,
			error:         false,
		},
		{
			rootfstype: string(XFS),
			expected: []Param{
				{"root", string(VirtioBlk)},
				{"rootflags", "data=ordered,errors=remount-ro ro"},
				{"rootfstype", string(XFS)},
			},
			disableNvdimm: true,
			dax:           false,
			error:         false,
		},

		// EROFS
		{
			rootfstype: string(EROFS),
			expected: []Param{
				{"root", string(Nvdimm)},
				{"rootflags", "ro"},
				{"rootfstype", string(EROFS)},
			},
			disableNvdimm: false,
			dax:           false,
			error:         false,
		},
		{
			rootfstype: string(EROFS),
			expected: []Param{
				{"root", string(Nvdimm)},
				{"rootflags", "dax ro"},
				{"rootfstype", string(EROFS)},
			},
			disableNvdimm: false,
			dax:           true,
			error:         false,
		},
		{
			rootfstype: string(EROFS),
			expected: []Param{
				{"root", string(VirtioBlk)},
				{"rootflags", "ro"},
				{"rootfstype", string(EROFS)},
			},
			disableNvdimm: true,
			dax:           false,
			error:         false,
		},

		// Unsupported rootfs type
		{
			rootfstype: "foo",
			expected: []Param{
				{"root", string(VirtioBlk)},
				{"rootflags", "data=ordered,errors=remount-ro ro"},
				{"rootfstype", string(EXT4)},
			},
			disableNvdimm: false,
			dax:           false,
			error:         true,
		},

		// Nvdimm does not support DAX
		{
			rootfstype: string(EXT4),
			expected: []Param{
				{"root", string(VirtioBlk)},
				{"rootflags", "dax,data=ordered,errors=remount-ro ro"},
				{"rootfstype", string(EXT4)},
			},
			disableNvdimm: true,
			dax:           true,
			error:         true,
		},
	}

	for _, t := range tests {
		kernelRootParams, err := GetKernelRootParams(t.rootfstype, t.disableNvdimm, t.dax)
		if t.error {
			assert.Error(err)
			continue
		} else {
			assert.NoError(err)
		}
		assert.Equal(t.expected, kernelRootParams,
			"Invalid parameters rootfstype: %v, disableNvdimm: %v, dax: %v, "+
				"unable to get kernel root params", t.rootfstype, t.disableNvdimm, t.dax)
	}
}

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

func TestSetRemoteHypervisorType(t *testing.T) {
	testSetHypervisorType(t, "remote", RemoteHypervisor)
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

func TestStringFromRemoteHypervisorType(t *testing.T) {
	hypervisorType := RemoteHypervisor
	testStringFromHypervisorType(t, hypervisorType, "remote")
}

func TestStringFromMockHypervisorType(t *testing.T) {
	hypervisorType := MockHypervisor
	testStringFromHypervisorType(t, hypervisorType, "mock")
}

func TestStringFromUnknownHypervisorType(t *testing.T) {
	var hypervisorType HypervisorType
	testStringFromHypervisorType(t, hypervisorType, "")
}

func testNewHypervisorFromHypervisorType(t *testing.T, hypervisorType HypervisorType, expected Hypervisor) {
	assert := assert.New(t)
	hy, err := NewHypervisor(hypervisorType)
	assert.NoError(err)
	assert.Exactly(hy, expected)
}

func TestNewHypervisorFromRemoteHypervisorType(t *testing.T) {
	hypervisorType := RemoteHypervisor
	expectedHypervisor := &remoteHypervisor{}
	testNewHypervisorFromHypervisorType(t, hypervisorType, expectedHypervisor)
}

func TestNewHypervisorFromMockHypervisorType(t *testing.T) {
	hypervisorType := MockHypervisor
	expectedHypervisor := &mockHypervisor{}
	testNewHypervisorFromHypervisorType(t, hypervisorType, expectedHypervisor)
}

func TestNewHypervisorFromUnknownHypervisorType(t *testing.T) {
	var hypervisorType HypervisorType
	assert := assert.New(t)

	hy, err := NewHypervisor(hypervisorType)
	assert.Error(err)
	assert.Nil(hy)
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

func TestCheckCmdline(t *testing.T) {
	assert := assert.New(t)

	cmdlineFp, err := os.CreateTemp("", "")
	assert.NoError(err)
	_, err = cmdlineFp.WriteString("quiet root=/dev/sda2")
	assert.NoError(err)
	cmdlinePath := cmdlineFp.Name()
	defer os.Remove(cmdlinePath)

	assert.True(CheckCmdline(cmdlinePath, "quiet", []string{}))
	assert.True(CheckCmdline(cmdlinePath, "root", []string{"/dev/sda1", "/dev/sda2"}))
	assert.False(CheckCmdline(cmdlinePath, "ro", []string{}))
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
		f, err := os.CreateTemp("", "cpuinfo")
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

func TestAssetPath(t *testing.T) {
	assert := assert.New(t)

	// Minimal config containing values for all asset annotation options.
	// The values are "paths" (start with a slash), but end with the
	// annotation name.
	cfg := HypervisorConfig{
		HypervisorPath: "/" + "io.katacontainers.config.hypervisor.path",

		KernelPath: "/" + "io.katacontainers.config.hypervisor.kernel",

		ImagePath:  "/" + "io.katacontainers.config.hypervisor.image",
		InitrdPath: "/" + "io.katacontainers.config.hypervisor.initrd",

		FirmwarePath:       "/" + "io.katacontainers.config.hypervisor.firmware",
		FirmwareVolumePath: "/" + "io.katacontainers.config.hypervisor.firmware_volume",
		JailerPath:         "/" + "io.katacontainers.config.hypervisor.jailer_path",
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

func TestKernelParamFields(t *testing.T) {
	assert := assert.New(t)
	tests := []struct {
		cmdLine                         string
		expectedFieldsResult            []string
		expectedKernelParamFieldsResult []string
	}{
		{
			cmdLine: "a=b x=y",
			expectedFieldsResult: []string{
				"a=b",
				"x=y",
			},
			expectedKernelParamFieldsResult: []string{
				"a=b",
				"x=y",
			},
		},
		{
			cmdLine: "a=b x=y  foo=bar",
			expectedFieldsResult: []string{
				"a=b",
				"x=y",
				"foo=bar",
			},
			expectedKernelParamFieldsResult: []string{
				"a=b",
				"x=y",
				"foo=bar",
			},
		},
		{
			cmdLine: "a x=y  foo=bar",
			expectedFieldsResult: []string{
				"a",
				"x=y",
				"foo=bar",
			},
			expectedKernelParamFieldsResult: []string{
				"a",
				"x=y",
				"foo=bar",
			},
		},
		{
			cmdLine: "a=b      x foo=bar",
			expectedFieldsResult: []string{
				"a=b",
				"x",
				"foo=bar",
			},
			expectedKernelParamFieldsResult: []string{
				"a=b",
				"x",
				"foo=bar",
			},
		},
		{
			cmdLine: "a=b      x foo     ",
			expectedFieldsResult: []string{
				"a=b",
				"x",
				"foo",
			},
			expectedKernelParamFieldsResult: []string{
				"a=b",
				"x",
				"foo",
			},
		},
		{
			cmdLine: "a=b x=\"y z\"",
			expectedFieldsResult: []string{
				"a=b",
				"x=\"y",
				"z\"",
			},
			expectedKernelParamFieldsResult: []string{
				"a=b",
				"x=\"y z\"",
			},
		},
		{
			cmdLine: "foo=\"bar baz\"",
			expectedFieldsResult: []string{
				"foo=\"bar",
				"baz\"",
			},
			expectedKernelParamFieldsResult: []string{
				"foo=\"bar baz\"",
			},
		},
		{
			cmdLine: "foo=\"bar baz\"           abc=\"123\"",
			expectedFieldsResult: []string{
				"foo=\"bar",
				"baz\"",
				"abc=\"123\"",
			},
			expectedKernelParamFieldsResult: []string{
				"foo=\"bar baz\"",
				"abc=\"123\"",
			},
		},
		{
			cmdLine: "\"a=b",
			expectedFieldsResult: []string{
				"\"a=b",
			},
			expectedKernelParamFieldsResult: []string{
				"\"a=b",
			},
		},
		{
			cmdLine: "\"a=b    x=y",
			expectedFieldsResult: []string{
				"\"a=b",
				"x=y",
			},
			expectedKernelParamFieldsResult: []string{
				"\"a=b    x=y",
			},
		},
	}

	for _, t := range tests {
		params := strings.Fields(t.cmdLine)
		assert.Equal(params, t.expectedFieldsResult, "Unexpected strings.Fields behavior")

		params = KernelParamFields(t.cmdLine)
		assert.Equal(params, t.expectedKernelParamFieldsResult, "Unexpected KernelParamFields behavior")
	}
}
