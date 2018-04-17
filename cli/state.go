// Copyright (c) 2014,2015,2016 Docker, Inc.
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/urfave/cli"
)

var stateCLICommand = cli.Command{
	Name:  "state",
	Usage: "output the state of a container",
	ArgsUsage: `<container-id>

   <container-id> is your name for the instance of the container`,
	Description: `The state command outputs current state information for the
instance of a container.`,
	Action: func(context *cli.Context) error {
		args := context.Args()
		if len(args) != 1 {
			return fmt.Errorf("Expecting only one container ID, got %d: %v", len(args), []string(args))
		}

		return state(args.First())
	},
}

func state(containerID string) error {
	// Checks the MUST and MUST NOT from OCI runtime specification
	status, _, err := getExistingContainerInfo(containerID)
	if err != nil {
		return err
	}

	// Convert the status to the expected State structure
	state := oci.StatusToOCIState(status)

	stateJSON, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}

	// Print stateJSON to stdout
	fmt.Fprintf(os.Stdout, "%s", stateJSON)

	return nil
}
