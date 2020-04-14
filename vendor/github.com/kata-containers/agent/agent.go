//
// Copyright (c) 2017-2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bufio"
	"errors"
	"flag"
	"fmt"
	"io"
	"io/ioutil"
	"net"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/gogo/protobuf/proto"
	"github.com/grpc-ecosystem/grpc-opentracing/go/otgrpc"
	"github.com/kata-containers/agent/pkg/uevent"
	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/mdlayher/vsock"
	"github.com/opencontainers/runc/libcontainer"
	"github.com/opencontainers/runc/libcontainer/configs"
	_ "github.com/opencontainers/runc/libcontainer/nsenter"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"golang.org/x/net/context"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const (
	procCgroups = "/proc/cgroups"

	bashPath         = "/bin/bash"
	shPath           = "/bin/sh"
	debugConsolePath = "/dev/console"
)

var (
	// List of shells that are tried (in order) to setup a debug console
	supportedShells = []string{bashPath, shPath}

	meminfo = "/proc/meminfo"

	// cgroup fs is mounted at /sys/fs when systemd is the init process
	sysfsDir                     = "/sys"
	cgroupPath                   = sysfsDir + "/fs/cgroup"
	cgroupCpusetPath             = cgroupPath + "/cpuset"
	cgroupMemoryPath             = cgroupPath + "/memory"
	cgroupMemoryUseHierarchyPath = cgroupMemoryPath + "/memory.use_hierarchy"
	cgroupMemoryUseHierarchyMode = os.FileMode(0400)

	cgroupControllersPath    = cgroupPath + "/cgroup.controllers"
	cgroupSubtreeControlPath = cgroupPath + "/cgroup.subtree_control"
	cgroupSubtreeControlMode = os.FileMode(0644)

	// Set by the build
	seccompSupport string

	// Set to the context that should be used for tracing gRPC calls.
	grpcContext context.Context

	rootContext context.Context
)

var initRootfsMounts = []initMount{
	{"proc", "proc", "/proc", []string{"nosuid", "nodev", "noexec"}},
	{"sysfs", "sysfs", sysfsDir, []string{"nosuid", "nodev", "noexec"}},
	{"devtmpfs", "dev", "/dev", []string{"nosuid"}},
	{"tmpfs", "tmpfs", "/dev/shm", []string{"nosuid", "nodev"}},
	{"devpts", "devpts", "/dev/pts", []string{"nosuid", "noexec"}},
	{"tmpfs", "tmpfs", "/run", []string{"nosuid", "nodev"}},
}

type process struct {
	sync.RWMutex

	id          string
	process     libcontainer.Process
	stdin       *os.File
	stdout      *os.File
	stderr      *os.File
	consoleSock *os.File
	termMaster  *os.File
	epoller     *epoller
	exitCodeCh  chan int
	sync.Once
	stdinClosed bool
}

type container struct {
	sync.RWMutex

	id              string
	initProcess     *process
	container       libcontainer.Container
	config          configs.Config
	processes       map[string]*process
	mounts          []string
	useSandboxPidNs bool
	ctx             context.Context
}

type sandboxStorage struct {
	refCount int
}

type sandbox struct {
	sync.RWMutex
	ctx context.Context

	id                string
	hostname          string
	containers        map[string]*container
	channel           channel
	network           network
	wg                sync.WaitGroup
	sharedPidNs       namespace
	mounts            []string
	subreaper         reaper
	server            *grpc.Server
	pciDeviceMap      map[string]string
	deviceWatchers    map[string](chan string)
	sharedUTSNs       namespace
	sharedIPCNs       namespace
	guestHooks        *specs.Hooks
	guestHooksPresent bool
	running           bool
	noPivotRoot       bool
	enableGrpcTrace   bool
	sandboxPidNs      bool
	storages          map[string]*sandboxStorage
	stopServer        chan struct{}
}

var agentFields = logrus.Fields{
	"name":   agentName,
	"pid":    os.Getpid(),
	"source": "agent",
}

var agentLog = logrus.WithFields(agentFields)

// version is the agent version. This variable is populated at build time.
var version = "unknown"

var debug = false

// tracing enables opentracing support
var tracing = false

// Associate agent traces with runtime traces. This can only be enabled using
// the traceModeFlag.
var collatedTrace = false

// if true, coredump when an internal error occurs or a fatal signal is received
var crashOnError = false

// if true, a shell (bash or sh) is started only if it's available in the rootfs.
var debugConsole = false

// Specify a vsock port where logs are written.
var logsVSockPort = uint32(0)

// Specify a vsock port where debug console is attached.
var debugConsoleVSockPort = uint32(0)

// Timeout waiting for a device to be hotplugged
var hotplugTimeout = 3 * time.Second

// Specify the log level
var logLevel = defaultLogLevel

// Specify whether the agent has to use cgroups v2 or not.
var unifiedCgroupHierarchy = false

// Size in bytes of the stdout/stderr pipes created for each container.
var containerPipeSize = uint32(0)

// commType is used to denote the communication channel type used.
type commType int

const (
	// virtio-serial channel
	serialCh commType = iota

	// vsock channel
	vsockCh

	// channel type not passed explicitly
	unknownCh
)

var commCh = unknownCh

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
		p.process.ConsoleSocket.Close()
	}

	if p.consoleSock != nil {
		p.consoleSock.Close()
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

	if p.epoller != nil {
		p.epoller.sockR.Close()
		unix.Close(p.epoller.fd)
	}
}

func (c *container) trace(name string) (*agentSpan, context.Context) {
	if c.ctx == nil {
		agentLog.WithField("type", "bug").Error("trace called before context set")
		c.ctx = context.Background()
	}

	return trace(c.ctx, "container", name)
}

func (c *container) setProcess(process *process) {
	c.Lock()
	c.processes[process.id] = process
	c.Unlock()
}

func (c *container) deleteProcess(execID string) {
	span, _ := c.trace("deleteProcess")
	span.setTag("exec-id", execID)
	defer span.finish()
	c.Lock()
	delete(c.processes, execID)
	c.Unlock()
}

func (c *container) removeContainer() error {
	span, _ := c.trace("removeContainer")
	defer span.finish()
	// This will terminates all processes related to this container, and
	// destroy the container right after. But this will error in case the
	// container in not in the right state.
	if err := c.container.Destroy(); err != nil {
		return err
	}

	return removeMounts(c.mounts)
}

func (c *container) getProcess(execID string) (*process, error) {
	c.RLock()
	defer c.RUnlock()

	proc, exist := c.processes[execID]
	if !exist {
		return nil, grpcStatus.Errorf(codes.NotFound, "Process %s not found (container %s)", execID, c.id)
	}

	return proc, nil
}

func (s *sandbox) trace(name string) (*agentSpan, context.Context) {
	if s.ctx == nil {
		agentLog.WithField("type", "bug").Error("trace called before context set")
		s.ctx = context.Background()
	}

	span, ctx := trace(s.ctx, "sandbox", name)

	span.setTag("sandbox", s.id)

	return span, ctx
}

// setSandboxStorage sets the sandbox level reference
// counter for the sandbox storage.
// This method also returns a boolean to let
// callers know if the storage already existed or not.
// It will return true if storage is new.
//
// It's assumed that caller is calling this method after
// acquiring a lock on sandbox.
func (s *sandbox) setSandboxStorage(path string) bool {
	if _, ok := s.storages[path]; !ok {
		sbs := &sandboxStorage{refCount: 1}
		s.storages[path] = sbs
		return true
	}
	sbs := s.storages[path]
	sbs.refCount++
	return false
}

// scanGuestHooks will search the given guestHookPath
// for any OCI hooks
func (s *sandbox) scanGuestHooks(guestHookPath string) {
	span, _ := s.trace("scanGuestHooks")
	span.setTag("guest-hook-path", guestHookPath)
	defer span.finish()

	fieldLogger := agentLog.WithField("oci-hook-path", guestHookPath)
	fieldLogger.Info("Scanning guest filesystem for OCI hooks")

	s.guestHooks.Prestart = findHooks(guestHookPath, "prestart")
	s.guestHooks.Poststart = findHooks(guestHookPath, "poststart")
	s.guestHooks.Poststop = findHooks(guestHookPath, "poststop")

	if len(s.guestHooks.Prestart) > 0 || len(s.guestHooks.Poststart) > 0 || len(s.guestHooks.Poststop) > 0 {
		s.guestHooksPresent = true
	} else {
		fieldLogger.Warn("Guest hooks were requested but none were found")
	}
}

// addGuestHooks will add any guest OCI hooks that were
// found to the OCI spec
func (s *sandbox) addGuestHooks(spec *specs.Spec) {
	span, _ := s.trace("addGuestHooks")
	defer span.finish()

	if spec == nil {
		return
	}

	if spec.Hooks == nil {
		spec.Hooks = &specs.Hooks{}
	}

	spec.Hooks.Prestart = append(spec.Hooks.Prestart, s.guestHooks.Prestart...)
	spec.Hooks.Poststart = append(spec.Hooks.Poststart, s.guestHooks.Poststart...)
	spec.Hooks.Poststop = append(spec.Hooks.Poststop, s.guestHooks.Poststop...)
}

// unSetSandboxStorage will decrement the sandbox storage
// reference counter. If there aren't any containers using
// that sandbox storage, this method will remove the
// storage reference from the sandbox and return 'true, nil' to
// let the caller know that they can clean up the storage
// related directories by calling removeSandboxStorage
//
// It's assumed that caller is calling this method after
// acquiring a lock on sandbox.
func (s *sandbox) unSetSandboxStorage(path string) (bool, error) {
	span, _ := s.trace("unSetSandboxStorage")
	span.setTag("path", path)
	defer span.finish()

	if sbs, ok := s.storages[path]; ok {
		sbs.refCount--
		// If this sandbox storage is not used by any container
		// then remove it's reference
		if sbs.refCount < 1 {
			delete(s.storages, path)
			return true, nil
		}
		return false, nil
	}
	return false, grpcStatus.Errorf(codes.NotFound, "Sandbox storage with path %s not found", path)
}

// removeSandboxStorage removes the sandbox storage if no
// containers are using that storage.
//
// It's assumed that caller is calling this method after
// acquiring a lock on sandbox.
func (s *sandbox) removeSandboxStorage(path string) error {
	span, _ := s.trace("removeSandboxStorage")
	span.setTag("path", path)
	defer span.finish()

	err := removeMounts([]string{path})
	if err != nil {
		return grpcStatus.Errorf(codes.Unknown, "Unable to unmount sandbox storage path %s: %v", path, err)
	}
	err = os.RemoveAll(path)
	if err != nil {
		return grpcStatus.Errorf(codes.Unknown, "Unable to delete sandbox storage path %s: %v", path, err)
	}
	return nil
}

// unsetAndRemoveSandboxStorage unsets the storage from sandbox
// and if there are no containers using this storage it will
// remove it from the sandbox.
//
// It's assumed that caller is calling this method after
// acquiring a lock on sandbox.
func (s *sandbox) unsetAndRemoveSandboxStorage(path string) error {
	span, _ := s.trace("unsetAndRemoveSandboxStorage")
	span.setTag("path", path)
	defer span.finish()

	removeSbs, err := s.unSetSandboxStorage(path)
	if err != nil {
		return err
	}

	if removeSbs {
		if err := s.removeSandboxStorage(path); err != nil {
			return err
		}
	}

	return nil
}

func (s *sandbox) getContainer(id string) (*container, error) {
	s.RLock()
	defer s.RUnlock()

	ctr, exist := s.containers[id]
	if !exist {
		return nil, grpcStatus.Errorf(codes.NotFound, "Container %s not found", id)
	}

	return ctr, nil
}

func (s *sandbox) setContainer(ctx context.Context, id string, ctr *container) {
	// Update the context. This is required since the function is called
	// from by gRPC functions meaning we must use the latest context
	// available.
	s.ctx = ctx

	span, _ := s.trace("setContainer")
	span.setTag("id", id)
	span.setTag("container", ctr.id)
	defer span.finish()

	s.Lock()
	s.containers[id] = ctr
	s.Unlock()
}

func (s *sandbox) deleteContainer(id string) {
	span, _ := s.trace("deleteContainer")
	span.setTag("container", id)
	defer span.finish()

	s.Lock()

	// Find the sandbox storage used by this container
	ctr, exist := s.containers[id]
	if !exist {
		agentLog.WithField("container-id", id).Debug("Container doesn't exist")
	} else {
		// Let's go over the mounts used by this container
		for _, k := range ctr.mounts {
			// Check if this mount is used from sandbox storage
			if _, ok := s.storages[k]; ok {
				if err := s.unsetAndRemoveSandboxStorage(k); err != nil {
					agentLog.WithError(err).Error()
				}
			}
		}
	}

	delete(s.containers, id)
	s.Unlock()
}

func (s *sandbox) getProcess(cid, execID string) (*process, *container, error) {
	if !s.running {
		return nil, nil, grpcStatus.Error(codes.FailedPrecondition, "Sandbox not started")
	}

	ctr, err := s.getContainer(cid)
	if err != nil {
		return nil, nil, err
	}

	// A container being in stopped state is not a valid reason for not
	// accepting a call to getProcess(). Indeed, we want to make sure a
	// shim can connect after the process has already terminated. Some
	// processes have a very short lifetime and the shim might end up
	// calling into WaitProcess() after this happened. This does not mean
	// we cannot retrieve the output and the exit code from the shim.
	proc, err := ctr.getProcess(execID)
	if err != nil {
		return nil, nil, err
	}

	return proc, ctr, nil
}

func (s *sandbox) readStdio(cid, execID string, length int, stdout bool) ([]byte, error) {
	proc, _, err := s.getProcess(cid, execID)
	if err != nil {
		return nil, err
	}

	var file *os.File
	if proc.termMaster != nil {
		// The process's epoller's run() will return a file descriptor of the process's
		// terminal or one end of its exited pipe. If it returns its terminal, it means
		// there is data needed to be read out or it has been closed; if it returns the
		// process's exited pipe, it means the process has exited and there is no data
		// needed to be read out in its terminal, thus following read on it will read out
		// "EOF" to terminate this process's io since the other end of this pipe has been
		// closed in reap().
		file, err = proc.epoller.run()
		if err != nil {
			return nil, err
		}
	} else {
		if stdout {
			file = proc.stdout
		} else {
			file = proc.stderr
		}
	}

	buf := make([]byte, length)

	bytesRead, err := file.Read(buf)
	if err != nil {
		return nil, err
	}

	return buf[:bytesRead], nil
}

func (s *sandbox) setupSharedNamespaces(ctx context.Context) error {
	span, _ := trace(ctx, "sandbox", "setupSharedNamespaces")
	defer span.finish()

	// Set up shared IPC namespace
	ns, err := setupPersistentNs(nsTypeIPC)
	if err != nil {
		return err
	}
	s.sharedIPCNs = *ns

	// Set up shared UTS namespace
	ns, err = setupPersistentNs(nsTypeUTS)
	if err != nil {
		return err
	}
	s.sharedUTSNs = *ns

	return nil
}

func (s *sandbox) unmountSharedNamespaces() error {
	span, _ := s.trace("unmountSharedNamespaces")
	defer span.finish()

	if err := unix.Unmount(s.sharedIPCNs.path, unix.MNT_DETACH); err != nil {
		return err
	}

	return unix.Unmount(s.sharedUTSNs.path, unix.MNT_DETACH)
}

// setupSharedPidNs will reexec this binary in order to execute the C routine
// defined into pause.go file. The pauseBinArg is very important since that is
// the flag allowing the C function to determine it should run the "pause".
// This pause binary will ensure that we always have the init process of the
// new PID namespace running into the namespace, preventing the namespace to
// be destroyed if other processes are terminated.
func (s *sandbox) setupSharedPidNs() error {
	span, _ := s.trace("setupSharedPidNs")
	defer span.finish()

	cmd := &exec.Cmd{
		Path: selfBinPath,
		Env:  []string{fmt.Sprintf("%s=%s", pauseBinKey, pauseBinValue)},
	}

	cmd.SysProcAttr = &syscall.SysProcAttr{
		Cloneflags: syscall.CLONE_NEWPID,
	}

	exitCodeCh, err := s.subreaper.start(cmd)
	if err != nil {
		return err
	}

	// Save info about this namespace inside sandbox structure.
	s.sharedPidNs = namespace{
		path:       fmt.Sprintf("/proc/%d/ns/pid", cmd.Process.Pid),
		init:       cmd.Process,
		exitCodeCh: exitCodeCh,
	}

	return nil
}

func (s *sandbox) teardownSharedPidNs() error {
	span, _ := s.trace("teardownSharedPidNs")
	defer span.finish()

	if !s.sandboxPidNs {
		// We are not in a case where we have created a pause process.
		// Simply clear out the sharedPidNs path.
		s.sharedPidNs.path = ""
		return nil
	}

	// Terminates the "init" process of the PID namespace.
	if err := s.sharedPidNs.init.Kill(); err != nil {
		return err
	}

	// Using helper function wait() to deal with the subreaper.
	osProcess := (*reaperOSProcess)(s.sharedPidNs.init)
	if _, err := s.subreaper.wait(s.sharedPidNs.exitCodeCh, osProcess); err != nil {
		return err
	}

	// Empty the sandbox structure.
	s.sharedPidNs = namespace{}

	return nil
}

func (s *sandbox) waitForStopServer() {
	span, _ := s.trace("waitForStopServer")
	defer span.finish()

	fieldLogger := agentLog.WithField("subsystem", "stopserverwatcher")

	fieldLogger.Info("Waiting for stopServer signal...")

	// Wait for DestroySandbox() to signal this thread about the need to
	// stop the server.
	<-s.stopServer

	fieldLogger.Info("stopServer signal received")

	if s.server == nil {
		fieldLogger.Info("No server initialized, nothing to stop")
		return
	}

	defer fieldLogger.Info("gRPC server stopped")

	// Try to gracefully stop the server for a minute
	timeout := time.Minute
	done := make(chan struct{})
	go func() {
		s.server.GracefulStop()
		close(done)
	}()

	select {
	case <-done:
		s.server = nil
		return
	case <-time.After(timeout):
		fieldLogger.WithField("timeout", timeout).Warn("Could not gracefully stop the server")
	}

	fieldLogger.Info("Force stopping the server now")

	span.setTag("forced", true)
	s.stopGRPC()
}

func (s *sandbox) listenToUdevEvents() {
	fieldLogger := agentLog.WithField("subsystem", "udevlistener")

	uEvHandler, err := uevent.NewHandler()
	if err != nil {
		fieldLogger.Warnf("Error starting uevent listening loop %s", err)
		return
	}
	defer uEvHandler.Close()

	fieldLogger.Infof("Started listening for uevents")

	for {
		uEv, err := uEvHandler.Read()
		if err != nil {
			fieldLogger.Error(err)
			continue
		}

		span, _ := trace(rootContext, "udev", "udev event")
		span.setTag("udev-action", uEv.Action)
		span.setTag("udev-name", uEv.DevName)
		span.setTag("udev-path", uEv.DevPath)
		span.setTag("udev-subsystem", uEv.SubSystem)
		span.setTag("udev-seqno", uEv.SeqNum)

		fieldLogger = fieldLogger.WithFields(logrus.Fields{
			"uevent-action":    uEv.Action,
			"uevent-devpath":   uEv.DevPath,
			"uevent-subsystem": uEv.SubSystem,
			"uevent-seqnum":    uEv.SeqNum,
			"uevent-devname":   uEv.DevName,
		})

		if uEv.Action == "remove" {
			fieldLogger.Infof("Remove dev from pciDeviceMap")
			s.Lock()
			delete(s.pciDeviceMap, uEv.DevPath)
			s.Unlock()
			goto FINISH_SPAN
		}

		if uEv.Action != "add" {
			goto FINISH_SPAN
		}

		fieldLogger.Infof("Received add uevent")

		// Check if device hotplug event results in a device node being created.
		if uEv.DevName != "" &&
			(strings.HasPrefix(uEv.DevPath, rootBusPath) || strings.HasPrefix(uEv.DevPath, acpiDevPath)) {
			// Lock is needed to safey read and modify the pciDeviceMap and deviceWatchers.
			// This makes sure that watchers do not access the map while it is being updated.
			s.Lock()

			// Add the device node name to the pci device map.
			s.pciDeviceMap[uEv.DevPath] = uEv.DevName

			// Notify watchers that are interested in the udev event.
			// Close the channel after watcher has been notified.
			for devAddress, ch := range s.deviceWatchers {
				if ch == nil {
					continue
				}

				fieldLogger.Infof("Got a wait channel for device %s", devAddress)

				// blk driver case
				if strings.HasPrefix(uEv.DevPath, filepath.Join(rootBusPath, devAddress)) {
					goto OUT
				}

				// pmem/nvdimm case
				if strings.Contains(uEv.DevPath, pfnDevPrefix) && strings.HasSuffix(uEv.DevPath, devAddress) {
					goto OUT
				}

				if strings.Contains(uEv.DevPath, devAddress) {
					// scsi driver case
					if strings.HasSuffix(devAddress, scsiBlockSuffix) {
						goto OUT
					}
					// blk-ccw driver case
					if strings.HasSuffix(devAddress, blkCCWSuffix) {
						goto OUT
					}
				}

				continue

			OUT:
				ch <- uEv.DevName
				close(ch)
				delete(s.deviceWatchers, devAddress)

			}

			s.Unlock()
		} else if onlinePath := filepath.Join(sysfsDir, uEv.DevPath, "online"); strings.HasPrefix(onlinePath, sysfsMemOnlinePath) {
			// Check memory hotplug and online if possible
			if err := ioutil.WriteFile(onlinePath, []byte("1"), 0600); err != nil {
				fieldLogger.WithError(err).Error("failed online device")
			}
		}
	FINISH_SPAN:
		span.finish()
	}
}

// This loop is meant to be run inside a separate Go routine.
func (s *sandbox) signalHandlerLoop(sigCh chan os.Signal, errCh chan error) {
	// Lock OS thread as subreaper is a thread local capability
	// and is not inherited by children created by fork(2) and clone(2).
	runtime.LockOSThread()
	// Set agent as subreaper
	err := unix.Prctl(unix.PR_SET_CHILD_SUBREAPER, uintptr(1), 0, 0, 0)
	if err != nil {
		errCh <- err
		return
	}
	close(errCh)

	for sig := range sigCh {
		logger := agentLog.WithField("signal", sig)

		if sig == unix.SIGCHLD {
			if err := s.subreaper.reap(); err != nil {
				logger.WithError(err).Error("failed to reap")
				continue
			}
		}

		nativeSignal, ok := sig.(syscall.Signal)
		if !ok {
			err := errors.New("unknown signal")
			logger.WithError(err).Error("failed to handle signal")
			continue
		}

		if fatalSignal(nativeSignal) {
			logger.Error("received fatal signal")
			die(s.ctx)
		}

		if debug && nonFatalSignal(nativeSignal) {
			logger.Debug("handling signal")
			backtrace()
			continue
		}

		logger.Info("ignoring unexpected signal")
	}
}

func (s *sandbox) setupSignalHandler() error {
	span, _ := s.trace("setupSignalHandler")
	defer span.finish()

	sigCh := make(chan os.Signal, 512)
	signal.Notify(sigCh, unix.SIGCHLD)

	for _, sig := range handledSignals() {
		signal.Notify(sigCh, sig)
	}

	errCh := make(chan error, 1)
	go s.signalHandlerLoop(sigCh, errCh)
	return <-errCh
}

// getMemory returns a string containing the total amount of memory reported
// by the kernel. The string includes a suffix denoting the units the memory
// is measured in.
func getMemory() (string, error) {
	bytes, err := ioutil.ReadFile(meminfo)
	if err != nil {
		return "", err
	}

	lines := string(bytes)

	for _, line := range strings.Split(lines, "\n") {
		if !strings.HasPrefix(line, "MemTotal") {
			continue
		}

		expectedFields := 2

		fields := strings.Split(line, ":")
		count := len(fields)

		if count != expectedFields {
			return "", fmt.Errorf("expected %d fields, got %d in line %q", expectedFields, count, line)
		}

		if fields[1] == "" {
			return "", fmt.Errorf("cannot determine total memory from line %q", line)
		}

		memTotal := strings.TrimSpace(fields[1])
		if memTotal == "" {
			return "", fmt.Errorf("cannot determine total memory from line %q", line)
		}

		return memTotal, nil
	}

	return "", fmt.Errorf("no lines in file %q", meminfo)
}

func getAnnounceFields() (logrus.Fields, error) {
	var deviceHandlers []string
	var storageHandlers []string

	for handler := range deviceHandlerList {
		deviceHandlers = append(deviceHandlers, handler)
	}

	for handler := range storageHandlerList {
		storageHandlers = append(storageHandlers, handler)
	}

	memTotal, err := getMemory()
	if err != nil {
		return logrus.Fields{}, err
	}

	return logrus.Fields{
		"version":          version,
		"device-handlers":  strings.Join(deviceHandlers, ","),
		"storage-handlers": strings.Join(storageHandlers, ","),
		"system-memory":    memTotal,
	}, nil
}

// formatFields converts logrus Fields (containing arbitrary types) into a string slice.
func formatFields(fields logrus.Fields) []string {
	var results []string

	for k, v := range fields {
		value, ok := v.(string)
		if !ok {
			// convert non-string value into a string
			value = fmt.Sprint(v)
		}

		results = append(results, fmt.Sprintf("%s=%q", k, value))
	}

	return results
}

// announce logs details of the agents version and capabilities.
func announce() error {
	announceFields, err := getAnnounceFields()
	if err != nil {
		return err
	}

	if os.Getpid() == 1 {
		fields := formatFields(agentFields)
		extraFields := formatFields(announceFields)

		fields = append(fields, extraFields...)

		fmt.Printf("announce: %s\n", strings.Join(fields, ","))
	} else {
		agentLog.WithFields(announceFields).Info("announce")
	}

	return nil
}

func logsToVPort() {
	l, err := vsock.Listen(logsVSockPort)
	if err != nil {
		// no body listening
		return
	}
	c, err := l.Accept()
	if err != nil {
		l.Close()
		// no connection
		return
	}

	r, w := io.Pipe()
	agentLog.Logger.Out = w
	io.Copy(c, r)

	w.Close()
	r.Close()
	c.Close()
	l.Close()
}

func (s *sandbox) initLogger(ctx context.Context) error {
	agentLog.Logger.Formatter = &logrus.TextFormatter{DisableColors: true, TimestampFormat: time.RFC3339Nano}

	agentLog.Logger.SetLevel(logLevel)

	agentLog = agentLog.WithField("debug_console", debugConsole)

	if logsVSockPort != 0 {
		go func() {
			// save original logger's output to restore it when there
			// is no process reading the logs in the host
			out := agentLog.Logger.Out
			for {
				select {
				case <-ctx.Done():
					// stop the thread
					return
				default:
					logsToVPort()
					if agentLog.Logger.Out != out {
						agentLog.Logger.Out = out
					}
					// waiting for the logs reader
					time.Sleep(time.Millisecond * 500)
				}
			}
		}()
	}

	return announce()
}

func (s *sandbox) initChannel() error {
	span, ctx := s.trace("initChannel")
	defer span.finish()

	c, err := newChannel(ctx)
	if err != nil {
		return err
	}

	s.channel = c

	return nil
}

func makeUnaryInterceptor() grpc.UnaryServerInterceptor {
	return func(origCtx context.Context, req interface{}, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (resp interface{}, err error) {
		var start time.Time
		var elapsed time.Duration
		var message proto.Message

		grpcCall := info.FullMethod
		var ctx context.Context
		var span *agentSpan

		if tracing {
			ctx = getGRPCContext()
			span, _ = trace(ctx, "gRPC", grpcCall)
			span.setTag("grpc-method-type", "unary")

			if strings.HasSuffix(grpcCall, "/ReadStdout") || strings.HasSuffix(grpcCall, "/WriteStdin") {
				// Add a tag to allow filtering of those calls dealing
				// input and output. These tend to be very long and
				// being able to filter them out allows the
				// performance of "core" calls to be determined
				// without the "noise" of these calls.
				span.setTag("api-category", "interactive")
			}
		} else {
			// Just log call details
			message = req.(proto.Message)

			agentLog.WithFields(logrus.Fields{
				"request": grpcCall,
				"req":     message.String()}).Debug("new request")
			start = time.Now()
		}

		// Use the context which will provide the correct trace
		// ordering, *NOT* the context provided to the function
		// returned by this function.
		resp, err = handler(getGRPCContext(), req)

		if !tracing {
			// Just log call details
			elapsed = time.Since(start)
			message = resp.(proto.Message)

			logger := agentLog.WithFields(logrus.Fields{
				"request":  info.FullMethod,
				"duration": elapsed.String(),
				"resp":     message.String()})
			logger.Debug("request end")
		}

		// Handle the following scenarios:
		//
		// - Tracing was (and still is) enabled.
		// - Tracing was enabled but the handler (StopTracing()) disabled it.
		// - Tracing was disabled but the handler (StartTracing()) enabled it.
		if span != nil {
			span.finish()
		}

		if stopTracingCalled {
			stopTracing(ctx)
		}

		return resp, err
	}
}

func (s *sandbox) startGRPC() {
	span, _ := s.trace("startGRPC")
	defer span.finish()

	// Save the context for gRPC calls. They are provided with a different
	// context, but we need them to use our context as it contains trace
	// metadata.
	grpcContext = s.ctx

	grpcImpl := &agentGRPC{
		sandbox: s,
		version: version,
	}

	var grpcServer *grpc.Server

	var serverOpts []grpc.ServerOption

	if collatedTrace {
		// "collated" tracing (allow agent traces to be
		// associated with runtime-initiated traces.
		tracer := span.tracer()

		serverOpts = append(serverOpts, grpc.UnaryInterceptor(otgrpc.OpenTracingServerInterceptor(tracer.tracer)))
	} else {
		// Enable interceptor whether tracing is enabled or not. This
		// is necessary to support StartTracing() and StopTracing()
		// since they require the interceptors to change their
		// behaviour depending on whether tracing is enabled.
		//
		// When tracing is enabled, the interceptor handles "isolated"
		// tracing (agent traces are not associated with runtime-initiated
		// traces).
		serverOpts = append(serverOpts, grpc.UnaryInterceptor(makeUnaryInterceptor()))
	}

	grpcServer = grpc.NewServer(serverOpts...)

	pb.RegisterAgentServiceServer(grpcServer, grpcImpl)
	pb.RegisterHealthServer(grpcServer, grpcImpl)
	s.server = grpcServer

	s.wg.Add(1)
	go func() {
		defer s.wg.Done()

		var err error
		var servErr error
		for {
			agentLog.Info("agent grpc server starts")

			err = s.channel.setup()
			if err != nil {
				agentLog.WithError(err).Warn("Failed to setup agent grpc channel")
				return
			}

			err = s.channel.wait()
			if err != nil {
				agentLog.WithError(err).Warn("Failed to wait agent grpc channel ready")
				return
			}

			var l net.Listener
			l, err = s.channel.listen()
			if err != nil {
				agentLog.WithError(err).Warn("Failed to create agent grpc listener")
				return
			}

			// l is closed when Serve() returns
			servErr = grpcServer.Serve(l)
			if servErr != nil {
				agentLog.WithError(servErr).Warn("agent grpc server quits")
			}

			err = s.channel.teardown()
			if err != nil {
				agentLog.WithError(err).Warn("agent grpc channel teardown failed")
			}

			// Based on the definition of grpc.Serve(), the function
			// returns nil in case of a proper stop triggered by either
			// grpc.GracefulStop() or grpc.Stop(). Those calls can only
			// be issued by the chain of events coming from DestroySandbox
			// and explicitly means the server should not try to listen
			// again, as the sandbox is being completely removed.
			if servErr == nil {
				agentLog.Info("agent grpc server has been explicitly stopped")
				return
			}
		}
	}()
}

func getGRPCContext() context.Context {
	if grpcContext != nil {
		return grpcContext
	}

	agentLog.Warn("Creating gRPC context as none found")

	return context.Background()
}

func (s *sandbox) stopGRPC() {
	if s.server != nil {
		s.server.Stop()
		s.server = nil
	}
}

type initMount struct {
	fstype, src, dest string
	options           []string
}

func getCgroupMounts(cgPath string) ([]initMount, error) {
	if unifiedCgroupHierarchy {
		return []initMount{
			{"cgroup2", "cgroup2", cgroupPath, []string{"nosuid", "nodev", "noexec", "relatime", "nsdelegate"}},
		}, nil
	}

	f, err := os.Open(cgPath)
	if err != nil {
		return []initMount{}, err
	}
	defer f.Close()

	hasDevicesCgroup := false

	cgroupMounts := []initMount{{"tmpfs", "tmpfs", cgroupPath, []string{"nosuid", "nodev", "noexec", "mode=755"}}}
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		text := scanner.Text()
		fields := strings.Split(text, "\t")

		// #subsys_name    hierarchy       num_cgroups     enabled
		// fields[0]       fields[1]       fields[2]       fields[3]
		cgroup := fields[0]
		if cgroup == "" || cgroup[0] == '#' || (len(fields) > 3 && fields[3] == "0") {
			continue
		}
		if cgroup == "devices" {
			hasDevicesCgroup = true
		}
		cgroupMounts = append(cgroupMounts, initMount{"cgroup", "cgroup",
			filepath.Join(cgroupPath, cgroup), []string{"nosuid", "nodev", "noexec", "relatime", cgroup}})
	}

	if err = scanner.Err(); err != nil {
		return []initMount{}, err
	}

	// refer to https://github.com/opencontainers/runc/blob/v1.0.0-rc5/libcontainer/cgroups/fs/apply_raw.go#L132
	if !hasDevicesCgroup {
		return []initMount{}, err
	}

	cgroupMounts = append(cgroupMounts, initMount{"tmpfs", "tmpfs",
		cgroupPath, []string{"remount", "ro", "nosuid", "nodev", "noexec", "mode=755"}})
	return cgroupMounts, nil
}

func mountToRootfs(m initMount) error {
	if err := os.MkdirAll(m.dest, os.FileMode(0755)); err != nil {
		return err
	}

	flags, options := parseMountFlagsAndOptions(m.options)

	if err := syscall.Mount(m.src, m.dest, m.fstype, uintptr(flags), options); err != nil {
		return grpcStatus.Errorf(codes.Internal, "Could not mount %v to %v: %v", m.src, m.dest, err)
	}
	return nil
}

func generalMount() error {
	for _, m := range initRootfsMounts {
		if err := mountToRootfs(m); err != nil {
			// dev is already mounted if the rootfs image is used
			if m.src != "dev" {
				return err
			}
			agentLog.WithError(err).WithField("src", m.src).Warnf("Could not mount filesystem")
		}
	}
	return nil
}

func cgroupsMount() error {
	cgroups, err := getCgroupMounts(procCgroups)
	if err != nil {
		return nil
	}
	for _, m := range cgroups {
		if err := mountToRootfs(m); err != nil {
			return err
		}
	}

	if !unifiedCgroupHierarchy {
		// Enable memory hierarchical account.
		// For more information see https://www.kernel.org/doc/Documentation/cgroup-v1/memory.txt
		return ioutil.WriteFile(cgroupMemoryUseHierarchyPath, []byte{'1'}, cgroupMemoryUseHierarchyMode)
	}

	// Enable all cgroup v2 controllers
	rawControllers, err := ioutil.ReadFile(cgroupControllersPath)
	if err != nil {
		return err
	}

	var controllers string
	for _, c := range strings.Fields(string(rawControllers)) {
		controllers += fmt.Sprintf("+%v ", c)
	}

	// https://www.kernel.org/doc/Documentation/cgroup-v2.txt
	return ioutil.WriteFile(cgroupSubtreeControlPath,
		[]byte(strings.TrimSpace(controllers)), cgroupSubtreeControlMode)
}

func setupDebugConsoleForVsock(ctx context.Context) error {
	var shellPath string
	for _, s := range supportedShells {
		var err error
		if _, err = os.Stat(s); err == nil {
			shellPath = s
			break
		}
		agentLog.WithError(err).WithField("shell", s).Warn("Shell not found")
	}

	if shellPath == "" {
		return fmt.Errorf("No available shells (checked %v)", supportedShells)
	}

	cmd := exec.Command(shellPath, "-i")
	cmd.Env = os.Environ()
	cmd.SysProcAttr = &syscall.SysProcAttr{
		// Create Session
		Setsid: true,
	}

	go func() {
		for {
			select {
			case <-ctx.Done():
				// stop the thread
				return
			default:
				dcmd := *cmd

				l, err := vsock.Listen(debugConsoleVSockPort)
				if err != nil {
					// nobody dialing
					continue
				}
				c, err := l.Accept()
				if err != nil {
					l.Close()
					// no connection
					continue
				}

				dcmd.Stdin = c
				dcmd.Stdout = c
				dcmd.Stderr = c

				if err := dcmd.Run(); err != nil {
					agentLog.WithError(err).Warn("failed to start debug console")
				}

				c.Close()
				l.Close()
			}
		}
	}()

	return nil
}

func setupDebugConsole(ctx context.Context, debugConsolePath string) error {
	if !debugConsole {
		return nil
	}

	if debugConsoleVSockPort != uint32(0) {
		return setupDebugConsoleForVsock(ctx)
	}

	var shellPath string
	for _, s := range supportedShells {
		var err error
		if _, err = os.Stat(s); err == nil {
			shellPath = s
			break
		}
		agentLog.WithError(err).WithField("shell", s).Warn("Shell not found")
	}

	if shellPath == "" {
		return fmt.Errorf("No available shells (checked %v)", supportedShells)
	}

	cmd := exec.Command(shellPath)
	cmd.Env = os.Environ()
	f, err := os.OpenFile(debugConsolePath, os.O_RDWR, 0600)
	if err != nil {
		return err
	}

	cmd.Stdin = f
	cmd.Stdout = f
	cmd.Stderr = f

	cmd.SysProcAttr = &syscall.SysProcAttr{
		// Create Session
		Setsid: true,
		// Set Controlling terminal to Ctty
		Setctty: true,
		Ctty:    int(f.Fd()),
	}

	go func() {
		for {
			select {
			case <-ctx.Done():
				// stop the thread
				return
			default:
				dcmd := *cmd
				if err := dcmd.Run(); err != nil {
					agentLog.WithError(err).Warn("failed to start debug console")
				}
			}
		}
	}()

	return nil
}

// initAgentAsInit will do the initializations such as setting up the rootfs
// when this agent has been run as the init process.
func initAgentAsInit() error {
	if err := generalMount(); err != nil {
		return err
	}
	if err := parseKernelCmdline(); err != nil {
		return err
	}
	if err := cgroupsMount(); err != nil {
		return err
	}
	if err := syscall.Unlink("/dev/ptmx"); err != nil {
		return err
	}
	if err := syscall.Symlink("/dev/pts/ptmx", "/dev/ptmx"); err != nil {
		return err
	}
	syscall.Setsid()
	syscall.Syscall(syscall.SYS_IOCTL, os.Stdin.Fd(), syscall.TIOCSCTTY, 1)
	os.Setenv("PATH", "/bin:/sbin/:/usr/bin/:/usr/sbin/")

	return announce()
}

func init() {
	if len(os.Args) > 1 && os.Args[1] == "init" {
		runtime.GOMAXPROCS(1)
		runtime.LockOSThread()
		factory, _ := libcontainer.New("")
		if err := factory.StartInitialization(); err != nil {
			agentLog.WithError(err).Error("init failed")
		}
		panic("--this line should have never been executed, congratulations--")
	}
}

func realMain() error {
	var err error
	var showVersion bool

	flag.BoolVar(&showVersion, "version", false, "display program version and exit")

	flag.Parse()

	if showVersion {
		fmt.Printf("%v version %v\n", agentName, version)
		return nil
	}

	// Check if this agent has been run as the init process.
	if os.Getpid() == 1 {
		if err = initAgentAsInit(); err != nil {
			panic(fmt.Sprintf("failed to setup agent as init: %v", err))
		}
	} else if err := parseKernelCmdline(); err != nil {
		return err
	}

	r := &agentReaper{}
	r.init()

	fsType, err := getMountFSType("/")
	if err != nil {
		return err
	}

	// Initialize unique sandbox structure.
	s := &sandbox{
		containers: make(map[string]*container),
		running:    false,
		// pivot_root won't work for initramfs, see
		// Documentation/filesystem/ramfs-rootfs-initramfs.txt
		noPivotRoot:    (fsType == typeRootfs),
		subreaper:      r,
		pciDeviceMap:   make(map[string]string),
		deviceWatchers: make(map[string](chan string)),
		storages:       make(map[string]*sandboxStorage),
		stopServer:     make(chan struct{}),
	}

	rootSpan, rootContext, err = setupTracing(agentName)
	if err != nil {
		return fmt.Errorf("failed to setup tracing: %v", err)
	}

	if err = s.initLogger(rootContext); err != nil {
		return fmt.Errorf("failed to setup logger: %v", err)
	}

	if err := setupDebugConsole(rootContext, debugConsolePath); err != nil {
		agentLog.WithError(err).Error("failed to setup debug console")
	}

	// Set the sandbox context now that the context contains the tracing
	// information.
	s.ctx = rootContext

	if err = s.setupSignalHandler(); err != nil {
		return fmt.Errorf("failed to setup signal handler: %v", err)
	}

	if err = s.handleLocalhost(); err != nil {
		return fmt.Errorf("failed to handle localhost: %v", err)
	}

	// Check for vsock vs serial. This will fill the sandbox structure with
	// information about the channel.
	if err = s.initChannel(); err != nil {
		return fmt.Errorf("failed to setup channels: %v", err)
	}

	// Start gRPC server.
	s.startGRPC()

	go s.waitForStopServer()

	go s.listenToUdevEvents()

	s.wg.Wait()

	if !tracing {
		// If tracing is not enabled, the agent should continue to run
		// until the VM is killed by the runtime.
		agentLog.Debug("waiting to be killed")
		syscall.Pause()
	}

	// Report any traces before shutdown. This is not required if the
	// client is using StartTracing()/StopTracing().
	if !stopTracingCalled {
		stopTracing(rootContext)
	}

	return nil
}

func main() {
	defer handlePanic()

	err := realMain()
	if err != nil {
		agentLog.WithError(err).Error("agent failed")
		os.Exit(1)
	}

	agentLog.Debug("agent exiting")

	os.Exit(0)
}
