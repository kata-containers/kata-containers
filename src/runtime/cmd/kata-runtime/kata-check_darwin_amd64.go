// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"
	"os"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/oci"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

const (
	archCPUVendorField = ""
	archCPUModelField  = ""

	successMessageCapable = "System is capable of running " + katautils.PROJECT
	successMessageCreate  = "System can currently create " + katautils.PROJECT
	failMessage           = "System is not capable of running " + katautils.PROJECT

	procCPUInfo = "N/A"

	// If set, do not perform any network checks
	noNetworkEnvVar = "KATA_CHECK_NO_NETWORK"
)

// archRequiredCPUFlags maps a CPU flag value to search for and a
// human-readable description of that value.
var archRequiredCPUFlags map[string]string

// archRequiredCPUAttribs maps a CPU (non-CPU flag) attribute value to search for
// and a human-readable description of that value.
var archRequiredCPUAttribs map[string]string

// archRequiredKernelModules maps a required module name to a human-readable
// description of the modules functionality and an optional list of
// required module parameters.
var archRequiredKernelModules map[string]kernelModule

func setCPUtype(hypervisorType vc.HypervisorType) error {
	return nil
}

func hostIsVMContainerCapable(details vmContainerCapableDetails) error {
	return nil
}

func archHostCanCreateVMContainer(hypervisorType vc.HypervisorType) error {
	return nil
}

func getCPUInfo(cpuInfoFile string) (string, error) {
	return "", nil
}

func getCPUDetails() (vendor, model string, err error) {
	return "", "", nil
}

var kataCheckCLICommand = cli.Command{
	Name:    "check",
	Aliases: []string{"kata-check"},
	Usage:   "tests if system can run " + katautils.PROJECT,
	Flags: []cli.Flag{
		cli.BoolFlag{
			Name:  "check-version-only",
			Usage: "Only compare the current and latest available versions (requires network, non-root only)",
		},
		cli.BoolFlag{
			Name:  "include-all-releases",
			Usage: "Don't filter out pre-release release versions",
		},
		cli.BoolFlag{
			Name:  "no-network-checks, n",
			Usage: "Do not run any checks using the network",
		},
		cli.BoolFlag{
			Name:  "only-list-releases",
			Usage: "Only list newer available releases (non-root only)",
		},
		cli.BoolFlag{
			Name:  "strict, s",
			Usage: "perform strict checking",
		},
		cli.BoolFlag{
			Name:  "verbose, v",
			Usage: "display the list of checks performed",
		},
	},
	Description: fmt.Sprintf(`tests if system can run %s and version is current.

ENVIRONMENT VARIABLES:

- %s: If set to any value, act as if "--no-network-checks" was specified.

EXAMPLES:

- Perform basic checks:

  $ %s check

- Local basic checks only:

  $ %s check --no-network-checks

- Perform further checks:

  $ sudo %s check

- Just check if a newer version is available:

  $ %s check --check-version-only

- List available releases (shows output in format "version;release-date;url"):

  $ %s check --only-list-releases

- List all available releases (includes pre-release versions):

  $ %s check --only-list-releases --include-all-releases
`,
		katautils.PROJECT,
		noNetworkEnvVar,
		katautils.NAME,
		katautils.NAME,
		katautils.NAME,
		katautils.NAME,
		katautils.NAME,
		katautils.NAME,
	),

	Action: func(context *cli.Context) error {
		verbose := context.Bool("verbose")
		if verbose {
			kataLog.Logger.SetLevel(logrus.InfoLevel)
		}

		if !context.Bool("no-network-checks") && os.Getenv(noNetworkEnvVar) == "" {
			cmd := RelCmdCheck

			if context.Bool("only-list-releases") {
				cmd = RelCmdList
			}

			if os.Geteuid() == 0 {
				kataLog.Warn("Not running network checks as super user")
			} else {
				err := HandleReleaseVersions(cmd, katautils.VERSION, context.Bool("include-all-releases"))
				if err != nil {
					return err
				}
			}
		}

		if context.Bool("check-version-only") || context.Bool("only-list-releases") {
			return nil
		}

		runtimeConfig, ok := context.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("check: cannot determine runtime config")
		}

		err := setCPUtype(runtimeConfig.HypervisorType)
		if err != nil {
			return err
		}

		details := vmContainerCapableDetails{
			cpuInfoFile:           procCPUInfo,
			requiredCPUFlags:      archRequiredCPUFlags,
			requiredCPUAttribs:    archRequiredCPUAttribs,
			requiredKernelModules: archRequiredKernelModules,
		}

		err = hostIsVMContainerCapable(details)
		if err != nil {
			return err
		}
		fmt.Println(successMessageCapable)

		if os.Geteuid() == 0 {
			err = archHostCanCreateVMContainer(runtimeConfig.HypervisorType)
			if err != nil {
				return err
			}

			fmt.Println(successMessageCreate)
		}

		return nil
	},
}
