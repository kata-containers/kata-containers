// Copyright (c) 2014,2015,2016,2017 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"
	"syscall"
	"text/tabwriter"
	"time"

	"github.com/urfave/cli"

	"github.com/kata-containers/runtime/pkg/katautils"
	vc "github.com/kata-containers/runtime/virtcontainers"
	oci "github.com/kata-containers/runtime/virtcontainers/pkg/oci"
)

const formatOptions = `table or json`

// containerState represents the platform agnostic pieces relating to a
// running container's status and state
type containerState struct {
	// Version is the OCI version for the container
	Version string `json:"ociVersion"`
	// ID is the container ID
	ID string `json:"id"`
	// InitProcessPid is the init process id in the parent namespace
	InitProcessPid int `json:"pid"`
	// Status is the current status of the container, running, paused, ...
	Status string `json:"status"`
	// Bundle is the path on the filesystem to the bundle
	Bundle string `json:"bundle"`
	// Rootfs is a path to a directory containing the container's root filesystem.
	Rootfs string `json:"rootfs"`
	// Created is the unix timestamp for the creation time of the container in UTC
	Created time.Time `json:"created"`
	// Annotations is the user defined annotations added to the config.
	Annotations map[string]string `json:"annotations,omitempty"`
	// The owner of the state directory (the owner of the container).
	Owner string `json:"owner"`
}

type asset struct {
	Path   string `json:"path"`
	Custom bool   `json:"bool"`
}

// hypervisorDetails stores details of the hypervisor used to host
// the container
type hypervisorDetails struct {
	HypervisorAsset asset `json:"hypervisorAsset"`
	ImageAsset      asset `json:"imageAsset"`
	KernelAsset     asset `json:"kernelAsset"`
}

// fullContainerState specifies the core state plus the hypervisor
// details
type fullContainerState struct {
	containerState
	CurrentHypervisorDetails hypervisorDetails `json:"currentHypervisor"`
	LatestHypervisorDetails  hypervisorDetails `json:"latestHypervisor"`
	StaleAssets              []string
}

type formatState interface {
	Write(state []fullContainerState, showAll bool, file *os.File) error
}

type formatJSON struct{}
type formatIDList struct{}
type formatTabular struct{}

var listCLICommand = cli.Command{
	Name:  "list",
	Usage: "lists containers started by " + name + " with the given root",
	ArgsUsage: `

Where the given root is specified via the global option "--root"
(default: "` + defaultRootDirectory + `").

EXAMPLE 1:
To list containers created via the default "--root":
       # ` + name + ` list

EXAMPLE 2:
To list containers created using a non-default value for "--root":
       # ` + name + ` --root value list`,
	Flags: []cli.Flag{
		cli.StringFlag{
			Name:  "format, f",
			Value: "table",
			Usage: `select one of: ` + formatOptions,
		},
		cli.BoolFlag{
			Name:  "quiet, q",
			Usage: "display only container IDs",
		},
		cli.BoolFlag{
			Name:  "kata-all",
			Usage: "display all available " + project + " information",
		},
	},
	Action: func(context *cli.Context) error {
		ctx, err := cliContextToContext(context)
		if err != nil {
			return err
		}

		span, ctx := katautils.Trace(ctx, "list")
		defer span.Finish()

		s, err := getContainers(ctx, context)
		if err != nil {
			return err
		}

		file := defaultOutputFile
		showAll := context.Bool("kata-all")

		var fs formatState = formatIDList{}

		if context.Bool("quiet") {
			fs = formatIDList{}
		} else {

			switch context.String("format") {
			case "table":
				fs = formatTabular{}

			case "json":
				fs = formatJSON{}

			default:
				return fmt.Errorf("invalid format option")
			}
		}

		return fs.Write(s, showAll, file)
	},
}

// getStaleAssetsreturns compares the two specified hypervisorDetails objects
// and returns a list of strings representing which assets in "old" are not
// current compared to "new". If old and new are identical, the empty string
// will be returned.
//
// Notes:
//
// - This function is trivial because it relies upon the fact that new
//   containers are always created with the latest versions of all assets.
//
// - WARNING: Since this function only compares local values, it is unable to
//   determine if newer (remote) assets are available.
func getStaleAssets(old, new hypervisorDetails) []string {
	var stale []string

	if old.KernelAsset.Path != new.KernelAsset.Path {
		if old.KernelAsset.Custom {
			// The workload kernel asset is a custom one, i.e. it's not coming
			// from the runtime configuration file. Thus it does not make sense
			// to compare it against the configured kernel asset.
			// We assume a custom kernel asset has been updated if the
			// corresponding path no longer exists, i.e. it's been replaced by
			// a new kernel, e.g. with a new version name.
			// Replacing a custom kernel asset binary with exactly the same
			// binary name won't allow us to detect if it's staled or not.
			if _, err := os.Stat(old.KernelAsset.Path); os.IsNotExist(err) {
				stale = append(stale, "kernel")
			}
		} else {
			stale = append(stale, "kernel")
		}
	}

	if old.ImageAsset.Path != new.ImageAsset.Path {
		if old.ImageAsset.Custom {
			// The workload image asset is a custom one, i.e. it's not coming
			// from the runtime configuration file. Thus it does not make sense
			// to compare it against the configured image asset.
			// We assume a custom image asset has been updated if the
			// corresponding path no longer exists, i.e. it's been replaced by
			// a new image, e.g. with a new version name.
			// Replacing a custom image asset binary with exactly the same
			// binary name won't allow us to detect if it's staled or not.
			if _, err := os.Stat(old.ImageAsset.Path); os.IsNotExist(err) {
				stale = append(stale, "image")
			}
		} else {
			stale = append(stale, "image")
		}
	}

	return stale
}

func (f formatIDList) Write(state []fullContainerState, showAll bool, file *os.File) error {
	for _, item := range state {
		_, err := fmt.Fprintln(file, item.ID)
		if err != nil {
			return err
		}
	}

	return nil
}

func (f formatTabular) Write(state []fullContainerState, showAll bool, file *os.File) error {
	// values used by runc
	flags := uint(0)
	minWidth := 12
	tabWidth := 1
	padding := 3

	w := tabwriter.NewWriter(file, minWidth, tabWidth, padding, ' ', flags)

	fmt.Fprint(w, "ID\tPID\tSTATUS\tBUNDLE\tCREATED\tOWNER")

	if showAll {
		fmt.Fprint(w, "\tHYPERVISOR\tKERNEL\tIMAGE\tLATEST-KERNEL\tLATEST-IMAGE\tSTALE\n")
	} else {
		fmt.Fprintf(w, "\n")
	}

	for _, item := range state {
		fmt.Fprintf(w, "%s\t%d\t%s\t%s\t%s\t%s",
			item.ID,
			item.InitProcessPid,
			item.Status,
			item.Bundle,
			item.Created.Format(time.RFC3339Nano),
			item.Owner)

		if showAll {
			stale := strings.Join(item.StaleAssets, ",")
			if stale == "" {
				stale = "-"
			}

			current := item.CurrentHypervisorDetails
			latest := item.LatestHypervisorDetails

			all := fmt.Sprintf("\t%s\t%s\t%s",
				current.HypervisorAsset.Path,
				current.KernelAsset.Path,
				current.ImageAsset.Path)

			if !current.KernelAsset.Custom {
				all += fmt.Sprintf("\t%s", latest.KernelAsset.Path)
			} else {
				all += fmt.Sprintf("\t%s", current.KernelAsset.Path)
			}

			if !current.ImageAsset.Custom {
				all += fmt.Sprintf("\t%s", latest.ImageAsset.Path)
			} else {
				all += fmt.Sprintf("\t%s", current.ImageAsset.Path)
			}

			all += fmt.Sprintf("\t%s\n", stale)

			fmt.Fprint(w, all)
		} else {
			fmt.Fprint(w, "\n")
		}
	}

	return w.Flush()
}

func (f formatJSON) Write(state []fullContainerState, showAll bool, file *os.File) error {
	return json.NewEncoder(file).Encode(state)
}

// getDirOwner returns the UID of the specified directory
func getDirOwner(dir string) (uint32, error) {
	if dir == "" {
		return 0, errors.New("BUG: need directory")
	}
	st, err := os.Stat(dir)
	if err != nil {
		return 0, err
	}

	if !st.IsDir() {
		return 0, fmt.Errorf("%q is not a directory", dir)
	}

	statType, ok := st.Sys().(*syscall.Stat_t)
	if !ok {
		return 0, fmt.Errorf("cannot convert %+v to stat type for directory %q", st, dir)
	}

	return statType.Uid, nil
}

func getContainers(ctx context.Context, context *cli.Context) ([]fullContainerState, error) {
	runtimeConfig, ok := context.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
	if !ok {
		return nil, errors.New("invalid runtime config")
	}

	latestHypervisorDetails := getHypervisorDetails(&runtimeConfig.HypervisorConfig)

	sandboxList, err := vci.ListSandbox(ctx)
	if err != nil {
		return nil, err
	}

	var s []fullContainerState

	for _, sandbox := range sandboxList {
		if len(sandbox.ContainersStatus) == 0 {
			// ignore empty sandboxes
			continue
		}

		currentHypervisorDetails := getHypervisorDetails(&sandbox.HypervisorConfig)

		for _, container := range sandbox.ContainersStatus {
			ociState := oci.StatusToOCIState(container)
			staleAssets := getStaleAssets(currentHypervisorDetails, latestHypervisorDetails)

			uid, err := getDirOwner(container.RootFs)
			if err != nil {
				return nil, err
			}

			owner := fmt.Sprintf("#%v", uid)

			s = append(s, fullContainerState{
				containerState: containerState{
					Version:        ociState.Version,
					ID:             ociState.ID,
					InitProcessPid: ociState.Pid,
					Status:         ociState.Status,
					Bundle:         ociState.Bundle,
					Rootfs:         container.RootFs,
					Created:        container.StartTime,
					Annotations:    ociState.Annotations,
					Owner:          owner,
				},
				CurrentHypervisorDetails: currentHypervisorDetails,
				LatestHypervisorDetails:  latestHypervisorDetails,
				StaleAssets:              staleAssets,
			})
		}
	}

	return s, nil
}

// getHypervisorDetails returns details of the latest version of the
// hypervisor and the associated assets.
func getHypervisorDetails(hypervisorConfig *vc.HypervisorConfig) hypervisorDetails {
	hypervisorPath, err := hypervisorConfig.HypervisorAssetPath()
	if err != nil {
		hypervisorPath = hypervisorConfig.HypervisorPath
	}

	kernelPath, err := hypervisorConfig.KernelAssetPath()
	if err != nil {
		kernelPath = hypervisorConfig.KernelPath
	}

	imagePath, err := hypervisorConfig.ImageAssetPath()
	if err != nil {
		imagePath = hypervisorConfig.ImagePath
	}

	return hypervisorDetails{
		HypervisorAsset: asset{
			Path:   hypervisorPath,
			Custom: hypervisorConfig.CustomHypervisorAsset(),
		},
		KernelAsset: asset{
			Path:   kernelPath,
			Custom: hypervisorConfig.CustomKernelAsset(),
		},
		ImageAsset: asset{
			Path:   imagePath,
			Custom: hypervisorConfig.CustomImageAsset(),
		},
	}
}
