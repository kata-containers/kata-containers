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
	"fmt"
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

	pod := &vcMock.Pod{
		MockID: testPodID,
		MockContainers: []*vcMock.Container{
			{MockID: testContainerID},
		},
	}

	// fake functions used to run containers
	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return pod, nil
	}

	testingImpl.StartPodFunc = func(podID string) (vc.VCPod, error) {
		return pod, nil
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		return []vc.PodStatus{}, nil
	}

	defer func() {
		testingImpl.CreatePodFunc = nil
		testingImpl.StartPodFunc = nil
		testingImpl.ListPodFunc = nil
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
	pod           *vcMock.Pod
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

	// pod id and container id must be the same otherwise delete will not works
	pod := &vcMock.Pod{
		MockID: testContainerID,
	}

	pod.MockContainers = []*vcMock.Container{
		{
			MockID:  testContainerID,
			MockPid: cmd.Process.Pid,
			MockPod: pod,
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
		pod:           pod,
		runtimeConfig: runtimeConfig,
		process:       cmd.Process,
		tmpDir:        tmpdir,
	}
}

func TestRunContainerSuccessful(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	// this flags is used to detect if createPodFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		flagCreate = true
		return d.pod, nil
	}

	testingImpl.StartPodFunc = func(podID string) (vc.VCPod, error) {
		return d.pod, nil
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return []vc.PodStatus{}, nil
		}

		// return a podStatus with the container status
		return []vc.PodStatus{
			{
				ID: d.pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: d.pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
							vcAnnotations.ConfigJSONKey:    d.configJSON,
						},
					},
				},
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		// now we can kill the fake container workload
		err := d.process.Kill()
		assert.NoError(err)

		return d.pod.MockContainers[0], nil
	}

	testingImpl.DeletePodFunc = func(podID string) (vc.VCPod, error) {
		return d.pod, nil
	}

	testingImpl.DeleteContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return d.pod.MockContainers[0], nil
	}

	defer func() {
		testingImpl.CreatePodFunc = nil
		testingImpl.StartPodFunc = nil
		testingImpl.ListPodFunc = nil
		testingImpl.StartContainerFunc = nil
		testingImpl.DeletePodFunc = nil
		testingImpl.DeleteContainerFunc = nil
	}()

	err := run(d.pod.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)

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

	// this flags is used to detect if createPodFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		flagCreate = true
		return d.pod, nil
	}

	testingImpl.StartPodFunc = func(podID string) (vc.VCPod, error) {
		return d.pod, nil
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return []vc.PodStatus{}, nil
		}

		// return a podStatus with the container status
		return []vc.PodStatus{
			{
				ID: d.pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: d.pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
							vcAnnotations.ConfigJSONKey:    d.configJSON,
						},
					},
				},
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		// now we can kill the fake container workload
		err := d.process.Kill()
		assert.NoError(err)

		return d.pod.MockContainers[0], nil
	}

	testingImpl.DeletePodFunc = func(podID string) (vc.VCPod, error) {
		return d.pod, nil
	}

	testingImpl.DeleteContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		return d.pod.MockContainers[0], nil
	}

	defer func() {
		testingImpl.CreatePodFunc = nil
		testingImpl.StartPodFunc = nil
		testingImpl.ListPodFunc = nil
		testingImpl.StartContainerFunc = nil
		testingImpl.DeletePodFunc = nil
		testingImpl.DeleteContainerFunc = nil
	}()

	err := run(d.pod.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, true, d.runtimeConfig)

	// should not return ExitError
	assert.NoError(err)
}

func TestRunContainerDeleteFail(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	// this flags is used to detect if createPodFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		flagCreate = true
		return d.pod, nil
	}

	testingImpl.StartPodFunc = func(podID string) (vc.VCPod, error) {
		return d.pod, nil
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return []vc.PodStatus{}, nil
		}

		// return a podStatus with the container status
		return []vc.PodStatus{
			{
				ID: d.pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: d.pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
							vcAnnotations.ConfigJSONKey:    d.configJSON,
						},
					},
				},
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		// now we can kill the fake container workload
		err := d.process.Kill()
		assert.NoError(err)

		return d.pod.MockContainers[0], nil
	}

	testingImpl.DeletePodFunc = func(podID string) (vc.VCPod, error) {
		// return an error to provoke a failure in delete
		return nil, fmt.Errorf("DeletePodFunc")
	}

	testingImpl.DeleteContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		// return an error to provoke a failure in delete
		return d.pod.MockContainers[0], fmt.Errorf("DeleteContainerFunc")
	}

	defer func() {
		testingImpl.CreatePodFunc = nil
		testingImpl.StartPodFunc = nil
		testingImpl.ListPodFunc = nil
		testingImpl.StartContainerFunc = nil
		testingImpl.DeletePodFunc = nil
		testingImpl.DeleteContainerFunc = nil
	}()

	err := run(d.pod.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)

	// should not return ExitError
	err, ok := err.(*cli.ExitError)
	assert.False(ok, "error should not be a cli.ExitError: %s", err)
}

func TestRunContainerWaitFail(t *testing.T) {
	assert := assert.New(t)

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	// this flags is used to detect if createPodFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		flagCreate = true
		return d.pod, nil
	}

	testingImpl.StartPodFunc = func(podID string) (vc.VCPod, error) {
		return d.pod, nil
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return []vc.PodStatus{}, nil
		}

		// return a podStatus with the container status
		return []vc.PodStatus{
			{
				ID: d.pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: d.pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
							vcAnnotations.ConfigJSONKey:    d.configJSON,
						},
					},
				},
			},
		}, nil
	}

	testingImpl.StartContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		// now we can kill the fake container workload
		err := d.process.Kill()
		assert.NoError(err)

		// change PID to provoke a failure in Wait
		d.pod.MockContainers[0].MockPid = -1

		return d.pod.MockContainers[0], nil
	}

	testingImpl.DeletePodFunc = func(podID string) (vc.VCPod, error) {
		// return an error to provoke a failure in delete
		return nil, fmt.Errorf("DeletePodFunc")
	}

	testingImpl.DeleteContainerFunc = func(podID, containerID string) (vc.VCContainer, error) {
		// return an error to provoke a failure in delete
		return d.pod.MockContainers[0], fmt.Errorf("DeleteContainerFunc")
	}

	defer func() {
		testingImpl.CreatePodFunc = nil
		testingImpl.StartPodFunc = nil
		testingImpl.ListPodFunc = nil
		testingImpl.StartContainerFunc = nil
		testingImpl.DeletePodFunc = nil
		testingImpl.DeleteContainerFunc = nil
	}()

	err := run(d.pod.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)

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

	// this flags is used to detect if createPodFunc was called
	flagCreate := false

	// fake functions used to run containers
	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		flagCreate = true
		return d.pod, nil
	}

	testingImpl.StartPodFunc = func(podID string) (vc.VCPod, error) {
		// start fails
		return nil, fmt.Errorf("StartPod")
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		// return an empty list on create
		if !flagCreate {
			return []vc.PodStatus{}, nil
		}

		// return a podStatus with the container status
		return []vc.PodStatus{
			{
				ID: d.pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: d.pod.ID(),
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodContainer),
							vcAnnotations.ConfigJSONKey:    d.configJSON,
						},
					},
				},
			},
		}, nil
	}

	defer func() {
		testingImpl.CreatePodFunc = nil
		testingImpl.StartPodFunc = nil
		testingImpl.ListPodFunc = nil
	}()

	err = run(d.pod.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)

	// should not return ExitError
	err, ok := err.(*cli.ExitError)
	assert.False(ok, "error should not be a cli.ExitError: %s", err)
}

func TestRunContainerStartFailNoContainers(t *testing.T) {
	assert := assert.New(t)

	listCallCount := 0

	d := testRunContainerSetup(t)
	defer os.RemoveAll(d.tmpDir)

	pod := &vcMock.Pod{
		MockID: testPodID,
	}

	pod.MockContainers = []*vcMock.Container{
		{
			MockID:  testContainerID,
			MockPod: pod,
		},
	}

	testingImpl.ListPodFunc = func() ([]vc.PodStatus, error) {
		listCallCount++

		if listCallCount == 1 {
			return []vc.PodStatus{}, nil
		}

		return []vc.PodStatus{
			{
				ID: pod.ID(),
				ContainersStatus: []vc.ContainerStatus{
					{
						ID: testContainerID,
						Annotations: map[string]string{
							vcAnnotations.ContainerTypeKey: string(vc.PodSandbox),
						},
					},
				},
			},
		}, nil
	}

	testingImpl.CreatePodFunc = func(podConfig vc.PodConfig) (vc.VCPod, error) {
		return pod, nil
	}

	testingImpl.StartPodFunc = func(podID string) (vc.VCPod, error) {
		// force no containers
		pod.MockContainers = nil

		return pod, nil
	}

	defer func() {
		testingImpl.ListPodFunc = nil
		testingImpl.CreatePodFunc = nil
		testingImpl.StartPodFunc = nil
	}()

	err := run(d.pod.ID(), d.bundlePath, d.consolePath, "", d.pidFilePath, false, d.runtimeConfig)
	assert.Error(err)
	assert.False(vcMock.IsMockError(err))
}
