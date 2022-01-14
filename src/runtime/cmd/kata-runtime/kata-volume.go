// Copyright (c) 2022 Databricks Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/kata-containers/kata-containers/src/runtime/pkg/direct-volume"
	"github.com/urfave/cli"
)

var volumeSubCmds = []cli.Command{
	addCommand,
	removeCommand,
	statsCommand,
	resizeCommand,
}

var (
	mountInfo  string
	volumePath string
	size       uint64
)

var kataVolumeCommand = cli.Command{
	Name:        "direct-volume",
	Usage:       "directly assign a volume to Kata Containers to manage",
	Subcommands: volumeSubCmds,
	Action: func(context *cli.Context) {
		cli.ShowSubcommandHelp(context)
	},
}

var addCommand = cli.Command{
	Name:  "add",
	Usage: "add a direct assigned block volume device to the Kata Containers runtime",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:        "volume-path",
			Usage:       "the target volume path the volume is published to",
			Destination: &volumePath,
		},
		cli.StringFlag{
			Name:        "mount-info",
			Usage:       "the mount info for the Kata Containers runtime to manage the volume",
			Destination: &mountInfo,
		},
	},
	Action: func(c *cli.Context) error {
		return volume.Add(volumePath, mountInfo)
	},
}

var removeCommand = cli.Command{
	Name:  "remove",
	Usage: "remove a direct assigned block volume device from the Kata Containers runtime",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:        "volume-path",
			Usage:       "the target volume path the volume is published to",
			Destination: &volumePath,
		},
	},
	Action: func(c *cli.Context) error {
		return volume.Remove(volumePath)
	},
}

var statsCommand = cli.Command{
	Name:  "stats",
	Usage: "get the filesystem stat of a direct assigned volume",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:        "volume-path",
			Usage:       "the target volume path the volume is published to",
			Destination: &volumePath,
		},
	},
	Action: func(c *cli.Context) (string, error) {
		stats, err := volume.Stats(volumePath)
		if err != nil {
			return "", err
		}

		return string(stats), nil
	},
}

var resizeCommand = cli.Command{
	Name:  "resize",
	Usage: "resize a direct assigned block volume",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:        "volume-path",
			Usage:       "the target volume path the volume is published to",
			Destination: &volumePath,
		},
		cli.Uint64Flag{
			Name:        "size",
			Usage:       "the new size of the volume",
			Destination: &size,
		},
	},
	Action: func(c *cli.Context) error {
		return volume.Resize(volumePath, size)
	},
}
