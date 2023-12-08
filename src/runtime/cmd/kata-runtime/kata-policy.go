// Copyright (c) 2023 Intel Inc.
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

var policySubCmds = []cli.Command{
	setPolicyCommand,
}

var kataPolicyCommand = cli.Command{
	Name:        "policy",
	Usage:       "set policy within the Kata Containers guest",
	Subcommands: policySubCmds,
	Action: func(context *cli.Context) {
		cli.ShowSubcommandHelp(context)
	},
}

var setPolicyCommand = cli.Command{
	Name:  "set",
	Usage: "set policy in a specifc Kata Containers guest based on file",
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:        "sandbox-id",
			Usage:       "the target sandbox for setting the policy",
			Required:    true,
			Destination: &sandboxID,
		},
	},
	Action: func(c *cli.Context) error {
		policyFile := c.Args().Get(0)

		// verify sandbox exists:
		if err := katautils.VerifyContainerID(sandboxID); err != nil {
			return err
		}

		// verify policy were provided:
		if policyFile == "" {
			return fmt.Errorf("policy file not provided")
		}

		if !katautils.FileExists(policyFile) {
			return fmt.Errorf("policy file does not exist: %s", policyFile)
		}

		// Read file into buffer, and make request to the appropriate shim
		buf, err := os.ReadFile(policyFile)
		if err != nil {
			return err
		}

		url := containerdshim.PolicyUrl

		if err = shimclient.DoPut(sandboxID, defaultTimeout, url, "application/octet-stream", buf); err != nil {
			return fmt.Errorf("Error observed when making policy-set request(%s): %s", policyFile, err)
		}

		return nil
	},
}
