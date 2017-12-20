//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"os/exec"
	"sync"

	"github.com/opencontainers/runc/libcontainer"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
)

type reaper struct {
	sync.RWMutex

	chansLock     sync.RWMutex
	exitCodeChans map[int]chan int
}

func exitStatus(status unix.WaitStatus) int {
	if status.Signaled() {
		return exitSignalOffset + int(status.Signal())
	}

	return status.ExitStatus()
}

func (r *reaper) getExitCodeCh(pid int) (chan int, error) {
	r.chansLock.RLock()
	defer r.chansLock.RUnlock()

	exitCodeCh, exist := r.exitCodeChans[pid]
	if !exist {
		return nil, fmt.Errorf("Process %d not found", pid)
	}

	return exitCodeCh, nil
}

func (r *reaper) setExitCodeCh(pid int, exitCodeCh chan int) {
	r.chansLock.Lock()
	defer r.chansLock.Unlock()

	r.exitCodeChans[pid] = exitCodeCh
}

func (r *reaper) deleteExitCodeCh(pid int) {
	r.chansLock.Lock()
	defer r.chansLock.Unlock()

	exitCodeCh, exist := r.exitCodeChans[pid]
	if !exist {
		return
	}

	close(exitCodeCh)

	delete(r.exitCodeChans, pid)
}

func (r *reaper) reap() error {
	var (
		ws  unix.WaitStatus
		rus unix.Rusage
	)

	// When running new processes, libcontainer expects to wait
	// for the first process actually spawning the container.
	// This lock allows any code starting a new process to take
	// the lock prior to the start of this new process, preventing
	// the subreaper from reaping unwanted processes.
	r.Lock()
	defer r.Unlock()

	for {
		pid, err := unix.Wait4(-1, &ws, unix.WNOHANG, &rus)
		if err != nil {
			if err == unix.ECHILD {
				return nil
			}

			return err
		}
		if pid < 1 {
			return nil
		}

		status := exitStatus(ws)

		agentLog.WithFields(logrus.Fields{
			"pid":    pid,
			"status": status,
		}).Debug("process exited")

		exitCodeCh, err := r.getExitCodeCh(pid)
		if err != nil {
			// No need to signal a process with no channel
			// associated. When a process has not been registered,
			// this means the spawner does not expect to get the
			// exit code from this process.
			continue
		}

		// Here, we have to signal the routine listening on
		// this channel so that it can complete the cleanup
		// of the process and return the exit code to the
		// caller of WaitProcess().
		exitCodeCh <- status
	}
}

// start starts the exec command and registers the process to the reaper.
// This function is a helper for exec.Cmd.Start() since this needs to be
// in sync with exec.Cmd.Wait().
func (r *reaper) start(c *exec.Cmd) error {
	// This lock is very important to avoid any race with reaper.reap().
	// We don't want the reaper to reap a process before we have added
	// it to the exit code channel list.
	r.RLock()
	defer r.RUnlock()

	if err := c.Start(); err != nil {
		return err
	}

	// This channel is buffered so that reaper.reap() will not
	// block until reaper.wait() listen onto this channel.
	r.setExitCodeCh(c.Process.Pid, make(chan int, 1))

	return nil
}

// wait blocks until the expected process has been reaped. After the reaping
// from the subreaper, the exit code is sent through the provided channel.
// This function is a helper for exec.Cmd.Wait() and os.Process.Wait() since
// both cannot be used directly, because of the subreaper.
func (r *reaper) wait(pid int, proc waitProcess) (int, error) {
	exitCodeCh, err := r.getExitCodeCh(pid)
	if err != nil {
		return -1, err
	}

	// Wait for the subreaper to receive the SIGCHLD signal. Once it gets
	// it, this channel will be notified by receiving the exit code of the
	// corresponding process.
	exitCode := <-exitCodeCh

	// Ignore errors since the process has already been reaped by the
	// subreaping loop. This call is only used to make sure libcontainer
	// properly cleans up its internal structures and pipes.
	proc.wait()

	r.deleteExitCodeCh(pid)

	return exitCode, nil
}

type waitProcess interface {
	wait()
}

type reaperOSProcess os.Process

func (p *reaperOSProcess) wait() {
	(*os.Process)(p).Wait()
}

type reaperExecCmd exec.Cmd

func (c *reaperExecCmd) wait() {
	(*exec.Cmd)(c).Wait()
}

type reaperLibcontainerProcess libcontainer.Process

func (p *reaperLibcontainerProcess) wait() {
	(*libcontainer.Process)(p).Wait()
}
