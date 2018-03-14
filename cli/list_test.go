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
	"encoding/json"
	"flag"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"time"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

type TestFileWriter struct {
	Name string
	File *os.File
}

var hypervisorDetails1 = hypervisorDetails{
	HypervisorAsset: asset{
		Path: "/hypervisor/path",
	},
	ImageAsset: asset{
		Path: "/image/path",
	},
	KernelAsset: asset{
		Path: "/kernel/path",
	},
}

var hypervisorDetails2 = hypervisorDetails{
	HypervisorAsset: asset{
		Path: "/hypervisor/path2",
	},
	ImageAsset: asset{
		Path: "/image/path2",
	},
	KernelAsset: asset{
		Path: "/kernel/path2",
	},
}

var hypervisorDetails3 = hypervisorDetails{
	HypervisorAsset: asset{
		Path: "/hypervisor/path3",
	},
	ImageAsset: asset{
		Path: "/image/path3",
	},
	KernelAsset: asset{
		Path: "/kernel/path3",
	},
}

var testStatuses = []fullContainerState{
	{
		containerState: containerState{
			Version:        "",
			ID:             "1",
			InitProcessPid: 1234,
			Status:         "running",
			Bundle:         "/somewhere/over/the/rainbow",
			Created:        time.Now().UTC(),
			Annotations:    map[string]string(nil),
			Owner:          "#0",
		},

		CurrentHypervisorDetails: hypervisorDetails1,
		LatestHypervisorDetails:  hypervisorDetails1,
		StaleAssets:              []string{},
	},
	{
		containerState: containerState{
			Version:        "",
			ID:             "2",
			InitProcessPid: 2345,
			Status:         "stopped",
			Bundle:         "/this/path/is/invalid",
			Created:        time.Now().UTC(),
			Annotations:    map[string]string(nil),
			Owner:          "#0",
		},

		CurrentHypervisorDetails: hypervisorDetails2,
		LatestHypervisorDetails:  hypervisorDetails2,
		StaleAssets:              []string{},
	},
	{
		containerState: containerState{
			Version:        "",
			ID:             "3",
			InitProcessPid: 9999,
			Status:         "ready",
			Bundle:         "/foo/bar/baz",
			Created:        time.Now().UTC(),
			Annotations:    map[string]string(nil),
			Owner:          "#0",
		},

		CurrentHypervisorDetails: hypervisorDetails3,
		LatestHypervisorDetails:  hypervisorDetails3,
		StaleAssets:              []string{},
	},
}

// Implement the io.Writer interface
func (w *TestFileWriter) Write(bytes []byte) (n int, err error) {
	return w.File.Write(bytes)
}

func formatListDataAsBytes(formatter formatState, state []fullContainerState, showAll bool) (bytes []byte, err error) {
	tmpfile, err := ioutil.TempFile("", "formatListData-")
	if err != nil {
		return nil, err
	}

	defer os.Remove(tmpfile.Name())

	err = formatter.Write(state, showAll, tmpfile)
	if err != nil {
		return nil, err
	}

	tmpfile.Close()

	return ioutil.ReadFile(tmpfile.Name())
}

func formatListDataAsString(formatter formatState, state []fullContainerState, showAll bool) (lines []string, err error) {
	bytes, err := formatListDataAsBytes(formatter, state, showAll)
	if err != nil {
		return nil, err
	}

	lines = strings.Split(string(bytes), "\n")

	// Remove last line if empty
	length := len(lines)
	last := lines[length-1]
	if last == "" {
		lines = lines[:length-1]
	}

	return lines, nil
}

func TestStateToIDList(t *testing.T) {

	// no header
	expectedLength := len(testStatuses)

	// showAll should not affect the output
	for _, showAll := range []bool{true, false} {
		lines, err := formatListDataAsString(&formatIDList{}, testStatuses, showAll)
		if err != nil {
			t.Fatal(err)
		}

		var expected []string
		for _, s := range testStatuses {
			expected = append(expected, s.ID)
		}

		length := len(lines)

		if length != expectedLength {
			t.Fatalf("Expected %d lines, got %d: %v", expectedLength, length, lines)
		}

		assert.Equal(t, lines, expected, "lines + expected")
	}
}

func TestStateToTabular(t *testing.T) {
	// +1 for header line
	expectedLength := len(testStatuses) + 1

	expectedDefaultHeaderPattern := `\AID\s+PID\s+STATUS\s+BUNDLE\s+CREATED\s+OWNER`
	expectedExtendedHeaderPattern := `HYPERVISOR\s+KERNEL\s+IMAGE\s+LATEST-KERNEL\s+LATEST-IMAGE\s+STALE`
	endingPattern := `\s*\z`

	lines, err := formatListDataAsString(&formatTabular{}, testStatuses, false)
	if err != nil {
		t.Fatal(err)
	}

	length := len(lines)

	expectedHeaderPattern := expectedDefaultHeaderPattern + endingPattern
	expectedHeaderRE := regexp.MustCompile(expectedHeaderPattern)

	if length != expectedLength {
		t.Fatalf("Expected %d lines, got %d", expectedLength, length)
	}

	header := lines[0]

	matches := expectedHeaderRE.FindAllStringSubmatch(header, -1)
	if matches == nil {
		t.Fatalf("Header line failed to match:\n"+
			"pattern : %v\n"+
			"line    : %v\n",
			expectedDefaultHeaderPattern,
			header)
	}

	for i, status := range testStatuses {
		lineIndex := i + 1
		line := lines[lineIndex]

		expectedLinePattern := fmt.Sprintf(`\A%s\s+%d\s+%s\s+%s\s+%s\s+%s\s*\z`,
			regexp.QuoteMeta(status.ID),
			status.InitProcessPid,
			regexp.QuoteMeta(status.Status),
			regexp.QuoteMeta(status.Bundle),
			regexp.QuoteMeta(status.Created.Format(time.RFC3339Nano)),
			regexp.QuoteMeta(status.Owner))

		expectedLineRE := regexp.MustCompile(expectedLinePattern)

		matches := expectedLineRE.FindAllStringSubmatch(line, -1)
		if matches == nil {
			t.Fatalf("Data line failed to match:\n"+
				"pattern : %v\n"+
				"line    : %v\n",
				expectedLinePattern,
				line)
		}
	}

	// Try again with full details this time
	lines, err = formatListDataAsString(&formatTabular{}, testStatuses, true)
	if err != nil {
		t.Fatal(err)
	}

	length = len(lines)

	expectedHeaderPattern = expectedDefaultHeaderPattern + `\s+` + expectedExtendedHeaderPattern + endingPattern
	expectedHeaderRE = regexp.MustCompile(expectedHeaderPattern)

	if length != expectedLength {
		t.Fatalf("Expected %d lines, got %d", expectedLength, length)
	}

	header = lines[0]

	matches = expectedHeaderRE.FindAllStringSubmatch(header, -1)
	if matches == nil {
		t.Fatalf("Header line failed to match:\n"+
			"pattern : %v\n"+
			"line    : %v\n",
			expectedDefaultHeaderPattern,
			header)
	}

	for i, status := range testStatuses {
		lineIndex := i + 1
		line := lines[lineIndex]

		expectedLinePattern := fmt.Sprintf(`\A%s\s+%d\s+%s\s+%s\s+%s\s+%s\s+%s\s+%s\s+%s\s+%s\s+%s\s+%s\s*\z`,
			regexp.QuoteMeta(status.ID),
			status.InitProcessPid,
			regexp.QuoteMeta(status.Status),
			regexp.QuoteMeta(status.Bundle),
			regexp.QuoteMeta(status.Created.Format(time.RFC3339Nano)),
			regexp.QuoteMeta(status.Owner),
			regexp.QuoteMeta(status.CurrentHypervisorDetails.HypervisorAsset.Path),
			regexp.QuoteMeta(status.CurrentHypervisorDetails.KernelAsset.Path),
			regexp.QuoteMeta(status.CurrentHypervisorDetails.ImageAsset.Path),
			regexp.QuoteMeta(status.LatestHypervisorDetails.KernelAsset.Path),
			regexp.QuoteMeta(status.LatestHypervisorDetails.ImageAsset.Path),
			regexp.QuoteMeta("-"))

		expectedLineRE := regexp.MustCompile(expectedLinePattern)

		matches := expectedLineRE.FindAllStringSubmatch(line, -1)
		if matches == nil {
			t.Fatalf("Data line failed to match:\n"+
				"pattern : %v\n"+
				"line    : %v\n",
				expectedLinePattern,
				line)
		}
	}
}

func TestStateToJSON(t *testing.T) {
	expectedLength := len(testStatuses)

	// showAll should not affect the output
	for _, showAll := range []bool{true, false} {
		bytes, err := formatListDataAsBytes(&formatJSON{}, testStatuses, showAll)
		if err != nil {
			t.Fatal(err)
		}

		// Force capacity to match the original otherwise assert.Equal() complains.
		states := make([]fullContainerState, 0, len(testStatuses))

		err = json.Unmarshal(bytes, &states)
		if err != nil {
			t.Fatal(err)
		}

		length := len(states)

		if length != expectedLength {
			t.Fatalf("Expected %d lines, got %d", expectedLength, length)
		}

		// golang tip (what will presumably become v1.9) now
		// stores a monotonic clock value as part of time.Time's
		// internal representation (this is shown by a suffix in
		// the form "m=±ddd.nnnnnnnnn" when calling String() on
		// the time.Time object). However, this monotonic value
		// is stripped out when marshaling.
		//
		// This behaviour change makes comparing the original
		// object and the marshaled-and-then-unmarshaled copy of
		// the object doomed to failure.
		//
		// The solution? Manually strip the monotonic time out
		// of the original before comparison (yuck!)
		//
		// See:
		//
		// - https://go-review.googlesource.com/c/36255/7/src/time/time.go#54
		//
		for i := 0; i < expectedLength; i++ {
			// remove monotonic time part
			testStatuses[i].Created = testStatuses[i].Created.Truncate(0)
		}

		assert.Equal(t, states, testStatuses, "states + testStatuses")
	}
}

func TestListCLIFunctionNoContainers(t *testing.T) {
	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"
	ctx.App.Metadata = map[string]interface{}{
		"foo": "bar",
	}

	fn, ok := listCLICommand.Action.(func(context *cli.Context) error)
	assert.True(t, ok)

	err := fn(ctx)

	// no config in the Metadata
	assert.Error(t, err)
}

func TestListGetContainersListPodFail(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}

	_, err = getContainers(ctx)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))
}

func TestListGetContainers(t *testing.T) {
	assert := assert.New(t)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// No pre-existing pods
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}

	state, err := getContainers(ctx)
	assert.NoError(err)
	assert.Equal(state, []fullContainerState(nil))
}

func TestListGetContainersPodWithoutContainers(t *testing.T) {
	assert := assert.New(t)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID:               pod.ID(),
				ContainersStatus: []vc.ContainerStatus(nil),
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}

	state, err := getContainers(ctx)
	assert.NoError(err)
	assert.Equal(state, []fullContainerState(nil))
}

func TestListGetContainersPodWithContainer(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	rootfs := filepath.Join(tmpdir, "rootfs")
	err = os.MkdirAll(rootfs, testDirMode)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID:          pod.ID(),
						Annotations: map[string]string{},
						RootFs:      rootfs,
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	app := cli.NewApp()
	ctx := cli.NewContext(app, nil, nil)
	app.Name = "foo"

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}

	_, err = getContainers(ctx)
	assert.NoError(err)
}

func TestListCLIFunctionFormatFail(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	quietFlags := flag.NewFlagSet("test", 0)
	quietFlags.Bool("quiet", true, "")

	tableFlags := flag.NewFlagSet("test", 0)
	tableFlags.String("format", "table", "")

	jsonFlags := flag.NewFlagSet("test", 0)
	jsonFlags.String("format", "json", "")

	invalidFlags := flag.NewFlagSet("test", 0)
	invalidFlags.String("format", "not-a-valid-format", "")

	type testData struct {
		format string
		flags  *flag.FlagSet
	}

	data := []testData{
		{"quiet", quietFlags},
		{"table", tableFlags},
		{"json", jsonFlags},
		{"invalid", invalidFlags},
	}

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	rootfs := filepath.Join(tmpdir, "rootfs")

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
						},
						RootFs: rootfs,
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	savedOutputFile := defaultOutputFile
	defer func() {
		defaultOutputFile = savedOutputFile
	}()

	// purposely invalid
	var invalidFile *os.File

	for _, d := range data {
		// start off with an invalid output file
		defaultOutputFile = invalidFile

		app := cli.NewApp()
		ctx := cli.NewContext(app, d.flags, nil)
		app.Name = "foo"
		ctx.App.Metadata = map[string]interface{}{
			"foo": "bar",
		}

		fn, ok := listCLICommand.Action.(func(context *cli.Context) error)
		assert.True(ok, d)

		err = fn(ctx)

		// no config in the Metadata
		assert.Error(err, d)

		runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
		assert.NoError(err, d)

		ctx.App.Metadata["runtimeConfig"] = runtimeConfig

		_ = os.Remove(rootfs)

		err = fn(ctx)
		assert.Error(err)

		err = os.MkdirAll(rootfs, testDirMode)
		assert.NoError(err)

		err = fn(ctx)

		// invalid output file
		assert.Error(err, d)
		assert.False(vcMock.IsMockError(err), d)

		output := filepath.Join(tmpdir, "output")
		f, err := os.OpenFile(output, os.O_WRONLY|os.O_CREATE, testFileMode)
		assert.NoError(err)
		defer f.Close()

		// output file is now valid
		defaultOutputFile = f

		err = fn(ctx)
		if d.format == "invalid" {
			assert.Error(err)
			assert.False(vcMock.IsMockError(err), d)
		} else {
			assert.NoError(err)
		}
	}
}

func TestListCLIFunctionQuiet(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, testConsole, true)
	assert.NoError(err)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	rootfs := filepath.Join(tmpdir, "rootfs")
	err = os.MkdirAll(rootfs, testDirMode)
	assert.NoError(err)

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
						},
						RootFs: rootfs,
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	set := flag.NewFlagSet("test", 0)
	set.Bool("quiet", true, "")

	app := cli.NewApp()
	ctx := cli.NewContext(app, set, nil)
	app.Name = "foo"
	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}

	savedOutputFile := defaultOutputFile
	defer func() {
		defaultOutputFile = savedOutputFile
	}()

	output := filepath.Join(tmpdir, "output")
	f, err := os.OpenFile(output, os.O_CREATE|os.O_WRONLY|os.O_SYNC, testFileMode)
	assert.NoError(err)
	defer f.Close()

	defaultOutputFile = f

	fn, ok := listCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.NoError(err)
	f.Close()

	text, err := getFileContents(output)
	assert.NoError(err)

	trimmed := strings.TrimSpace(text)
	assert.Equal(testPodID, trimmed)
}

func TestListGetDirOwner(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir(testDir, "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	_, err = getDirOwner("")
	// invalid parameter
	assert.Error(err)

	dir := filepath.Join(tmpdir, "dir")

	_, err = getDirOwner(dir)
	// ENOENT
	assert.Error(err)

	err = createEmptyFile(dir)
	assert.NoError(err)

	_, err = getDirOwner(dir)
	// wrong file type
	assert.Error(err)

	err = os.Remove(dir)
	assert.NoError(err)

	err = os.MkdirAll(dir, testDirMode)
	assert.NoError(err)

	uid := uint32(os.Getuid())

	dirUID, err := getDirOwner(dir)
	assert.NoError(err)
	assert.Equal(dirUID, uid)
}
