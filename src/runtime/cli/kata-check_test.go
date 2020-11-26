// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"flag"
	"fmt"
	"html/template"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"strings"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

type testModuleData struct {
	path     string
	isDir    bool
	contents string
}

// nolint: structcheck, unused, deadcode
type testCPUData struct {
	vendorID    string
	flags       string
	expectError bool
}

// nolint: structcheck, unused, deadcode
type testCPUDetail struct {
	contents       string
	expectedVendor string
	expectedModel  string
	expectError    bool
}

var fakeCPUData = testCPUData{"", "", false}

func createFile(file, contents string) error {
	return ioutil.WriteFile(file, []byte(contents), testFileMode)
}

func createModules(assert *assert.Assertions, cpuInfoFile string, moduleData []testModuleData) {
	for _, d := range moduleData {
		var dir string

		if d.isDir {
			dir = d.path
		} else {
			dir = path.Dir(d.path)
		}

		err := os.MkdirAll(dir, testDirMode)
		assert.NoError(err)

		if !d.isDir {
			err = createFile(d.path, d.contents)
			assert.NoError(err)
		}

		details := vmContainerCapableDetails{
			cpuInfoFile: cpuInfoFile,
		}

		err = hostIsVMContainerCapable(details)
		if katautils.FileExists(cpuInfoFile) {
			assert.NoError(err)
		} else {
			assert.Error(err)
		}
	}
}

func checkKernelParamHandler(assert *assert.Assertions, kernelModulesToCreate, expectedKernelModules map[string]kernelModule, handler kernelParamHandler, expectHandlerError bool, expectedErrorCount uint32) {
	err := os.RemoveAll(sysModuleDir)
	assert.NoError(err)

	count, err := checkKernelModules(map[string]kernelModule{}, handler)

	// No required modules means no error
	assert.NoError(err)
	assert.Equal(count, uint32(0))

	count, err = checkKernelModules(expectedKernelModules, handler)
	assert.NoError(err)

	// No modules exist
	expectedCount := len(expectedKernelModules)
	assert.Equal(count, uint32(expectedCount))

	err = os.MkdirAll(sysModuleDir, testDirMode)
	assert.NoError(err)

	for module, details := range kernelModulesToCreate {
		path := filepath.Join(sysModuleDir, module)
		err = os.MkdirAll(path, testDirMode)
		assert.NoError(err)

		paramDir := filepath.Join(path, "parameters")
		err = os.MkdirAll(paramDir, testDirMode)
		assert.NoError(err)

		for param, value := range details.parameters {
			paramPath := filepath.Join(paramDir, param)
			err = createFile(paramPath, value)
			assert.NoError(err)
		}
	}

	count, err = checkKernelModules(expectedKernelModules, handler)

	if expectHandlerError {
		assert.Error(err)
		return
	}

	assert.NoError(err)
	assert.Equal(count, expectedErrorCount)
}

func makeCPUInfoFile(path, vendorID, flags string) error {
	t := template.New("cpuinfo")

	t, err := t.Parse(testCPUInfoTemplate)
	if err != nil {
		return err
	}

	args := map[string]string{
		"Flags":    flags,
		"VendorID": vendorID,
	}

	contents := &bytes.Buffer{}

	err = t.Execute(contents, args)
	if err != nil {
		return err
	}

	return ioutil.WriteFile(path, contents.Bytes(), testFileMode)
}

// nolint: unused, deadcode
func genericTestGetCPUDetails(t *testing.T, validVendor string, validModel string, validContents string, data []testCPUDetail) {
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
			assert.Equal(t, d.expectedVendor, vendor)
			assert.Equal(t, d.expectedModel, model)
		}
	}
}

func genericCheckCLIFunction(t *testing.T, cpuData []testCPUData, moduleData []testModuleData) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	_, config, err := makeRuntimeConfig(dir)
	assert.NoError(err)

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

	// Replace sysModuleDir in moduleData with the test temp path
	for i := range moduleData {
		moduleData[i].path = strings.Replace(moduleData[i].path, savedSysModuleDir, sysModuleDir, 1)
	}

	err = os.MkdirAll(sysModuleDir, testDirMode)
	if err != nil {
		t.Fatal(err)
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

	flagSet := &flag.FlagSet{}
	ctx := createCLIContext(flagSet)
	ctx.App.Name = "foo"
	ctx.App.Metadata["runtimeConfig"] = config

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
		if c == fakeCPUData {
			continue
		}

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
func TestCheckGetCPUInfo(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		contents       string
		expectedResult string
		expectError    bool
	}

	data := []testData{
		{"", "", true},
		{" ", "", true},
		{"\n", "", true},
		{"\n\n", "", true},
		{"hello\n", "hello", false},
		{"foo\n\n", "foo", false},
		{"foo\n\nbar\n\n", "foo", false},
		{"foo\n\nbar\nbaz\n\n", "foo", false},
	}

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	file := filepath.Join(dir, "cpuinfo")
	// file doesn't exist
	_, err = getCPUInfo(file)
	assert.Error(err)

	for _, d := range data {
		err = ioutil.WriteFile(file, []byte(d.contents), testFileMode)
		if err != nil {
			t.Fatal(err)
		}
		defer os.Remove(file)

		contents, err := getCPUInfo(file)
		if d.expectError {
			assert.Error(err, fmt.Sprintf("got %q, test data: %+v", contents, d))
		} else {
			assert.NoError(err, fmt.Sprintf("got %q, test data: %+v", contents, d))
		}

		assert.Equal(d.expectedResult, contents)
	}
}

func TestCheckFindAnchoredString(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		haystack      string
		needle        string
		expectSuccess bool
	}

	data := []testData{
		{"", "", false},
		{"", "foo", false},
		{"foo", "", false},
		{"food", "foo", false},
		{"foo", "foo", true},
		{"foo bar", "foo", true},
		{"foo bar baz", "bar", true},
	}

	for _, d := range data {
		result := findAnchoredString(d.haystack, d.needle)

		if d.expectSuccess {
			assert.True(result)
		} else {
			assert.False(result)
		}
	}
}

func TestCheckGetCPUFlags(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		cpuinfo       string
		expectedFlags string
	}

	data := []testData{
		{"", ""},
		{"foo", ""},
		{"foo bar", ""},
		{":", ""},

		{
			cpuFlagsTag,
			"",
		},
		{
			cpuFlagsTag + ":",
			"",
		},
		{
			fmt.Sprintf("%s: a b c", cpuFlagsTag),
			"a b c",
		},
		{
			fmt.Sprintf("%s: a b c foo bar d", cpuFlagsTag),
			"a b c foo bar d",
		},
	}

	for _, d := range data {
		result := getCPUFlags(d.cpuinfo)
		assert.Equal(d.expectedFlags, result)
	}
}

func TestCheckCheckCPUFlags(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		cpuflags    string
		required    map[string]string
		expectCount uint32
	}

	data := []testData{
		{
			"",
			map[string]string{},
			0,
		},
		{
			"",
			map[string]string{
				"a": "A flag",
			},
			0,
		},
		{
			"",
			map[string]string{
				"a": "A flag",
				"b": "B flag",
			},
			0,
		},
		{
			"a b c",
			map[string]string{
				"b": "B flag",
			},
			0,
		},
		{
			"a b c",
			map[string]string{
				"x": "X flag",
				"y": "Y flag",
				"z": "Z flag",
			},
			3,
		},
	}

	for _, d := range data {
		count := checkCPUFlags(d.cpuflags, d.required)
		assert.Equal(d.expectCount, count, "%+v", d)
	}
}

func TestCheckCheckCPUAttribs(t *testing.T) {
	assert := assert.New(t)

	type testData struct {
		cpuinfo     string
		required    map[string]string
		expectCount uint32
	}

	data := []testData{
		{
			"",
			map[string]string{},
			0,
		},
		{
			"",
			map[string]string{
				"a": "",
			},
			0,
		},
		{
			"a: b",
			map[string]string{
				"b": "B attribute",
			},
			0,
		},
		{
			"a: b\nc: d\ne: f",
			map[string]string{
				"b": "B attribute",
			},
			0,
		},
		{
			"a: b\n",
			map[string]string{
				"b": "B attribute",
				"c": "C attribute",
				"d": "D attribute",
			},
			2,
		},
		{
			"a: b\nc: d\ne: f",
			map[string]string{
				"b": "B attribute",
				"d": "D attribute",
				"f": "F attribute",
			},
			0,
		},
	}

	for _, d := range data {
		count := checkCPUAttribs(d.cpuinfo, d.required)
		assert.Equal(d.expectCount, count, "%+v", d)
	}
}

func TestCheckHaveKernelModule(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(testDisabledAsNonRoot)
	}

	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	savedModProbeCmd := modProbeCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modProbeCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modProbeCmd = savedModProbeCmd
		sysModuleDir = savedSysModuleDir
	}()

	err = os.MkdirAll(sysModuleDir, testDirMode)
	if err != nil {
		t.Fatal(err)
	}

	module := "foo"

	result := haveKernelModule(module)
	assert.False(result)

	// XXX: override - make our fake "modprobe" succeed
	modProbeCmd = "true"

	result = haveKernelModule(module)
	assert.True(result)

	// disable "modprobe" again
	modProbeCmd = "false"

	fooDir := filepath.Join(sysModuleDir, module)
	err = os.MkdirAll(fooDir, testDirMode)
	if err != nil {
		t.Fatal(err)
	}

	result = haveKernelModule(module)
	assert.True(result)
}

func TestCheckCheckKernelModules(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	savedModProbeCmd := modProbeCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modProbeCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modProbeCmd = savedModProbeCmd
		sysModuleDir = savedSysModuleDir
	}()

	err = os.MkdirAll(sysModuleDir, testDirMode)
	if err != nil {
		t.Fatal(err)
	}

	testData := map[string]kernelModule{
		"foo": {
			desc:       "desc",
			parameters: map[string]string{},
			required:   true,
		},
		"bar": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "hello",
				"param2": "world",
				"param3": "a",
				"param4": ".",
			},
			required: true,
		},
	}

	count, err := checkKernelModules(map[string]kernelModule{}, nil)
	// No required modules means no error
	assert.NoError(err)
	assert.Equal(count, uint32(0))

	count, err = checkKernelModules(testData, nil)
	assert.NoError(err)
	// No modules exist
	assert.Equal(count, uint32(2))

	for module, details := range testData {
		path := filepath.Join(sysModuleDir, module)
		err = os.MkdirAll(path, testDirMode)
		if err != nil {
			t.Fatal(err)
		}

		paramDir := filepath.Join(path, "parameters")
		err = os.MkdirAll(paramDir, testDirMode)
		if err != nil {
			t.Fatal(err)
		}

		for param, value := range details.parameters {
			paramPath := filepath.Join(paramDir, param)
			err = createFile(paramPath, value)
			if err != nil {
				t.Fatal(err)
			}
		}
	}

	count, err = checkKernelModules(testData, nil)
	assert.NoError(err)
	assert.Equal(count, uint32(0))
}

func TestCheckCheckKernelModulesUnreadableFile(t *testing.T) {
	assert := assert.New(t)

	if tc.NotValid(ktu.NeedNonRoot()) {
		t.Skip(ktu.TestDisabledNeedNonRoot)
	}

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	testData := map[string]kernelModule{
		"foo": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "wibble",
			},
			required: true,
		},
	}

	savedModProbeCmd := modProbeCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modProbeCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modProbeCmd = savedModProbeCmd
		sysModuleDir = savedSysModuleDir
	}()

	modPath := filepath.Join(sysModuleDir, "foo/parameters")
	err = os.MkdirAll(modPath, testDirMode)
	assert.NoError(err)

	modParamFile := filepath.Join(modPath, "param1")

	err = createEmptyFile(modParamFile)
	assert.NoError(err)

	// make file unreadable by non-root user
	err = os.Chmod(modParamFile, 0000)
	assert.NoError(err)

	_, err = checkKernelModules(testData, nil)
	assert.Error(err)
}

func TestCheckCheckKernelModulesInvalidFileContents(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	testData := map[string]kernelModule{
		"foo": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "wibble",
			},
			required: true,
		},
	}

	savedModProbeCmd := modProbeCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modProbeCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modProbeCmd = savedModProbeCmd
		sysModuleDir = savedSysModuleDir
	}()

	modPath := filepath.Join(sysModuleDir, "foo/parameters")
	err = os.MkdirAll(modPath, testDirMode)
	assert.NoError(err)

	modParamFile := filepath.Join(modPath, "param1")

	err = createFile(modParamFile, "burp")
	assert.NoError(err)

	count, err := checkKernelModules(testData, nil)
	assert.NoError(err)
	assert.Equal(count, uint32(1))
}

func TestCheckCLIFunctionFail(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	_, config, err := makeRuntimeConfig(dir)
	assert.NoError(err)

	oldProcCPUInfo := procCPUInfo

	// doesn't exist
	procCPUInfo = filepath.Join(dir, "cpuinfo")

	defer func() {
		procCPUInfo = oldProcCPUInfo
	}()

	flagSet := &flag.FlagSet{}
	ctx := createCLIContext(flagSet)
	ctx.App.Name = "foo"
	ctx.App.Metadata["runtimeConfig"] = config

	fn, ok := kataCheckCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
}

func TestCheckKernelParamHandler(t *testing.T) {
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	savedModProbeCmd := modProbeCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modProbeCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modProbeCmd = savedModProbeCmd
		sysModuleDir = savedSysModuleDir
	}()

	handler := func(onVMM bool, fields logrus.Fields, msg string) bool {
		param, ok := fields["parameter"].(string)
		if !ok {
			return false
		}

		if param == "param1" {
			return true
		}

		// don't ignore the error
		return false
	}

	testData1 := map[string]kernelModule{
		"foo": {
			desc:       "desc",
			parameters: map[string]string{},
			required:   true,
		},
		"bar": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "hello",
				"param2": "world",
			},
			required: true,
		},
	}

	checkKernelParamHandler(assert, testData1, testData1, handler, false, uint32(0))

	testDataToCreate := map[string]kernelModule{
		"foo": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "moo",
			},
			required: true,
		},
	}

	testDataToExpect := map[string]kernelModule{
		"foo": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "bar",
			},
			required: true,
		},
	}

	// Expected and actual are different, but the handler should deal with
	// the problem.
	checkKernelParamHandler(assert, testDataToCreate, testDataToExpect, handler, false, uint32(0))

	// Expected and actual are different, so with no handler we expect a
	// single error (due to "param1"'s value being different)
	checkKernelParamHandler(assert, testDataToCreate, testDataToExpect, nil, false, uint32(1))
}

func TestArchRequiredKernelModules(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	_, config, err := makeRuntimeConfig(tmpdir)
	assert.NoError(err)

	err = setCPUtype(config.HypervisorType)
	assert.NoError(err)

	if len(archRequiredKernelModules) == 0 {
		// No modules to check
		return
	}

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	savedModProbeCmd := modProbeCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override
	sysModuleDir = filepath.Join(dir, "sys/module")
	modProbeCmd = "false"

	defer func() {
		sysModuleDir = savedSysModuleDir
		modProbeCmd = savedModProbeCmd
	}()

	// Running check with no modules
	count, err := checkKernelModules(archRequiredKernelModules, nil)
	assert.NoError(err)

	// Test that count returned matches the # of modules with required set.
	expectedCount := 0
	for _, module := range archRequiredKernelModules {
		if module.required {
			expectedCount++
		}
	}

	assert.EqualValues(count, expectedCount)
}
