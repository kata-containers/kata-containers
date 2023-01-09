// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"

	containerdshim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils/shimclient"
	"github.com/urfave/cli"
)

var (
	sandboxID string
	isIPv6    bool
)
var iptablesSubCmds = []cli.Command{
	getIPTablesCommand,
	setIPTablesCommand,
}

var kataIPTablesCommand = cli.Command{
	Name:        "iptables",
	Usage:       "get or set iptables within the Kata Containers guest",
	Subcommands: iptablesSubCmds,
	Action: func(context *cli.Context) {
		cli.ShowSubcommandHelp(context)
	},
}

var getIPTablesCommand = cli.Command{
	Name:  "get",
	Usage: "get iptables from the Kata Containers guest",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:        "sandbox-id",
			Usage:       "the target sandbox for getting the iptables",
			Required:    true,
			Destination: &sandboxID,
		},
		cli.BoolFlag{
			Name:        "v6",
			Usage:       "indicate we're requesting ipv6 iptables",
			Destination: &isIPv6,
		},
	},
	Action: func(c *cli.Context) error {
		// verify sandbox exists:
		if err := katautils.VerifyContainerID(sandboxID); err != nil {
			return err
		}

		url := containerdshim.IPTablesUrl
		if isIPv6 {
			url = containerdshim.IP6TablesUrl
		}
		body, err := shimclient.DoGet(sandboxID, defaultTimeout, url)
		if err != nil {
			return err
		}

		fmt.Println(string(body))
		return nil
	},
}

var setIPTablesCommand = cli.Command{
	Name:  "set",
	Usage: "set iptables in a specifc Kata Containers guest based on file",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:        "sandbox-id",
			Usage:       "the target sandbox for setting the iptables",
			Required:    true,
			Destination: &sandboxID,
		},
		cli.BoolFlag{
			Name:        "v6",
			Usage:       "indicate we're requesting ipv6 iptables",
			Destination: &isIPv6,
		},
	},
	Action: func(c *cli.Context) error {
		iptablesFile := c.Args().Get(0)

		// verify sandbox exists:
		if err := katautils.VerifyContainerID(sandboxID); err != nil {
			return err
		}

		// verify iptables were provided:
		if iptablesFile == "" {
			return fmt.Errorf("iptables file not provided")
		}

		if !katautils.FileExists(iptablesFile) {
			return fmt.Errorf("iptables file does not exist: %s", iptablesFile)
		}

		// Read file into buffer, and make request to the appropriate shim
		buf, err := os.ReadFile(iptablesFile)
		if err != nil {
			return err
		}

		url := containerdshim.IPTablesUrl
		if isIPv6 {
			url = containerdshim.IP6TablesUrl
		}

		if err = shimclient.DoPut(sandboxID, defaultTimeout, url, "application/octet-stream", buf); err != nil {
			return fmt.Errorf("Error observed when making iptables-set request(%s): %s", iptablesFile, err)
		}

		return nil
	},
}
