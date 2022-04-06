// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package fs

import (
	"fmt"
	"path/filepath"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
)

var mockRootPath = ""

type MockFS struct {
	// inherit from FS. Overwrite if needed.
	*FS
}

func EnableMockTesting(rootPath string) {
	mockRootPath = rootPath
}

func MockStorageRootPath() string {
	if mockRootPath == "" {
		panic("Using uninitialized mock storage root path")
	}
	return mockRootPath
}

func MockRunStoragePath() string {
	return filepath.Join(MockStorageRootPath(), sandboxPathSuffix)
}

func MockRunVMStoragePath() string {
	return filepath.Join(MockStorageRootPath(), vmPathSuffix)
}

func MockFSInit(rootPath string) (persistapi.PersistDriver, error) {
	driver, err := Init()
	if err != nil {
		return nil, fmt.Errorf("Could not create Mock FS driver: %v", err)
	}

	fsDriver, ok := driver.(*FS)
	if !ok {
		return nil, fmt.Errorf("Could not create Mock FS driver")
	}

	fsDriver.storageRootPath = rootPath
	fsDriver.driverName = "mockfs"

	return &MockFS{fsDriver}, nil
}

func MockAutoInit() (persistapi.PersistDriver, error) {
	if mockRootPath != "" {
		return MockFSInit(MockStorageRootPath())
	}
	return nil, nil
}
