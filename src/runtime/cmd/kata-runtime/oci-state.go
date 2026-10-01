// Copyright (c) 2024 Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"encoding/json"
	"fmt"

	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	vctypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/urfave/cli"
)

// ociState is the JSON output format of the OCI state command.
type ociState struct {
	OCIVersion  string
	ID          string
	Status      string
	PID         int
	Bundle      string
	Annotations map[string]string
}

// kataStateToOCI maps a Kata container state string to the OCI state string.
func kataStateToOCI(s vctypes.StateString) string {
	switch s {
	case vctypes.StateReady:
		return "created"
	case vctypes.StateRunning:
		return "running"
	case vctypes.StatePaused:
		return "paused"
	case vctypes.StateStopped:
		return "stopped"
	default:
		return "stopped"
	}
}

var stateCLICommand = cli.Command{
	Name:      "state",
	Usage:     "output the state of a container (OCI)",
	ArgsUsage: "<container-id>",
	Action: func(c *cli.Context) error {
		ctx, err := cliContextToContext(c)
		if err != nil {
			return err
		}
		return runStateCommand(ctx, c.Args().First())
	},
}

func runStateCommand(ctx context.Context, containerID string) error {
	if containerID == "" {
		return fmt.Errorf("container ID must be provided")
	}

	sandbox, err := vci.FetchSandbox(ctx, containerID)
	if err != nil {
		return fmt.Errorf("failed to fetch sandbox %q: %w", containerID, err)
	}
	defer sandbox.Release(ctx)

	status, err := sandbox.StatusContainer(containerID)
	if err != nil {
		return fmt.Errorf("failed to get container status for %q: %w", containerID, err)
	}

	bundlePath := ""
	if status.Annotations != nil {
		bundlePath = status.Annotations[vcAnnotations.BundlePathKey]
	}
	if bundlePath == "" && status.Spec != nil {
		bundlePath = getBundleFromSpec(status.Spec)
	}

	state := ociState{
		OCIVersion:  specs.Version,
		ID:          status.ID,
		Status:      kataStateToOCI(status.State.State),
		PID:         status.PID,
		Bundle:      bundlePath,
		Annotations: status.Annotations,
	}

	out, err := json.Marshal(state)
	if err != nil {
		return fmt.Errorf("failed to marshal state: %w", err)
	}

	fmt.Println(string(out))
	return nil
}

func getBundleFromSpec(spec *specs.Spec) string {
	if spec == nil || spec.Annotations == nil {
		return ""
	}
	return spec.Annotations[vcAnnotations.BundlePathKey]
}
