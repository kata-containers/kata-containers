// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

var (
	controllerLogger = logrus.WithField("source", "virtcontainers/pkg/resourcecontrol")
)

// SetLogger sets up a logger for this pkg
func SetLogger(logger *logrus.Entry) {
	fields := controllerLogger.Data

	controllerLogger = logger.WithFields(fields)
}

// ResourceControllerType describes a resource controller type.
type ResourceControllerType string

const (
	LinuxCgroups                 ResourceControllerType = "cgroups"
	DarwinResourceControllerType ResourceControllerType = "darwin"
)

// String converts a resource type to a string.
func (rType *ResourceControllerType) String() string {
	switch *rType {
	case LinuxCgroups:
		return string(LinuxCgroups)
	default:
		return "Unknown controller type"
	}
}

// ResourceController represents a system resources controller.
// On Linux this interface is implemented through the cgroups API.
type ResourceController interface {
	// Type returns the resource controller implementation type.
	Type() ResourceControllerType

	// The controller identifier, e.g. a Linux cgroups path.
	ID() string

	// Parent returns the parent controller, on hierarchically
	// defined resource (e.g. Linux cgroups).
	Parent() string

	// Delete the controller.
	Delete() error

	// Stat returns the statistics for the controller.
	Stat() (interface{}, error)

	// AddProcess adds a process to a set of controllers.
	AddProcess(int, ...string) error

	// AddThread adds a process thread to a set of controllers.
	AddThread(int, ...string) error

	// Update updates the set of resources controlled, based on
	// an OCI resources description.
	Update(*specs.LinuxResources) error

	// MoveTo moves a controller to another one.
	MoveTo(string) error

	// AddDevice adds a device resource to the controller.
	AddDevice(string) error

	// RemoveDevice removes a device resource to the controller.
	RemoveDevice(string) error

	// UpdateCpuSet updates the set of controlled CPUs and memory nodes.
	UpdateCpuSet(string, string) error
}
