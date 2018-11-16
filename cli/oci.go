// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bufio"
	"context"
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"path/filepath"
	goruntime "runtime"
	"strings"
	"syscall"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/kata-containers/runtime/pkg/katautils"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/opencontainers/runc/libcontainer/utils"
)

// Contants related to cgroup memory directory
const (
	ctrsMappingDirMode = os.FileMode(0750)

	// Filesystem type corresponding to CGROUP_SUPER_MAGIC as listed
	// here: http://man7.org/linux/man-pages/man2/statfs.2.html
	cgroupFsType = 0x27e0eb
)

var cgroupsDirPath string

var procMountInfo = "/proc/self/mountinfo"

var ctrsMapTreePath = "/var/run/kata-containers/containers-mapping"

// getContainerInfo returns the container status and its sandbox ID.
func getContainerInfo(ctx context.Context, containerID string) (vc.ContainerStatus, string, error) {
	// container ID MUST be provided.
	if containerID == "" {
		return vc.ContainerStatus{}, "", fmt.Errorf("Missing container ID")
	}

	sandboxID, err := fetchContainerIDMapping(containerID)
	if err != nil {
		return vc.ContainerStatus{}, "", err
	}
	if sandboxID == "" {
		// Not finding a container should not trigger an error as
		// getContainerInfo is used for checking the existence and
		// the absence of a container ID.
		return vc.ContainerStatus{}, "", nil
	}

	ctrStatus, err := vci.StatusContainer(ctx, sandboxID, containerID)
	if err != nil {
		return vc.ContainerStatus{}, "", err
	}

	return ctrStatus, sandboxID, nil
}

func getExistingContainerInfo(ctx context.Context, containerID string) (vc.ContainerStatus, string, error) {
	cStatus, sandboxID, err := getContainerInfo(ctx, containerID)
	if err != nil {
		return vc.ContainerStatus{}, "", err
	}

	// container ID MUST exist.
	if cStatus.ID == "" {
		return vc.ContainerStatus{}, "", fmt.Errorf("Container ID (%v) does not exist", containerID)
	}

	return cStatus, sandboxID, nil
}

func validCreateParams(ctx context.Context, containerID, bundlePath string) (string, error) {
	// container ID MUST be provided.
	if containerID == "" {
		return "", fmt.Errorf("Missing container ID")
	}

	// container ID MUST be unique.
	cStatus, _, err := getContainerInfo(ctx, containerID)
	if err != nil {
		return "", err
	}

	if cStatus.ID != "" {
		return "", fmt.Errorf("ID already in use, unique ID should be provided")
	}

	// bundle path MUST be provided.
	if bundlePath == "" {
		return "", fmt.Errorf("Missing bundle path")
	}

	// bundle path MUST be valid.
	fileInfo, err := os.Stat(bundlePath)
	if err != nil {
		return "", fmt.Errorf("Invalid bundle path '%s': %s", bundlePath, err)
	}
	if fileInfo.IsDir() == false {
		return "", fmt.Errorf("Invalid bundle path '%s', it should be a directory", bundlePath)
	}

	resolved, err := katautils.ResolvePath(bundlePath)
	if err != nil {
		return "", err
	}

	return resolved, nil
}

func isCgroupMounted(cgroupPath string) bool {
	var statFs syscall.Statfs_t

	if err := syscall.Statfs(cgroupPath, &statFs); err != nil {
		return false
	}

	if statFs.Type != archConvertStatFs(cgroupFsType) {
		return false
	}

	return true
}

func setupConsole(consolePath, consoleSockPath string) (string, error) {
	if consolePath != "" {
		return consolePath, nil
	}

	if consoleSockPath == "" {
		return "", nil
	}

	console, err := newConsole()
	if err != nil {
		return "", err
	}
	defer console.master.Close()

	// Open the socket path provided by the caller
	conn, err := net.Dial("unix", consoleSockPath)
	if err != nil {
		return "", err
	}

	uConn, ok := conn.(*net.UnixConn)
	if !ok {
		return "", fmt.Errorf("casting to *net.UnixConn failed")
	}

	socket, err := uConn.File()
	if err != nil {
		return "", err
	}

	// Send the parent fd through the provided socket
	if err := utils.SendFd(socket, console.master.Name(), console.master.Fd()); err != nil {
		return "", err
	}

	return console.slavePath, nil
}

func noNeedForOutput(detach bool, tty bool) bool {
	if !detach {
		return false
	}

	if !tty {
		return false
	}

	return true
}

func getCgroupsDirPath(mountInfoFile string) (string, error) {
	if cgroupsDirPath != "" {
		return cgroupsDirPath, nil
	}

	f, err := os.Open(mountInfoFile)
	if err != nil {
		return "", err
	}
	defer f.Close()

	var cgroupRootPath string
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		text := scanner.Text()
		index := strings.Index(text, " - ")
		if index < 0 {
			continue
		}
		fields := strings.Split(text, " ")
		postSeparatorFields := strings.Fields(text[index+3:])
		numPostFields := len(postSeparatorFields)

		if len(fields) < 5 || postSeparatorFields[0] != "cgroup" || numPostFields < 3 {
			continue
		}

		cgroupRootPath = filepath.Dir(fields[4])
		break
	}

	if _, err = os.Stat(cgroupRootPath); err != nil {
		return "", err
	}

	return cgroupRootPath, nil
}

// This function assumes it should find only one file inside the container
// ID directory. If there are several files, we could not determine which
// file name corresponds to the sandbox ID associated, and this would throw
// an error.
func fetchContainerIDMapping(containerID string) (string, error) {
	if containerID == "" {
		return "", fmt.Errorf("Missing container ID")
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

func addContainerIDMapping(ctx context.Context, containerID, sandboxID string) error {
	span, _ := trace(ctx, "addContainerIDMapping")
	defer span.Finish()

	if containerID == "" {
		return fmt.Errorf("Missing container ID")
	}

	if sandboxID == "" {
		return fmt.Errorf("Missing sandbox ID")
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

func delContainerIDMapping(ctx context.Context, containerID string) error {
	span, _ := trace(ctx, "delContainerIDMapping")
	defer span.Finish()

	if containerID == "" {
		return fmt.Errorf("Missing container ID")
	}

	path := filepath.Join(ctrsMapTreePath, containerID)

	return os.RemoveAll(path)
}

// enterNetNS is free from any call to a go routine, and it calls
// into runtime.LockOSThread(), meaning it won't be executed in a
// different thread than the one expected by the caller.
func enterNetNS(netNSPath string, cb func() error) error {
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
