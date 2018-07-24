// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/stretchr/testify/assert"
	"github.com/urfave/cli"
)

func TestRunCliAction(t *testing.T) {
	assert := assert.New(t)

	flagSet := flag.NewFlagSet("flag", flag.ContinueOnError)
	flagSet.Parse([]string{"runtime"})

	// create a new fake context
	ctx := cli.NewContext(&cli.App{Metadata: map[string]interface{}{}}, flagSet, nil)

	// get Action function
	actionFunc, ok := runCLICommand.Action.(func(ctx *cli.Context) error)
	assert.True(ok)

	err := actionFunc(ctx)
	assert.Error(err, "missing runtime configuration")

	// temporal dir to place container files
	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	// create a new runtime config
	runtimeConfig, err := newTestRuntimeConfig(tmpdir, "/dev/ptmx", true)
	assert.NoError(err)

	ctx.App.Metadata = map[string]interface{}{
		"runtimeConfig": runtimeConfig,
	}

	err = actionFunc(ctx)
	assert.Error(err, "run without args")
}

func TestRunInvalidArgs(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
		MockContainers: []*vcmock.Container{
			{MockID: testContainerID},
		},
	}

	// fake functions used to run containers
	testingImpl.CreateSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	testingImpl.StartSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	defer func() {
		testingImpl.CreateSandboxFunc = nil
		testingImpl.StartSandboxFunc = nil
	}()

	// temporal dir to place container files
	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	// create a new bundle
	bundlePath := filepath.Join(tmpdir, "bundle")

	err = os.MkdirAll(bundlePath, testDirMode)
	assert.NoError(err)

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	// pid file
	pidFilePath := filepath.Join(tmpdir, "pid")

	// console file
	consolePath := "/dev/ptmx"

	// inexistent path
	inexistentPath := "/this/path/does/not/exist"

	runtimeConfig, err := newTestRuntimeConfig(tmpdir, consolePath, true)
	assert.NoError(err)

	type testArgs struct {
		containerID   string
		bundle        string
		console       string
		consoleSocket string
		pidFile       string
		detach        bool
		runtimeConfig oci.RuntimeConfig
	}

	args := []testArgs{
		{"", "", "", "", "", true, oci.RuntimeConfig{}},
		{"", "", "", "", "", false, oci.RuntimeConfig{}},
		{"", "", "", "", "", true, runtimeConfig},
		{"", "", "", "", "", false, runtimeConfig},
		{"", "", "", "", pidFilePath, false, runtimeConfig},
		{"", "", "", "", inexistentPath, false, runtimeConfig},
		{"", "", "", "", pidFilePath, false, runtimeConfig},
		{"", "", "", inexistentPath, pidFilePath, false, runtimeConfig},
		{"", "", inexistentPath, inexistentPath, pidFilePath, false, runtimeConfig},
		{"", "", inexistentPath, "", pidFilePath, false, runtimeConfig},
		{"", "", consolePath, "", pidFilePath, false, runtimeConfig},
		{"", bundlePath, consolePath, "", pidFilePath, false, runtimeConfig},
		{testContainerID, inexistentPath, consolePath, "", pidFilePath, false, oci.RuntimeConfig{}},
		{testContainerID, inexistentPath, consolePath, "", inexistentPath, false, oci.RuntimeConfig{}},
		{testContainerID, bundlePath, consolePath, "", pidFilePath, false, oci.RuntimeConfig{}},
		{testContainerID, inexistentPath, consolePath, "", pidFilePath, false, runtimeConfig},
		{testContainerID, inexistentPath, consolePath, "", inexistentPath, false, runtimeConfig},
		{testContainerID, bundlePath, consolePath, "", pidFilePath, false, runtimeConfig},
	}

	for i, a := range args {
		err := run(a.containerID, a.bundle, a.console, a.consoleSocket, a.pidFile, a.detach, a.runtimeConfig)
		assert.Errorf(err, "test %d (%+v)", i, a)
	}
}

type runContainerData struct {
	pidFilePath   string
	consolePath   string
	bundlePath    string
	configJSON    string
	sandbox       *vcmock.Sandbox
	runtimeConfig oci.RuntimeConfig
	process       *os.Process
	tmpDir        string
}

func testRunContainerSetup(t *testing.T) runContainerData {
	assert := assert.New(t)

	// create a fake container workload
	workload := []string{"/bin/sleep", "10"}
	cmd := exec.Command(workload[0], workload[1:]...)
	err := cmd.Start()
	assert.NoError(err, "unable to start fake container workload %+v: %s", workload, err)

	// temporal dir to place container files
	// Note - it is returned to the caller, who does the defer remove to clean up.
	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)

	// pid file
	pidFilePath := filepath.Join(tmpdir, "pid")

	// console file
	consolePath := "/dev/ptmx"

	// create a new bundle
	bundlePath := filepath.Join(tmpdir, "bundle")

	err = makeOCIBundle(bundlePath)
	assert.NoError(err)

	// config json path
	configPath := filepath.Join(bundlePath, specConfig)

	// sandbox id and container id must be the same otherwise delete will not works
	sandbox := &vcmock.Sandbox{
		MockID: testContainerID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      testContainerID,
			MockPid:     cmd.Process.Pid,
			MockSandbox: sandbox,
		},
	}

	// create a new runtime config
	runtimeConfig, err := newTestRuntimeConfig(tmpdir, consolePath, true)
	assert.NoError(err)

	configJSON, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	return runContainerData{
		pidFilePath:   pidFilePath,
		consolePath:   consolePath,
		bundlePath:    bundlePath,
		configJSON:    configJSON,
		sandbox:       sandbox,
		runtimeConfig: runtimeConfig,
		process:       cmd.Process,
		tmpDir:        tmpdir,
	}
}

func TestRunContainerSuccessful(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	// this flags is used to detect if createSandboxFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreateSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		flagCreate = true
		return d.sandbox, nil
	}

	testingImpl.StartSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return d.sandbox, nil
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return vc.ContainerStatus{}, nil
		}

		// return a sandboxStatus with the container status
		return vc.ContainerStatus{
			ID: d.sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    d.configJSON,
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		// now we can kill the fake container workload
		err := d.process.Kill()
		assert.NoError(err)

		return d.sandbox.MockContainers[0], nil
	}

	testingImpl.DeleteSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return d.sandbox, nil
	}

	testingImpl.DeleteContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		return d.sandbox.MockContainers[0], nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
		testingImpl.StartSandboxFunc = nil
		testingImpl.StatusContainerFunc = nil
		testingImpl.StartContainerFunc = nil
		testingImpl.DeleteSandboxFunc = nil
		testingImpl.DeleteContainerFunc = nil
	}()

	err = run(d.sandbox.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)

	// should return ExitError with the message and exit code
	e, ok := err.(*cli.ExitError)
	assert.True(ok, "error should be a cli.ExitError: %s", err)
	assert.Empty(e.Error())
	assert.NotZero(e.ExitCode())
}

func TestRunContainerDetachSuccessful(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	// this flags is used to detect if createSandboxFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreateSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		flagCreate = true
		return d.sandbox, nil
	}

	testingImpl.StartSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return d.sandbox, nil
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return vc.ContainerStatus{}, nil
		}

		// return a sandboxStatus with the container status
		return vc.ContainerStatus{
			ID: d.sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    d.configJSON,
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		// now we can kill the fake container workload
		err := d.process.Kill()
		assert.NoError(err)

		return d.sandbox.MockContainers[0], nil
	}

	testingImpl.DeleteSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return d.sandbox, nil
	}

	testingImpl.DeleteContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		return d.sandbox.MockContainers[0], nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
		testingImpl.StartSandboxFunc = nil
		testingImpl.StatusContainerFunc = nil
		testingImpl.StartContainerFunc = nil
		testingImpl.DeleteSandboxFunc = nil
		testingImpl.DeleteContainerFunc = nil
	}()

	err = run(d.sandbox.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, true, d.runtimeConfig)

	// should not return ExitError
	assert.NoError(err)
}

func TestRunContainerDeleteFail(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	// this flags is used to detect if createSandboxFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreateSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		flagCreate = true
		return d.sandbox, nil
	}

	testingImpl.StartSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return d.sandbox, nil
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return vc.ContainerStatus{}, nil
		}

		// return a sandboxStatus with the container status
		return vc.ContainerStatus{
			ID: d.sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    d.configJSON,
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		// now we can kill the fake container workload
		err := d.process.Kill()
		assert.NoError(err)

		return d.sandbox.MockContainers[0], nil
	}

	testingImpl.DeleteSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		// return an error to provoke a failure in delete
		return nil, fmt.Errorf("DeleteSandboxFunc")
	}

	testingImpl.DeleteContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		// return an error to provoke a failure in delete
		return d.sandbox.MockContainers[0], fmt.Errorf("DeleteContainerFunc")
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
		testingImpl.StartSandboxFunc = nil
		testingImpl.StatusContainerFunc = nil
		testingImpl.StartContainerFunc = nil
		testingImpl.DeleteSandboxFunc = nil
		testingImpl.DeleteContainerFunc = nil
	}()

	err = run(d.sandbox.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)

	// should not return ExitError
	err, ok := err.(*cli.ExitError)
	assert.False(ok, "error should not be a cli.ExitError: %s", err)
}

func TestRunContainerWaitFail(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	// this flags is used to detect if createSandboxFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreateSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		flagCreate = true
		return d.sandbox, nil
	}

	testingImpl.StartSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		return d.sandbox, nil
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return vc.ContainerStatus{}, nil
		}

		// return a sandboxStatus with the container status
		return vc.ContainerStatus{
			ID: d.sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    d.configJSON,
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		// now we can kill the fake container workload
		err := d.process.Kill()
		assert.NoError(err)

		// change PID to provoke a failure in Wait
		d.sandbox.MockContainers[0].MockPid = -1

		return d.sandbox.MockContainers[0], nil
	}

	testingImpl.DeleteSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		// return an error to provoke a failure in delete
		return nil, fmt.Errorf("DeleteSandboxFunc")
	}

	testingImpl.DeleteContainerFunc = func(sandboxID, containerID string) (vc.VCContainer, error) {
		// return an error to provoke a failure in delete
		return d.sandbox.MockContainers[0], fmt.Errorf("DeleteContainerFunc")
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
		testingImpl.StartSandboxFunc = nil
		testingImpl.StatusContainerFunc = nil
		testingImpl.StartContainerFunc = nil
		testingImpl.DeleteSandboxFunc = nil
		testingImpl.DeleteContainerFunc = nil
	}()

	err = run(d.sandbox.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)

	// should not return ExitError
	err, ok := err.(*cli.ExitError)
	assert.False(ok, "error should not be a cli.ExitError: %s", err)
}

func TestRunContainerStartFail(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	// now we can kill the fake container workload
	err := d.process.Kill()
	assert.NoError(err)

	// this flags is used to detect if createSandboxFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreateSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		flagCreate = true
		return d.sandbox, nil
	}

	testingImpl.StartSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		// start fails
		return nil, fmt.Errorf("StartSandbox")
	}

	path, err := ioutil.TempDir("", "containers-mapping")
	assert.NoError(err)
	defer os.RemoveAll(path)
	ctrsMapTreePath = path

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return vc.ContainerStatus{}, nil
		}

		// return a sandboxStatus with the container status
		return vc.ContainerStatus{
			ID: d.sandbox.ID(),
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
				vcAnnotations.ConfigJSONKey:    d.configJSON,
			},
		}, nil
	}

	defer func() {
		testingImpl.CreateSandboxFunc = nil
		testingImpl.StartSandboxFunc = nil
		testingImpl.StatusContainerFunc = nil
	}()

	err = run(d.sandbox.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)

	// should not return ExitError
	err, ok := err.(*cli.ExitError)
	assert.False(ok, "error should not be a cli.ExitError: %s", err)
}

func TestRunContainerStartFailExistingContainer(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	sandbox.MockContainers = []*vcmock.Container{
		{
			MockID:      testContainerID,
			MockSandbox: sandbox,
		},
	}

	path, err := createTempContainerIDMapping(testContainerID, sandbox.ID())
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(sandboxID, containerID string) (vc.ContainerStatus, error) {
		// return the container status
		return vc.ContainerStatus{
			ID: testContainerID,
			Annotations: map[string]string{
				vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
			},
		}, nil
	}

	testingImpl.CreateSandboxFunc = func(sandboxConfig vc.SandboxConfig) (vc.VCSandbox, error) {
		return sandbox, nil
	}

	testingImpl.StartSandboxFunc = func(sandboxID string) (vc.VCSandbox, error) {
		// force no containers
		sandbox.MockContainers = nil

		return sandbox, nil
	}

	defer func() {
		testingImpl.StatusContainerFunc = nil
		testingImpl.CreateSandboxFunc = nil
		testingImpl.StartSandboxFunc = nil
	}()

	err = run(d.sandbox.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)
	assert.Error(err)
	assert.False(vcmock.IsMockError(err))
}
