//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"net"
	"os"
	"runtime"
	"sync"
	"time"

	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/opencontainers/runc/libcontainer"
	"github.com/opencontainers/runc/libcontainer/configs"
	"github.com/sirupsen/logrus"
	"google.golang.org/grpc"
)

type process struct {
	id          int
	process     libcontainer.Process
	stdin       *os.File
	stdout      *os.File
	stderr      *os.File
	consoleSock *os.File
	termMaster  *os.File
}

type container struct {
	sync.RWMutex

	id          string
	initProcess *process
	container   libcontainer.Container
	config      configs.Config
	processes   map[int]*process
	mounts      []string
}

type sandbox struct {
	sync.RWMutex

	id           string
	running      bool
	containers   map[string]*container
	channel      channel
	network      network
	wg           sync.WaitGroup
	grpcListener net.Listener
	sharedPidNs  bool
	mounts       []string
}

var agentLog = logrus.WithFields(logrus.Fields{
	"name": agentName,
	"pid":  os.Getpid(),
})

// Version is the agent version. This variable is populated at build time.
var Version = "unknown"

// This is the list of file descriptors we can properly close after the process
// has been started. When the new process is exec(), those file descriptors are
// duplicated and it is our responsibility to close them since we have opened
// them.
func (p *process) closePostStartFDs() {
	if p.process.Stdin != nil {
		p.process.Stdin.(*os.File).Close()
	}

	if p.process.Stdout != nil {
		p.process.Stdout.(*os.File).Close()
	}

	if p.process.Stderr != nil {
		p.process.Stderr.(*os.File).Close()
	}

	if p.process.ConsoleSocket != nil {
		p.process.Stderr.(*os.File).Close()
	}

	if p.consoleSock != nil {
		p.process.Stderr.(*os.File).Close()
	}
}

// This is the list of file descriptors we can properly close after the process
// has exited. These are the remaining file descriptors that we have opened and
// are no longer needed.
func (p *process) closePostExitFDs() {
	if p.termMaster != nil {
		p.termMaster.Close()
	}

	if p.stdin != nil {
		p.stdin.Close()
	}

	if p.stdout != nil {
		p.stdout.Close()
	}

	if p.stderr != nil {
		p.stderr.Close()
	}
}

func (c *container) setProcess(pid int, process *process) {
	c.Lock()
	c.processes[pid] = process
	c.Unlock()
}

func (c *container) deleteProcess(pid int) {
	c.Lock()
	delete(c.processes, pid)
	c.Unlock()
}

func (c *container) removeContainer() error {
	// This will terminates all processes related to this container, and
	// destroy the container right after. But this will error in case the
	// container in not in the right state.
	if err := c.container.Destroy(); err != nil {
		return err
	}

	return removeMounts(c.mounts)
}

func (c *container) getProcess(pid int) (*process, error) {
	c.RLock()
	defer c.RUnlock()

	proc, exist := c.processes[pid]
	if !exist {
		return nil, fmt.Errorf("Process %d not found (container %s)", pid, c.id)
	}

	return proc, nil
}

func (s *sandbox) getContainer(id string) (*container, error) {
	s.RLock()
	defer s.RUnlock()

	ctr, exist := s.containers[id]
	if !exist {
		return nil, fmt.Errorf("Container %s not found", id)
	}

	return ctr, nil
}

func (s *sandbox) setContainer(id string, ctr *container) {
	s.Lock()
	s.containers[id] = ctr
	s.Unlock()
}

func (s *sandbox) deleteContainer(id string) {
	s.Lock()
	delete(s.containers, id)
	s.Unlock()
}

func (s *sandbox) getRunningProcess(cid string, pid int) (*process, *container, error) {
	if s.running == false {
		return nil, nil, fmt.Errorf("Sandbox not started")
	}

	ctr, err := s.getContainer(cid)
	if err != nil {
		return nil, nil, err
	}

	status, err := ctr.container.Status()
	if err != nil {
		return nil, nil, err
	}

	if status != libcontainer.Running {
		return nil, nil, fmt.Errorf("Container %s %s, should be %s", cid, status.String(), libcontainer.Running.String())
	}

	proc, err := ctr.getProcess(pid)
	if err != nil {
		return nil, nil, err
	}

	return proc, ctr, nil
}

func (s *sandbox) readStdio(cid string, pid int, length int, stdout bool) ([]byte, error) {
	proc, _, err := s.getRunningProcess(cid, pid)
	if err != nil {
		return nil, err
	}

	var file *os.File
	if proc.termMaster != nil {
		file = proc.termMaster
	} else {
		if stdout {
			file = proc.stdout
		} else {
			file = proc.stderr
		}
	}

	buf := make([]byte, length)

	if _, err := file.Read(buf); err != nil {
		return nil, err
	}

	return buf, nil
}

func (s *sandbox) initLogger() error {
	agentLog.Logger.Formatter = &logrus.TextFormatter{TimestampFormat: time.RFC3339Nano}

	config := newConfig(defaultLogLevel)
	if err := config.getConfig(kernelCmdlineFile); err != nil {
		agentLog.WithError(err).Warn("Failed to get config from kernel cmdline")
	}
	config.applyConfig()

	agentLog.WithField("version", Version).Info()

	return nil
}

func (s *sandbox) initChannel() error {
	c, err := newChannel()
	if err != nil {
		return err
	}

	s.channel = c

	return s.channel.setup()
}

func (s *sandbox) startGRPC() error {
	l, err := s.channel.listen()
	if err != nil {
		return err
	}

	s.grpcListener = l

	grpcImpl := &agentGRPC{
		sandbox: s,
	}

	grpcServer := grpc.NewServer()
	pb.RegisterAgentServiceServer(grpcServer, grpcImpl)

	s.wg.Add(1)
	go func() {
		defer s.wg.Done()

		grpcServer.Serve(l)
	}()

	return nil
}

func (s *sandbox) teardown() error {
	if err := s.grpcListener.Close(); err != nil {
		return err
	}

	return s.channel.teardown()
}

func init() {
	if len(os.Args) > 1 && os.Args[1] == "init" {
		runtime.GOMAXPROCS(1)
		runtime.LockOSThread()
		factory, _ := libcontainer.New("")
		if err := factory.StartInitialization(); err != nil {
			agentLog.Errorf("init went wrong: %v", err)
		}
		panic("--this line should have never been executed, congratulations--")
	}
}

func main() {
	var err error

	defer func() {
		if err != nil {
			agentLog.Error(err)
			os.Exit(exitFailure)
		}

		os.Exit(exitSuccess)
	}()

	// Initialize unique sandbox structure.
	s := &sandbox{
		containers: make(map[string]*container),
		running:    false,
	}

	if err = s.initLogger(); err != nil {
		return
	}

	// Check for vsock vs serial. This will fill the sandbox structure with
	// information about the channel.
	if err = s.initChannel(); err != nil {
		return
	}

	// Start gRPC server.
	if err = s.startGRPC(); err != nil {
		return
	}

	s.wg.Wait()

	// Tear down properly.
	if err = s.teardown(); err != nil {
		return
	}
}
