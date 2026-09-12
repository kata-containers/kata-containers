// Copyright (c) 2024 Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"fmt"

	"github.com/urfave/cli"
)

var pauseCLICommand = cli.Command{
	Name:      "pause",
	Usage:     "pause a running container (OCI)",
	ArgsUsage: "<container-id>",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}
		if c.NArg() < 1 {
			return fmt.Errorf("container ID must be provided")
		}
		return runPauseCommand(ctx, c.Args().First())
	},
}

var resumeCLICommand = cli.Command{
	Name:      "resume",
	Usage:     "resume a paused container (OCI)",
	ArgsUsage: "<container-id>",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}
		if c.NArg() < 1 {
			return fmt.Errorf("container ID must be provided")
		}
		return runResumeCommand(ctx, c.Args().First())
	},
}

func runPauseCommand(ctx context.Context, containerID string) error {
	sandbox, err := vci.FetchSandbox(ctx, containerID)
	if err != nil {
		return fmt.Errorf("failed to fetch sandbox %q: %w", containerID, err)
	}
	defer sandbox.Release(ctx)

	if err := sandbox.PauseContainer(ctx, containerID); err != nil {
		return fmt.Errorf("failed to pause container %q: %w", containerID, err)
	}
	return nil
}

func runResumeCommand(ctx context.Context, containerID string) error {
	sandbox, err := vci.FetchSandbox(ctx, containerID)
	if err != nil {
		return fmt.Errorf("failed to fetch sandbox %q: %w", containerID, err)
	}
	defer sandbox.Release(ctx)

	if err := sandbox.ResumeContainer(ctx, containerID); err != nil {
		return fmt.Errorf("failed to resume container %q: %w", containerID, err)
	}
	return nil
}
