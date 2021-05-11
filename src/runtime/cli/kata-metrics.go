// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"

	kataMonitor "github.com/kata-containers/kata-containers/src/runtime/pkg/kata-monitor"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/urfave/cli"
)

var kataMetricsCLICommand = cli.Command{
	Name:      "metrics",
	Usage:     "gather metrics associated with infrastructure used to run a sandbox",
	UsageText: "metrics <sandbox id>",
	Action: func(context *cli.Context) error {

		sandboxID := context.Args().Get(0)

		if err := katautils.VerifyContainerID(sandboxID); err != nil {
			return err
		}

		// Get the metrics!
		metrics, err := kataMonitor.GetSandboxMetrics(sandboxID)
		if err != nil {
			return err
		}

		fmt.Printf("%s\n", metrics)

		return nil
	},
}
