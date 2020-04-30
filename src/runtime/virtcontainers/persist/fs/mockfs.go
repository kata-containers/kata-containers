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

type MockFS struct {
	// inherit from FS. Overwrite if needed.
	*FS
}

func MockStorageRootPath() string {
	return filepath.Join(os.TempDir(), "vc", "mockfs")
}

func MockRunStoragePath() string {
	return filepath.Join(MockStorageRootPath(), sandboxPathSuffix)
}

func MockRunVMStoragePath() string {
	return filepath.Join(MockStorageRootPath(), vmPathSuffix)
}

func MockStorageDestroy() {
	os.RemoveAll(MockStorageRootPath())
}

func MockFSInit() (persistapi.PersistDriver, error) {
	driver, err := Init()
	if err != nil {
		return nil, fmt.Errorf("Could not create Mock FS driver: %v", err)
	}

	fsDriver, ok := driver.(*FS)
	if !ok {
		return nil, fmt.Errorf("Could not create Mock FS driver")
	}

	fsDriver.storageRootPath = MockStorageRootPath()
	fsDriver.driverName = "mockfs"

	return &MockFS{fsDriver}, nil
}
