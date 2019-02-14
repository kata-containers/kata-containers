// Copyright (c) 2018 ARM Limited
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

	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func setupCheckHostIsVMContainerCapable(assert *assert.Assertions, cpuInfoFile string, moduleData []testModuleData) {
	//For now, Arm64 only deal with module check
	createModules(assert, cpuInfoFile, moduleData)

	err := makeCPUInfoFile(cpuInfoFile, "", "")
	assert.NoError(err)

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

	moduleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), true, ""},
		{filepath.Join(sysModuleDir, "vhost"), true, ""},
		{filepath.Join(sysModuleDir, "vhost_net"), true, ""},
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

	setupCheckHostIsVMContainerCapable(assert, cpuInfoFile, moduleData)

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

	tmpdir, err := ioutil.TempDir("", "")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpdir)

	savedProcCPUInfo := procCPUInfo

	testProcCPUInfo := filepath.Join(tmpdir, "cpuinfo")

	// override
	procCPUInfo = testProcCPUInfo

	defer func() {
		procCPUInfo = savedProcCPUInfo
	}()

	_, _, err = getCPUDetails()
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
