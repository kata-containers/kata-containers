// Copyright (c) 2024 Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/compatoci"
	"github.com/urfave/cli"
)

var runCLICommand = cli.Command{
	Name:      "run",
	Usage:     "create and immediately start a container (OCI)",
	ArgsUsage: "<container-id>",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "bundle, b",
			Value: "",
			Usage: "path to the OCI bundle directory (defaults to current directory)",
		},
		cli.StringFlag{
			Name:  "pid-file",
			Value: "",
			Usage: "path to write the shim PID to",
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
		runtimeConfig, ok := c.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return fmt.Errorf("invalid runtime config in metadata")
		}
		bundlePath := c.String("bundle")
		if bundlePath == "" {
			bundlePath, err = os.Getwd()
			if err != nil {
				return fmt.Errorf("failed to get current directory: %w", err)
			}
		}
		return runRunCommand(ctx, c.Args().First(), bundlePath, c.String("pid-file"), runtimeConfig)
	},
}

func runRunCommand(ctx context.Context, containerID, bundlePath, pidFile string, runtimeConfig oci.RuntimeConfig) error {
	absBundle, err := filepath.Abs(bundlePath)
	if err != nil {
		return fmt.Errorf("failed to resolve bundle path: %w", err)
	}

	ociSpec, err := compatoci.ParseConfigJSON(absBundle)
	if err != nil {
		return fmt.Errorf("failed to parse OCI config from bundle %q: %w", absBundle, err)
	}

	if ociSpec.Annotations == nil {
		ociSpec.Annotations = make(map[string]string)
	}
	ociSpec.Annotations[vcAnnotations.BundlePathKey] = absBundle

	rootFs := vc.RootFs{
		Source:  filepath.Join(absBundle, "rootfs"),
		Mounted: true,
	}
	if ociSpec.Root != nil && ociSpec.Root.Path != "" {
		rootFs.Source = ociSpec.Root.Path
		if !filepath.IsAbs(rootFs.Source) {
			rootFs.Source = filepath.Join(absBundle, rootFs.Source)
		}
	}

	sandbox, _, err := katautils.CreateSandbox(ctx, vci, ociSpec, runtimeConfig, rootFs, containerID, absBundle, false, false)
	if err != nil {
		return fmt.Errorf("failed to create sandbox for container %q: %w", containerID, err)
	}
	defer sandbox.Release(ctx)

	if _, err := sandbox.StartContainer(ctx, containerID); err != nil {
		return fmt.Errorf("failed to start container %q: %w", containerID, err)
	}

	if pidFile != "" {
		if err := writePidFile(pidFile, os.Getpid()); err != nil {
			return fmt.Errorf("failed to write pid file: %w", err)
		}
	}

	return nil
}
