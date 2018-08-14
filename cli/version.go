// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/urfave/cli"
)

var versionCLICommand = cli.Command{
	Name:  "version",
	Usage: "display version details",
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		span, _ := opentracing.StartSpanFromContext(ctx, "version")
		defer span.Finish()

		cli.VersionPrinter(context)
		return nil
	},
}
