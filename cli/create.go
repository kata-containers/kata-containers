// Copyright (c) 2014,2015,2016 Docker, Inc.
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
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

var createCLICommand = cli.Command{
	Name:  "create",
	Usage: "Create a container",
	ArgsUsage: `<container-id>

   <container-id> is your name for the instance of the container that you
   are starting. The name you provide for the container instance must be unique
   on your host.`,
	Description: `The create command creates an instance of a container for a bundle. The
   bundle is a directory with a specification file named "` + specConfig + `" and a
   root filesystem.
   The specification file includes an args parameter. The args parameter is
   used to specify command(s) that get run when the container is started.
   To change the command(s) that get executed on start, edit the args
   parameter of the spec.`,
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "bundle, b",
			Value: "",
			Usage: `path to the root of the bundle directory, defaults to the current directory`,
		},
		cli.StringFlag{
			Name:  "console",
			Value: "",
			Usage: "path to a pseudo terminal",
		},
		cli.StringFlag{
			Name:  "console-socket",
			Value: "",
			Usage: "path to an AF_UNIX socket which will receive a file descriptor referencing the master end of the console's pseudoterminal",
		},
		cli.StringFlag{
			Name:  "pid-file",
			Value: "",
			Usage: "specify the file to write the process id to",
		},
	},
	Action: func(context *cli.Context) error {
		runtimeConfig, ok := context.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		console, err := setupConsole(context.String("console"), context.String("console-socket"))
		if err != nil {
			return err
		}

		return create(context.Args().First(),
			context.String("bundle"),
			console,
			context.String("pid-file"),
			true,
			runtimeConfig,
		)
	},
}

// Use a variable to allow tests to modify its value
var getKernelParamsFunc = getKernelParams

func create(containerID, bundlePath, console, pidFilePath string, detach bool,
	runtimeConfig oci.RuntimeConfig) error {
	var err error

	// Checks the MUST and MUST NOT from OCI runtime specification
	if bundlePath, err = validCreateParams(containerID, bundlePath); err != nil {
		return err
	}

	ociSpec, err := oci.ParseConfigJSON(bundlePath)
	if err != nil {
		return err
	}

	containerType, err := ociSpec.ContainerType()
	if err != nil {
		return err
	}

	disableOutput := noNeedForOutput(detach, ociSpec.Process.Terminal)

	var process vc.Process

	switch containerType {
	case vc.PodSandbox:
		process, err = createPod(ociSpec, runtimeConfig, containerID, bundlePath, console, disableOutput)
		if err != nil {
			return err
		}
	case vc.PodContainer:
		process, err = createContainer(ociSpec, containerID, bundlePath, console, disableOutput)
		if err != nil {
			return err
		}
	}

	// config.json provides a cgroups path that has to be used to create "tasks"
	// and "cgroups.procs" files. Those files have to be filled with a PID, which
	// is shim's in our case. This is mandatory to make sure there is no one
	// else (like Docker) trying to create those files on our behalf. We want to
	// know those files location so that we can remove them when delete is called.
	cgroupsPathList, err := processCgroupsPath(ociSpec, containerType.IsPod())
	if err != nil {
		return err
	}

	// cgroupsDirPath is CgroupsPath fetch from OCI spec
	var cgroupsDirPath string
	if ociSpec.Linux != nil {
		cgroupsDirPath = ociSpec.Linux.CgroupsPath
	}

	if err := createCgroupsFiles(containerID, cgroupsDirPath, cgroupsPathList, process.Pid); err != nil {
		return err
	}

	// Creation of PID file has to be the last thing done in the create
	// because containerd considers the create complete after this file
	// is created.
	return createPIDFile(pidFilePath, process.Pid)
}

func getKernelParams(containerID string) []vc.Param {
	return []vc.Param{
		{
			Key:   "init",
			Value: "/usr/lib/systemd/systemd",
		},
		{
			Key:   "systemd.unit",
			Value: systemdUnitName,
		},
		{
			Key:   "systemd.mask",
			Value: "systemd-networkd.service",
		},
		{
			Key:   "systemd.mask",
			Value: "systemd-networkd.socket",
		},
		{
			Key:   "ip",
			Value: fmt.Sprintf("::::::%s::off::", containerID),
		},
	}
}

// setKernelParams adds the user-specified kernel parameters (from the
// configuration file) to the defaults so that the former take priority.
func setKernelParams(containerID string, runtimeConfig *oci.RuntimeConfig) error {
	defaultKernelParams := getKernelParamsFunc(containerID)

	if runtimeConfig.HypervisorConfig.Debug {
		strParams := vc.SerializeParams(defaultKernelParams, "=")
		formatted := strings.Join(strParams, " ")

		kataLog.WithField("default-kernel-parameters", formatted).Debug()
	}

	// retrieve the parameters specified in the config file
	userKernelParams := runtimeConfig.HypervisorConfig.KernelParams

	// reset
	runtimeConfig.HypervisorConfig.KernelParams = []vc.Param{}

	// first, add default values
	for _, p := range defaultKernelParams {
		if err := (runtimeConfig).AddKernelParam(p); err != nil {
			return err
		}
	}

	// now re-add the user-specified values so that they take priority.
	for _, p := range userKernelParams {
		if err := (runtimeConfig).AddKernelParam(p); err != nil {
			return err
		}
	}

	return nil
}

func createPod(ociSpec oci.CompatOCISpec, runtimeConfig oci.RuntimeConfig,
	containerID, bundlePath, console string, disableOutput bool) (vc.Process, error) {

	err := setKernelParams(containerID, &runtimeConfig)
	if err != nil {
		return vc.Process{}, err
	}

	podConfig, err := oci.PodConfig(ociSpec, runtimeConfig, bundlePath, containerID, console, disableOutput)
	if err != nil {
		return vc.Process{}, err
	}

	pod, err := vci.CreatePod(podConfig)
	if err != nil {
		return vc.Process{}, err
	}

	containers := pod.GetAllContainers()
	if len(containers) != 1 {
		return vc.Process{}, fmt.Errorf("BUG: Container list from pod is wrong, expecting only one container, found %d containers", len(containers))
	}

	return containers[0].Process(), nil
}

func createContainer(ociSpec oci.CompatOCISpec, containerID, bundlePath,
	console string, disableOutput bool) (vc.Process, error) {

	contConfig, err := oci.ContainerConfig(ociSpec, bundlePath, containerID, console, disableOutput)
	if err != nil {
		return vc.Process{}, err
	}

	podID, err := ociSpec.PodID()
	if err != nil {
		return vc.Process{}, err
	}

	_, c, err := vci.CreateContainer(podID, contConfig)
	if err != nil {
		return vc.Process{}, err
	}

	return c.Process(), nil
}

func createCgroupsFiles(containerID string, cgroupsDirPath string, cgroupsPathList []string, pid int) error {
	if len(cgroupsPathList) == 0 {
		fields := logrus.Fields{
			"container": containerID,
			"pid":       pid,
		}
		kataLog.WithFields(fields).Info("Cgroups files not created because cgroupsPath was empty")
		return nil
	}

	for _, cgroupsPath := range cgroupsPathList {
		if err := os.MkdirAll(cgroupsPath, cgroupsDirMode); err != nil {
			return err
		}

		if strings.Contains(cgroupsPath, "cpu") && cgroupsDirPath != "" {
			parent := strings.TrimSuffix(cgroupsPath, cgroupsDirPath)
			copyParentCPUSet(cgroupsPath, parent)
		}

		tasksFilePath := filepath.Join(cgroupsPath, cgroupsTasksFile)
		procsFilePath := filepath.Join(cgroupsPath, cgroupsProcsFile)

		pidStr := fmt.Sprintf("%d", pid)

		for _, path := range []string{tasksFilePath, procsFilePath} {
			f, err := os.OpenFile(path, os.O_RDWR|os.O_CREATE, cgroupsFileMode)
			if err != nil {
				return err
			}
			defer f.Close()

			n, err := f.WriteString(pidStr)
			if err != nil {
				return err
			}

			if n < len(pidStr) {
				return fmt.Errorf("Could not write pid to %q: only %d bytes written out of %d",
					path, n, len(pidStr))
			}
		}
	}

	return nil
}

func createPIDFile(pidFilePath string, pid int) error {
	if pidFilePath == "" {
		// runtime should not fail since pid file is optional
		return nil
	}

	if err := os.RemoveAll(pidFilePath); err != nil {
		return err
	}

	f, err := os.Create(pidFilePath)
	if err != nil {
		return err
	}
	defer f.Close()

	pidStr := fmt.Sprintf("%d", pid)

	n, err := f.WriteString(pidStr)
	if err != nil {
		return err
	}

	if n < len(pidStr) {
		return fmt.Errorf("Could not write pid to '%s': only %d bytes written out of %d", pidFilePath, n, len(pidStr))
	}

	return nil
}

// copyParentCPUSet copies the cpuset.cpus and cpuset.mems from the parent
// directory to the current directory if the file's contents are 0
func copyParentCPUSet(current, parent string) error {
	currentCpus, currentMems, err := getCPUSet(current)
	if err != nil {
		return err
	}

	parentCpus, parentMems, err := getCPUSet(parent)
	if err != nil {
		return err
	}

	if len(parentCpus) < 1 || len(parentMems) < 1 {
		return nil
	}

	var cgroupsFileMode = os.FileMode(0600)
	if isEmptyString(currentCpus) {
		if err := writeFile(filepath.Join(current, "cpuset.cpus"), string(parentCpus), cgroupsFileMode); err != nil {
			return err
		}
	}

	if isEmptyString(currentMems) {
		if err := writeFile(filepath.Join(current, "cpuset.mems"), string(parentMems), cgroupsFileMode); err != nil {
			return err
		}
	}

	return nil
}

func getCPUSet(parent string) (cpus []byte, mems []byte, err error) {
	if cpus, err = ioutil.ReadFile(filepath.Join(parent, "cpuset.cpus")); err != nil {
		return
	}

	if mems, err = ioutil.ReadFile(filepath.Join(parent, "cpuset.mems")); err != nil {
		return
	}

	return cpus, mems, nil
}
