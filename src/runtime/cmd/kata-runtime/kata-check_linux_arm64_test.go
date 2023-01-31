// Copyright (c) 2018 ARM Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
)

func setupCheckHostIsVMContainerCapable(assert *assert.Assertions, cpuInfoFile string, cpuData []testCPUData, moduleData []testModuleData) {
	//For now, Arm64 only deal with module check
	_ = cpuData

	createModules(assert, cpuInfoFile, moduleData)

	err := makeCPUInfoFile(cpuInfoFile, "", "")
	assert.NoError(err)
}

func TestCCCheckCLIFunction(t *testing.T) {
	var cpuData []testCPUData
	moduleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), "", true},
		{filepath.Join(sysModuleDir, "vhost"), "", true},
		{filepath.Join(sysModuleDir, "vhost_net"), "", true},
	}

	genericCheckCLIFunction(t, cpuData, moduleData)
}

func TestGetCPUDetails(t *testing.T) {
	type testData struct {
		contents                string
		expectedNormalizeVendor string
		expectedNormalizeModel  string
		expectError             bool
	}

	validVendorName := "0x41"
	validNormalizeVendorName := "ARM Limited"
	validVendor := fmt.Sprintf(`%s  : %s`, archCPUVendorField, validVendorName)

	validModelName := "8"
	validNormalizeModelName := "v8"
	validModel := fmt.Sprintf(`%s   : %s`, archCPUModelField, validModelName)

	validContents := fmt.Sprintf(`
a       : b
%s
foo     : bar
%s
`, validVendor, validModel)

	data := []testData{
		{"", "", "", true},
		{"invalid", "", "", true},
		{archCPUVendorField, "", "", true},
		{validVendor, "", "", true},
		{validModel, "", "", true},
		{validContents, validNormalizeVendorName, validNormalizeModelName, false},
	}

	tmpdir := t.TempDir()

	savedProcCPUInfo := procCPUInfo

	testProcCPUInfo := filepath.Join(tmpdir, "cpuinfo")

	// override
	procCPUInfo = testProcCPUInfo

	defer func() {
		procCPUInfo = savedProcCPUInfo
	}()

	_, _, err := getCPUDetails()
	// ENOENT
	assert.Error(t, err)
	assert.True(t, os.IsNotExist(err))

	for _, d := range data {
		err := createFile(procCPUInfo, d.contents)
		assert.NoError(t, err)

		vendor, model, err := getCPUDetails()

		if d.expectError {
			assert.Error(t, err, fmt.Sprintf("%+v", d))
			continue
		} else {
			assert.NoError(t, err, fmt.Sprintf("%+v", d))
			assert.Equal(t, d.expectedNormalizeVendor, vendor)
			assert.Equal(t, d.expectedNormalizeModel, model)
		}
	}
}

func TestSetCPUtype(t *testing.T) {
	testSetCPUTypeGeneric(t)
}
