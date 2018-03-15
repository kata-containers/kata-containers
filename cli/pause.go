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
	"github.com/urfave/cli"
)

var noteText = `Use "` + name + ` list" to identify container statuses.`

var pauseCLICommand = cli.Command{
	Name:  "pause",
	Usage: "suspend all processes in a container",
	ArgsUsage: `<container-id>

Where "<container-id>" is the container name to be paused.`,
	Description: `The pause command suspends all processes in a container.

	` + noteText,
	Action: func(context *cli.Context) error {
		return toggleContainerPause(context.Args().First(), true)
	},
}

var resumeCLICommand = cli.Command{
	Name:  "resume",
	Usage: "unpause all previously paused processes in a container",
	ArgsUsage: `<container-id>

Where "<container-id>" is the container name to be resumed.`,
	Description: `The resume command unpauses all processes in a container.

	` + noteText,
	Action: func(context *cli.Context) error {
		return toggleContainerPause(context.Args().First(), false)
	},
}

func toggleContainerPause(containerID string, pause bool) (err error) {
	// Checks the MUST and MUST NOT from OCI runtime specification
	_, podID, err := getExistingContainerInfo(containerID)
	if err != nil {
		return err
	}

	if pause {
		_, err = vci.PausePod(podID)
	} else {
		_, err = vci.ResumePod(podID)
	}

	return err
}
