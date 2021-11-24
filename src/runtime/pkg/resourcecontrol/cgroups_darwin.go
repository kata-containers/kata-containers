// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package resourcecontrol

import (
	"errors"

	"github.com/opencontainers/runtime-spec/specs-go"
)

type DarwinResourceController struct{}

func RenameCgroupPath(path string) (string, error) {
	return "", errors.New("RenameCgroupPath not supported on Darwin")
}

func NewResourceController(path string, resources *specs.LinuxResources) (ResourceController, error) {
	return &DarwinResourceController{}, nil
}

func NewSandboxResourceController(path string, resources *specs.LinuxResources, sandboxCgroupOnly bool) (ResourceController, error) {
	return &DarwinResourceController{}, nil
}

func LoadResourceController(path string) (ResourceController, error) {
	return &DarwinResourceController{}, nil
}

func (c *DarwinResourceController) Delete() error {
	return nil
}

func (c *DarwinResourceController) Stat() (interface{}, error) {
	return nil, nil
}

func (c *DarwinResourceController) AddProcess(pid int, subsystems ...string) error {
	return nil
}

func (c *DarwinResourceController) AddThread(pid int, subsystems ...string) error {
	return nil
}

func (c *DarwinResourceController) AddTask(pid int, subsystems ...string) error {
	return nil
}

func (c *DarwinResourceController) Update(resources *specs.LinuxResources) error {
	return nil
}

func (c *DarwinResourceController) MoveTo(path string) error {
	return nil
}

func (c *DarwinResourceController) ID() string {
	return ""
}

func (c *DarwinResourceController) Parent() string {
	return ""
}

func (c *DarwinResourceController) Type() ResourceControllerType {
	return DarwinResourceControllerType
}

func (c *DarwinResourceController) AddDevice(deviceHostPath string) error {
	return nil
}

func (c *DarwinResourceController) RemoveDevice(deviceHostPath string) error {
	return nil
}

func (c *DarwinResourceController) UpdateCpuSet(cpuset, memset string) error {
	return nil
}

func (c *DarwinResourceController) Path() string {
	return ""
}
