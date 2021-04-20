// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"net/http"

	ctrshim "github.com/kata-containers/kata-containers/src/runtime/containerd-shim-v2"
	kataMonitor "github.com/kata-containers/kata-containers/src/runtime/pkg/kata-monitor"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/urfave/cli"
)

const (
	podID          = "sandbox-id"
	hostBlkDevName = "host-block-device-name"
	volumeSize     = "volume-size"
)

var fsSubCmds = []cli.Command{
	getStatsCommand,
	resizeCommand,
}

var kataFsCLICommand = cli.Command{
	Name:        "fs",
	Usage:       "file system management",
	Subcommands: fsSubCmds,
	Action: func(context *cli.Context) {
		cli.ShowSubcommandHelp(context)
	},
}

var podIDFlag = cli.StringFlag{
	Name:  podID,
	Usage: "Pod ID associated with the given volume",
}

var blkDevFlag = cli.StringFlag{
	Name:  hostBlkDevName,
	Usage: "Block device on host",
}

var volSizeFlag = cli.StringFlag{
	Name:  volumeSize,
	Usage: "Requested size of volume in bytes",
}

var getStatsCommand = cli.Command{
	Name:  "get-stats",
	Usage: "Get filesystem stats for a given volume mount",
	Flags: []cli.Flag{
		podIDFlag,
		blkDevFlag,
	},
	Action: func(c *cli.Context) error {

		sandboxID := c.String(podID)
		if err := katautils.VerifyContainerID(sandboxID); err != nil {
			return err
		}

		// get connection to the appropriate containerd shim
		client, err := kataMonitor.BuildShimClient(sandboxID, defaultTimeout)
		if err != nil {
			return err
		}

		req, err := json.Marshal(
			ctrshim.FsStatsRequest{
				BlkDevice: c.String(hostBlkDevName),
			},
		)
		if err != nil {
			return err
		}

		resp, err := client.Post("http://shim/fs-stats", "application/json", bytes.NewReader(req))
		if err != nil {
			fmt.Printf("Couldn't find sandbox with ID %v\n", sandboxID)
			return err
		}
		defer resp.Body.Close()

		if resp.StatusCode != http.StatusOK {
			// check to see if there's a body to read:
			data, _ := ioutil.ReadAll(resp.Body)
			fmt.Println("Error with fs-stats request: ", string(data))
			return fmt.Errorf("Failed from %s shim-monitor: %d", sandboxID, resp.StatusCode)
		}
		data, err := ioutil.ReadAll(resp.Body)
		if err != nil {
			return err
		}

		fmt.Printf("%s\n", data)

		return nil
	},
}

var resizeCommand = cli.Command{
	Name:  "resize",
	Usage: "Resize filesystem for a given volume mount",
	Flags: []cli.Flag{
		podIDFlag,
		blkDevFlag,
		volSizeFlag,
	},
	Action: func(c *cli.Context) error {
		return fmt.Errorf("not yet supported")
	},
}
