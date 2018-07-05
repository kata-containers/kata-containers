// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/sirupsen/logrus"
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
	status, sandboxID, err := getExistingContainerInfo(containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	kataLog = kataLog.WithFields(logrus.Fields{
		"container": containerID,
		"sandbox":   sandboxID,
	})

	setExternalLoggers(kataLog)

	if pause {
		err = vci.PauseContainer(sandboxID, containerID)
	} else {
		err = vci.ResumeContainer(sandboxID, containerID)
	}

	return err
}
