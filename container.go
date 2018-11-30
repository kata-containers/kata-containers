// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package tests

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
)

// Container represents a kata container
type Container struct {
	// Bundle contains the container information
	// if nil then try to run the container without --bundle option
	Bundle *Bundle

	// Console pty slave path
	// if nil then try to run the container without --console option
	Console *string

	// PidFile where process id is written
	// if nil then try to run the container without --pid-file option
	PidFile *string

	// LogFile where debug information is written
	// if nil then try to run the container without --log option
	LogFile *string

	// Detach allows to run the process detached from the shell
	Detach bool

	// ID of the container
	// if nil then try to run the container without container ID
	ID *string
}

// Process describes a process to be executed on a running container.
type Process struct {
	ContainerID *string
	Console     *string
	Tty         *string
	Detach      bool
	Workload    []string
}

// NewContainer returns a new Container
func NewContainer(workload []string, detach bool) (*Container, error) {
	b, err := NewBundle(workload)
	if err != nil {
		return nil, err
	}

	console := ""

	pidFile := filepath.Join(b.Path, "pid")
	logFile := filepath.Join(b.Path, "log")
	id := RandID(20)

	return &Container{
		Bundle:  b,
		Console: &console,
		PidFile: &pidFile,
		LogFile: &logFile,
		Detach:  detach,
		ID:      &id,
	}, nil
}

// Run the container
// calls to run command returning its stdout, stderr and exit code
func (c *Container) Run() (string, string, int) {
	args := []string{}

	if c.LogFile != nil {
		args = append(args, fmt.Sprintf("--log=%s", *c.LogFile))
	}

	args = append(args, "run")

	if c.Bundle != nil {
		args = append(args, fmt.Sprintf("--bundle=%s", c.Bundle.Path))
	}

	if c.Console != nil {
		args = append(args, fmt.Sprintf("--console=%s", *c.Console))
	}

	if c.PidFile != nil {
		args = append(args, fmt.Sprintf("--pid-file=%s", *c.PidFile))
	}

	if c.Detach {
		args = append(args, "--detach")
	}

	if c.ID != nil {
		args = append(args, *c.ID)
	}

	cmd := NewCommand(Runtime, args...)

	return cmd.Run()
}

// Delete the container
// calls to delete command returning its stdout, stderr and exit code
func (c *Container) Delete(force bool) (string, string, int) {
	args := []string{"delete"}

	if force {
		args = append(args, "--force")
	}

	if c.ID != nil {
		args = append(args, *c.ID)
	}

	cmd := NewCommand(Runtime, args...)

	return cmd.Run()
}

// Kill the container
// calls to kill command returning its stdout, stderr and exit code
func (c *Container) Kill(all bool, signal interface{}) (string, string, int) {
	args := []string{"kill"}

	if all {
		args = append(args, "--all")
	}

	if c.ID != nil {
		args = append(args, *c.ID)
	}

	switch t := signal.(type) {
	case syscall.Signal:
		args = append(args, strconv.Itoa(int(t)))
	case string:
		args = append(args, t)
	}

	cmd := NewCommand(Runtime, args...)

	return cmd.Run()
}

// Exec the container
// calls into exec command returning its stdout, stderr and exit code
func (c *Container) Exec(process Process) (string, string, int) {
	args := []string{}

	if c.LogFile != nil {
		args = append(args, fmt.Sprintf("--log=%s", *c.LogFile))
	}

	args = append(args, "exec")

	if process.Console != nil {
		args = append(args, fmt.Sprintf("--console=%s", *process.Console))
	}

	if process.Tty != nil {
		args = append(args, fmt.Sprintf("--tty=%s", *process.Tty))
	}

	if process.Detach {
		args = append(args, "--detach")
	}

	if process.ContainerID != nil {
		args = append(args, *process.ContainerID)
	}

	args = append(args, process.Workload...)

	cmd := NewCommand(Runtime, args...)

	return cmd.Run()
}

// State returns the state of the container
// calls into state command returning its stdout, stderr and exit code
func (c *Container) State() (string, string, int) {
	args := []string{}

	args = append(args, "state")

	if c.ID != nil {
		args = append(args, *c.ID)
	}

	cmd := NewCommand(Runtime, args...)

	return cmd.Run()
}

// List the containers
// calls to list command returning its stdout, stderr and exit code
func (c *Container) List(format string, quiet bool, all bool) (string, string, int) {
	args := []string{"list"}

	if format != "" {
		args = append(args, fmt.Sprintf("--format=%s", format))
	}

	if quiet {
		args = append(args, "--quiet")
	}

	if all {
		args = append(args, "--all")
	}

	cmd := NewCommand(Runtime, args...)

	return cmd.Run()
}

// SetWorkload sets a workload for the container
func (c *Container) SetWorkload(workload []string) error {
	c.Bundle.Config.Process.Args = workload
	return c.Bundle.Save()
}

// RemoveOption removes a specific option
// container will run without the specific option
func (c *Container) RemoveOption(option string) error {
	switch option {
	case "--bundle", "-b":
		defer c.Bundle.Remove()
		c.Bundle = nil
	case "--console":
		c.Console = nil
	case "--pid-file":
		c.PidFile = nil
	default:
		return fmt.Errorf("undefined option '%s'", option)
	}

	return nil
}

// Teardown deletes the container if it is running,
// ensures the container is not running and removes any
// file created by the container
func (c *Container) Teardown() error {
	var cid string

	if c.ID != nil {
		cid = *c.ID
	}

	// if container exist then delete it
	if c.Exist() {
		_, stderr, exitCode := c.Delete(true)
		if exitCode != 0 {
			return fmt.Errorf("failed to delete container %s %s", cid, stderr)
		}

		// if container still exist then fail
		if c.Exist() {
			return fmt.Errorf("unable to delete container %s", cid)
		}
	}

	if c.Bundle != nil {
		return c.Bundle.Remove()
	}

	return nil
}

// Exist returns true if any of next cases is true:
// - list command shows the container
// - the process id specified in the pid file is running (cc-shim)
// - the VM is running (qemu)
// - the proxy is running
// - the shim is running
// else false is returned
func (c *Container) Exist() bool {
	return c.isListed() || c.isWorkloadRunning() ||
		HypervisorRunning(*c.ID) || ProxyRunning(*c.ID) ||
		ShimRunning(*c.ID)
}

func (c *Container) isListed() bool {
	if c.ID == nil {
		return false
	}

	stdout, _, ret := c.List("", true, false)
	if ret != 0 {
		return false
	}

	return strings.Contains(stdout, *c.ID)
}

func (c *Container) isWorkloadRunning() bool {
	if c.PidFile == nil {
		return false
	}

	content, err := ioutil.ReadFile(*c.PidFile)
	if err != nil {
		return false
	}

	if _, err := os.Stat(fmt.Sprintf("/proc/%s/stat", string(content))); os.IsNotExist(err) {
		return false
	}

	return true
}
