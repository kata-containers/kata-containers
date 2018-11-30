//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"errors"
	"fmt"
	"os/exec"
)

type mockreaper struct {
}

func (r *mockreaper) init() {
}

func (r *mockreaper) getEpoller(pid int) (*epoller, error) {
	return nil, nil
}

func (r *mockreaper) setEpoller(pid int, epoller *epoller) {
}

func (r *mockreaper) deleteEpoller(pid int) {
}

func (r *mockreaper) getExitCodeCh(pid int) (chan<- int, error) {
	return nil, nil
}

func (r *mockreaper) setExitCodeCh(pid int, exitCodeCh chan<- int) {
}

func (r *mockreaper) deleteExitCodeCh(pid int) {
}

func (r *mockreaper) reap() error {
	return nil
}

func (r *mockreaper) start(c *exec.Cmd) (<-chan int, error) {
	return nil, nil
}

func (r *mockreaper) wait(exitCodeCh <-chan int, proc waitProcess) (int, error) {
	return 0, nil
}

func (r *mockreaper) lock() {
}

func (r *mockreaper) unlock() {
}

func (r *mockreaper) run(c *exec.Cmd) error {
	if err := c.Run(); err != nil {
		return fmt.Errorf("reaper: Could not start process: %v", err)
	}
	return nil
}

func (r *mockreaper) combinedOutput(c *exec.Cmd) ([]byte, error) {
	if c.Stdout != nil {
		return nil, errors.New("reaper: Stdout already set")
	}
	if c.Stderr != nil {
		return nil, errors.New("reaper: Stderr already set")
	}

	var b bytes.Buffer
	c.Stdout = &b
	c.Stderr = &b
	err := r.run(c)
	return b.Bytes(), err
}
