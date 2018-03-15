// Copyright (c) 2018 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"bytes"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"strings"
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
		{"GenuineIntel", "lm vmx sse4_1", false},
	}

	moduleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/unrestricted_guest"), false, "Y"},
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/nested"), false, "Y"},
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

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"

	// create buffer to save logger output
	buf := &bytes.Buffer{}

	// capture output this time
	kataLog.Logger.Out = buf

	fn, ok := kataCheckCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.NoError(err)

	output := buf.String()

	for _, c := range cpuData {
		assert.True(findAnchoredString(output, c.vendorID))
		for _, flag := range strings.Fields(c.flags) {
			assert.True(findAnchoredString(output, flag))
		}
	}

	for _, m := range moduleData {
		name := path.Base(m.path)
		assert.True(findAnchoredString(output, name))
	}
}

func TestCheckCheckKernelModulesNoNesting(t *testing.T) {
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

	requiredModules := map[string]kernelModule{
		"kvm_intel": {
			desc: "Intel KVM",
			parameters: map[string]string{
				"nested":             "Y",
				"unrestricted_guest": "Y",
			},
		},
	}

	actualModuleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/unrestricted_guest"), false, "Y"},

		// XXX: force a warning
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/nested"), false, "N"},
	}

	vendor := "GenuineIntel"
	flags := "vmx lm sse4_1 hypervisor"

	_, err = checkKernelModules(requiredModules, archKernelParamHandler)
	// no cpuInfoFile yet
	assert.Error(err)

	createModules(assert, cpuInfoFile, actualModuleData)

	err = makeCPUInfoFile(cpuInfoFile, vendor, flags)
	assert.NoError(err)

	count, err := checkKernelModules(requiredModules, archKernelParamHandler)
	assert.NoError(err)
	assert.Equal(count, uint32(0))

	// create buffer to save logger output
	buf := &bytes.Buffer{}

	savedLogOutput := kataLog.Logger.Out

	defer func() {
		kataLog.Logger.Out = savedLogOutput
	}()

	kataLog.Logger.Out = buf

	count, err = checkKernelModules(requiredModules, archKernelParamHandler)

	assert.NoError(err)
	assert.Equal(count, uint32(0))

	re := regexp.MustCompile(`\bwarning\b.*\bnested\b`)
	matches := re.FindAllStringSubmatch(buf.String(), -1)
	assert.NotEmpty(matches)
}

func TestCheckCheckKernelModulesNoUnrestrictedGuest(t *testing.T) {
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

	requiredModules := map[string]kernelModule{
		"kvm_intel": {
			desc: "Intel KVM",
			parameters: map[string]string{
				"nested":             "Y",
				"unrestricted_guest": "Y",
			},
		},
	}

	actualModuleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/nested"), false, "Y"},

		// XXX: force a failure on non-VMM systems
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/unrestricted_guest"), false, "N"},
	}

	vendor := "GenuineIntel"
	flags := "vmx lm sse4_1"

	_, err = checkKernelModules(requiredModules, archKernelParamHandler)
	// no cpuInfoFile yet
	assert.Error(err)

	err = makeCPUInfoFile(cpuInfoFile, vendor, flags)
	assert.NoError(err)

	createModules(assert, cpuInfoFile, actualModuleData)

	count, err := checkKernelModules(requiredModules, archKernelParamHandler)

	assert.NoError(err)
	// fails due to unrestricted_guest not being available
	assert.Equal(count, uint32(1))

	// pretend test is running under a hypervisor
	flags += " hypervisor"

	// recreate
	err = makeCPUInfoFile(cpuInfoFile, vendor, flags)
	assert.NoError(err)

	// create buffer to save logger output
	buf := &bytes.Buffer{}

	savedLogOutput := kataLog.Logger.Out

	defer func() {
		kataLog.Logger.Out = savedLogOutput
	}()

	kataLog.Logger.Out = buf

	count, err = checkKernelModules(requiredModules, archKernelParamHandler)

	// no error now because running under a hypervisor
	assert.NoError(err)
	assert.Equal(count, uint32(0))

	re := regexp.MustCompile(`\bwarning\b.*\bunrestricted_guest\b`)
	matches := re.FindAllStringSubmatch(buf.String(), -1)
	assert.NotEmpty(matches)
}

func TestCheckHostIsVMContainerCapable(t *testing.T) {
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
		{"", "", true},
		{"Intel", "", true},
		{"GenuineIntel", "", true},
		{"GenuineIntel", "lm", true},
		{"GenuineIntel", "lm vmx", true},
		{"GenuineIntel", "lm vmx sse4_1", false},
	}

	moduleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/nested"), false, "Y"},
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/unrestricted_guest"), false, "Y"},
	}

	setupCheckHostIsVMContainerCapable(assert, cpuInfoFile, cpuData, moduleData)

	// remove the modules to force a failure
	err = os.RemoveAll(sysModuleDir)
	assert.NoError(err)

	details := vmContainerCapableDetails{
		cpuInfoFile:           cpuInfoFile,
		requiredCPUFlags:      archRequiredCPUFlags,
		requiredCPUAttribs:    archRequiredCPUAttribs,
		requiredKernelModules: archRequiredKernelModules,
	}

	err = hostIsVMContainerCapable(details)
	assert.Error(err)
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
