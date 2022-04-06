// Copyright (c) 2019 Huawei Corporation
// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persist

import (
	"fmt"

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
	expErr           error
	supportedDrivers = map[string]initFunc{

		RootFSName:     fs.Init,
		RootlessFSName: fs.RootlessInit,
	}
)

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

	mock, err := fs.MockAutoInit()
	if mock != nil || err != nil {
		return mock, err
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
