// Copyright (c) 2018 Huawei Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/sys/unix"

	"github.com/containernetworking/plugins/pkg/ns"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
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
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		return networkModifyCommand(ctx, context.Args().First(), context.Args().Get(1), interfaceType, true)
	},
}

var delIfaceCommand = cli.Command{
	Name:      "del-iface",
	Usage:     "delete an interface from a container",
	ArgsUsage: `del-iface <container-id> file or - for stdin`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		return networkModifyCommand(ctx, context.Args().First(), context.Args().Get(1), interfaceType, false)
	},
}

var listIfacesCommand = cli.Command{
	Name:      "list-ifaces",
	Usage:     "list network interfaces in a container",
	ArgsUsage: `list-ifaces <container-id>`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		return networkListCommand(ctx, context.Args().First(), interfaceType)
	},
}

var updateRoutesCommand = cli.Command{
	Name:      "update-routes",
	Usage:     "update routes of a container",
	ArgsUsage: `update-routes <container-id> file or - for stdin`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		return networkModifyCommand(ctx, context.Args().First(), context.Args().Get(1), routeType, true)
	},
}

var listRoutesCommand = cli.Command{
	Name:      "list-routes",
	Usage:     "list network routes in a container",
	ArgsUsage: `list-routes <container-id>`,
	Flags:     []cli.Flag{},
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		return networkListCommand(ctx, context.Args().First(), routeType)
	},
}

func networkModifyCommand(ctx context.Context, containerID, input string, opType networkType, add bool) (err error) {
	status, sandboxID, err := getExistingContainerInfo(ctx, containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	kataLog = kataLog.WithFields(logrus.Fields{
		"container": containerID,
		"sandbox":   sandboxID,
	})

	setExternalLoggers(ctx, kataLog)

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
		var inf, resultingInf *types.Interface
		if err = json.NewDecoder(f).Decode(&inf); err != nil {
			return err
		}
		if add {
			resultingInf, err = vci.AddInterface(ctx, sandboxID, inf)
			if err != nil {
				kataLog.WithField("resulting-interface", fmt.Sprintf("%+v", resultingInf)).
					WithError(err).Error("add interface failed")
			}
		} else {
			resultingInf, err = vci.RemoveInterface(ctx, sandboxID, inf)
			if err != nil {
				kataLog.WithField("resulting-interface", fmt.Sprintf("%+v", resultingInf)).
					WithError(err).Error("delete interface failed")
			}
		}
		json.NewEncoder(output).Encode(resultingInf)
	case routeType:
		var routes, resultingRoutes []*types.Route
		if err = json.NewDecoder(f).Decode(&routes); err != nil {
			return err
		}
		resultingRoutes, err = vci.UpdateRoutes(ctx, sandboxID, routes)
		json.NewEncoder(output).Encode(resultingRoutes)
		if err != nil {
			kataLog.WithField("resulting-routes", fmt.Sprintf("%+v", resultingRoutes)).
				WithError(err).Error("update routes failed")
		}
	}
	return err
}

func networkListCommand(ctx context.Context, containerID string, opType networkType) (err error) {
	status, sandboxID, err := getExistingContainerInfo(ctx, containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	kataLog = kataLog.WithFields(logrus.Fields{
		"container": containerID,
		"sandbox":   sandboxID,
	})

	setExternalLoggers(ctx, kataLog)

	// container MUST be running
	if status.State.State != vc.StateRunning {
		return fmt.Errorf("container %s is not running", containerID)
	}

	var file = defaultOutputFile

	switch opType {
	case interfaceType:
		var interfaces []*types.Interface
		interfaces, err = vci.ListInterfaces(ctx, sandboxID)
		if err != nil {
			kataLog.WithField("existing-interfaces", fmt.Sprintf("%+v", interfaces)).
				WithError(err).Error("list interfaces failed")
		}
		json.NewEncoder(file).Encode(interfaces)
	case routeType:
		var routes []*types.Route
		routes, err = vci.ListRoutes(ctx, sandboxID)
		if err != nil {
			kataLog.WithField("resulting-routes", fmt.Sprintf("%+v", routes)).
				WithError(err).Error("update routes failed")
		}
		json.NewEncoder(file).Encode(routes)
	}
	return err
}

const procMountInfoFile = "/proc/self/mountinfo"

// getNetNsFromBindMount returns the network namespace for the bind-mounted path
func getNetNsFromBindMount(nsPath string, procMountFile string) (string, error) {
	netNsMountType := "nsfs"

	// Resolve all symlinks in the path as the mountinfo file contains
	// resolved paths.
	nsPath, err := filepath.EvalSymlinks(nsPath)
	if err != nil {
		return "", err
	}

	f, err := os.Open(procMountFile)
	if err != nil {
		return "", err
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		text := scanner.Text()

		// Scan the mountinfo file to search for the network namespace path
		// This file contains mounts in the eg format:
		// "711 26 0:3 net:[4026532009] /run/docker/netns/default rw shared:535 - nsfs nsfs rw"
		//
		// Reference: https://www.kernel.org/doc/Documentation/filesystems/proc.txt

		// We are interested in the first 9 fields of this file,
		// to check for the correct mount type.
		fields := strings.Split(text, " ")
		if len(fields) < 9 {
			continue
		}

		// We check here if the mount type is a network namespace mount type, namely "nsfs"
		mountTypeFieldIdx := 8
		if fields[mountTypeFieldIdx] != netNsMountType {
			continue
		}

		// This is the mount point/destination for the mount
		mntDestIdx := 4
		if fields[mntDestIdx] != nsPath {
			continue
		}

		// This is the root/source of the mount
		return fields[3], nil
	}

	return "", nil
}

// hostNetworkingRequested checks if the network namespace requested is the
// same as the current process.
func hostNetworkingRequested(configNetNs string) (bool, error) {
	var evalNS, nsPath, currentNsPath string
	var err error

	// Net namespace provided as "/proc/pid/ns/net" or "/proc/<pid>/task/<tid>/ns/net"
	if strings.HasPrefix(configNetNs, "/proc") && strings.HasSuffix(configNetNs, "/ns/net") {
		if _, err := os.Stat(configNetNs); err != nil {
			return false, err
		}

		// Here we are trying to resolve the path but it fails because
		// namespaces links don't really exist. For this reason, the
		// call to EvalSymlinks will fail when it will try to stat the
		// resolved path found. As we only care about the path, we can
		// retrieve it from the PathError structure.
		if _, err = filepath.EvalSymlinks(configNetNs); err != nil {
			nsPath = err.(*os.PathError).Path
		} else {
			return false, fmt.Errorf("Net namespace path %s is not a symlink", configNetNs)
		}

		_, evalNS = filepath.Split(nsPath)

	} else {
		// Bind-mounted path provided
		evalNS, _ = getNetNsFromBindMount(configNetNs, procMountInfoFile)
	}

	currentNS := fmt.Sprintf("/proc/%d/task/%d/ns/net", os.Getpid(), unix.Gettid())
	if _, err = filepath.EvalSymlinks(currentNS); err != nil {
		currentNsPath = err.(*os.PathError).Path
	} else {
		return false, fmt.Errorf("Unexpected: Current network namespace path is not a symlink")
	}

	_, evalCurrentNS := filepath.Split(currentNsPath)

	if evalNS == evalCurrentNS {
		return true, nil
	}

	return false, nil
}

func setupNetworkNamespace(config *vc.NetworkConfig) error {
	if config.DisableNewNetNs {
		kataLog.Info("DisableNewNetNs is on, shim and hypervisor are running in the host netns")
		return nil
	}

	if config.NetNSPath == "" {
		n, err := ns.NewNS()
		if err != nil {
			return err
		}

		config.NetNSPath = n.Path()
		config.NetNsCreated = true

		return nil
	}

	isHostNs, err := hostNetworkingRequested(config.NetNSPath)
	if err != nil {
		return err
	}
	if isHostNs {
		return fmt.Errorf("Host networking requested, not supported by runtime")
	}

	return nil
}
