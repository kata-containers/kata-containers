// Copyright (c) 2022 Databricks Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"fmt"
	"net/url"

	containerdshim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	volume "github.com/kata-containers/kata-containers/src/runtime/pkg/direct-volume"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils/shimclient"

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
		if err := volume.Add(volumePath, mountInfo); err != nil {
			return cli.NewExitError(err.Error(), 1)
		}
		return nil
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
		if err := volume.Remove(volumePath); err != nil {
			return cli.NewExitError(err.Error(), 1)
		}
		return nil
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
	Action: func(c *cli.Context) error {
		stats, err := Stats(volumePath)
		if err != nil {
			return cli.NewExitError(err.Error(), 1)
		}

		fmt.Println(string(stats))
		return nil
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
		if err := Resize(volumePath, size); err != nil {
			return cli.NewExitError(err.Error(), 1)
		}
		return nil
	},
}

// Stats retrieves the filesystem stats of the direct volume inside the guest.
func Stats(volumePath string) ([]byte, error) {
	sandboxId, err := volume.GetSandboxIdForVolume(volumePath)
	if err != nil {
		return nil, err
	}
	volumeMountInfo, err := volume.VolumeMountInfo(volumePath)
	if err != nil {
		return nil, err
	}

	urlSafeDevicePath := url.PathEscape(volumeMountInfo.Device)
	body, err := shimclient.DoGet(sandboxId, defaultTimeout,
		fmt.Sprintf("%s?%s=%s", containerdshim.DirectVolumeStatUrl, containerdshim.DirectVolumePathKey, urlSafeDevicePath))
	if err != nil {
		return nil, err
	}
	return body, nil
}

// Resize resizes a direct volume inside the guest.
func Resize(volumePath string, size uint64) error {
	sandboxId, err := volume.GetSandboxIdForVolume(volumePath)
	if err != nil {
		return err
	}
	volumeMountInfo, err := volume.VolumeMountInfo(volumePath)
	if err != nil {
		return err
	}

	resizeReq := containerdshim.ResizeRequest{
		VolumePath: volumeMountInfo.Device,
		Size:       size,
	}
	encoded, err := json.Marshal(resizeReq)
	if err != nil {
		return err
	}
	return shimclient.DoPost(sandboxId, defaultTimeout, containerdshim.DirectVolumeResizeUrl, "application/json", encoded)
}
