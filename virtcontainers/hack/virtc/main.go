// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"
	"os"
	"strings"
	"text/tabwriter"

	"github.com/kata-containers/runtime/virtcontainers/pkg/uuid"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"

	vc "github.com/kata-containers/runtime/virtcontainers"
)

var virtcLog = logrus.New()

var listFormat = "%s\t%s\t%s\t%s\n"
var statusFormat = "%s\t%s\n"

var (
	errNeedContainerID = errors.New("Container ID cannot be empty")
	errNeedSandboxID   = errors.New("Sandbox ID cannot be empty")
)

var sandboxConfigFlags = []cli.Flag{
	cli.GenericFlag{
		Name:  "agent",
		Value: new(vc.AgentType),
		Usage: "the guest agent",
	},

	cli.StringFlag{
		Name:  "id",
		Value: "",
		Usage: "the sandbox identifier (default: auto-generated)",
	},

	cli.StringFlag{
		Name:  "machine-type",
		Value: vc.QemuPC,
		Usage: "hypervisor machine type",
	},

	cli.GenericFlag{
		Name:  "network",
		Value: new(vc.NetworkModel),
		Usage: "the network model",
	},

	cli.GenericFlag{
		Name:  "proxy",
		Value: new(vc.ProxyType),
		Usage: "the agent's proxy",
	},

	cli.StringFlag{
		Name:  "proxy-path",
		Value: "",
		Usage: "path to proxy binary",
	},

	cli.GenericFlag{
		Name:  "shim",
		Value: new(vc.ShimType),
		Usage: "the shim type",
	},

	cli.StringFlag{
		Name:  "shim-path",
		Value: "",
		Usage: "the shim binary path",
	},

	cli.StringFlag{
		Name:  "hyper-ctl-sock-name",
		Value: "",
		Usage: "the hyperstart control socket name",
	},

	cli.StringFlag{
		Name:  "hyper-tty-sock-name",
		Value: "",
		Usage: "the hyperstart tty socket name",
	},

	cli.UintFlag{
		Name:  "cpus",
		Value: 0,
		Usage: "the number of virtual cpus available for this sandbox",
	},

	cli.UintFlag{
		Name:  "memory",
		Value: 0,
		Usage: "the amount of memory available for this sandbox in MiB",
	},
}

var ccKernelParams = []vc.Param{
	{
		Key:   "init",
		Value: "/usr/lib/systemd/systemd",
	},
	{
		Key:   "systemd.unit",
		Value: "clear-containers.target",
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

func buildKernelParams(config *vc.HypervisorConfig) error {
	for _, p := range ccKernelParams {
		if err := config.AddKernelParam(p); err != nil {
			return err
		}
	}

	return nil
}

func buildSandboxConfig(context *cli.Context) (vc.SandboxConfig, error) {
	var agConfig interface{}

	hyperCtlSockName := context.String("hyper-ctl-sock-name")
	hyperTtySockName := context.String("hyper-tty-sock-name")
	proxyPath := context.String("proxy-path")
	shimPath := context.String("shim-path")
	machineType := context.String("machine-type")
	vmMemory := context.Uint("vm-memory")
	agentType, ok := context.Generic("agent").(*vc.AgentType)
	if ok != true {
		return vc.SandboxConfig{}, fmt.Errorf("Could not convert agent type")
	}

	networkModel, ok := context.Generic("network").(*vc.NetworkModel)
	if ok != true {
		return vc.SandboxConfig{}, fmt.Errorf("Could not convert network model")
	}

	proxyType, ok := context.Generic("proxy").(*vc.ProxyType)
	if ok != true {
		return vc.SandboxConfig{}, fmt.Errorf("Could not convert proxy type")
	}

	shimType, ok := context.Generic("shim").(*vc.ShimType)
	if ok != true {
		return vc.SandboxConfig{}, fmt.Errorf("Could not convert shim type")
	}

	kernelPath := "/usr/share/clear-containers/vmlinuz.container"
	if machineType == vc.QemuPCLite {
		kernelPath = "/usr/share/clear-containers/vmlinux.container"
	}

	hypervisorConfig := vc.HypervisorConfig{
		KernelPath:            kernelPath,
		ImagePath:             "/usr/share/clear-containers/clear-containers.img",
		HypervisorMachineType: machineType,
	}

	if err := buildKernelParams(&hypervisorConfig); err != nil {
		return vc.SandboxConfig{}, err
	}

	netConfig := vc.NetworkConfig{
		NumInterfaces: 1,
	}

	switch *agentType {
	case vc.HyperstartAgent:
		agConfig = vc.HyperConfig{
			SockCtlName: hyperCtlSockName,
			SockTtyName: hyperTtySockName,
		}
	default:
		agConfig = nil
	}

	proxyConfig := getProxyConfig(*proxyType, proxyPath)

	shimConfig := getShimConfig(*shimType, shimPath)

	vmConfig := vc.Resources{
		Memory: vmMemory,
	}

	id := context.String("id")
	if id == "" {
		// auto-generate sandbox name
		id = uuid.Generate().String()
	}

	sandboxConfig := vc.SandboxConfig{
		ID:       id,
		VMConfig: vmConfig,

		HypervisorType:   vc.QemuHypervisor,
		HypervisorConfig: hypervisorConfig,

		AgentType:   *agentType,
		AgentConfig: agConfig,

		NetworkModel:  *networkModel,
		NetworkConfig: netConfig,

		ProxyType:   *proxyType,
		ProxyConfig: proxyConfig,

		ShimType:   *shimType,
		ShimConfig: shimConfig,

		Containers: []vc.ContainerConfig{},
	}

	return sandboxConfig, nil
}

func getProxyConfig(proxyType vc.ProxyType, path string) vc.ProxyConfig {
	var proxyConfig vc.ProxyConfig

	switch proxyType {
	case vc.KataProxyType:
		fallthrough
	case vc.CCProxyType:
		proxyConfig = vc.ProxyConfig{
			Path: path,
		}
	}

	return proxyConfig
}

func getShimConfig(shimType vc.ShimType, path string) interface{} {
	var shimConfig interface{}

	switch shimType {
	case vc.CCShimType, vc.KataShimType:
		shimConfig = vc.ShimConfig{
			Path: path,
		}

	default:
		shimConfig = nil
	}

	return shimConfig
}

// checkRequiredSandboxArgs checks to ensure the required command-line
// arguments have been specified for the sandbox sub-command specified by
// the context argument.
func checkRequiredSandboxArgs(context *cli.Context) error {
	if context == nil {
		return fmt.Errorf("BUG: need Context")
	}

	// sub-sub-command name
	name := context.Command.Name

	switch name {
	case "create":
		fallthrough
	case "list":
		fallthrough
	case "run":
		// these commands don't require any arguments
		return nil
	}

	id := context.String("id")
	if id == "" {
		return errNeedSandboxID
	}

	return nil
}

// checkRequiredContainerArgs checks to ensure the required command-line
// arguments have been specified for the container sub-command specified
// by the context argument.
func checkRequiredContainerArgs(context *cli.Context) error {
	if context == nil {
		return fmt.Errorf("BUG: need Context")
	}

	// sub-sub-command name
	name := context.Command.Name

	sandboxID := context.String("sandbox-id")
	if sandboxID == "" {
		return errNeedSandboxID
	}

	rootfs := context.String("rootfs")
	if name == "create" && rootfs == "" {
		return fmt.Errorf("%s: need rootfs", name)
	}

	id := context.String("id")
	if id == "" {
		return errNeedContainerID
	}

	return nil
}

func runSandbox(context *cli.Context) error {
	sandboxConfig, err := buildSandboxConfig(context)
	if err != nil {
		return fmt.Errorf("Could not build sandbox config: %s", err)
	}

	_, err = vc.RunSandbox(sandboxConfig)
	if err != nil {
		return fmt.Errorf("Could not run sandbox: %s", err)
	}

	return nil
}

func createSandbox(context *cli.Context) error {
	sandboxConfig, err := buildSandboxConfig(context)
	if err != nil {
		return fmt.Errorf("Could not build sandbox config: %s", err)
	}

	p, err := vc.CreateSandbox(sandboxConfig)
	if err != nil {
		return fmt.Errorf("Could not create sandbox: %s", err)
	}

	fmt.Printf("Sandbox %s created\n", p.ID())

	return nil
}

func checkSandboxArgs(context *cli.Context, f func(context *cli.Context) error) error {
	if err := checkRequiredSandboxArgs(context); err != nil {
		return err
	}

	return f(context)
}

func checkContainerArgs(context *cli.Context, f func(context *cli.Context) error) error {
	if err := checkRequiredContainerArgs(context); err != nil {
		return err
	}

	return f(context)
}

func deleteSandbox(context *cli.Context) error {
	p, err := vc.DeleteSandbox(context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not delete sandbox: %s", err)
	}

	fmt.Printf("Sandbox %s deleted\n", p.ID())

	return nil
}

func startSandbox(context *cli.Context) error {
	p, err := vc.StartSandbox(context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not start sandbox: %s", err)
	}

	fmt.Printf("Sandbox %s started\n", p.ID())

	return nil
}

func stopSandbox(context *cli.Context) error {
	p, err := vc.StopSandbox(context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not stop sandbox: %s", err)
	}

	fmt.Printf("Sandbox %s stopped\n", p.ID())

	return nil
}

func pauseSandbox(context *cli.Context) error {
	p, err := vc.PauseSandbox(context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not pause sandbox: %s", err)
	}

	fmt.Printf("Sandbox %s paused\n", p.ID())

	return nil
}

func resumeSandbox(context *cli.Context) error {
	p, err := vc.ResumeSandbox(context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not resume sandbox: %s", err)
	}

	fmt.Printf("Sandbox %s resumed\n", p.ID())

	return nil
}

func listSandboxes(context *cli.Context) error {
	sandboxStatusList, err := vc.ListSandbox()
	if err != nil {
		return fmt.Errorf("Could not list sandbox: %s", err)
	}

	w := tabwriter.NewWriter(os.Stdout, 2, 8, 1, '\t', 0)
	fmt.Fprintf(w, listFormat, "SB ID", "STATE", "HYPERVISOR", "AGENT")

	for _, sandboxStatus := range sandboxStatusList {
		fmt.Fprintf(w, listFormat,
			sandboxStatus.ID, sandboxStatus.State.State, sandboxStatus.Hypervisor, sandboxStatus.Agent)
	}

	w.Flush()

	return nil
}

func statusSandbox(context *cli.Context) error {
	sandboxStatus, err := vc.StatusSandbox(context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not get sandbox status: %s", err)
	}

	w := tabwriter.NewWriter(os.Stdout, 2, 8, 1, '\t', 0)
	fmt.Fprintf(w, listFormat, "SB ID", "STATE", "HYPERVISOR", "AGENT")

	fmt.Fprintf(w, listFormat+"\n",
		sandboxStatus.ID, sandboxStatus.State.State, sandboxStatus.Hypervisor, sandboxStatus.Agent)

	fmt.Fprintf(w, statusFormat, "CONTAINER ID", "STATE")

	for _, contStatus := range sandboxStatus.ContainersStatus {
		fmt.Fprintf(w, statusFormat, contStatus.ID, contStatus.State.State)
	}

	w.Flush()

	return nil
}

var runSandboxCommand = cli.Command{
	Name:  "run",
	Usage: "run a sandbox",
	Flags: sandboxConfigFlags,
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, runSandbox)
	},
}

var createSandboxCommand = cli.Command{
	Name:  "create",
	Usage: "create a sandbox",
	Flags: sandboxConfigFlags,
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, createSandbox)
	},
}

var deleteSandboxCommand = cli.Command{
	Name:  "delete",
	Usage: "delete an existing sandbox",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, deleteSandbox)
	},
}

var startSandboxCommand = cli.Command{
	Name:  "start",
	Usage: "start an existing sandbox",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, startSandbox)
	},
}

var stopSandboxCommand = cli.Command{
	Name:  "stop",
	Usage: "stop an existing sandbox",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, stopSandbox)
	},
}

var listSandboxesCommand = cli.Command{
	Name:  "list",
	Usage: "list all existing sandboxes",
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, listSandboxes)
	},
}

var statusSandboxCommand = cli.Command{
	Name:  "status",
	Usage: "returns a detailed sandbox status",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, statusSandbox)
	},
}

var pauseSandboxCommand = cli.Command{
	Name:  "pause",
	Usage: "pause an existing sandbox",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, pauseSandbox)
	},
}

var resumeSandboxCommand = cli.Command{
	Name:  "resume",
	Usage: "unpause a paused sandbox",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkSandboxArgs(context, resumeSandbox)
	},
}

func createContainer(context *cli.Context) error {
	console := context.String("console")

	interactive := false
	if console != "" {
		interactive = true
	}

	envs := []vc.EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := vc.Cmd{
		Args:        strings.Split(context.String("cmd"), " "),
		Envs:        envs,
		WorkDir:     "/",
		Interactive: interactive,
		Console:     console,
	}

	id := context.String("id")
	if id == "" {
		// auto-generate container name
		id = uuid.Generate().String()
	}

	containerConfig := vc.ContainerConfig{
		ID:     id,
		RootFs: context.String("rootfs"),
		Cmd:    cmd,
	}

	_, c, err := vc.CreateContainer(context.String("sandbox-id"), containerConfig)
	if err != nil {
		return fmt.Errorf("Could not create container: %s", err)
	}

	fmt.Printf("Container %s created\n", c.ID())

	return nil
}

func deleteContainer(context *cli.Context) error {
	c, err := vc.DeleteContainer(context.String("sandbox-id"), context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not delete container: %s", err)
	}

	fmt.Printf("Container %s deleted\n", c.ID())

	return nil
}

func startContainer(context *cli.Context) error {
	c, err := vc.StartContainer(context.String("sandbox-id"), context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not start container: %s", err)
	}

	fmt.Printf("Container %s started\n", c.ID())

	return nil
}

func stopContainer(context *cli.Context) error {
	c, err := vc.StopContainer(context.String("sandbox-id"), context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not stop container: %s", err)
	}

	fmt.Printf("Container %s stopped\n", c.ID())

	return nil
}

func enterContainer(context *cli.Context) error {
	console := context.String("console")

	interactive := false
	if console != "" {
		interactive = true
	}

	envs := []vc.EnvVar{
		{
			Var:   "PATH",
			Value: "/bin:/usr/bin:/sbin:/usr/sbin",
		},
	}

	cmd := vc.Cmd{
		Args:        strings.Split(context.String("cmd"), " "),
		Envs:        envs,
		WorkDir:     "/",
		Interactive: interactive,
		Console:     console,
	}

	_, c, _, err := vc.EnterContainer(context.String("sandbox-id"), context.String("id"), cmd)
	if err != nil {
		return fmt.Errorf("Could not enter container: %s", err)
	}

	fmt.Printf("Container %s entered\n", c.ID())

	return nil
}

func statusContainer(context *cli.Context) error {
	contStatus, err := vc.StatusContainer(context.String("sandbox-id"), context.String("id"))
	if err != nil {
		return fmt.Errorf("Could not get container status: %s", err)
	}

	w := tabwriter.NewWriter(os.Stdout, 2, 8, 1, '\t', 0)
	fmt.Fprintf(w, statusFormat, "CONTAINER ID", "STATE")
	fmt.Fprintf(w, statusFormat, contStatus.ID, contStatus.State.State)

	w.Flush()

	return nil
}

var createContainerCommand = cli.Command{
	Name:  "create",
	Usage: "create a container",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the container identifier (default: auto-generated)",
		},
		cli.StringFlag{
			Name:  "sandbox-id",
			Value: "",
			Usage: "the sandbox identifier",
		},
		cli.StringFlag{
			Name:  "rootfs",
			Value: "",
			Usage: "the container rootfs directory",
		},
		cli.StringFlag{
			Name:  "cmd",
			Value: "",
			Usage: "the command executed inside the container",
		},
		cli.StringFlag{
			Name:  "console",
			Value: "",
			Usage: "the container console",
		},
	},
	Action: func(context *cli.Context) error {
		return checkContainerArgs(context, createContainer)
	},
}

var deleteContainerCommand = cli.Command{
	Name:  "delete",
	Usage: "delete an existing container",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the container identifier",
		},
		cli.StringFlag{
			Name:  "sandbox-id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkContainerArgs(context, deleteContainer)
	},
}

var startContainerCommand = cli.Command{
	Name:  "start",
	Usage: "start an existing container",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the container identifier",
		},
		cli.StringFlag{
			Name:  "sandbox-id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkContainerArgs(context, startContainer)
	},
}

var stopContainerCommand = cli.Command{
	Name:  "stop",
	Usage: "stop an existing container",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the container identifier",
		},
		cli.StringFlag{
			Name:  "sandbox-id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkContainerArgs(context, stopContainer)
	},
}

var enterContainerCommand = cli.Command{
	Name:  "enter",
	Usage: "enter an existing container",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the container identifier",
		},
		cli.StringFlag{
			Name:  "sandbox-id",
			Value: "",
			Usage: "the sandbox identifier",
		},
		cli.StringFlag{
			Name:  "cmd",
			Value: "echo",
			Usage: "the command executed inside the container",
		},
		cli.StringFlag{
			Name:  "console",
			Value: "",
			Usage: "the process console",
		},
	},
	Action: func(context *cli.Context) error {
		return checkContainerArgs(context, enterContainer)
	},
}

var statusContainerCommand = cli.Command{
	Name:  "status",
	Usage: "returns detailed container status",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "id",
			Value: "",
			Usage: "the container identifier",
		},
		cli.StringFlag{
			Name:  "sandbox-id",
			Value: "",
			Usage: "the sandbox identifier",
		},
	},
	Action: func(context *cli.Context) error {
		return checkContainerArgs(context, statusContainer)
	},
}

func main() {
	cli.VersionFlag = cli.BoolFlag{
		Name:  "version",
		Usage: "print the version",
	}

	virtc := cli.NewApp()
	virtc.Name = "VirtContainers CLI"
	virtc.Version = "0.0.1"

	virtc.Flags = []cli.Flag{
		cli.BoolFlag{
			Name:  "debug",
			Usage: "enable debug output for logging",
		},
		cli.StringFlag{
			Name:  "log",
			Value: "",
			Usage: "set the log file path where internal debug information is written",
		},
		cli.StringFlag{
			Name:  "log-format",
			Value: "text",
			Usage: "set the format used by logs ('text' (default), or 'json')",
		},
	}

	virtc.Commands = []cli.Command{
		{
			Name:  "sandbox",
			Usage: "sandbox commands",
			Subcommands: []cli.Command{
				createSandboxCommand,
				deleteSandboxCommand,
				listSandboxesCommand,
				pauseSandboxCommand,
				resumeSandboxCommand,
				runSandboxCommand,
				startSandboxCommand,
				stopSandboxCommand,
				statusSandboxCommand,
			},
		},
		{
			Name:  "container",
			Usage: "container commands",
			Subcommands: []cli.Command{
				createContainerCommand,
				deleteContainerCommand,
				startContainerCommand,
				stopContainerCommand,
				enterContainerCommand,
				statusContainerCommand,
			},
		},
	}

	virtc.Before = func(context *cli.Context) error {
		if context.GlobalBool("debug") {
			virtcLog.Level = logrus.DebugLevel
		}

		if path := context.GlobalString("log"); path != "" {
			f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND|os.O_SYNC, 0640)
			if err != nil {
				return err
			}
			virtcLog.Out = f
		}

		switch context.GlobalString("log-format") {
		case "text":
			// retain logrus's default.
		case "json":
			virtcLog.Formatter = new(logrus.JSONFormatter)
		default:
			return fmt.Errorf("unknown log-format %q", context.GlobalString("log-format"))
		}

		// Set virtcontainers logger.
		vc.SetLogger(virtcLog)

		return nil
	}

	err := virtc.Run(os.Args)
	if err != nil {
		virtcLog.Fatal(err)
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
