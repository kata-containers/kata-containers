// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"syscall"

	"github.com/kata-containers/runtime/pkg/katautils"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/urfave/cli"
)

type execParams struct {
	ociProcess   oci.CompatOCIProcess
	cID          string
	pidFile      string
	console      string
	consoleSock  string
	processLabel string
	detach       bool
	noSubreaper  bool
}

var execCLICommand = cli.Command{
	Name:  "exec",
	Usage: "Execute new process inside the container",
	ArgsUsage: `<container-id> <command> [command options]  || -p process.json <container-id>

   <container-id> is the name for the instance of the container and <command>
   is the command to be executed in the container. <command> can't be empty
   unless a "-p" flag provided.

EXAMPLE:
   If the container is configured to run the linux ps command the following
   will output a list of processes running in the container:

       # ` + name + ` <container-id> ps`,
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "console",
			Usage: "path to a pseudo terminal",
		},
		cli.StringFlag{
			Name:  "console-socket",
			Value: "",
			Usage: "path to an AF_UNIX socket which will receive a file descriptor referencing the master end of the console's pseudoterminal",
		},
		cli.StringFlag{
			Name:  "cwd",
			Usage: "current working directory in the container",
		},
		cli.StringSliceFlag{
			Name:  "env, e",
			Usage: "set environment variables",
		},
		cli.BoolFlag{
			Name:  "tty, t",
			Usage: "allocate a pseudo-TTY",
		},
		cli.StringFlag{
			Name:  "user, u",
			Usage: "UID (format: <uid>[:<gid>])",
		},
		cli.StringFlag{
			Name:  "process, p",
			Usage: "path to the process.json",
		},
		cli.BoolFlag{
			Name:  "detach,d",
			Usage: "detach from the container's process",
		},
		cli.StringFlag{
			Name:  "pid-file",
			Value: "",
			Usage: "specify the file to write the process id to",
		},
		cli.StringFlag{
			Name:  "process-label",
			Usage: "set the asm process label for the process commonly used with selinux",
		},
		cli.StringFlag{
			Name:  "apparmor",
			Usage: "set the apparmor profile for the process",
		},
		cli.BoolFlag{
			Name:  "no-new-privs",
			Usage: "set the no new privileges value for the process",
		},
		cli.StringSliceFlag{
			Name:  "cap, c",
			Value: &cli.StringSlice{},
			Usage: "add a capability to the bounding set for the process",
		},
		cli.BoolFlag{
			Name:   "no-subreaper",
			Usage:  "disable the use of the subreaper used to reap reparented processes",
			Hidden: true,
		},
	},
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		return execute(ctx, context)
	},
}

func generateExecParams(context *cli.Context, specProcess *oci.CompatOCIProcess) (execParams, error) {
	ctxArgs := context.Args()

	params := execParams{
		cID:          ctxArgs.First(),
		pidFile:      context.String("pid-file"),
		console:      context.String("console"),
		consoleSock:  context.String("console-socket"),
		detach:       context.Bool("detach"),
		processLabel: context.String("process-label"),
		noSubreaper:  context.Bool("no-subreaper"),
	}

	if context.String("process") != "" {
		var ociProcess oci.CompatOCIProcess

		fileContent, err := ioutil.ReadFile(context.String("process"))
		if err != nil {
			return execParams{}, err
		}

		if err := json.Unmarshal(fileContent, &ociProcess); err != nil {
			return execParams{}, err
		}

		params.ociProcess = ociProcess
	} else {
		params.ociProcess = *specProcess

		// Override terminal
		if context.IsSet("tty") {
			params.ociProcess.Terminal = context.Bool("tty")
		}

		// Override user
		if context.String("user") != "" {
			params.ociProcess.User = specs.User{
				// This field is a Windows-only field
				// according to the specification. However, it
				// is abused here to allow the username
				// specified in the OCI runtime configuration
				// file to be overridden by a CLI request.
				Username: context.String("user"),
			}
		}

		// Override env
		params.ociProcess.Env = append(params.ociProcess.Env, context.StringSlice("env")...)

		// Override cwd
		if context.String("cwd") != "" {
			params.ociProcess.Cwd = context.String("cwd")
		}

		// Override no-new-privs
		if context.IsSet("no-new-privs") {
			params.ociProcess.NoNewPrivileges = context.Bool("no-new-privs")
		}

		// Override apparmor
		if context.String("apparmor") != "" {
			params.ociProcess.ApparmorProfile = context.String("apparmor")
		}

		params.ociProcess.Args = ctxArgs.Tail()
	}

	return params, nil
}

func execute(ctx context.Context, context *cli.Context) error {
	span, ctx := katautils.Trace(ctx, "execute")
	defer span.Finish()

	containerID := context.Args().First()

	kataLog = kataLog.WithField("container", containerID)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("container", containerID)

	status, sandboxID, err := getExistingContainerInfo(ctx, containerID)
	if err != nil {
		return err
	}

	kataLog = kataLog.WithField("sandbox", sandboxID)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("sandbox", sandboxID)

	// Retrieve OCI spec configuration.
	ociSpec, err := oci.GetOCIConfig(status)
	if err != nil {
		return err
	}

	params, err := generateExecParams(context, ociSpec.Process)
	if err != nil {
		return err
	}

	params.cID = status.ID
	containerID = params.cID

	kataLog = kataLog.WithField("container", containerID)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("container", containerID)

	// container MUST be ready or running.
	if status.State.State != vc.StateReady &&
		status.State.State != vc.StateRunning {
		return fmt.Errorf("Container %s is not ready or running",
			params.cID)
	}

	envVars, err := oci.EnvVars(params.ociProcess.Env)
	if err != nil {
		return err
	}

	consolePath, err := setupConsole(params.console, params.consoleSock)
	if err != nil {
		return err
	}

	user := fmt.Sprintf("%d:%d", params.ociProcess.User.UID, params.ociProcess.User.GID)

	if params.ociProcess.User.Username != "" {
		user = params.ociProcess.User.Username
	}

	cmd := vc.Cmd{
		Args:        params.ociProcess.Args,
		Envs:        envVars,
		WorkDir:     params.ociProcess.Cwd,
		User:        user,
		Interactive: params.ociProcess.Terminal,
		Console:     consolePath,
		Detach:      noNeedForOutput(params.detach, params.ociProcess.Terminal),
	}

	_, _, process, err := vci.EnterContainer(ctx, sandboxID, params.cID, cmd)
	if err != nil {
		return err
	}

	// Creation of PID file has to be the last thing done in the exec
	// because containerd considers the exec to have finished starting
	// after this file is created.
	if err := createPIDFile(ctx, params.pidFile, process.Pid); err != nil {
		return err
	}

	if params.detach {
		return nil
	}

	p, err := os.FindProcess(process.Pid)
	if err != nil {
		return err
	}

	ps, err := p.Wait()
	if err != nil {
		return fmt.Errorf("Process state %s, container info %+v: %v",
			ps.String(), status, err)
	}

	// Exit code has to be forwarded in this case.
	return cli.NewExitError("", ps.Sys().(syscall.WaitStatus).ExitStatus())
}
