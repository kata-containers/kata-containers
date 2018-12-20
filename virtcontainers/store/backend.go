// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"fmt"
)

type backendType string

const (
	filesystemBackend backendType = "filesystem"
)

const (
	filesystemScheme string = "file"
)

func schemeToBackendType(scheme string) (backendType, error) {
	switch scheme {
	case filesystemScheme:
		return filesystemBackend, nil
	}

	return "", fmt.Errorf("Unsupported scheme %s", scheme)
}

func newBackend(scheme string) (backend, error) {
	t, err := schemeToBackendType(scheme)
	if err != nil {
		return nil, err
	}

	switch t {
	case filesystemBackend:
		return &filesystem{}, nil
	}

	return nil, fmt.Errorf("Unsupported scheme %s", scheme)
}

type backend interface {
	new(ctx context.Context, path string, host string) error
	load(item Item, data interface{}) error
	store(item Item, data interface{}) error
}
