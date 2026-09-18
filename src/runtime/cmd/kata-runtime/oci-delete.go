// Copyright (c) 2024 Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"fmt"

	"github.com/urfave/cli"
)

var deleteCLICommand = cli.Command{
	Name:      "delete",
	Usage:     "delete a stopped container and its resources (OCI)",
	ArgsUsage: "<container-id>",
	Flags: []cli.Flag{
		cli.BoolFlag{
			Name:  "force, f",
			Usage: "forcibly delete the container even if it is still running",
		},
	},
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}
		if c.NArg() < 1 {
			return fmt.Errorf("container ID must be provided")
		}
		return runDeleteCommand(ctx, c.Args().First(), c.Bool("force"))
	},
}

func runDeleteCommand(ctx context.Context, containerID string, force bool) error {
	if err := vci.CleanupContainer(ctx, containerID, containerID, force); err != nil {
		if !force {
			return fmt.Errorf("failed to delete container %q: %w", containerID, err)
		}
		kataLog.WithError(err).Warnf("non-fatal error while force-deleting container %q", containerID)
	}
	return nil
}
