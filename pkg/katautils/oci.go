// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

	"github.com/kata-containers/runtime/pkg/rootless"
)

const ctrsMappingDirMode = os.FileMode(0750)

var ctrsMapTreePath = "/var/run/kata-containers/containers-mapping"

// SetCtrsMapTreePath let the testcases change the ctrsMapTreePath to a test dir
func SetCtrsMapTreePath(path string) {
	ctrsMapTreePath = path
}

// doUpdatePath returns whether a ctrsMapTreePath needs to be updated with a rootless prefix
func doUpdatePath() bool {
	return rootless.IsRootless() && !strings.HasPrefix(ctrsMapTreePath, rootless.GetRootlessDir())
}

// FetchContainerIDMapping This function assumes it should find only one file inside the container
// ID directory. If there are several files, we could not determine which
// file name corresponds to the sandbox ID associated, and this would throw
// an error.
func FetchContainerIDMapping(containerID string) (string, error) {
	if containerID == "" {
		return "", fmt.Errorf("Missing container ID")
	}

	if doUpdatePath() {
		SetCtrsMapTreePath(filepath.Join(rootless.GetRootlessDir(), ctrsMapTreePath))
	}

	dirPath := filepath.Join(ctrsMapTreePath, containerID)

	files, err := ioutil.ReadDir(dirPath)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}

		return "", err
	}

	if len(files) != 1 {
		return "", fmt.Errorf("Too many files (%d) in %q", len(files), dirPath)
	}

	return files[0].Name(), nil
}

// AddContainerIDMapping add a container id mapping to sandbox id
func AddContainerIDMapping(ctx context.Context, containerID, sandboxID string) error {
	span, _ := Trace(ctx, "addContainerIDMapping")
	defer span.Finish()

	if containerID == "" {
		return fmt.Errorf("Missing container ID")
	}

	if sandboxID == "" {
		return fmt.Errorf("Missing sandbox ID")
	}

	if doUpdatePath() {
		SetCtrsMapTreePath(filepath.Join(rootless.GetRootlessDir(), ctrsMapTreePath))
	}
	parentPath := filepath.Join(ctrsMapTreePath, containerID)

	if err := os.RemoveAll(parentPath); err != nil {
		return err
	}

	path := filepath.Join(parentPath, sandboxID)

	if err := os.MkdirAll(path, ctrsMappingDirMode); err != nil {
		return err
	}

	return nil
}

// DelContainerIDMapping delete container id mapping from a sandbox
func DelContainerIDMapping(ctx context.Context, containerID string) error {
	span, _ := Trace(ctx, "delContainerIDMapping")
	defer span.Finish()

	if containerID == "" {
		return fmt.Errorf("Missing container ID")
	}

	if doUpdatePath() {
		SetCtrsMapTreePath(filepath.Join(rootless.GetRootlessDir(), ctrsMapTreePath))
	}
	path := filepath.Join(ctrsMapTreePath, containerID)

	return os.RemoveAll(path)
}
