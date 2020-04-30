// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package fs

import (
	"fmt"
	"os"
	"path/filepath"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
)

// default xdg runtime directory just in case XDG_RUNTIME_DIR is not set
var defaultXdgRuntimeDir = fmt.Sprintf("/run/user/%d", os.Getuid())

type RootlessFS struct {
	// inherit from FS. Overwrite if needed.
	*FS
}

func RootlessInit() (persistapi.PersistDriver, error) {
	driver, err := Init()
	if err != nil {
		return nil, fmt.Errorf("Could not create Rootless FS driver: %v", err)
	}

	fsDriver, ok := driver.(*FS)
	if !ok {
		return nil, fmt.Errorf("Could not create Rootless FS driver")
	}

	// XDG_RUNTIME_DIR defines the base directory relative to
	// which user-specific non-essential runtime files are stored.
	rootlessDir := os.Getenv("XDG_RUNTIME_DIR")
	if rootlessDir == "" {
		rootlessDir = defaultXdgRuntimeDir
		fsLog.WithField("default-runtime-dir", defaultXdgRuntimeDir).
			Warnf("XDG_RUNTIME_DIR variable is not set. Using default runtime directory")
	}

	fsDriver.storageRootPath = filepath.Join(rootlessDir, fsDriver.storageRootPath)
	fsDriver.driverName = "rootlessfs"

	return &RootlessFS{fsDriver}, nil
}
