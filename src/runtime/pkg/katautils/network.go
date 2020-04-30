// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	goruntime "runtime"
	"strings"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/containernetworking/plugins/pkg/testutils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/rootless"
	"golang.org/x/sys/unix"
)

const procMountInfoFile = "/proc/self/mountinfo"

// EnterNetNS is free from any call to a go routine, and it calls
// into runtime.LockOSThread(), meaning it won't be executed in a
// different thread than the one expected by the caller.
func EnterNetNS(netNSPath string, cb func() error) error {
	if netNSPath == "" {
		return cb()
	}

	goruntime.LockOSThread()
	defer goruntime.UnlockOSThread()

	currentNS, err := ns.GetCurrentNS()
	if err != nil {
		return err
	}
	defer currentNS.Close()

	targetNS, err := ns.GetNS(netNSPath)
	if err != nil {
		return err
	}

	if err := targetNS.Set(); err != nil {
		return err
	}
	defer currentNS.Set()

	return cb()
}

// SetupNetworkNamespace create a network namespace
func SetupNetworkNamespace(config *vc.NetworkConfig) error {
	if config.DisableNewNetNs {
		kataUtilsLogger.Info("DisableNewNetNs is on, shim and hypervisor are running in the host netns")
		return nil
	}

	var err error
	var n ns.NetNS

	if config.NetNSPath == "" {
		if rootless.IsRootless() {
			n, err = rootless.NewNS()
			if err != nil {
				return err
			}
		} else {
			n, err = testutils.NewNS()
			if err != nil {
				return err
			}
		}

		config.NetNSPath = n.Path()
		config.NetNsCreated = true
		kataUtilsLogger.WithField("netns", n.Path()).Info("create netns")

		return nil
	}

	isHostNs, err := hostNetworkingRequested(config.NetNSPath)
	if err != nil {
		return err
	}
	if isHostNs {
		return fmt.Errorf("Host networking requested, not supported by runtime")
	}

	return nil
}

// getNetNsFromBindMount returns the network namespace for the bind-mounted path
func getNetNsFromBindMount(nsPath string, procMountFile string) (string, error) {
	netNsMountType := "nsfs"

	// Resolve all symlinks in the path as the mountinfo file contains
	// resolved paths.
	nsPath, err := filepath.EvalSymlinks(nsPath)
	if err != nil {
		return "", err
	}

	f, err := os.Open(procMountFile)
	if err != nil {
		return "", err
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		text := scanner.Text()

		// Scan the mountinfo file to search for the network namespace path
		// This file contains mounts in the eg format:
		// "711 26 0:3 net:[4026532009] /run/docker/netns/default rw shared:535 - nsfs nsfs rw"
		//
		// Reference: https://www.kernel.org/doc/Documentation/filesystems/proc.txt

		// We are interested in the first 9 fields of this file,
		// to check for the correct mount type.
		fields := strings.Split(text, " ")
		if len(fields) < 9 {
			continue
		}

		// We check here if the mount type is a network namespace mount type, namely "nsfs"
		mountTypeFieldIdx := 8
		if fields[mountTypeFieldIdx] != netNsMountType {
			continue
		}

		// This is the mount point/destination for the mount
		mntDestIdx := 4
		if fields[mntDestIdx] != nsPath {
			continue
		}

		// This is the root/source of the mount
		return fields[3], nil
	}

	return "", nil
}

// hostNetworkingRequested checks if the network namespace requested is the
// same as the current process.
func hostNetworkingRequested(configNetNs string) (bool, error) {
	var evalNS, nsPath, currentNsPath string
	var err error

	// Net namespace provided as "/proc/pid/ns/net" or "/proc/<pid>/task/<tid>/ns/net"
	if strings.HasPrefix(configNetNs, "/proc") && strings.HasSuffix(configNetNs, "/ns/net") {
		if _, err := os.Stat(configNetNs); err != nil {
			return false, err
		}

		// Here we are trying to resolve the path but it fails because
		// namespaces links don't really exist. For this reason, the
		// call to EvalSymlinks will fail when it will try to stat the
		// resolved path found. As we only care about the path, we can
		// retrieve it from the PathError structure.
		if _, err = filepath.EvalSymlinks(configNetNs); err != nil {
			nsPath = err.(*os.PathError).Path
		} else {
			return false, fmt.Errorf("Net namespace path %s is not a symlink", configNetNs)
		}

		_, evalNS = filepath.Split(nsPath)

	} else {
		// Bind-mounted path provided
		evalNS, _ = getNetNsFromBindMount(configNetNs, procMountInfoFile)
	}

	currentNS := fmt.Sprintf("/proc/%d/task/%d/ns/net", os.Getpid(), unix.Gettid())
	if _, err = filepath.EvalSymlinks(currentNS); err != nil {
		currentNsPath = err.(*os.PathError).Path
	} else {
		return false, fmt.Errorf("Unexpected: Current network namespace path is not a symlink")
	}

	_, evalCurrentNS := filepath.Split(currentNsPath)

	if evalNS == evalCurrentNS {
		return true, nil
	}

	return false, nil
}

// cleanupNetNS cleanup netns created by kata, trigger only create sandbox fails
func cleanupNetNS(netNSPath string) error {
	n, err := ns.GetNS(netNSPath)
	if err != nil {
		return fmt.Errorf("failed to get netns %s: %v", netNSPath, err)
	}

	err = n.Close()
	if err != nil {
		return fmt.Errorf("failed to close netns %s: %v", netNSPath, err)
	}

	if err = unix.Unmount(netNSPath, unix.MNT_DETACH); err != nil {
		return fmt.Errorf("failed to unmount namespace %s: %v", netNSPath, err)
	}
	if err := os.RemoveAll(netNSPath); err != nil {
		return fmt.Errorf("failed to clean up namespace %s: %v", netNSPath, err)
	}

	return nil
}
