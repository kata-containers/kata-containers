// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
)

// SpawnerType describes the type of guest agent a Sandbox should run.
type SpawnerType string

const (
	// NsEnter is the nsenter spawner type
	NsEnter SpawnerType = "nsenter"
)

// Set sets an agent type based on the input string.
func (spawnerType *SpawnerType) Set(value string) error {
	switch value {
	case "nsenter":
		*spawnerType = NsEnter
		return nil
	default:
		return fmt.Errorf("Unknown spawner type %s", value)
	}
}

// String converts an agent type to a string.
func (spawnerType *SpawnerType) String() string {
	switch *spawnerType {
	case NsEnter:
		return string(NsEnter)
	default:
		return ""
	}
}

// newSpawner returns an agent from and agent type.
func newSpawner(spawnerType SpawnerType) spawner {
	switch spawnerType {
	case NsEnter:
		return &nsenter{}
	default:
		return nil
	}
}

// spawner is the virtcontainers spawner interface.
type spawner interface {
	formatArgs(args []string) ([]string, error)
}
