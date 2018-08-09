// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/opencontainers/runc/libcontainer/utils"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

// Contants related to cgroup memory directory
const (
	cgroupsTasksFile   = "tasks"
	cgroupsProcsFile   = "cgroup.procs"
	cgroupsDirMode     = os.FileMode(0750)
	cgroupsFileMode    = os.FileMode(0640)
	ctrsMappingDirMode = os.FileMode(0750)

	// Filesystem type corresponding to CGROUP_SUPER_MAGIC as listed
	// here: http://man7.org/linux/man-pages/man2/statfs.2.html
	cgroupFsType = 0x27e0eb
)

var errNeedLinuxResource = errors.New("Linux resource cannot be empty")

var cgroupsDirPath string

var procMountInfo = "/proc/self/mountinfo"

var ctrsMapTreePath = "/var/run/kata-containers/containers-mapping"

// getContainerInfo returns the container status and its sandbox ID.
func getContainerInfo(containerID string) (vc.ContainerStatus, string, error) {
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

	ctrStatus, err := vci.StatusContainer(sandboxID, containerID)
	if err != nil {
		return vc.ContainerStatus{}, "", err
	}

	return ctrStatus, sandboxID, nil
}

func getExistingContainerInfo(containerID string) (vc.ContainerStatus, string, error) {
	cStatus, sandboxID, err := getContainerInfo(containerID)
	if err != nil {
		return vc.ContainerStatus{}, "", err
	}

	// container ID MUST exist.
	if cStatus.ID == "" {
		return vc.ContainerStatus{}, "", fmt.Errorf("Container ID (%v) does not exist", containerID)
	}

	return cStatus, sandboxID, nil
}

func validCreateParams(containerID, bundlePath string) (string, error) {
	// container ID MUST be provided.
	if containerID == "" {
		return "", fmt.Errorf("Missing container ID")
	}

	// container ID MUST be unique.
	cStatus, _, err := getContainerInfo(containerID)
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

	resolved, err := resolvePath(bundlePath)
	if err != nil {
		return "", err
	}

	return resolved, nil
}

// processCgroupsPath process the cgroups path as expected from the
// OCI runtime specification. It returns a list of complete paths
// that should be created and used for every specified resource.
func processCgroupsPath(ctx context.Context, ociSpec oci.CompatOCISpec, isSandbox bool) ([]string, error) {
	span, _ := opentracing.StartSpanFromContext(ctx, "processCgroupsPath")
	defer span.Finish()

	var cgroupsPathList []string

	if ociSpec.Linux.CgroupsPath == "" {
		return []string{}, nil
	}

	if ociSpec.Linux.Resources == nil {
		return []string{}, nil
	}

	if ociSpec.Linux.Resources.Memory != nil {
		memCgroupsPath, err := processCgroupsPathForResource(ociSpec, "memory", isSandbox)
		if err != nil {
			return []string{}, err
		}

		if memCgroupsPath != "" {
			cgroupsPathList = append(cgroupsPathList, memCgroupsPath)
		}
	}

	if ociSpec.Linux.Resources.CPU != nil {
		cpuCgroupsPath, err := processCgroupsPathForResource(ociSpec, "cpu", isSandbox)
		if err != nil {
			return []string{}, err
		}

		if cpuCgroupsPath != "" {
			cgroupsPathList = append(cgroupsPathList, cpuCgroupsPath)
		}
	}

	if ociSpec.Linux.Resources.Pids != nil {
		pidsCgroupsPath, err := processCgroupsPathForResource(ociSpec, "pids", isSandbox)
		if err != nil {
			return []string{}, err
		}

		if pidsCgroupsPath != "" {
			cgroupsPathList = append(cgroupsPathList, pidsCgroupsPath)
		}
	}

	if ociSpec.Linux.Resources.BlockIO != nil {
		blkIOCgroupsPath, err := processCgroupsPathForResource(ociSpec, "blkio", isSandbox)
		if err != nil {
			return []string{}, err
		}

		if blkIOCgroupsPath != "" {
			cgroupsPathList = append(cgroupsPathList, blkIOCgroupsPath)
		}
	}

	return cgroupsPathList, nil
}

func processCgroupsPathForResource(ociSpec oci.CompatOCISpec, resource string, isSandbox bool) (string, error) {
	if resource == "" {
		return "", errNeedLinuxResource
	}

	var err error
	cgroupsDirPath, err = getCgroupsDirPath(procMountInfo)
	if err != nil {
		return "", fmt.Errorf("get CgroupsDirPath error: %s", err)
	}

	// Relative cgroups path provided.
	if filepath.IsAbs(ociSpec.Linux.CgroupsPath) == false {
		return filepath.Join(cgroupsDirPath, resource, ociSpec.Linux.CgroupsPath), nil
	}

	// Absolute cgroups path provided.
	var cgroupMount specs.Mount
	cgroupMountFound := false
	for _, mount := range ociSpec.Mounts {
		if mount.Type == "cgroup" {
			cgroupMount = mount
			cgroupMountFound = true
			break
		}
	}

	if !cgroupMountFound {
		// According to the OCI spec, an absolute path should be
		// interpreted as relative to the system cgroup mount point
		// when there is no cgroup mount point.
		return filepath.Join(cgroupsDirPath, resource, ociSpec.Linux.CgroupsPath), nil
	}

	if cgroupMount.Destination == "" {
		return "", fmt.Errorf("cgroupsPath is absolute, cgroup mount destination cannot be empty")
	}

	cgroupPath := filepath.Join(cgroupMount.Destination, resource)

	// It is not an error to have this cgroup not mounted. It is usually
	// due to an old kernel version with missing support for specific
	// cgroups.
	fields := logrus.Fields{
		"path": cgroupPath,
		"type": "cgroup",
	}

	if !isCgroupMounted(cgroupPath) {
		kataLog.WithFields(fields).Info("path not mounted")
		return "", nil
	}

	kataLog.WithFields(fields).Info("path mounted")

	return filepath.Join(cgroupPath, ociSpec.Linux.CgroupsPath), nil
}

func isCgroupMounted(cgroupPath string) bool {
	var statFs syscall.Statfs_t

	if err := syscall.Statfs(cgroupPath, &statFs); err != nil {
		return false
	}

	if statFs.Type != int64(cgroupFsType) {
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
	span, _ := opentracing.StartSpanFromContext(ctx, "addContainerIDMapping")
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
	span, _ := opentracing.StartSpanFromContext(ctx, "delContainerIDMapping")
	defer span.Finish()

	if containerID == "" {
		return fmt.Errorf("Missing container ID")
	}

	path := filepath.Join(ctrsMapTreePath, containerID)

	return os.RemoveAll(path)
}
