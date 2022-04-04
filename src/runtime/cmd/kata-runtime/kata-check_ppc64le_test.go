// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"path/filepath"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
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
	cpuData := []testCPUData{
		fakeCPUData,
	}

	moduleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), "", true},
		{filepath.Join(sysModuleDir, "kvm_hv"), "", true},
	}

	genericCheckCLIFunction(t, cpuData, moduleData)
}

func TestArchKernelParamHandler(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		fields       logrus.Fields
		msg          string
		onVMM        bool
		expectIgnore bool
	}

	data := []testData{
		{logrus.Fields{}, "", true, false},
		{logrus.Fields{}, "", false, false},

		{
			logrus.Fields{
				// wrong type
				"parameter": 123,
			},
			"foo",
			false,
			false,
		},

		{
			logrus.Fields{
				"parameter": "unrestricted_guest",
			},
			"",
			false,
			false,
		},

		{
			logrus.Fields{
				"parameter": "unrestricted_guest",
			},
			"",
			true,
			true,
		},

		{
			logrus.Fields{
				"parameter": "nested",
			},
			"",
			false,
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

	dir := t.TempDir()

	savedKvmDevice := kvmDevice
	fakeKVMDevice := filepath.Join(dir, "kvm")
	kvmDevice = fakeKVMDevice

	defer func() {
		kvmDevice = savedKvmDevice
	}()

	err := kvmIsUsable()
	assert.Error(err)

	err = createEmptyFile(fakeKVMDevice)
	assert.NoError(err)

	err = kvmIsUsable()
	assert.Error(err)
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

	data := []testCPUDetail{
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
