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
	"os"
	"syscall"

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
	},
	Action: func(context *cli.Context) error {
		runtimeConfig, ok := context.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		return run(context.Args().First(),
			context.String("bundle"),
			context.String("console"),
			context.String("console-socket"),
			context.String("pid-file"),
			context.Bool("detach"),
			runtimeConfig)
	},
}

func run(containerID, bundle, console, consoleSocket, pidFile string, detach bool,
	runtimeConfig oci.RuntimeConfig) error {

	consolePath, err := setupConsole(console, consoleSocket)
	if err != nil {
		return err
	}

	if err := create(containerID, bundle, consolePath, pidFile, detach, runtimeConfig); err != nil {
		return err
	}

	pod, err := start(containerID)
	if err != nil {
		return err
	}

	if detach {
		return nil
	}

	containers := pod.GetAllContainers()
	if len(containers) == 0 {
		return fmt.Errorf("There are no containers running in the pod: %s", pod.ID())
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
	if err := delete(pod.ID(), true); err != nil {
		return err
	}

	//runtime should forward container exit code to the system
	return cli.NewExitError("", ps.Sys().(syscall.WaitStatus).ExitStatus())
}
