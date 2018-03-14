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
	"flag"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcMock"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestExecCLIFunction(t *testing.T) {
	assert := assert.New(t)

	flagSet := &flag.FlagSet{}
	app := cli.NewApp()
	ctx := cli.NewContext(app, flagSet, nil)

	fn, ok := startCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// no container-id in the Metadata
	err := fn(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	// pass container-id
	flagSet = flag.NewFlagSet("container-id", flag.ContinueOnError)
	flagSet.Parse([]string{"xyz"})
	ctx = cli.NewContext(app, flagSet, nil)

	err = fn(ctx)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))
}

func TestExecuteErrors(t *testing.T) {
	assert := assert.New(t)

	flagSet := flag.NewFlagSet("", 0)
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	// missing container id
	err := execute(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	// ListPod error
	flagSet.Parse([]string{testContainerID})
	err = execute(ctx)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	// Config path missing in annotations
	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, vc.State{}, vc.State{}, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	err = execute(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	// Container not running
	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations = map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, vc.State{}, vc.State{}, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	err = execute(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestExecuteErrorReadingProcessJson(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	// non-existent path
	processPath := filepath.Join(tmpdir, "process.json")

	flagSet := flag.NewFlagSet("", 0)
	flagSet.String("process", processPath, "")
	flagSet.Parse([]string{testContainerID})
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	// Note: flags can only be tested with the CLI command function
	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestExecuteErrorOpeningConsole(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	consoleSock := filepath.Join(tmpdir, "console-sock")

	flagSet := flag.NewFlagSet("", 0)
	flagSet.String("console-socket", consoleSock, "")
	flagSet.Parse([]string{testContainerID})
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	// Note: flags can only be tested with the CLI command function
	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func testExecParamsSetup(t *testing.T, pidFilePath, consolePath string, detach bool) *flag.FlagSet {
	flagSet := flag.NewFlagSet("", 0)

	flagSet.String("pid-file", pidFilePath, "")
	flagSet.String("console", consolePath, "")
	flagSet.String("console-socket", "", "")
	flagSet.Bool("detach", detach, "")
	flagSet.String("process-label", "testlabel", "")
	flagSet.Bool("no-subreaper", false, "")

	return flagSet
}

func TestExecuteWithFlags(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	pidFilePath := filepath.Join(tmpdir, "pid")
	consolePath := "/dev/ptmx"

	flagSet := testExecParamsSetup(t, pidFilePath, consolePath, false)
	flagSet.String("user", "root", "")
	flagSet.String("cwd", "/home/root", "")
	flagSet.String("apparmor", "/tmp/profile", "")
	flagSet.Bool("no-new-privs", false, "")

	flagSet.Parse([]string{testContainerID, "/tmp/foo"})
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// EnterContainer error
	err = fn(ctx)
	assert.Error(err)
	assert.True(vcMock.IsMockError(err))

	testingImpl.EnterContainerFunc = func(podID, containerID string, cmd vc.Cmd) (vc.VCPod, vc.VCContainer, *vc.Process, error) {
		return &vcMock.Pod{}, &vcMock.Container{}, &vc.Process{}, nil
	}

	defer func() {
		testingImpl.EnterContainerFunc = nil
		os.Remove(pidFilePath)
	}()

	// Process not running error
	err = fn(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))

	os.Remove(pidFilePath)

	// Process ran and exited successfully
	testingImpl.EnterContainerFunc = func(podID, containerID string, cmd vc.Cmd) (vc.VCPod, vc.VCContainer, *vc.Process, error) {
		// create a fake container process
		workload := []string{"cat", "/dev/null"}
		command := exec.Command(workload[0], workload[1:]...)
		err := command.Start()
		assert.NoError(err, "Unable to start process %v: %s", workload, err)

		vcProcess := vc.Process{}
		vcProcess.Pid = command.Process.Pid
		return &vcMock.Pod{}, &vcMock.Container{}, &vcProcess, nil
	}

	defer func() {
		testingImpl.EnterContainerFunc = nil
		os.Remove(pidFilePath)
	}()

	// Should get an exit code when run in non-detached mode.
	err = fn(ctx)
	_, ok = err.(*cli.ExitError)
	assert.True(ok, true, "Exit code not received for fake workload process")
}

func TestExecuteWithFlagsDetached(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	pidFilePath := filepath.Join(tmpdir, "pid")
	consolePath := "/dev/ptmx"
	detach := true

	flagSet := testExecParamsSetup(t, pidFilePath, consolePath, detach)
	flagSet.Parse([]string{testContainerID, "/tmp/foo"})
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	testingImpl.EnterContainerFunc = func(podID, containerID string, cmd vc.Cmd) (vc.VCPod, vc.VCContainer, *vc.Process, error) {
		// create a fake container process
		workload := []string{"cat", "/dev/null"}
		command := exec.Command(workload[0], workload[1:]...)
		err := command.Start()
		assert.NoError(err, "Unable to start process %v: %s", workload, err)

		vcProcess := vc.Process{}
		vcProcess.Pid = command.Process.Pid
		return &vcMock.Pod{}, &vcMock.Container{}, &vcProcess, nil
	}

	defer func() {
		testingImpl.EnterContainerFunc = nil
		os.Remove(pidFilePath)
	}()

	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.NoError(err)
}

func TestExecuteWithInvalidProcessJson(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	pidFilePath := filepath.Join(tmpdir, "pid")
	consolePath := "/dev/ptmx"
	detach := false

	flagSet := testExecParamsSetup(t, pidFilePath, consolePath, detach)

	processPath := filepath.Join(tmpdir, "process.json")
	flagSet.String("process", processPath, "")

	f, err := os.OpenFile(processPath, os.O_RDWR|os.O_CREATE, testFileMode)
	assert.NoError(err)

	// invalidate the JSON
	_, err = f.WriteString("{")
	assert.NoError(err)
	f.Close()

	defer os.Remove(processPath)

	flagSet.Parse([]string{testContainerID})
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestExecuteWithValidProcessJson(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	pidFilePath := filepath.Join(tmpdir, "pid")
	consolePath := "/dev/ptmx"

	flagSet := testExecParamsSetup(t, pidFilePath, consolePath, false)

	processPath := filepath.Join(tmpdir, "process.json")
	flagSet.String("process", processPath, "")

	flagSet.Parse([]string{testContainerID, "/tmp/foo"})
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	processJSON := `{
				"consoleSize": {
					"height": 15,
					"width": 15
				},
				"terminal": true,
				"user": {
					"uid": 0,
					"gid": 0
				},
				"args": [
					"sh"
				],
				"env": [
					"PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
					"TERM=xterm"
				],
				"cwd": "/"
			}`

	f, err := os.OpenFile(processPath, os.O_RDWR|os.O_CREATE, testFileMode)
	assert.NoError(err)

	_, err = f.WriteString(processJSON)
	assert.NoError(err)
	f.Close()

	defer os.Remove(processPath)

	workload := []string{"cat", "/dev/null"}

	testingImpl.EnterContainerFunc = func(podID, containerID string, cmd vc.Cmd) (vc.VCPod, vc.VCContainer, *vc.Process, error) {
		// create a fake container process
		command := exec.Command(workload[0], workload[1:]...)
		err := command.Start()
		assert.NoError(err, "Unable to start process %v: %s", workload, err)

		vcProcess := vc.Process{}
		vcProcess.Pid = command.Process.Pid

		return &vcMock.Pod{}, &vcMock.Container{}, &vcProcess, nil
	}

	defer func() {
		testingImpl.EnterContainerFunc = nil
		os.Remove(pidFilePath)
	}()

	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	exitErr, ok := err.(*cli.ExitError)
	assert.True(ok, true, "Exit code not received for fake workload process")
	assert.Equal(exitErr.ExitCode(), 0, "Exit code should have been 0 for fake workload %s", workload)
}

func TestExecuteWithInvalidEnvironment(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	processPath := filepath.Join(tmpdir, "process.json")

	flagSet := flag.NewFlagSet("", 0)
	flagSet.String("process", processPath, "")
	flagSet.Parse([]string{testContainerID})
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	configPath := testConfigSetup(t)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return newSingleContainerPodStatusList(testPodID, testContainerID, state, state, annotations), nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
	}()

	processJSON := `{
				"env": [
					"TERM="
				]
			}`

	f, err := os.OpenFile(processPath, os.O_RDWR|os.O_CREATE, testFileMode)
	assert.NoError(err)

	_, err = f.WriteString(processJSON)
	assert.NoError(err)
	f.Close()

	defer os.Remove(processPath)

	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// vcAnnotations.EnvVars error due to incorrect environment
	err = fn(ctx)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}

func TestGenerateExecParams(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	pidFilePath := filepath.Join(tmpdir, "pid")
	consolePath := "/dev/ptmx"
	consoleSocket := "/tmp/console-sock"
	processLabel := "testlabel"
	user := "root"
	cwd := "cwd"
	apparmor := "apparmorProf"

	flagSet := flag.NewFlagSet("", 0)
	flagSet.String("pid-file", pidFilePath, "")
	flagSet.String("console", consolePath, "")
	flagSet.String("console-socket", consoleSocket, "")
	flagSet.String("process-label", processLabel, "")

	flagSet.String("user", user, "")
	flagSet.String("cwd", cwd, "")
	flagSet.String("apparmor", apparmor, "")

	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)
	process := &oci.CompatOCIProcess{}
	params, err := generateExecParams(ctx, process)
	assert.NoError(err)

	assert.Equal(params.pidFile, pidFilePath)
	assert.Equal(params.console, consolePath)
	assert.Equal(params.consoleSock, consoleSocket)
	assert.Equal(params.processLabel, processLabel)
	assert.Equal(params.noSubreaper, false)
	assert.Equal(params.detach, false)

	assert.Equal(params.ociProcess.Terminal, false)
	assert.Equal(params.ociProcess.User.UID, uint32(0))
	assert.Equal(params.ociProcess.User.GID, uint32(0))
	assert.Equal(params.ociProcess.Cwd, cwd)
	assert.Equal(params.ociProcess.ApparmorProfile, apparmor)
}

func TestGenerateExecParamsWithProcessJsonFile(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	pidFilePath := filepath.Join(tmpdir, "pid")
	consolePath := "/dev/ptmx"
	consoleSocket := "/tmp/console-sock"
	detach := true
	processLabel := "testlabel"

	flagSet := flag.NewFlagSet("", 0)
	flagSet.String("pid-file", pidFilePath, "")
	flagSet.String("console", consolePath, "")
	flagSet.String("console-socket", consoleSocket, "")
	flagSet.Bool("detach", detach, "")
	flagSet.String("process-label", processLabel, "")

	processPath := filepath.Join(tmpdir, "process.json")
	flagSet.String("process", processPath, "")

	flagSet.Parse([]string{testContainerID})
	ctx := cli.NewContext(cli.NewApp(), flagSet, nil)

	processJSON := `{
				"consoleSize": {
					"height": 15,
					"width": 15
				},
				"terminal": true,
				"user": {
					"uid": 0,
					"gid": 0
				},
				"args": [
					"sh"
				],
				"env": [
					"TERM=xterm",
					"foo=bar"
				],
				"cwd": "/"
			}`

	f, err := os.OpenFile(processPath, os.O_RDWR|os.O_CREATE, testFileMode)
	assert.NoError(err)

	_, err = f.WriteString(processJSON)
	assert.NoError(err)
	f.Close()

	defer os.Remove(processPath)

	process := &oci.CompatOCIProcess{}
	params, err := generateExecParams(ctx, process)
	assert.NoError(err)

	assert.Equal(params.pidFile, pidFilePath)
	assert.Equal(params.console, consolePath)
	assert.Equal(params.consoleSock, consoleSocket)
	assert.Equal(params.processLabel, processLabel)
	assert.Equal(params.noSubreaper, false)
	assert.Equal(params.detach, detach)

	assert.Equal(params.ociProcess.Terminal, true)
	assert.Equal(params.ociProcess.ConsoleSize.Height, uint(15))
	assert.Equal(params.ociProcess.ConsoleSize.Width, uint(15))
	assert.Equal(params.ociProcess.User.UID, uint32(0))
	assert.Equal(params.ociProcess.User.GID, uint32(0))
	assert.Equal(params.ociProcess.Cwd, "/")
	assert.Equal(params.ociProcess.Env[0], "TERM=xterm")
	assert.Equal(params.ociProcess.Env[1], "foo=bar")
}
