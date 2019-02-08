// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persist

import (
	"fmt"

	"github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/persist/fs"
)

type initFunc (func() (persistapi.PersistDriver, error))

var (
	supportedDrivers = map[string]initFunc{

		"fs": fs.Init,
	}
)

// GetDriver returns new PersistDriver according to driver name
func GetDriver(name string) (persistapi.PersistDriver, error) {
	if f, ok := supportedDrivers[name]; ok {
		return f()
	}

	return nil, fmt.Errorf("failed to get storage driver %q", name)
}
