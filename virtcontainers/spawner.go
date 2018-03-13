//
// Copyright (c) 2016 Intel Corporation
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
//

package virtcontainers

import (
	"fmt"
)

// SpawnerType describes the type of guest agent a Pod should run.
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
