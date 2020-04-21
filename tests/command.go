// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package tests

import (
	"bytes"
	"fmt"
	"os/exec"
	"syscall"
	"time"
)

// Command contains the information of the command to run
type Command struct {
	// cmd exec.Cmd
	cmd *exec.Cmd

	// Timeout is the time limit of seconds of the command
	Timeout time.Duration
}

// NewCommand returns a new instance of Command
func NewCommand(path string, args ...string) *Command {
	c := new(Command)
	c.cmd = exec.Command(path, args...)
	c.Timeout = time.Duration(Timeout)

	return c
}

// Run runs a command returning its stdout, stderr and exit code
func (c *Command) Run() (string, string, int) {
	return c.RunWithPipe(nil)
}

// RunWithPipe runs a command with stdin as an input and returning its stdout, stderr and exit code
func (c *Command) RunWithPipe(stdin *bytes.Buffer) (string, string, int) {
	LogIfFail("Running command '%s %s'\n", c.cmd.Path, c.cmd.Args)

	keepAliveTime := 1 * time.Minute

	var stdout, stderr bytes.Buffer
	c.cmd.Stdout = &stdout
	c.cmd.Stderr = &stderr

	if stdin != nil {
		c.cmd.Stdin = stdin
	}

	if err := c.cmd.Start(); err != nil {
		LogIfFail("could no start command: %v\n", err)
	}

	done := make(chan error)
	go func() { done <- c.cmd.Wait() }()

	var timeout <-chan time.Time
	if c.Timeout > 0 {
		timeout = time.After(c.Timeout * time.Second)
	}

	keepAliveCh := time.NewTimer(keepAliveTime)

	for {
		select {
		case <-timeout:
			keepAliveCh.Stop()
			LogIfFail("Killing process timeout reached '%d' seconds\n", c.Timeout)
			_ = c.cmd.Process.Kill()
			return "", "", -1

		case <-keepAliveCh.C:
			// Avoid CI (i.e jenkins) kills the process for inactivity by printing a dot
			fmt.Println(".")
			keepAliveCh.Reset(keepAliveTime)

		case err := <-done:
			keepAliveCh.Stop()
			if err != nil {
				LogIfFail("command failed error '%s'\n", err)
			}

			exitCode := c.cmd.ProcessState.Sys().(syscall.WaitStatus).ExitStatus()

			LogIfFail("%+v\nTimeout: %d seconds\nExit Code: %d\nStdout: %s\nStderr: %s\n",
				c.cmd.Args, c.Timeout, exitCode, stdout.String(), stderr.String())

			return stdout.String(), stderr.String(), exitCode
		}
	}
}
