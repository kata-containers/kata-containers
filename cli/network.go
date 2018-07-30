// Copyright (c) 2018 Huawei Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/kata-containers/agent/protocols/grpc"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/sirupsen/logrus"
	"github.com/urfave/cli"
)

type networkType int

const (
	// interfaceType for interface operation
	interfaceType networkType = iota

	routeType
)

var kataNetworkCLICommand = cli.Command{
	Name:  "kata-network",
	Usage: "manage interfaces and routes for container",
	Subcommands: []cli.Command{
		addIfaceCommand,
		delIfaceCommand,
		listIfacesCommand,
		updateRoutesCommand,
		listRoutesCommand,
	},
	Action: func(context *cli.Context) error {
		return cli.ShowSubcommandHelp(context)
	},
}

var addIfaceCommand = cli.Command{
	Name:      "add-iface",
	Usage:     "add an interface to a container",
	ArgsUsage: `add-iface <container-id> file or - for stdin`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		return networkModifyCommand(context.Args().First(), context.Args().Get(1), interfaceType, true)
	},
}

var delIfaceCommand = cli.Command{
	Name:      "del-iface",
	Usage:     "delete an interface from a container",
	ArgsUsage: `del-iface <container-id> file or - for stdin`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		return networkModifyCommand(context.Args().First(), context.Args().Get(1), interfaceType, false)
	},
}

var listIfacesCommand = cli.Command{
	Name:      "list-ifaces",
	Usage:     "list network interfaces in a container",
	ArgsUsage: `list-ifaces <container-id>`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		return networkListCommand(context.Args().First(), interfaceType)
	},
}

var updateRoutesCommand = cli.Command{
	Name:      "update-routes",
	Usage:     "update routes of a container",
	ArgsUsage: `update-routes <container-id> file or - for stdin`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		return networkModifyCommand(context.Args().First(), context.Args().Get(1), routeType, true)
	},
}

var listRoutesCommand = cli.Command{
	Name:      "list-routes",
	Usage:     "list network routes in a container",
	ArgsUsage: `list-routes <container-id>`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		return networkListCommand(context.Args().First(), routeType)
	},
}

func networkModifyCommand(containerID, input string, opType networkType, add bool) (err error) {
	status, sandboxID, err := getExistingContainerInfo(containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	kataLog = kataLog.WithFields(logrus.Fields{
		"container": containerID,
		"sandbox":   sandboxID,
	})

	setExternalLoggers(kataLog)

	// container MUST be running
	if status.State.State != vc.StateRunning {
		return fmt.Errorf("container %s is not running", containerID)
	}

	var (
		f      *os.File
		output = defaultOutputFile
	)

	if input == "-" {
		f = os.Stdin
	} else {
		f, err = os.Open(input)
		if err != nil {
			return err
		}
		defer f.Close()
	}
	switch opType {
	case interfaceType:
		var inf, resultingInf *grpc.Interface
		if err = json.NewDecoder(f).Decode(&inf); err != nil {
			return err
		}
		if add {
			resultingInf, err = vci.AddInterface(sandboxID, inf)
			if err != nil {
				kataLog.WithField("resulting-interface", fmt.Sprintf("%+v", resultingInf)).
					WithError(err).Error("add interface failed")
			}
		} else {
			resultingInf, err = vci.RemoveInterface(sandboxID, inf)
			if err != nil {
				kataLog.WithField("resulting-interface", fmt.Sprintf("%+v", resultingInf)).
					WithError(err).Error("delete interface failed")
			}
		}
		json.NewEncoder(output).Encode(resultingInf)
	case routeType:
		var routes, resultingRoutes []*grpc.Route
		if err = json.NewDecoder(f).Decode(&routes); err != nil {
			return err
		}
		resultingRoutes, err = vci.UpdateRoutes(sandboxID, routes)
		json.NewEncoder(output).Encode(resultingRoutes)
		if err != nil {
			kataLog.WithField("resulting-routes", fmt.Sprintf("%+v", resultingRoutes)).
				WithError(err).Error("update routes failed")
		}
	}
	return err
}

func networkListCommand(containerID string, opType networkType) (err error) {
	status, sandboxID, err := getExistingContainerInfo(containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	kataLog = kataLog.WithFields(logrus.Fields{
		"container": containerID,
		"sandbox":   sandboxID,
	})

	setExternalLoggers(kataLog)

	// container MUST be running
	if status.State.State != vc.StateRunning {
		return fmt.Errorf("container %s is not running", containerID)
	}

	var file = defaultOutputFile

	switch opType {
	case interfaceType:
		var interfaces []*grpc.Interface
		interfaces, err = vci.ListInterfaces(sandboxID)
		if err != nil {
			kataLog.WithField("existing-interfaces", fmt.Sprintf("%+v", interfaces)).
				WithError(err).Error("list interfaces failed")
		}
		json.NewEncoder(file).Encode(interfaces)
	case routeType:
		var routes []*grpc.Route
		routes, err = vci.ListRoutes(sandboxID)
		if err != nil {
			kataLog.WithField("resulting-routes", fmt.Sprintf("%+v", routes)).
				WithError(err).Error("update routes failed")
		}
		json.NewEncoder(file).Encode(routes)
	}
	return err
}
