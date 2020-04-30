// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"regexp"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
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
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	var cpuData []testCPUData
	var moduleData []testModuleData

	if cpuType == cpuTypeIntel {
		cpuData = []testCPUData{
			{archGenuineIntel, "lm vmx sse4_1", false},
		}

		moduleData = []testModuleData{}
	} else if cpuType == cpuTypeAMD {
		cpuData = []testCPUData{
			{archAuthenticAMD, "lm svm sse4_1", false},
		}

		moduleData = []testModuleData{}
	}

	genericCheckCLIFunction(t, cpuData, moduleData)
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
			required: true,
		},
	}

	actualModuleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/unrestricted_guest"), false, "Y"},

		// XXX: force a warning
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/nested"), false, "N"},
	}

	vendor := archGenuineIntel
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

	re := regexp.MustCompile(`.*\bnested\b`)
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
			required: true,
		},
	}

	actualModuleData := []testModuleData{
		{filepath.Join(sysModuleDir, "kvm"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel"), true, ""},
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/nested"), false, "Y"},

		// XXX: force a failure on non-VMM systems
		{filepath.Join(sysModuleDir, "kvm_intel/parameters/unrestricted_guest"), false, "N"},
	}

	vendor := archGenuineIntel
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

	re := regexp.MustCompile(`.*\bunrestricted_guest\b`)
	matches := re.FindAllStringSubmatch(buf.String(), -1)
	assert.NotEmpty(matches)
}

func TestCheckHostIsVMContainerCapable(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

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

	var cpuData []testCPUData
	var moduleData []testModuleData

	if cpuType == cpuTypeIntel {
		cpuData = []testCPUData{
			{"", "", true},
			{"Intel", "", true},
			{archGenuineIntel, "", true},
			{archGenuineIntel, "lm", true},
			{archGenuineIntel, "lm vmx", true},
			{archGenuineIntel, "lm vmx sse4_1", false},
		}

		moduleData = []testModuleData{
			{filepath.Join(sysModuleDir, "kvm"), true, ""},
			{filepath.Join(sysModuleDir, "kvm_intel"), true, ""},
			{filepath.Join(sysModuleDir, "kvm_intel/parameters/nested"), false, "Y"},
			{filepath.Join(sysModuleDir, "kvm_intel/parameters/unrestricted_guest"), false, "Y"},
		}
	} else if cpuType == cpuTypeAMD {
		cpuData = []testCPUData{
			{"", "", true},
			{"AMD", "", true},
			{archAuthenticAMD, "", true},
			{archAuthenticAMD, "lm", true},
			{archAuthenticAMD, "lm svm", true},
			{archAuthenticAMD, "lm svm sse4_1", false},
		}

		moduleData = []testModuleData{
			{filepath.Join(sysModuleDir, "kvm"), true, ""},
			{filepath.Join(sysModuleDir, "kvm_amd"), true, ""},
			{filepath.Join(sysModuleDir, "kvm_amd/parameters/nested"), false, "1"},
		}
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
	assert.Nil(err)
}

func TestArchKernelParamHandler(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		onVMM        bool
		expectIgnore bool
		fields       logrus.Fields
		msg          string
	}

	data := []testData{
		{true, false, logrus.Fields{}, ""},
		{false, false, logrus.Fields{}, ""},

		{
			false,
			false,
			logrus.Fields{
				// wrong type
				"parameter": 123,
			},
			"foo",
		},

		{
			false,
			false,
			logrus.Fields{
				"parameter": "unrestricted_guest",
			},
			"",
		},

		{
			true,
			true,
			logrus.Fields{
				"parameter": "unrestricted_guest",
			},
			"",
		},

		{
			false,
			true,
			logrus.Fields{
				"parameter": "nested",
			},
			"",
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

func TestGetCPUDetails(t *testing.T) {
	const validVendorName = "a vendor"
	validVendor := fmt.Sprintf(`%s  : %s`, archCPUVendorField, validVendorName)

	const validModelName = "some CPU model"
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
		{validModel, "", "", true},
		{validContents, validVendorName, validModelName, false},
	}
	genericTestGetCPUDetails(t, validVendor, validModel, validContents, data)
}

func TestSetCPUtype(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	savedArchRequiredCPUFlags := archRequiredCPUFlags
	savedArchRequiredCPUAttribs := archRequiredCPUAttribs
	savedArchRequiredKernelModules := archRequiredKernelModules

	defer func() {
		archRequiredCPUFlags = savedArchRequiredCPUFlags
		archRequiredCPUAttribs = savedArchRequiredCPUAttribs
		archRequiredKernelModules = savedArchRequiredKernelModules
	}()

	archRequiredCPUFlags = map[string]string{}
	archRequiredCPUAttribs = map[string]string{}
	archRequiredKernelModules = map[string]kernelModule{}

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	setCPUtype(config.HypervisorType)

	assert.NotEmpty(archRequiredCPUFlags)
	assert.NotEmpty(archRequiredCPUAttribs)
	assert.NotEmpty(archRequiredKernelModules)

	assert.Equal(archRequiredCPUFlags["vmx"], "Virtualization support")

	_, ok := archRequiredKernelModules["kvm"]
	assert.True(ok)
}
