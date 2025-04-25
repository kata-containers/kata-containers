// Copyright (c) 2022 Databricks Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"fmt"

	containerdshim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils/shimclient"

	"github.com/urfave/cli"
)

var (
	blockDevice string
)

var blockDeviceSubCmds = []cli.Command{
	resizeBlockDeviceCommand,
}

var kataBlockDeviceCommand = cli.Command{
	Name:        "block-device",
	Usage:       "manage raw block device assignment for Kata Containers",
	Description: "Operations for raw block device management in Kata Containers",
	Subcommands: blockDeviceSubCmds,
	Action: func(context *cli.Context) {
		cli.ShowSubcommandHelp(context)
	},
}

var resizeBlockDeviceCommand = cli.Command{
	Name:      "resize",
	Usage:     "resize a raw block device",
	ArgsUsage: "[options]",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:        "sandbox-id, s",
			Usage:       "the sandbox id of the Kata container",
			Required:    true,
			Destination: &sandboxID,
		},
		cli.StringFlag{
			Name:        "block-device, b",
			Usage:       "host path to the raw block device",
			Required:    true,
			Destination: &blockDevice,
		},
		cli.Uint64Flag{
			Name:        "size",
			Usage:       "the new size of the raw block device in bytes",
			Required:    true,
			Destination: &size,
		},
	},
	Action: func(c *cli.Context) error {
		if err := ResizeDevice(sandboxID, blockDevice, size); err != nil {
			return cli.NewExitError(fmt.Sprintf("Failed to resize device: %v", err), 1)
		}

		return nil
	},
}

// ResizeDevice resizes a direct volume inside the guest.
func ResizeDevice(sandboxID string, blockDevice string, size uint64) error {

	resizeReq := containerdshim.BlockResizeRequest{
		BlockDevice: blockDevice,
		Size:        size,
	}

	encoded, err := json.Marshal(resizeReq)
	if err != nil {
		return fmt.Errorf("failed to marshal resize request: %v", err)
	}

	if err := shimclient.DoPost(sandboxID, defaultTimeout, containerdshim.BlockDeviceResizeUrl, "application/json", encoded); err != nil {
		return fmt.Errorf("shim client request failed: %v", err)
	}

	return nil
}
