// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vf "github.com/kata-containers/runtime/virtcontainers/factory"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/urfave/cli"
)

var factorySubCmds = []cli.Command{
	initFactoryCommand,
	destroyFactoryCommand,
	statusFactoryCommand,
}

var factoryCLICommand = cli.Command{
	Name:        "factory",
	Usage:       "manage vm factory",
	Subcommands: factorySubCmds,
	Action: func(context *cli.Context) {
		cli.ShowSubcommandHelp(context)
	},
}

var initFactoryCommand = cli.Command{
	Name:  "init",
	Usage: "initialize a VM factory based on kata-runtime configuration",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}

		runtimeConfig, ok := c.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		if runtimeConfig.FactoryConfig.Template {
			factoryConfig := vf.Config{
				Template: true,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
					AgentType:        runtimeConfig.AgentType,
					AgentConfig:      runtimeConfig.AgentConfig,
				},
			}
			kataLog.WithField("factory", factoryConfig).Info("create vm factory")
			_, err := vf.NewFactory(ctx, factoryConfig, false)
			if err != nil {
				kataLog.WithError(err).Error("create vm factory failed")
				return err
			}
			fmt.Fprintln(defaultOutputFile, "vm factory initialized")
		} else {
			kataLog.Error("vm factory is not enabled")
			fmt.Fprintln(defaultOutputFile, "vm factory is not enabled")
		}

		return nil
	},
}

var destroyFactoryCommand = cli.Command{
	Name:  "destroy",
	Usage: "destroy the VM factory",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}

		runtimeConfig, ok := c.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		if runtimeConfig.FactoryConfig.Template {
			factoryConfig := vf.Config{
				Template: true,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
					AgentType:        runtimeConfig.AgentType,
					AgentConfig:      runtimeConfig.AgentConfig,
				},
			}
			kataLog.WithField("factory", factoryConfig).Info("load vm factory")
			f, err := vf.NewFactory(ctx, factoryConfig, true)
			if err != nil {
				kataLog.WithError(err).Error("load vm factory failed")
				// ignore error
			} else {
				f.CloseFactory(ctx)
			}
		}
		fmt.Fprintln(defaultOutputFile, "vm factory destroyed")
		return nil
	},
}

var statusFactoryCommand = cli.Command{
	Name:  "status",
	Usage: "query the status of VM factory",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}

		runtimeConfig, ok := c.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
		if !ok {
			return errors.New("invalid runtime config")
		}

		if runtimeConfig.FactoryConfig.Template {
			factoryConfig := vf.Config{
				Template: true,
				VMConfig: vc.VMConfig{
					HypervisorType:   runtimeConfig.HypervisorType,
					HypervisorConfig: runtimeConfig.HypervisorConfig,
					AgentType:        runtimeConfig.AgentType,
					AgentConfig:      runtimeConfig.AgentConfig,
				},
			}
			kataLog.WithField("factory", factoryConfig).Info("load vm factory")
			_, err := vf.NewFactory(ctx, factoryConfig, true)
			if err != nil {
				fmt.Fprintln(defaultOutputFile, "vm factory is off")
			} else {
				fmt.Fprintln(defaultOutputFile, "vm factory is on")
			}
		} else {
			fmt.Fprintln(defaultOutputFile, "vm factory not enabled")
		}
		return nil
	},
}
