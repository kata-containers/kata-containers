// Copyright (c) 2024 Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"fmt"
	"os"

	"github.com/urfave/cli"
)

var startCLICommand = cli.Command{
	Name:      "start",
	Usage:     "start a created container (OCI)",
	ArgsUsage: "<container-id>",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}
		if c.NArg() < 1 {
			return fmt.Errorf("container ID must be provided")
		}
		return runStartCommand(ctx, c.Args().First())
	},
}

func runStartCommand(ctx context.Context, containerID string) error {
	sandbox, err := vci.FetchSandbox(ctx, containerID)
	if err != nil {
		return fmt.Errorf("failed to fetch sandbox %q: %w", containerID, err)
	}
	defer sandbox.Release(ctx)

	if err := sandbox.Start(ctx); err != nil {
		return fmt.Errorf("failed to start container %q: %w", containerID, err)
	}

	exitCode, err := sandbox.WaitProcess(ctx, containerID, containerID)
	if err != nil {
		sandbox.Stop(ctx, true)
		return fmt.Errorf("failed to wait for container %q: %w", containerID, err)
	}

	// Stop the sandbox so the shim process exits, which conmon needs to detect container completion.
	sandbox.Stop(ctx, true)

	if exitCode != 0 {
		os.Exit(int(exitCode))
	}
	return nil
}
