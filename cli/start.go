// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"fmt"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

var startCLICommand = cli.Command{
	Name:  "start",
	Usage: "executes the user defined process in a created container",
	ArgsUsage: `<container-id> [container-id...]

   <container-id> is your name for the instance of the container that you
   are starting. The name you provide for the container instance must be
   unique on your host.`,
	Description: `The start command executes the user defined process in a created container .`,
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		args := context.Args()
		if args.Present() == false {
			return fmt.Errorf("Missing container ID, should at least provide one")
		}

		for _, cID := range []string(args) {
			if _, err := start(ctx, cID); err != nil {
				return err
			}
		}

		return nil
	},
}

func start(ctx context.Context, containerID string) (vc.VCSandbox, error) {
	span, _ := trace(ctx, "start")
	defer span.Finish()

	kataLog = kataLog.WithField("container", containerID)
	setExternalLoggers(ctx, kataLog)
	span.SetTag("container", containerID)

	// Checks the MUST and MUST NOT from OCI runtime specification
	status, sandboxID, err := getExistingContainerInfo(ctx, containerID)
	if err != nil {
		return nil, err
	}

	containerID = status.ID

	kataLog = kataLog.WithFields(logrus.Fields{
		"container": containerID,
		"sandbox":   sandboxID,
	})

	setExternalLoggers(ctx, kataLog)
	span.SetTag("container", containerID)
	span.SetTag("sandbox", sandboxID)

	containerType, err := oci.GetContainerType(status.Annotations)
	if err != nil {
		return nil, err
	}

	if containerType.IsSandbox() {
		return vci.StartSandbox(ctx, sandboxID)
	}

	c, err := vci.StartContainer(ctx, sandboxID, containerID)
	if err != nil {
		return nil, err
	}

	return c.Sandbox(), nil
}
