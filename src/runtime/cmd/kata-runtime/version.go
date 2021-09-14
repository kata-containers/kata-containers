// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"github.com/urfave/cli"
)

var versionCLICommand = cli.Command{
	Name:  "version",
	Usage: "display version details",
	Action: func(context *cli.Context) error {
		cli.VersionPrinter(context)
		return nil
	},
}
