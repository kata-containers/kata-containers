// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persist

import (
	"fmt"

	exp "github.com/kata-containers/runtime/virtcontainers/experimental"
	"github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/persist/fs"
)

type initFunc (func() (persistapi.PersistDriver, error))

var (
	// NewStoreFeature is an experimental feature
	NewStoreFeature = exp.Feature{
		Name:        "newstore",
		Description: "This is a new storage driver which reorganized disk data structures, it has to be an experimental feature since it breaks backward compatibility.",
		ExpRelease:  "2.0",
	}
	expErr           error
	supportedDrivers = map[string]initFunc{

		"fs": fs.Init,
	}
)

func init() {
	expErr = exp.Register(NewStoreFeature)
}

// GetDriver returns new PersistDriver according to driver name
func GetDriver(name string) (persistapi.PersistDriver, error) {
	if expErr != nil {
		return nil, expErr
	}

	if f, ok := supportedDrivers[name]; ok {
		return f()
	}

	return nil, fmt.Errorf("failed to get storage driver %q", name)
}
