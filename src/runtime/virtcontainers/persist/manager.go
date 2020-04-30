// Copyright (c) 2019 Huawei Corporation
// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persist

import (
	"fmt"

	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
)

type initFunc (func() (persistapi.PersistDriver, error))

const (
	RootFSName     = "fs"
	RootlessFSName = "rootlessfs"
)

var (
	// NewStoreFeature is an experimental feature
	NewStoreFeature = exp.Feature{
		Name:        "newstore",
		Description: "This is a new storage driver which reorganized disk data structures, it has to be an experimental feature since it breaks backward compatibility.",
		ExpRelease:  "2.0",
	}
	expErr           error
	supportedDrivers = map[string]initFunc{

		RootFSName:     fs.Init,
		RootlessFSName: fs.RootlessInit,
	}
	mockTesting = false
)

func init() {
	expErr = exp.Register(NewStoreFeature)
}

func EnableMockTesting() {
	mockTesting = true
}

// GetDriver returns new PersistDriver according to driver name
func GetDriverByName(name string) (persistapi.PersistDriver, error) {
	if expErr != nil {
		return nil, expErr
	}

	if f, ok := supportedDrivers[name]; ok {
		return f()
	}

	return nil, fmt.Errorf("failed to get storage driver %q", name)
}

// GetDriver returns new PersistDriver according to current needs.
// For example, a rootless FS driver is returned if the process is running
// as unprivileged process.
func GetDriver() (persistapi.PersistDriver, error) {
	if expErr != nil {
		return nil, expErr
	}

	if mockTesting {
		return fs.MockFSInit()
	}

	if rootless.IsRootless() {
		if f, ok := supportedDrivers[RootlessFSName]; ok {
			return f()
		}
	}

	if f, ok := supportedDrivers[RootFSName]; ok {
		return f()
	}

	return nil, fmt.Errorf("Could not find a FS driver")
}
