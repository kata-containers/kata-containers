// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package cgroups

import (
	"fmt"
	"path/filepath"

	v1 "github.com/containerd/cgroups/stats/v1"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

// prepend a kata specific string to oci cgroup path to
// form a different cgroup path, thus cAdvisor couldn't
// find kata containers cgroup path on host to prevent it
// from grabbing the stats data.
const CgroupKataPrefix = "kata"

// DefaultCgroupPath runtime-determined location in the cgroups hierarchy.
const DefaultCgroupPath = "/vc"

var (
	cgroupsLogger = logrus.WithField("source", "virtcontainers/pkg/cgroups")
)

// SetLogger sets up a logger for this pkg
func SetLogger(logger *logrus.Entry) {
	fields := cgroupsLogger.Data

	cgroupsLogger = logger.WithFields(fields)
}

func RenameCgroupPath(path string) (string, error) {
	if path == "" {
		path = DefaultCgroupPath
	}

	cgroupPathDir := filepath.Dir(path)
	cgroupPathName := fmt.Sprintf("%s_%s", CgroupKataPrefix, filepath.Base(path))
	return filepath.Join(cgroupPathDir, cgroupPathName), nil

}

type Cgroup interface {
	Delete() error
	Stat() (*v1.Metrics, error)
	AddProcess(int, ...string) error
	AddTask(int, ...string) error
	Update(*specs.LinuxResources) error
	MoveTo(string) error
	MoveToParent() error
	AddDevice(string) error
	RemoveDevice(string) error
	UpdateCpuSet(string, string) error
	Path() string
}
