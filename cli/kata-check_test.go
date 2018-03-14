// Copyright (c) 2017 Intel Corporation
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
	"fmt"
	"html/template"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

type testModuleData struct {
	path     string
	isDir    bool
	contents string
}

type testCPUData struct {
	vendorID    string
	flags       string
	expectError bool
}

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
		if fileExists(cpuInfoFile) {
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
		{"flags", ""},
		{"flags:", ""},
		{"flags: a b c", "a b c"},
		{"flags: a b c foo bar d", "a b c foo bar d"},
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
	assert := assert.New(t)

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	savedModInfoCmd := modInfoCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modInfoCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modInfoCmd = savedModInfoCmd
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
	modInfoCmd = "true"

	result = haveKernelModule(module)
	assert.True(result)

	// disable "modprobe" again
	modInfoCmd = "false"

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

	savedModInfoCmd := modInfoCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modInfoCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modInfoCmd = savedModInfoCmd
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
		},
		"bar": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "hello",
				"param2": "world",
				"param3": "a",
				"param4": ".",
			},
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

	if os.Geteuid() == 0 {
		t.Skip(testDisabledNeedNonRoot)
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
		},
	}

	savedModInfoCmd := modInfoCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modInfoCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modInfoCmd = savedModInfoCmd
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
		},
	}

	savedModInfoCmd := modInfoCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modInfoCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modInfoCmd = savedModInfoCmd
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

	oldProcCPUInfo := procCPUInfo

	// doesn't exist
	procCPUInfo = filepath.Join(dir, "cpuinfo")

	defer func() {
		procCPUInfo = oldProcCPUInfo
	}()

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"

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

	savedModInfoCmd := modInfoCmd
	savedSysModuleDir := sysModuleDir

	// XXX: override (fake the modprobe command failing)
	modInfoCmd = "false"
	sysModuleDir = filepath.Join(dir, "sys/module")

	defer func() {
		modInfoCmd = savedModInfoCmd
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
		},
		"bar": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "hello",
				"param2": "world",
			},
		},
	}

	checkKernelParamHandler(assert, testData1, testData1, handler, false, uint32(0))

	testDataToCreate := map[string]kernelModule{
		"foo": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "moo",
			},
		},
	}

	testDataToExpect := map[string]kernelModule{
		"foo": {
			desc: "desc",
			parameters: map[string]string{
				"param1": "bar",
			},
		},
	}

	// Expected and actual are different, but the handler should deal with
	// the problem.
	checkKernelParamHandler(assert, testDataToCreate, testDataToExpect, handler, false, uint32(0))

	// Expected and actual are different, so with no handler we expect a
	// single error (due to "param1"'s value being different)
	checkKernelParamHandler(assert, testDataToCreate, testDataToExpect, nil, false, uint32(1))
}
