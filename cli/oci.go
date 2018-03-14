// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"bufio"
	"errors"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/opencontainers/runc/libcontainer/utils"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// Contants related to cgroup memory directory
const (
	cgroupsTasksFile = "tasks"
	cgroupsProcsFile = "cgroup.procs"
	cgroupsDirMode   = os.FileMode(0750)
	cgroupsFileMode  = os.FileMode(0640)

	// Filesystem type corresponding to CGROUP_SUPER_MAGIC as listed
	// here: http://man7.org/linux/man-pages/man2/statfs.2.html
	cgroupFsType = 0x27e0eb
)

var errNeedLinuxResource = errors.New("Linux resource cannot be empty")

var cgroupsDirPath string

var procMountInfo = "/proc/self/mountinfo"

// getContainerInfo returns the container status and its pod ID.
// It internally expands the container ID from the prefix provided.
func getContainerInfo(containerID string) (vc.ContainerStatus, string, error) {
	// container ID MUST be provided.
	if containerID == "" {
		return vc.ContainerStatus{}, "", fmt.Errorf("Missing container ID")
	}

	podStatusList, err := vci.ListPod()
	if err != nil {
		return vc.ContainerStatus{}, "", err
	}

	for _, podStatus := range podStatusList {
		for _, containerStatus := range podStatus.ContainersStatus {
			if containerStatus.ID == containerID {
				return containerStatus, podStatus.ID, nil
			}
		}
	}

	// Not finding a container should not trigger an error as
	// getContainerInfo is used for checking the existence and
	// the absence of a container ID.
	return vc.ContainerStatus{}, "", nil
}

func getExistingContainerInfo(containerID string) (vc.ContainerStatus, string, error) {
	cStatus, podID, err := getContainerInfo(containerID)
	if err != nil {
		return vc.ContainerStatus{}, "", err
	}

	// container ID MUST exist.
	if cStatus.ID == "" {
		return vc.ContainerStatus{}, "", fmt.Errorf("Container ID (%v) does not exist", containerID)
	}

	return cStatus, podID, nil
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
func processCgroupsPath(ociSpec oci.CompatOCISpec, isPod bool) ([]string, error) {
	var cgroupsPathList []string

	if ociSpec.Linux.CgroupsPath == "" {
		return []string{}, nil
	}

	if ociSpec.Linux.Resources == nil {
		return []string{}, nil
	}

	if ociSpec.Linux.Resources.Memory != nil {
		memCgroupsPath, err := processCgroupsPathForResource(ociSpec, "memory", isPod)
		if err != nil {
			return []string{}, err
		}

		if memCgroupsPath != "" {
			cgroupsPathList = append(cgroupsPathList, memCgroupsPath)
		}
	}

	if ociSpec.Linux.Resources.CPU != nil {
		cpuCgroupsPath, err := processCgroupsPathForResource(ociSpec, "cpu", isPod)
		if err != nil {
			return []string{}, err
		}

		if cpuCgroupsPath != "" {
			cgroupsPathList = append(cgroupsPathList, cpuCgroupsPath)
		}
	}

	if ociSpec.Linux.Resources.Pids != nil {
		pidsCgroupsPath, err := processCgroupsPathForResource(ociSpec, "pids", isPod)
		if err != nil {
			return []string{}, err
		}

		if pidsCgroupsPath != "" {
			cgroupsPathList = append(cgroupsPathList, pidsCgroupsPath)
		}
	}

	if ociSpec.Linux.Resources.BlockIO != nil {
		blkIOCgroupsPath, err := processCgroupsPathForResource(ociSpec, "blkio", isPod)
		if err != nil {
			return []string{}, err
		}

		if blkIOCgroupsPath != "" {
			cgroupsPathList = append(cgroupsPathList, blkIOCgroupsPath)
		}
	}

	return cgroupsPathList, nil
}

func processCgroupsPathForResource(ociSpec oci.CompatOCISpec, resource string, isPod bool) (string, error) {
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
