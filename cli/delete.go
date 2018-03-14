// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"fmt"
	"os"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/urfave/cli"
)

var deleteCLICommand = cli.Command{
	Name:  "delete",
	Usage: "Delete any resources held by one or more containers",
	ArgsUsage: `<container-id> [container-id...]

   <container-id> is the name for the instance of the container.

EXAMPLE:
   If the container id is "ubuntu01" and ` + name + ` list currently shows the
   status of "ubuntu01" as "stopped" the following will delete resources held
   for "ubuntu01" removing "ubuntu01" from the ` + name + ` list of containers:

       # ` + name + ` delete ubuntu01`,
	Flags: []cli.Flag{
		cli.BoolFlag{
			Name:  "force, f",
			Usage: "Forcibly deletes the container if it is still running (uses SIGKILL)",
		},
	},
	Action: func(context *cli.Context) error {
		args := context.Args()
		if args.Present() == false {
			return fmt.Errorf("Missing container ID, should at least provide one")
		}

		force := context.Bool("force")
		for _, cID := range []string(args) {
			if err := delete(cID, force); err != nil {
				return err
			}
		}

		return nil
	},
}

func delete(containerID string, force bool) error {
	// Checks the MUST and MUST NOT from OCI runtime specification
	status, podID, err := getExistingContainerInfo(containerID)
	if err != nil {
		return err
	}

	containerID = status.ID

	containerType, err := oci.GetContainerType(status.Annotations)
	if err != nil {
		return err
	}

	// Retrieve OCI spec configuration.
	ociSpec, err := oci.GetOCIConfig(status)
	if err != nil {
		return err
	}

	forceStop := false
	if oci.StateToOCIState(status.State) == oci.StateRunning {
		if !force {
			return fmt.Errorf("Container still running, should be stopped")
		}

		forceStop = true
	}

	switch containerType {
	case vc.PodSandbox:
		if err := deletePod(podID); err != nil {
			return err
		}
	case vc.PodContainer:
		if err := deleteContainer(podID, containerID, forceStop); err != nil {
			return err
		}
	default:
		return fmt.Errorf("Invalid container type found")
	}

	// In order to prevent any file descriptor leak related to cgroups files
	// that have been previously created, we have to remove them before this
	// function returns.
	cgroupsPathList, err := processCgroupsPath(ociSpec, containerType.IsPod())
	if err != nil {
		return err
	}

	return removeCgroupsPath(containerID, cgroupsPathList)
}

func deletePod(podID string) error {
	if _, err := vci.StopPod(podID); err != nil {
		return err
	}

	if _, err := vci.DeletePod(podID); err != nil {
		return err
	}

	return nil
}

func deleteContainer(podID, containerID string, forceStop bool) error {
	if forceStop {
		if _, err := vci.StopContainer(podID, containerID); err != nil {
			return err
		}
	}

	if _, err := vci.DeleteContainer(podID, containerID); err != nil {
		return err
	}

	return nil
}

func removeCgroupsPath(containerID string, cgroupsPathList []string) error {
	if len(cgroupsPathList) == 0 {
		kataLog.WithField("container", containerID).Info("Cgroups files not removed because cgroupsPath was empty")
		return nil
	}

	for _, cgroupsPath := range cgroupsPathList {
		if err := os.RemoveAll(cgroupsPath); err != nil {
			return err
		}
	}

	return nil
}
