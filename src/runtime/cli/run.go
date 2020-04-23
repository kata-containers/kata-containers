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
	"os"
	"syscall"

	"github.com/kata-containers/runtime/pkg/katautils"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/urfave/cli"
)

var runCLICommand = cli.Command{
	Name:  "run",
	Usage: "create and run a container",
	ArgsUsage: `<container-id>

   <container-id> is your name for the instance of the container that you
   are starting. The name you provide for the container instance must be unique
   on your host.`,
	Description: `The run command creates an instance of a container for a bundle. The bundle
   is a directory with a specification file named "config.json" and a root
   filesystem.`,
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
			Name:  "detach, d",
			Usage: "detach from the container's process",
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

		return run(ctx, context.Args().First(),
			context.String("bundle"),
			context.String("console"),
			context.String("console-socket"),
			context.String("pid-file"),
			context.Bool("detach"),
			context.Bool("systemd-cgroup"),
			runtimeConfig)
	},
}

func run(ctx context.Context, containerID, bundle, console, consoleSocket, pidFile string, detach, systemdCgroup bool,
	runtimeConfig oci.RuntimeConfig) error {
	span, ctx := katautils.Trace(ctx, "run")
	defer span.Finish()

	consolePath, err := setupConsole(console, consoleSocket)
	if err != nil {
		return err
	}

	if err := create(ctx, containerID, bundle, consolePath, pidFile, detach, systemdCgroup, runtimeConfig); err != nil {
		return err
	}

	sandbox, err := start(ctx, containerID)
	if err != nil {
		return err
	}

	if detach {
		return nil
	}

	containers := sandbox.GetAllContainers()
	if len(containers) == 0 {
		return fmt.Errorf("There are no containers running in the sandbox: %s", sandbox.ID())
	}

	p, err := os.FindProcess(containers[0].GetPid())
	if err != nil {
		return err
	}

	ps, err := p.Wait()
	if err != nil {
		return fmt.Errorf("Process state %s: %s", ps.String(), err)
	}

	// delete container's resources
	if err := delete(ctx, sandbox.ID(), true); err != nil {
		return err
	}

	//runtime should forward container exit code to the system
	return cli.NewExitError("", ps.Sys().(syscall.WaitStatus).ExitStatus())
}
