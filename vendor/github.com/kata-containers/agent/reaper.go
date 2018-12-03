//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"sync"

	"github.com/opencontainers/runc/libcontainer"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

type reaper interface {
	init()
	getExitCodeCh(pid int) (chan<- int, error)
	setExitCodeCh(pid int, exitCodeCh chan<- int)
	deleteExitCodeCh(pid int)
	getEpoller(pid int) (*epoller, error)
	setEpoller(pid int, epoller *epoller)
	deleteEpoller(pid int)
	reap() error
	start(c *exec.Cmd) (<-chan int, error)
	wait(exitCodeCh <-chan int, proc waitProcess) (int, error)
	lock()
	unlock()
	run(c *exec.Cmd) error
	combinedOutput(c *exec.Cmd) ([]byte, error)
}

type agentReaper struct {
	sync.RWMutex

	chansLock     sync.RWMutex
	exitCodeChans map[int]chan<- int
	epoller       map[int]*epoller
}

func exitStatus(status unix.WaitStatus) int {
	if status.Signaled() {
		return exitSignalOffset + int(status.Signal())
	}

	return status.ExitStatus()
}

func (r *agentReaper) init() {
	r.exitCodeChans = make(map[int]chan<- int)
	r.epoller = make(map[int]*epoller)
}

func (r *agentReaper) lock() {
	r.RLock()
}

func (r *agentReaper) unlock() {
	r.RUnlock()
}

func (r *agentReaper) getEpoller(pid int) (*epoller, error) {
	r.chansLock.RLock()
	defer r.chansLock.RUnlock()

	epoller, exist := r.epoller[pid]
	if !exist {
		return nil, fmt.Errorf("epoller doesn't exist for process %d", pid)
	}

	return epoller, nil
}

func (r *agentReaper) setEpoller(pid int, ep *epoller) {
	r.chansLock.Lock()
	defer r.chansLock.Unlock()

	r.epoller[pid] = ep
}

func (r *agentReaper) deleteEpoller(pid int) {
	r.chansLock.Lock()
	defer r.chansLock.Unlock()

	delete(r.epoller, pid)
}

func (r *agentReaper) getExitCodeCh(pid int) (chan<- int, error) {
	r.chansLock.RLock()
	defer r.chansLock.RUnlock()

	exitCodeCh, exist := r.exitCodeChans[pid]
	if !exist {
		return nil, grpcStatus.Errorf(codes.NotFound, "PID %d not found", pid)
	}

	return exitCodeCh, nil
}

func (r *agentReaper) setExitCodeCh(pid int, exitCodeCh chan<- int) {
	r.chansLock.Lock()
	defer r.chansLock.Unlock()

	r.exitCodeChans[pid] = exitCodeCh
}

func (r *agentReaper) deleteExitCodeCh(pid int) {
	r.chansLock.Lock()
	defer r.chansLock.Unlock()

	delete(r.exitCodeChans, pid)
}

func (r *agentReaper) reap() error {
	var (
		ws  unix.WaitStatus
		rus unix.Rusage
	)

	// When running new processes, agent expects to wait
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

		// Let's delete the entry here since the channel has been
		// stored by the caller, in order to wait for the exit code.
		r.deleteExitCodeCh(pid)

		// Here, we have to signal the routine listening on
		// this channel so that it can complete the cleanup
		// of the process and return the exit code to the
		// caller of WaitProcess().
		exitCodeCh <- status

		epoller, err := r.getEpoller(pid)
		if err == nil {
			//close the socket file to notify readStdio to close terminal specifically
			//in case this process's terminal has been inherited by its children.
			epoller.sockW.Close()
		}
		r.deleteEpoller(pid)
	}
}

// start starts the exec command and registers the process to the reaper.
// This function is a helper for exec.Cmd.Start() since this needs to be
// in sync with exec.Cmd.Wait().
func (r *agentReaper) start(c *exec.Cmd) (<-chan int, error) {
	// This lock is very important to avoid any race with reaper.reap().
	// We don't want the reaper to reap a process before we have added
	// it to the exit code channel list.
	r.RLock()
	defer r.RUnlock()

	if err := c.Start(); err != nil {
		return nil, err
	}

	exitCodeCh := make(chan int, 1)

	// This channel is buffered so that reaper.reap() will not
	// block until reaper.wait() listen onto this channel.
	r.setExitCodeCh(c.Process.Pid, exitCodeCh)

	return exitCodeCh, nil
}

// wait blocks until the expected process has been reaped. After the reaping
// from the subreaper, the exit code is sent through the provided channel.
// This function is a helper for exec.Cmd.Wait() and os.Process.Wait() since
// both cannot be used directly, because of the subreaper.
func (r *agentReaper) wait(exitCodeCh <-chan int, proc waitProcess) (int, error) {
	// Wait for the subreaper to receive the SIGCHLD signal. Once it gets
	// it, this channel will be notified by receiving the exit code of the
	// corresponding process.
	exitCode := <-exitCodeCh

	// Ignore errors since the process has already been reaped by the
	// subreaping loop. This call is only used to make sure libcontainer
	// properly cleans up its internal structures and pipes.
	proc.wait()

	return exitCode, nil
}

// run runs the exec command and waits for it, returns once the command
// has been reaped
func (r *agentReaper) run(c *exec.Cmd) error {
	exitCodeCh, err := r.start(c)
	if err != nil {
		return fmt.Errorf("reaper: Could not start process: %v", err)
	}
	_, err = r.wait(exitCodeCh, (*reaperOSProcess)(c.Process))
	return err
}

// combinedOutput combines command's stdout and stderr in one buffer,
// returns once the command has been reaped
func (r *agentReaper) combinedOutput(c *exec.Cmd) ([]byte, error) {
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

type waitProcess interface {
	wait()
}

type reaperOSProcess os.Process

func (p *reaperOSProcess) wait() {
	(*os.Process)(p).Wait()
}

type reaperLibcontainerProcess libcontainer.Process

func (p *reaperLibcontainerProcess) wait() {
	(*libcontainer.Process)(p).Wait()
}
