// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vf "github.com/kata-containers/runtime/virtcontainers/factory"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
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
		cli.BoolFlag{
			Name:  "no-pivot",
			Usage: "warning: this flag is meaningless to kata-runtime, just defined in order to be compatible with docker in ramdisk",
		},
	},
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		runtimeConfig, ok := context.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		console, err := setupConsole(context.String("console"), context.String("console-socket"))
		if err != nil {
			return err
		}

		return create(ctx, context.Args().First(),
			context.String("bundle"),
			console,
			context.String("pid-file"),
			true,
			context.Bool("systemd-cgroup"),
			runtimeConfig,
		)
	},
}

// Use a variable to allow tests to modify its value
var getKernelParamsFunc = getKernelParams

func handleFactory(ctx context.Context, runtimeConfig oci.RuntimeConfig) {
	if !runtimeConfig.FactoryConfig.Template {
		return
	}

	factoryConfig := vf.Config{
		Template: true,
		VMConfig: vc.VMConfig{
			HypervisorType:   runtimeConfig.HypervisorType,
			HypervisorConfig: runtimeConfig.HypervisorConfig,
			AgentType:        runtimeConfig.AgentType,
			AgentConfig:      runtimeConfig.AgentConfig,
		},
	}

	kataLog.WithField("factory", factoryConfig).Info("load vm factory")

	f, err := vf.NewFactory(ctx, factoryConfig, true)
	if err != nil {
		kataLog.WithError(err).Warn("load vm factory failed, about to create new one")
		f, err = vf.NewFactory(ctx, factoryConfig, false)
		if err != nil {
			kataLog.WithError(err).Warn("create vm factory failed")
			return
		}
	}

	vci.SetFactory(ctx, f)
}

func create(ctx context.Context, containerID, bundlePath, console, pidFilePath string, detach, systemdCgroup bool,
	runtimeConfig oci.RuntimeConfig) error {
	var err error

	span, ctx := trace(ctx, "create")
	defer span.Finish()

	kataLog = kataLog.WithField("container", containerID)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("container", containerID)

	if bundlePath == "" {
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		kataLog.WithField("directory", cwd).Debug("Defaulting bundle path to current directory")

		bundlePath = cwd
	}

	// Checks the MUST and MUST NOT from OCI runtime specification
	if bundlePath, err = validCreateParams(ctx, containerID, bundlePath); err != nil {
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

	handleFactory(ctx, runtimeConfig)

	disableOutput := noNeedForOutput(detach, ociSpec.Process.Terminal)

	var process vc.Process
	switch containerType {
	case vc.PodSandbox:
		process, err = createSandbox(ctx, ociSpec, runtimeConfig, containerID, bundlePath, console, disableOutput, systemdCgroup)
		if err != nil {
			return err
		}
	case vc.PodContainer:
		process, err = createContainer(ctx, ociSpec, containerID, bundlePath, console, disableOutput)
		if err != nil {
			return err
		}
	}

	// config.json provides a cgroups path that has to be used to create "tasks"
	// and "cgroups.procs" files. Those files have to be filled with a PID, which
	// is shim's in our case. This is mandatory to make sure there is no one
	// else (like Docker) trying to create those files on our behalf. We want to
	// know those files location so that we can remove them when delete is called.
	cgroupsPathList, err := processCgroupsPath(ctx, ociSpec, containerType.IsSandbox())
	if err != nil {
		return err
	}

	// cgroupsDirPath is CgroupsPath fetch from OCI spec
	var cgroupsDirPath string
	if ociSpec.Linux != nil {
		cgroupsDirPath = ociSpec.Linux.CgroupsPath
	}

	if err := createCgroupsFiles(ctx, containerID, cgroupsDirPath, cgroupsPathList, process.Pid); err != nil {
		return err
	}

	// Creation of PID file has to be the last thing done in the create
	// because containerd considers the create complete after this file
	// is created.
	return createPIDFile(ctx, pidFilePath, process.Pid)
}

var systemdKernelParam = []vc.Param{
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
}

func getKernelParams(needSystemd bool) []vc.Param {
	p := []vc.Param{}

	if needSystemd {
		p = append(p, systemdKernelParam...)
	}

	return p
}

func needSystemd(config vc.HypervisorConfig) bool {
	return config.ImagePath != ""
}

// setKernelParams adds the user-specified kernel parameters (from the
// configuration file) to the defaults so that the former take priority.
func setKernelParams(containerID string, runtimeConfig *oci.RuntimeConfig) error {
	defaultKernelParams := getKernelParamsFunc(needSystemd(runtimeConfig.HypervisorConfig))

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

func createSandbox(ctx context.Context, ociSpec oci.CompatOCISpec, runtimeConfig oci.RuntimeConfig,
	containerID, bundlePath, console string, disableOutput, systemdCgroup bool) (vc.Process, error) {
	span, ctx := trace(ctx, "createSandbox")
	defer span.Finish()

	err := setKernelParams(containerID, &runtimeConfig)
	if err != nil {
		return vc.Process{}, err
	}

	sandboxConfig, err := oci.SandboxConfig(ociSpec, runtimeConfig, bundlePath, containerID, console, disableOutput, systemdCgroup)
	if err != nil {
		return vc.Process{}, err
	}

	// Important to create the network namespace before the sandbox is
	// created, because it is not responsible for the creation of the
	// netns if it does not exist.
	if err := setupNetworkNamespace(&sandboxConfig.NetworkConfig); err != nil {
		return vc.Process{}, err
	}

	// Run pre-start OCI hooks.
	err = enterNetNS(sandboxConfig.NetworkConfig.NetNSPath, func() error {
		return preStartHooks(ctx, ociSpec, containerID, bundlePath)
	})
	if err != nil {
		return vc.Process{}, err
	}

	sandbox, err := vci.CreateSandbox(ctx, sandboxConfig)
	if err != nil {
		return vc.Process{}, err
	}

	sid := sandbox.ID()
	kataLog = kataLog.WithField("sandbox", sid)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("sandbox", sid)

	containers := sandbox.GetAllContainers()
	if len(containers) != 1 {
		return vc.Process{}, fmt.Errorf("BUG: Container list from sandbox is wrong, expecting only one container, found %d containers", len(containers))
	}

	if err := addContainerIDMapping(ctx, containerID, sandbox.ID()); err != nil {
		return vc.Process{}, err
	}

	return containers[0].Process(), nil
}

// setEphemeralStorageType sets the mount type to 'ephemeral'
// if the mount source path is provisioned by k8s for ephemeral storage.
// For the given pod ephemeral volume is created only once
// backed by tmpfs inside the VM. For successive containers
// of the same pod the already existing volume is reused.
func setEphemeralStorageType(ociSpec oci.CompatOCISpec) oci.CompatOCISpec {
	for idx, mnt := range ociSpec.Mounts {
		if IsEphemeralStorage(mnt.Source) {
			ociSpec.Mounts[idx].Type = "ephemeral"
		}
	}
	return ociSpec
}

func createContainer(ctx context.Context, ociSpec oci.CompatOCISpec, containerID, bundlePath,
	console string, disableOutput bool) (vc.Process, error) {

	span, ctx := trace(ctx, "createContainer")
	defer span.Finish()

	ociSpec = setEphemeralStorageType(ociSpec)

	contConfig, err := oci.ContainerConfig(ociSpec, bundlePath, containerID, console, disableOutput)
	if err != nil {
		return vc.Process{}, err
	}

	sandboxID, err := ociSpec.SandboxID()
	if err != nil {
		return vc.Process{}, err
	}

	kataLog = kataLog.WithField("sandbox", sandboxID)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("sandbox", sandboxID)

	s, c, err := vci.CreateContainer(ctx, sandboxID, contConfig)
	if err != nil {
		return vc.Process{}, err
	}

	// Run pre-start OCI hooks.
	err = enterNetNS(s.GetNetNs(), func() error {
		return preStartHooks(ctx, ociSpec, containerID, bundlePath)
	})
	if err != nil {
		return vc.Process{}, err
	}

	if err := addContainerIDMapping(ctx, containerID, sandboxID); err != nil {
		return vc.Process{}, err
	}

	return c.Process(), nil
}

func createCgroupsFiles(ctx context.Context, containerID string, cgroupsDirPath string, cgroupsPathList []string, pid int) error {
	span, _ := trace(ctx, "createCgroupsFiles")
	defer span.Finish()

	if len(cgroupsPathList) == 0 {
		kataLog.WithField("pid", pid).Info("Cgroups files not created because cgroupsPath was empty")
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

func createPIDFile(ctx context.Context, pidFilePath string, pid int) error {
	span, _ := trace(ctx, "createPIDFile")
	defer span.Finish()

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
