// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func setupCheckHostIsVMContainerCapable(assert *assert.Assertions, cpuInfoFile string, cpuData []testCPUData, moduleData []testModuleData) {
	createModules(assert, cpuInfoFile, moduleData)

	// all the modules files have now been created, so deal with the
	// cpuinfo data.
	for _, d := range cpuData {
		err := makeCPUInfoFile(cpuInfoFile, d.vendorID, d.flags)
		assert.NoError(err)

		details := vmContainerCapableDetails{
			cpuInfoFile:           cpuInfoFile,
			requiredCPUFlags:      archRequiredCPUFlags,
			requiredCPUAttribs:    archRequiredCPUAttribs,
			requiredKernelModules: archRequiredKernelModules,
		}

		err = hostIsVMContainerCapable(details)
		if d.expectError {
			assert.Error(err)
		} else {
			assert.NoError(err)
		}
	}
}

func TestCCCheckCLIFunction(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	savedSysModuleDir := sysModuleDir
	savedProcCPUInfo := procCPUInfo

	cpuInfoFile := filepath.Join(dir, "cpuinfo")

	// XXX: override
	sysModuleDir = filepath.Join(dir, "sys/module")
	procCPUInfo = cpuInfoFile

	defer func() {
		sysModuleDir = savedSysModuleDir
		procCPUInfo = savedProcCPUInfo
	}()

	err = os.MkdirAll(sysModuleDir, testDirMode)
	if err != nil {
		t.Fatal(err)
	}

	cpuData := []testCPUData{
		{"", "", false},
	}

	moduleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), false, "Y"},
		{filepath.Join(sysModuleDir, "kvm_hv"), false, "Y"},
	}

	devNull, err := os.OpenFile(os.DevNull, os.O_WRONLY, 0666)
	assert.NoError(err)
	defer devNull.Close()

	savedLogOutput := kataLog.Logger.Out

	// discard normal output
	kataLog.Logger.Out = devNull

	defer func() {
		kataLog.Logger.Out = savedLogOutput
	}()

	setupCheckHostIsVMContainerCapable(assert, cpuInfoFile, cpuData, moduleData)

	ctx := createCLIContext(nil)
	ctx.App.Name = "foo"

	// create buffer to save logger output
	buf := &bytes.Buffer{}

	// capture output this time
	kataLog.Logger.Out = buf

	fn, ok := kataCheckCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.NoError(err)

	output := buf.String()

	for _, m := range moduleData {
		name := path.Base(m.path)
		assert.True(findAnchoredString(output, name))
	}
}

func TestArchKernelParamHandler(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		onVMM        bool
		fields       logrus.Fields
		msg          string
		expectIgnore bool
	}

	data := []testData{
		{true, logrus.Fields{}, "", false},
		{false, logrus.Fields{}, "", false},

		{
			false,
			logrus.Fields{
				// wrong type
				"parameter": 123,
			},
			"foo",
			false,
		},

		{
			false,
			logrus.Fields{
				"parameter": "unrestricted_guest",
			},
			"",
			false,
		},

		{
			true,
			logrus.Fields{
				"parameter": "unrestricted_guest",
			},
			"",
			true,
		},

		{
			false,
			logrus.Fields{
				"parameter": "nested",
			},
			"",
			true,
		},
	}

	for i, d := range data {
		result := archKernelParamHandler(d.onVMM, d.fields, d.msg)
		if d.expectIgnore {
			assert.True(result, "test %d (%+v)", i, d)
		} else {
			assert.False(result, "test %d (%+v)", i, d)
		}
	}
}

func TestKvmIsUsable(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	savedKvmDevice := kvmDevice
	fakeKVMDevice := filepath.Join(dir, "kvm")
	kvmDevice = fakeKVMDevice

	defer func() {
		kvmDevice = savedKvmDevice
	}()

	err = kvmIsUsable()
	assert.Error(err)

	err = createEmptyFile(fakeKVMDevice)
	assert.NoError(err)

	err = kvmIsUsable()
	assert.Error(err)
}

type TestDataa struct {
	contents       string
	expectedVendor string
	expectedModel  string
	expectError    bool
}

func TestGetCPUDetails(t *testing.T) {

	const validVendorName = ""
	validVendor := fmt.Sprintf(`%s  : %s`, archCPUVendorField, validVendorName)

	const validModelName = "8247-22L"
	validModel := fmt.Sprintf(`%s   : %s`, archCPUModelField, validModelName)

	validContents := fmt.Sprintf(`
a       : b
%s
foo     : bar
%s
`, validVendor, validModel)

	data := []TestDataa{
		{"", "", "", true},
		{"invalid", "", "", true},
		{archCPUVendorField, "", "", true},
		{validVendor, "", "", true},
		{validModel, "", validModelName, false},
		{validContents, validVendorName, validModelName, false},
	}

	genericTestGetCPUDetails(t, validVendor, validModel, validContents, data)
}

func TestSetCPUtype(t *testing.T) {
	testSetCPUTypeGeneric(t)
}
