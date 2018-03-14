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
	"fmt"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/urfave/cli"
)

var psCLICommand = cli.Command{
	Name:      "ps",
	Usage:     "ps displays the processes running inside a container",
	ArgsUsage: `<container-id> [ps options]`,
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "format, f",
			Value: "table",
			Usage: `select one of: ` + formatOptions,
		},
	},
	Action: func(context *cli.Context) error {
		if context.Args().Present() == false {
			return fmt.Errorf("Missing container ID, should at least provide one")
		}

		var args []string
		if len(context.Args()) > 1 {
			// [1:] is to remove container_id:
			// context.Args(): [container_id ps_arg1 ps_arg2 ...]
			// args:           [ps_arg1 ps_arg2 ...]
			args = context.Args()[1:]
		}

		return ps(context.Args().First(), context.String("format"), args)
	},
	SkipArgReorder: true,
}

func ps(containerID, format string, args []string) error {
	if containerID == "" {
		return fmt.Errorf("Missing container ID")
	}

	// Checks the MUST and MUST NOT from OCI runtime specification
	status, podID, err := getExistingContainerInfo(containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	// container MUST be running
	if status.State.State != vc.StateRunning {
		return fmt.Errorf("Container %s is not running", containerID)
	}

	var options vc.ProcessListOptions

	options.Args = args
	if len(options.Args) == 0 {
		options.Args = []string{"-ef"}
	}

	options.Format = format

	msg, err := vci.ProcessListContainer(containerID, podID, options)
	if err != nil {
		return err
	}

	fmt.Print(string(msg))

	return nil
}
