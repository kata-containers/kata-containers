//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sync"

	"golang.org/x/sys/unix"
)

var persistentNsDir = "/var/run/sandbox-ns"

// nsType defines a namespace type.
type nsType string

type namespace struct {
	path       string
	init       *os.Process
	exitCodeCh <-chan int
}

// List of namespace types.
const (
	nsTypeIPC nsType = "ipc"
	nsTypeNet nsType = "net"
	nsTypeUTS nsType = "uts"
)

var cloneFlagsTable = map[nsType]int{
	nsTypeIPC: unix.CLONE_NEWIPC,
	nsTypeNet: unix.CLONE_NEWNET,
	nsTypeUTS: unix.CLONE_NEWUTS,
}

func getCurrentThreadNSPath(nType nsType) string {
	return fmt.Sprintf("/proc/%d/task/%d/ns/%s", os.Getpid(), unix.Gettid(), nType)
}

// setupPersistentNs creates persistent namespace without switchin to it.
// Note, pid namespaces cannot be persisted.
func setupPersistentNs(namespaceType nsType) (*namespace, error) {

	err := os.MkdirAll(persistentNsDir, 0755)
	if err != nil {
		return nil, err
	}

	// Create an empty file at the mount point.
	nsPath := filepath.Join(persistentNsDir, string(namespaceType))

	mountFd, err := os.Create(nsPath)
	if err != nil {
		return nil, err
	}
	mountFd.Close()

	var wg sync.WaitGroup
	wg.Add(1)

	go (func() {
		defer wg.Done()
		runtime.LockOSThread()

		var origNsFd *os.File
		origNsPath := getCurrentThreadNSPath(namespaceType)
		origNsFd, err = os.Open(origNsPath)
		if err != nil {
			return
		}
		defer origNsFd.Close()

		// Create a new netns on the current thread.
		err = unix.Unshare(cloneFlagsTable[namespaceType])
		if err != nil {
			return
		}

		// Bind mount the new namespace from the current thread onto the mount point to persist it.
		err = unix.Mount(getCurrentThreadNSPath(namespaceType), nsPath, "none", unix.MS_BIND, "")
		if err != nil {
			return
		}

		// Switch back to original namespace.
		if err = unix.Setns(int(origNsFd.Fd()), cloneFlagsTable[namespaceType]); err != nil {
			return
		}

	})()
	wg.Wait()

	if err != nil {
		unix.Unmount(nsPath, unix.MNT_DETACH)
		return nil, fmt.Errorf("failed to create namespace: %v", err)
	}

	return &namespace{path: nsPath}, nil
}
