// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"flag"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestExecCLIFunction(t *testing.T) {
	assert := assert.New(t)

	flagSet := &flag.FlagSet{}
	ctx := createCLIContext(flagSet)

	fn, ok := startCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// no container-id in the Metadata
	err := fn(ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	path, err := createTempContainerIDMapping("xyz", "xyz")
	assert.NoError(err)
	defer os.RemoveAll(path)

	// pass container-id
	flagSet = flag.NewFlagSet("container-id", flag.ContinueOnError)
	flagSet.Parse([]string{"xyz"})
	ctx = createCLIContext(flagSet)

	err = fn(ctx)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))
}

func TestExecuteErrors(t *testing.T) {
	assert := assert.New(t)

	flagSet := flag.NewFlagSet("", 0)
	ctx := createCLIContext(flagSet)

	// missing container id
	err := execute(context.Background(), ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	// StatusSandbox error
	flagSet.Parse([]string{testContainerID})
	err = execute(context.Background(), ctx)
	assert.Error(err)
	assert.True(vcmock.IsMockError(err))

	// Config path missing in annotations
	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
	}

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, types.State{}, annotations), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	err = execute(context.Background(), ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	// Container state undefined
	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations = map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	containerState := types.State{}
	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, containerState, annotations), nil
	}

	err = execute(context.Background(), ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	// Container paused
	containerState = types.State{
		State: types.StatePaused,
	}
	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, containerState, annotations), nil
	}

	err = execute(context.Background(), ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	// Container stopped
	containerState = types.State{
		State: types.StateStopped,
	}
	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, containerState, annotations), nil
	}

	err = execute(context.Background(), ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
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
	ctx := createCLIContext(flagSet)

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	// Note: flags can only be tested with the CLI command function
	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
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
	ctx := createCLIContext(flagSet)

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	// Note: flags can only be tested with the CLI command function
	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
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
	ctx := createCLIContext(flagSet)

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// EnterContainer error
	err = fn(ctx)
	assert.Error(err)

	assert.True(vcmock.IsMockError(err))

	testingImpl.EnterContainerFunc = func(ctx context.Context, sandboxID, containerID string, cmd types.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error) {
		return &vcmock.Sandbox{}, &vcmock.Container{}, &vc.Process{}, nil
	}

	defer func() {
		testingImpl.EnterContainerFunc = nil
		os.Remove(pidFilePath)
	}()

	// Process not running error
	err = fn(ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))

	os.Remove(pidFilePath)

	// Process ran and exited successfully
	testingImpl.EnterContainerFunc = func(ctx context.Context, sandboxID, containerID string, cmd types.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error) {
		// create a fake container process
		workload := []string{"cat", "/dev/null"}
		command := exec.Command(workload[0], workload[1:]...)
		err := command.Start()
		assert.NoError(err, "Unable to start process %v: %s", workload, err)

		vcProcess := vc.Process{}
		vcProcess.Pid = command.Process.Pid
		return &vcmock.Sandbox{}, &vcmock.Container{}, &vcProcess, nil
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
	ctx := createCLIContext(flagSet)

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	testingImpl.EnterContainerFunc = func(ctx context.Context, sandboxID, containerID string, cmd types.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error) {
		// create a fake container process
		workload := []string{"cat", "/dev/null"}
		command := exec.Command(workload[0], workload[1:]...)
		err := command.Start()
		assert.NoError(err, "Unable to start process %v: %s", workload, err)

		vcProcess := vc.Process{}
		vcProcess.Pid = command.Process.Pid
		return &vcmock.Sandbox{}, &vcmock.Container{}, &vcProcess, nil
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
	ctx := createCLIContext(flagSet)

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
	}()

	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	err = fn(ctx)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
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
	ctx := createCLIContext(flagSet)

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
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

	testingImpl.EnterContainerFunc = func(ctx context.Context, sandboxID, containerID string, cmd types.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error) {
		// create a fake container process
		command := exec.Command(workload[0], workload[1:]...)
		err := command.Start()
		assert.NoError(err, "Unable to start process %v: %s", workload, err)

		vcProcess := vc.Process{}
		vcProcess.Pid = command.Process.Pid

		return &vcmock.Sandbox{}, &vcmock.Container{}, &vcProcess, nil
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

func TestExecuteWithEmptyEnvironmentValue(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	pidFilePath := filepath.Join(tmpdir, "pid")
	consolePath := "/dev/ptmx"

	flagSet := testExecParamsSetup(t, pidFilePath, consolePath, false)

	processPath := filepath.Join(tmpdir, "process.json")

	flagSet.String("process", processPath, "")
	flagSet.Parse([]string{testContainerID})
	ctx := createCLIContext(flagSet)

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	annotations := map[string]string{
		vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
		vcAnnotations.ConfigJSONKey:    configJSON,
	}

	state := types.State{
		State: types.StateRunning,
	}

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, annotations), nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
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
					"TERM="
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

	testingImpl.EnterContainerFunc = func(ctx context.Context, sandboxID, containerID string, cmd types.Cmd) (vc.VCSandbox, vc.VCContainer, *vc.Process, error) {
		// create a fake container process
		command := exec.Command(workload[0], workload[1:]...)
		err := command.Start()
		assert.NoError(err, "Unable to start process %v: %s", workload, err)

		vcProcess := vc.Process{}
		vcProcess.Pid = command.Process.Pid

		return &vcmock.Sandbox{}, &vcmock.Container{}, &vcProcess, nil
	}

	defer func() {
		testingImpl.EnterContainerFunc = nil
		os.Remove(pidFilePath)
	}()

	fn, ok := execCLICommand.Action.(func(context *cli.Context) error)
	assert.True(ok)

	// vcAnnotations.EnvVars error due to incorrect environment
	err = fn(ctx)
	exitErr, ok := err.(*cli.ExitError)
	assert.True(ok, true, "Exit code not received for fake workload process")
	assert.Equal(exitErr.ExitCode(), 0, "Exit code should have been 0 for empty environment variable value")
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

	ctx := createCLIContext(flagSet)
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
	ctx := createCLIContext(flagSet)

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
