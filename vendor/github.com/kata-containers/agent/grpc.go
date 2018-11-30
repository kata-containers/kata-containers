//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"syscall"
	"time"

	gpb "github.com/gogo/protobuf/types"
	"github.com/kata-containers/agent/pkg/types"
	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/opencontainers/runc/libcontainer"
	"github.com/opencontainers/runc/libcontainer/configs"
	"github.com/opencontainers/runc/libcontainer/seccomp"
	"github.com/opencontainers/runc/libcontainer/specconv"
	"github.com/opencontainers/runc/libcontainer/utils"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"golang.org/x/net/context"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

type agentGRPC struct {
	sandbox *sandbox
	version string
}

// CPU and Memory hotplug
const (
	cpuRegexpPattern = "cpu[0-9]*"
	memRegexpPattern = "memory[0-9]*"
)

var (
	sysfsCPUOnlinePath       = "/sys/devices/system/cpu"
	sysfsMemOnlinePath       = "/sys/devices/system/memory"
	sysfsMemoryBlockSizePath = "/sys/devices/system/memory/block_size_bytes"
	sysfsConnectedCPUsPath   = filepath.Join(sysfsCPUOnlinePath, "online")
)

type onlineResource struct {
	sysfsOnlinePath string
	regexpPattern   string
}

type cookie map[string]bool

var emptyResp = &gpb.Empty{}

const onlineCPUMemWaitTime = 100 * time.Millisecond

const onlineCPUMaxTries = 10

const cpusetMode = 0644

// handleError will log the specified error if wait is false
func handleError(wait bool, err error) error {
	if !wait {
		agentLog.WithError(err).Error()
	}

	return err
}

// Online resources, nbResources specifies the maximum number of resources to online.
// If nbResources is <= 0 then there is no limit and all resources are connected.
// Returns the number of resources connected.
func onlineResources(resource onlineResource, nbResources int32) (uint32, error) {
	files, err := ioutil.ReadDir(resource.sysfsOnlinePath)
	if err != nil {
		return 0, err
	}

	var count uint32
	for _, file := range files {
		matched, err := regexp.MatchString(resource.regexpPattern, file.Name())
		if err != nil {
			return count, err
		}

		if !matched {
			continue
		}

		onlinePath := filepath.Join(resource.sysfsOnlinePath, file.Name(), "online")
		status, err := ioutil.ReadFile(onlinePath)
		if err != nil {
			// resource cold plugged
			continue
		}

		if strings.Trim(string(status), "\n\t ") == "0" {
			if err := ioutil.WriteFile(onlinePath, []byte("1"), 0600); err != nil {
				agentLog.WithField("online-path", onlinePath).WithError(err).Errorf("Could not online resource")
				continue
			}
			count++
			if nbResources > 0 && count == uint32(nbResources) {
				return count, nil
			}
		}
	}

	return count, nil
}

func onlineCPUResources(nbCpus uint32) error {
	resource := onlineResource{
		sysfsOnlinePath: sysfsCPUOnlinePath,
		regexpPattern:   cpuRegexpPattern,
	}

	var count uint32
	for i := uint32(0); i < onlineCPUMaxTries; i++ {
		r, err := onlineResources(resource, int32(nbCpus-count))
		if err != nil {
			return err
		}
		count += r
		if count == nbCpus {
			return nil
		}
		time.Sleep(onlineCPUMemWaitTime)
	}

	return fmt.Errorf("only %d of %d were connected", count, nbCpus)
}

func onlineMemResources() error {
	resource := onlineResource{
		sysfsOnlinePath: sysfsMemOnlinePath,
		regexpPattern:   memRegexpPattern,
	}

	_, err := onlineResources(resource, -1)
	return err
}

// updates container's cpuset cgroups visiting each sub-directory in cgroupPath and writing
// newCpuset in the cpuset.cpus file, cookies are used for performance reasons in order to
// don't update a cgroup twice.
func updateContainerCpuset(cgroupPath string, newCpuset string, cookies cookie) error {
	// Each cpuset cgroup MUST BE updated with the actual number of vCPUs.
	cpusetPath := cgroupCpusetPath
	cgroupsPaths := strings.Split(cgroupPath, "/")
	for _, path := range cgroupsPaths {
		// Skip if empty.
		if path == "" {
			continue
		}

		cpusetPath = filepath.Join(cpusetPath, path)

		// check if the cgroup was already updated.
		if cookies[cpusetPath] == true {
			agentLog.WithField("path", cpusetPath).Debug("cpuset cgroup already updated")
			continue
		}

		// Don't use c.container.Set because of it will modify container's config.
		// c.container.Set MUST BE used only on update.
		cpusetCpusPath := filepath.Join(cpusetPath, "cpuset.cpus")
		agentLog.WithField("path", cpusetPath).Debug("updating cpuset cgroup")
		if err := ioutil.WriteFile(cpusetCpusPath, []byte(newCpuset), cpusetMode); err != nil {
			return fmt.Errorf("Could not update cpuset cgroup '%s': %v", newCpuset, err)
		}

		// add cgroup path to the cookies.
		cookies[cpusetPath] = true
	}

	return nil
}

func (a *agentGRPC) onlineCPUMem(req *pb.OnlineCPUMemRequest) error {
	if req.NbCpus == 0 && req.CpuOnly {
		return handleError(req.Wait, fmt.Errorf("requested number of CPUs '%d' must be greater than 0", req.NbCpus))
	}

	// we are going to update the containers of the sandbox, we have to lock it
	a.sandbox.Lock()
	defer a.sandbox.Unlock()

	if req.NbCpus > 0 {
		agentLog.WithField("vcpus-to-connect", req.NbCpus).Debug("connecting vCPUs")
		if err := onlineCPUResources(req.NbCpus); err != nil {
			return handleError(req.Wait, err)
		}
	}

	if !req.CpuOnly {
		if err := onlineMemResources(); err != nil {
			return handleError(req.Wait, err)
		}
	}

	// At this point all CPUs have been connected, we need to know
	// the actual range of CPUs
	cpus, err := ioutil.ReadFile(sysfsConnectedCPUsPath)
	if err != nil {
		return handleError(req.Wait, fmt.Errorf("Could not get the actual range of connected CPUs: %v", err))
	}
	connectedCpus := strings.Trim(string(cpus), "\t\n ")
	agentLog.WithField("range-of-vcpus", connectedCpus).Debug("connecting vCPUs")

	cookies := make(cookie)

	// Now that we know the actual range of connected CPUs, we need to iterate over
	// all containers an update each cpuset cgroup. This is not required in docker
	// containers since they don't hot add/remove CPUs.
	for _, c := range a.sandbox.containers {
		agentLog.WithField("container", c.container.ID()).Debug("updating cpuset cgroup")
		contConfig := c.container.Config()
		cgroupPath := contConfig.Cgroups.Path

		// In order to avoid issues updating the container cpuset cgroup, its cpuset cgroup *parents*
		// MUST BE updated, otherwise we'll get next errors:
		// - write /sys/fs/cgroup/cpuset/XXXXX/cpuset.cpus: permission denied
		// - write /sys/fs/cgroup/cpuset/XXXXX/cpuset.cpus: device or resource busy
		// NOTE: updating container cpuset cgroup *parents* won't affect container cpuset cgroup, for example if container cpuset cgroup has "0"
		// and its cpuset cgroup *parents* have "0-5", the container will be able to use only the CPU 0.
		if contConfig.Cgroups.Resources.CpusetCpus != "" {
			agentLog.WithField("cpuset", contConfig.Cgroups.Resources.CpusetCpus).Debug("updating container cpuset cgroup parents")
			// remove container cgroup directory
			cgroupPath = filepath.Dir(cgroupPath)
		}

		if err := updateContainerCpuset(cgroupPath, connectedCpus, cookies); err != nil {
			return handleError(req.Wait, err)
		}
	}

	return nil
}

func setConsoleCarriageReturn(fd int) error {
	termios, err := unix.IoctlGetTermios(fd, unix.TCGETS)
	if err != nil {
		return err
	}

	termios.Oflag |= unix.ONLCR

	return unix.IoctlSetTermios(fd, unix.TCSETS, termios)
}

func buildProcess(agentProcess *pb.Process, procID string) (*process, error) {
	user := agentProcess.User.Username
	if user == "" {
		// We can specify the user and the group separated by ":"
		user = fmt.Sprintf("%d:%d", agentProcess.User.UID, agentProcess.User.GID)
	}

	additionalGids := []string{}
	for _, gid := range agentProcess.User.AdditionalGids {
		additionalGids = append(additionalGids, fmt.Sprintf("%d", gid))
	}

	proc := &process{
		id: procID,
		process: libcontainer.Process{
			Cwd:              agentProcess.Cwd,
			Args:             agentProcess.Args,
			Env:              agentProcess.Env,
			User:             user,
			AdditionalGroups: additionalGids,
		},
	}

	if agentProcess.Terminal {
		parentSock, childSock, err := utils.NewSockPair("console")
		if err != nil {
			return nil, err
		}

		proc.process.ConsoleSocket = childSock
		proc.consoleSock = parentSock

		epoller, err := newEpoller()
		if err != nil {
			return nil, err
		}

		proc.epoller = epoller

		return proc, nil
	}

	rStdin, wStdin, err := os.Pipe()
	if err != nil {
		return nil, err
	}

	rStdout, wStdout, err := os.Pipe()
	if err != nil {
		return nil, err
	}

	rStderr, wStderr, err := os.Pipe()
	if err != nil {
		return nil, err
	}

	proc.process.Stdin = rStdin
	proc.process.Stdout = wStdout
	proc.process.Stderr = wStderr

	proc.stdin = wStdin
	proc.stdout = rStdout
	proc.stderr = rStderr

	return proc, nil
}

func (a *agentGRPC) Check(ctx context.Context, req *pb.CheckRequest) (*pb.HealthCheckResponse, error) {
	return &pb.HealthCheckResponse{Status: pb.HealthCheckResponse_SERVING}, nil
}

func (a *agentGRPC) Version(ctx context.Context, req *pb.CheckRequest) (*pb.VersionCheckResponse, error) {
	return &pb.VersionCheckResponse{
		GrpcVersion:  pb.APIVersion,
		AgentVersion: a.version,
	}, nil

}

func (a *agentGRPC) getContainer(cid string) (*container, error) {
	if a.sandbox.running == false {
		return nil, grpcStatus.Error(codes.FailedPrecondition, "Sandbox not started")
	}

	ctr, err := a.sandbox.getContainer(cid)
	if err != nil {
		return nil, err
	}

	return ctr, nil
}

// Shared function between CreateContainer and ExecProcess, because those expect
// a process to be run.
func (a *agentGRPC) execProcess(ctr *container, proc *process, createContainer bool) (err error) {
	if ctr == nil {
		return grpcStatus.Error(codes.InvalidArgument, "Container cannot be nil")
	}

	if proc == nil {
		return grpcStatus.Error(codes.InvalidArgument, "Process cannot be nil")
	}

	// This lock is very important to avoid any race with reaper.reap().
	// Indeed, if we don't lock this here, we could potentially get the
	// SIGCHLD signal before the channel has been created, meaning we will
	// miss the opportunity to get the exit code, leading WaitProcess() to
	// wait forever on the new channel.
	// This lock has to be taken before we run the new process.
	a.sandbox.subreaper.lock()
	defer a.sandbox.subreaper.unlock()

	if createContainer {
		err = ctr.container.Start(&proc.process)
	} else {
		err = ctr.container.Run(&(proc.process))
	}
	if err != nil {
		return grpcStatus.Errorf(codes.Internal, "Could not run process: %v", err)
	}

	// Get process PID
	pid, err := proc.process.Pid()
	if err != nil {
		return err
	}

	proc.exitCodeCh = make(chan int, 1)

	// Create process channel to allow WaitProcess to wait on it.
	// This channel is buffered so that reaper.reap() will not
	// block until WaitProcess listen onto this channel.
	a.sandbox.subreaper.setExitCodeCh(pid, proc.exitCodeCh)

	return nil
}

// Shared function between CreateContainer and ExecProcess, because those expect
// the console to be properly setup after the process has been started.
func (a *agentGRPC) postExecProcess(ctr *container, proc *process) error {
	if ctr == nil {
		return grpcStatus.Error(codes.InvalidArgument, "Container cannot be nil")
	}

	if proc == nil {
		return grpcStatus.Error(codes.InvalidArgument, "Process cannot be nil")
	}

	defer proc.closePostStartFDs()

	// Setup terminal if enabled.
	if proc.consoleSock != nil {
		termMaster, err := utils.RecvFd(proc.consoleSock)
		if err != nil {
			return err
		}

		if err := setConsoleCarriageReturn(int(termMaster.Fd())); err != nil {
			return err
		}

		proc.termMaster = termMaster

		// Get process PID
		pid, err := proc.process.Pid()
		if err != nil {
			return err
		}
		a.sandbox.subreaper.setEpoller(pid, proc.epoller)

		if err = proc.epoller.add(proc.termMaster); err != nil {
			return err
		}
	}

	ctr.setProcess(proc)

	return nil
}

// This function updates the container namespaces configuration based on the
// sandbox information. When the sandbox is created, it can be setup in a way
// that all containers will share some specific namespaces. This is the agent
// responsibility to create those namespaces so that they can be shared across
// several containers.
// If the sandbox has not been setup to share namespaces, then we assume all
// containers will be started in their own new namespace.
// The value of a.sandbox.sharedPidNs.path will always override the namespace
// path set by the spec, since we will always ignore it. Indeed, it makes no
// sense to rely on the namespace path provided by the host since namespaces
// are different inside the guest.
func (a *agentGRPC) updateContainerConfigNamespaces(config *configs.Config, ctr *container) {
	var ipcNs, utsNs bool

	for idx, ns := range config.Namespaces {
		if ns.Type == configs.NEWIPC {
			config.Namespaces[idx].Path = a.sandbox.sharedIPCNs.path
			ipcNs = true
		}

		if ns.Type == configs.NEWUTS {
			config.Namespaces[idx].Path = a.sandbox.sharedUTSNs.path
			utsNs = true
		}
	}

	if !ipcNs {
		newIPCNs := configs.Namespace{
			Type: configs.NEWIPC,
			Path: a.sandbox.sharedIPCNs.path,
		}
		config.Namespaces = append(config.Namespaces, newIPCNs)
	}

	if !utsNs {
		newUTSNs := configs.Namespace{
			Type: configs.NEWUTS,
			Path: a.sandbox.sharedUTSNs.path,
		}
		config.Namespaces = append(config.Namespaces, newUTSNs)
	}

	// Update PID namespace.
	var pidNsPath string

	// Use shared pid ns if useSandboxPidns has been set in either
	// the CreateSandbox request or CreateContainer request.
	// Else set this to empty string so that a new pid namespace is
	// created for the container.
	if ctr.useSandboxPidNs || a.sandbox.sandboxPidNs {
		pidNsPath = a.sandbox.sharedPidNs.path
	} else {
		pidNsPath = ""
	}

	newPidNs := configs.Namespace{
		Type: configs.NEWPID,
		Path: pidNsPath,
	}
	config.Namespaces = append(config.Namespaces, newPidNs)
}

func (a *agentGRPC) updateContainerConfigPrivileges(spec *specs.Spec, config *configs.Config) error {
	if spec == nil || spec.Process == nil {
		// Don't throw an error in case the Spec does not contain any
		// information about NoNewPrivileges.
		return nil
	}

	// Add the value for NoNewPrivileges option.
	config.NoNewPrivileges = spec.Process.NoNewPrivileges

	return nil
}

func (a *agentGRPC) updateContainerConfig(spec *specs.Spec, config *configs.Config, ctr *container) error {
	a.updateContainerConfigNamespaces(config, ctr)
	return a.updateContainerConfigPrivileges(spec, config)
}

// rollbackFailingContainerCreation rolls back important steps that might have
// been performed before the container creation failed.
// - Destroy the container created by libcontainer
// - Delete the container from the agent internal map
// - Unmount all mounts related to this container
func (a *agentGRPC) rollbackFailingContainerCreation(ctr *container) {
	if ctr.container != nil {
		ctr.container.Destroy()
	}

	a.sandbox.deleteContainer(ctr.id)

	if err := removeMounts(ctr.mounts); err != nil {
		agentLog.WithError(err).Error("rollback failed removeMounts()")
	}
}

func (a *agentGRPC) finishCreateContainer(ctr *container, req *pb.CreateContainerRequest, config *configs.Config) (resp *gpb.Empty, err error) {
	containerPath := filepath.Join("/tmp/libcontainer", a.sandbox.id)
	factory, err := libcontainer.New(containerPath, libcontainer.Cgroupfs)
	if err != nil {
		return emptyResp, err
	}

	ctr.container, err = factory.Create(req.ContainerId, config)
	if err != nil {
		return emptyResp, err
	}
	ctr.config = *config

	ctr.initProcess, err = buildProcess(req.OCI.Process, req.ExecId)
	if err != nil {
		return emptyResp, err
	}

	if err = a.execProcess(ctr, ctr.initProcess, true); err != nil {
		return emptyResp, err
	}

	if err := a.updateSharedPidNs(ctr); err != nil {
		return emptyResp, err
	}

	return emptyResp, a.postExecProcess(ctr, ctr.initProcess)
}

func (a *agentGRPC) CreateContainer(ctx context.Context, req *pb.CreateContainerRequest) (resp *gpb.Empty, err error) {
	if err := a.createContainerChecks(req); err != nil {
		return emptyResp, err
	}

	// re-scan PCI bus
	// looking for hidden devices
	if err = rescanPciBus(); err != nil {
		agentLog.WithError(err).Warn("Could not rescan PCI bus")
	}

	// Some devices need some extra processing (the ones invoked with
	// --device for instance), and that's what this call is doing. It
	// updates the devices listed in the OCI spec, so that they actually
	// match real devices inside the VM. This step is necessary since we
	// cannot predict everything from the caller.
	if err = addDevices(req.Devices, req.OCI, a.sandbox); err != nil {
		return emptyResp, err
	}

	// Both rootfs and volumes (invoked with --volume for instance) will
	// be processed the same way. The idea is to always mount any provided
	// storage to the specified MountPoint, so that it will match what's
	// inside oci.Mounts.
	// After all those storages have been processed, no matter the order
	// here, the agent will rely on libcontainer (using the oci.Mounts
	// list) to bind mount all of them inside the container.
	mountList, err := addStorages(req.Storages, a.sandbox)
	if err != nil {
		return emptyResp, err
	}

	ctr := &container{
		id:              req.ContainerId,
		processes:       make(map[string]*process),
		mounts:          mountList,
		useSandboxPidNs: req.SandboxPidns,
	}

	a.sandbox.setContainer(req.ContainerId, ctr)

	// In case the container creation failed, make sure we cleanup
	// properly by rolling back the actions previously performed.
	defer func() {
		if err != nil {
			a.rollbackFailingContainerCreation(ctr)
		}
	}()

	// Convert the spec to an actual OCI specification structure.
	ociSpec, err := pb.GRPCtoOCI(req.OCI)
	if err != nil {
		return emptyResp, err
	}

	if a.sandbox.guestHooksPresent {
		// Add any custom OCI hooks to the spec
		a.sandbox.addGuestHooks(ociSpec)

		// write the OCI spec to a file so that hooks can read it
		err = writeSpecToFile(ociSpec)
		if err != nil {
			return emptyResp, err
		}

		// Change cwd because libcontainer assumes the bundle path is the cwd:
		// https://github.com/opencontainers/runc/blob/v1.0.0-rc5/libcontainer/specconv/spec_linux.go#L157
		oldcwd, err := changeToBundlePath(ociSpec)
		if err != nil {
			return emptyResp, err
		}
		defer os.Chdir(oldcwd)
	}

	// Convert the OCI specification into a libcontainer configuration.
	config, err := specconv.CreateLibcontainerConfig(&specconv.CreateOpts{
		CgroupName:   req.ContainerId,
		NoNewKeyring: true,
		Spec:         ociSpec,
		NoPivotRoot:  a.sandbox.noPivotRoot,
	})
	if err != nil {
		return emptyResp, err
	}

	// Update libcontainer configuration for specific cases not handled
	// by the specconv converter.
	if err = a.updateContainerConfig(ociSpec, config, ctr); err != nil {
		return emptyResp, err
	}

	return a.finishCreateContainer(ctr, req, config)
}

func (a *agentGRPC) createContainerChecks(req *pb.CreateContainerRequest) (err error) {
	if a.sandbox.running == false {
		return grpcStatus.Error(codes.FailedPrecondition, "Sandbox not started, impossible to run a new container")
	}

	if _, err = a.sandbox.getContainer(req.ContainerId); err == nil {
		return grpcStatus.Errorf(codes.AlreadyExists, "Container %s already exists, impossible to create", req.ContainerId)
	}

	if a.pidNsExists(req.OCI) {
		return grpcStatus.Errorf(codes.FailedPrecondition, "Unexpected PID namespace received for container %s, should have been cleared out", req.ContainerId)
	}

	return nil
}

func (a *agentGRPC) pidNsExists(grpcSpec *pb.Spec) bool {
	if grpcSpec.Linux != nil {
		for _, n := range grpcSpec.Linux.Namespaces {
			if n.Type == string(configs.NEWPID) {
				return true
			}
		}
	}
	return false
}

func (a *agentGRPC) updateSharedPidNs(ctr *container) error {
	// Populate the shared pid path only if this is an infra container and
	// SandboxPidns has not been passed in the CreateSandbox request.
	// This means a  separate pause process has not been created. We treat the
	// first container created as the infra container in that case
	// and use its pid namespace in case pid namespace needs to be shared.
	if !a.sandbox.sandboxPidNs && len(a.sandbox.containers) == 1 {
		pid, err := ctr.initProcess.process.Pid()
		if err != nil {
			return err
		}

		a.sandbox.sharedPidNs.path = fmt.Sprintf("/proc/%d/ns/pid", pid)
	}

	return nil
}

func (a *agentGRPC) StartContainer(ctx context.Context, req *pb.StartContainerRequest) (*gpb.Empty, error) {
	ctr, err := a.getContainer(req.ContainerId)
	if err != nil {
		return emptyResp, err
	}

	status, err := ctr.container.Status()
	if err != nil {
		return nil, err
	}

	if status != libcontainer.Created {
		return nil, grpcStatus.Errorf(codes.FailedPrecondition, "Container %s status %s, should be %s", req.ContainerId, status.String(), libcontainer.Created.String())
	}

	if err := ctr.container.Exec(); err != nil {
		return emptyResp, err
	}

	return emptyResp, nil
}

func (a *agentGRPC) ExecProcess(ctx context.Context, req *pb.ExecProcessRequest) (*gpb.Empty, error) {
	ctr, err := a.getContainer(req.ContainerId)
	if err != nil {
		return emptyResp, err
	}

	status, err := ctr.container.Status()
	if err != nil {
		return nil, err
	}

	if status == libcontainer.Stopped {
		return nil, grpcStatus.Errorf(codes.FailedPrecondition, "Cannot exec in stopped container %s", req.ContainerId)
	}

	proc, err := buildProcess(req.Process, req.ExecId)
	if err != nil {
		return emptyResp, err
	}

	if err := a.execProcess(ctr, proc, false); err != nil {
		return emptyResp, err
	}

	return emptyResp, a.postExecProcess(ctr, proc)
}

func (a *agentGRPC) SignalProcess(ctx context.Context, req *pb.SignalProcessRequest) (*gpb.Empty, error) {
	if a.sandbox.running == false {
		return emptyResp, grpcStatus.Error(codes.FailedPrecondition, "Sandbox not started, impossible to signal the container")
	}

	ctr, err := a.sandbox.getContainer(req.ContainerId)
	if err != nil {
		return emptyResp, grpcStatus.Errorf(codes.FailedPrecondition, "Could not signal process %s: %v", req.ExecId, err)
	}

	status, err := ctr.container.Status()
	if err != nil {
		return emptyResp, err
	}

	signal := syscall.Signal(req.Signal)

	if status == libcontainer.Stopped {
		agentLog.WithFields(logrus.Fields{
			"containerID": req.ContainerId,
			"signal":      signal.String(),
		}).Info("discarding signal as container stopped")
		return emptyResp, nil
	}

	// If the exec ID provided is empty, let's apply the signal to all
	// processes inside the container.
	// If the process is the container process, let's use the container
	// API for that.
	// Frozen processes are thawed when `all` is true, allowing them to receive and process signals.
	if req.ExecId == "" || status == libcontainer.Paused {
		return emptyResp, ctr.container.Signal(signal, true)
	} else if ctr.initProcess.id == req.ExecId {
		return emptyResp, ctr.container.Signal(signal, false)
	}

	proc, err := ctr.getProcess(req.ExecId)
	if err != nil {
		return emptyResp, grpcStatus.Errorf(grpcStatus.Convert(err).Code(), "Could not signal process: %v", err)
	}

	if err := proc.process.Signal(signal); err != nil {
		return emptyResp, err
	}

	return emptyResp, nil
}

func (a *agentGRPC) WaitProcess(ctx context.Context, req *pb.WaitProcessRequest) (*pb.WaitProcessResponse, error) {
	proc, ctr, err := a.sandbox.getProcess(req.ContainerId, req.ExecId)
	if err != nil {
		return &pb.WaitProcessResponse{}, err
	}

	defer proc.Do(func() {
		proc.closePostExitFDs()
		ctr.deleteProcess(proc.id)
	})

	// Using helper function wait() to deal with the subreaper.
	libContProcess := (*reaperLibcontainerProcess)(&(proc.process))
	exitCode, err := a.sandbox.subreaper.wait(proc.exitCodeCh, libContProcess)
	if err != nil {
		return &pb.WaitProcessResponse{}, err
	}
	//refill the exitCodeCh with the exitcode which can be read out
	//by another WaitProcess(). Since this channel isn't be closed,
	//here the refill will always success and it will be free by GC
	//once the process exits.
	proc.exitCodeCh <- exitCode

	return &pb.WaitProcessResponse{
		Status: int32(exitCode),
	}, nil
}

func getPIDIndex(title string) int {
	// looking for PID field in ps title
	fields := strings.Fields(title)
	for i, f := range fields {
		if f == "PID" {
			return i
		}
	}
	return -1
}

func (a *agentGRPC) ListProcesses(ctx context.Context, req *pb.ListProcessesRequest) (*pb.ListProcessesResponse, error) {
	resp := &pb.ListProcessesResponse{}

	c, err := a.sandbox.getContainer(req.ContainerId)
	if err != nil {
		return resp, err
	}

	// Get the list of processes that are running inside the containers.
	// the PIDs match with the system PIDs, not with container's namespace
	pids, err := c.container.Processes()
	if err != nil {
		return resp, err
	}

	switch req.Format {
	case "table":
	case "json":
		resp.ProcessList, err = json.Marshal(pids)
		return resp, err
	default:
		return resp, fmt.Errorf("invalid format option")
	}

	psArgs := req.Args
	if len(psArgs) == 0 {
		psArgs = []string{"-ef"}
	}

	// All container's processes are visibles from agent's namespace.
	// pids already contains the list of processes that are running
	// inside a container, now we have to use that list to filter
	// ps output and return just container's processes
	cmd := exec.Command("ps", psArgs...)
	output, err := a.sandbox.subreaper.combinedOutput(cmd)
	if err != nil {
		return nil, fmt.Errorf("%s: %s", err, output)
	}

	lines := strings.Split(string(output), "\n")

	pidIndex := getPIDIndex(lines[0])

	// PID field not found
	if pidIndex == -1 {
		return nil, fmt.Errorf("failed to find PID field in ps output")
	}

	// append title
	var result bytes.Buffer

	result.WriteString(lines[0] + "\n")

	for _, line := range lines[1:] {
		if len(line) == 0 {
			continue
		}
		fields := strings.Fields(line)
		if pidIndex >= len(fields) {
			return nil, fmt.Errorf("missing PID field: %s", line)
		}

		p, err := strconv.Atoi(fields[pidIndex])
		if err != nil {
			return nil, fmt.Errorf("failed to convert pid to int: %s", fields[pidIndex])
		}

		// appends pid line
		for _, pid := range pids {
			if pid == p {
				result.WriteString(line + "\n")
				break
			}
		}
	}

	resp.ProcessList = result.Bytes()
	return resp, nil
}

func (a *agentGRPC) UpdateContainer(ctx context.Context, req *pb.UpdateContainerRequest) (*gpb.Empty, error) {
	if req.Resources == nil {
		return emptyResp, fmt.Errorf("Resources in the request are nil")
	}

	c, err := a.sandbox.getContainer(req.ContainerId)
	if err != nil {
		return emptyResp, err
	}

	// c.container.Config returns a copy of non-pointer members
	// in configs.Config, configs.Config.Cgroup is a pointer hence
	// if it is modified, the container cgroup is modifed too and
	// c.container.Set won't be able to rollback in case of failure.
	contConfig := c.container.Config()
	var resources configs.Resources
	if contConfig.Cgroups != nil && contConfig.Cgroups.Resources != nil {
		resources = *contConfig.Cgroups.Resources
	}

	// Update the value
	if req.Resources.BlockIO != nil {
		resources.BlkioWeight = uint16(req.Resources.BlockIO.Weight)
	}

	if req.Resources.CPU != nil {
		resources.CpuPeriod = req.Resources.CPU.Period
		resources.CpuQuota = req.Resources.CPU.Quota
		resources.CpuShares = req.Resources.CPU.Shares
		resources.CpuRtPeriod = req.Resources.CPU.RealtimePeriod
		resources.CpuRtRuntime = req.Resources.CPU.RealtimeRuntime
		resources.CpusetCpus = req.Resources.CPU.Cpus
		resources.CpusetMems = req.Resources.CPU.Mems
	}

	if req.Resources.Memory != nil {
		resources.KernelMemory = req.Resources.Memory.Kernel
		resources.KernelMemoryTCP = req.Resources.Memory.KernelTCP
		resources.Memory = req.Resources.Memory.Limit
		resources.MemoryReservation = req.Resources.Memory.Reservation
		resources.MemorySwap = req.Resources.Memory.Swap
	}

	if req.Resources.Pids != nil {
		resources.PidsLimit = req.Resources.Pids.Limit
	}

	// cpuset is a special case where container's cpuset cgroup MUST BE updated
	if resources.CpusetCpus != "" {
		cookies := make(cookie)
		if err = updateContainerCpuset(contConfig.Cgroups.Path, resources.CpusetCpus, cookies); err != nil {
			agentLog.WithError(err).Warn("Could not update container cpuset cgroup")
		}
	}

	// Create a copy of container's cgroup, if c.container.Set fails,
	// configuration won't be modified and it will be able to rollback
	// to the original container cgroup configuration.
	config := contConfig
	var cgroupsCopy configs.Cgroup
	if contConfig.Cgroups != nil {
		cgroupsCopy = *contConfig.Cgroups
	}
	cgroupsCopy.Resources = &resources
	config.Cgroups = &cgroupsCopy
	return emptyResp, c.container.Set(config)
}

func (a *agentGRPC) StatsContainer(ctx context.Context, req *pb.StatsContainerRequest) (*pb.StatsContainerResponse, error) {
	c, err := a.sandbox.getContainer(req.ContainerId)
	if err != nil {
		return nil, err
	}

	stats, err := c.container.Stats()
	if err != nil {
		return nil, err
	}

	data, err := json.Marshal(stats.CgroupStats)
	if err != nil {
		return nil, err
	}

	var cgroupStats pb.CgroupStats
	err = json.Unmarshal(data, &cgroupStats)
	if err != nil {
		return nil, err
	}
	resp := &pb.StatsContainerResponse{
		CgroupStats: &cgroupStats,
	}

	return resp, nil

}

func (a *agentGRPC) PauseContainer(ctx context.Context, req *pb.PauseContainerRequest) (*gpb.Empty, error) {
	c, err := a.sandbox.getContainer(req.ContainerId)
	if err != nil {
		return emptyResp, err
	}

	a.sandbox.Lock()
	defer a.sandbox.Unlock()

	return emptyResp, c.container.Pause()
}

func (a *agentGRPC) ResumeContainer(ctx context.Context, req *pb.ResumeContainerRequest) (*gpb.Empty, error) {
	c, err := a.sandbox.getContainer(req.ContainerId)
	if err != nil {
		return emptyResp, err
	}

	a.sandbox.Lock()
	defer a.sandbox.Unlock()

	return emptyResp, c.container.Resume()
}

func (a *agentGRPC) RemoveContainer(ctx context.Context, req *pb.RemoveContainerRequest) (*gpb.Empty, error) {
	ctr, err := a.sandbox.getContainer(req.ContainerId)
	if err != nil {
		return emptyResp, err
	}

	timeout := int(req.Timeout)

	a.sandbox.Lock()
	defer a.sandbox.Unlock()

	if timeout == 0 {
		if err := ctr.removeContainer(); err != nil {
			return emptyResp, err
		}

		// Find the sandbox storage used by this container
		for _, path := range ctr.mounts {
			if _, ok := a.sandbox.storages[path]; ok {
				if err := a.sandbox.unsetAndRemoveSandboxStorage(path); err != nil {
					return emptyResp, err
				}
			}
		}
	} else {
		done := make(chan error)
		go func() {
			if err := ctr.removeContainer(); err != nil {
				done <- err
				close(done)
				return
			}

			//Find the sandbox storage used by this container
			for _, path := range ctr.mounts {
				if _, ok := a.sandbox.storages[path]; ok {
					if err := a.sandbox.unsetAndRemoveSandboxStorage(path); err != nil {
						done <- err
						close(done)
						return
					}
				}
			}
			close(done)
		}()

		select {
		case err := <-done:
			if err != nil {
				return emptyResp, err
			}
		case <-time.After(time.Duration(req.Timeout) * time.Second):
			return emptyResp, grpcStatus.Errorf(codes.DeadlineExceeded, "Timeout reached after %ds", timeout)
		}
	}

	delete(a.sandbox.containers, ctr.id)

	return emptyResp, nil
}

func (a *agentGRPC) WriteStdin(ctx context.Context, req *pb.WriteStreamRequest) (*pb.WriteStreamResponse, error) {
	proc, _, err := a.sandbox.getProcess(req.ContainerId, req.ExecId)
	if err != nil {
		return &pb.WriteStreamResponse{}, err
	}

	proc.RLock()
	defer proc.RUnlock()
	stdinClosed := proc.stdinClosed

	// Ignore this call to WriteStdin() if STDIN has already been closed
	// earlier.
	if stdinClosed {
		return &pb.WriteStreamResponse{}, nil
	}

	var file *os.File
	if proc.termMaster != nil {
		file = proc.termMaster
	} else {
		file = proc.stdin
	}

	n, err := file.Write(req.Data)
	if err != nil {
		return &pb.WriteStreamResponse{}, err
	}

	return &pb.WriteStreamResponse{
		Len: uint32(n),
	}, nil
}

func (a *agentGRPC) ReadStdout(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	data, err := a.sandbox.readStdio(req.ContainerId, req.ExecId, int(req.Len), true)
	if err != nil {
		return &pb.ReadStreamResponse{}, err
	}

	return &pb.ReadStreamResponse{
		Data: data,
	}, nil
}

func (a *agentGRPC) ReadStderr(ctx context.Context, req *pb.ReadStreamRequest) (*pb.ReadStreamResponse, error) {
	data, err := a.sandbox.readStdio(req.ContainerId, req.ExecId, int(req.Len), false)
	if err != nil {
		return &pb.ReadStreamResponse{}, err
	}

	return &pb.ReadStreamResponse{
		Data: data,
	}, nil
}

func (a *agentGRPC) CloseStdin(ctx context.Context, req *pb.CloseStdinRequest) (*gpb.Empty, error) {
	proc, _, err := a.sandbox.getProcess(req.ContainerId, req.ExecId)
	if err != nil {
		return emptyResp, err
	}

	// If stdin is nil, which can be the case when using a terminal,
	// there is nothing to do.
	if proc.stdin == nil {
		return emptyResp, nil
	}

	proc.Lock()
	defer proc.Unlock()

	if err := proc.stdin.Close(); err != nil {
		return emptyResp, err
	}

	proc.stdinClosed = true

	return emptyResp, nil
}

func (a *agentGRPC) TtyWinResize(ctx context.Context, req *pb.TtyWinResizeRequest) (*gpb.Empty, error) {
	proc, _, err := a.sandbox.getProcess(req.ContainerId, req.ExecId)
	if err != nil {
		return emptyResp, err
	}

	if proc.termMaster == nil {
		return emptyResp, grpcStatus.Error(codes.FailedPrecondition, "Terminal is not set, impossible to resize it")
	}

	winsize := &unix.Winsize{
		Row: uint16(req.Row),
		Col: uint16(req.Column),
	}

	// Set new terminal size.
	if err := unix.IoctlSetWinsize(int(proc.termMaster.Fd()), unix.TIOCSWINSZ, winsize); err != nil {
		return emptyResp, err
	}

	return emptyResp, nil
}

func (a *agentGRPC) CreateSandbox(ctx context.Context, req *pb.CreateSandboxRequest) (*gpb.Empty, error) {
	if a.sandbox.running == true {
		return emptyResp, grpcStatus.Error(codes.AlreadyExists, "Sandbox already started, impossible to start again")
	}

	a.sandbox.hostname = req.Hostname
	a.sandbox.containers = make(map[string]*container)
	a.sandbox.network.ifaces = make(map[string]*types.Interface)
	a.sandbox.network.dns = req.Dns
	a.sandbox.running = true
	a.sandbox.sandboxPidNs = req.SandboxPidns
	a.sandbox.storages = make(map[string]*sandboxStorage)
	a.sandbox.guestHooks = &specs.Hooks{}
	a.sandbox.guestHooksPresent = false

	if req.GuestHookPath != "" {
		a.sandbox.scanGuestHooks(req.GuestHookPath)
	}

	if req.SandboxId != "" {
		a.sandbox.id = req.SandboxId
		agentLog = agentLog.WithField("sandbox", a.sandbox.id)
	}

	// Set up shared UTS and IPC namespaces
	if err := a.sandbox.setupSharedNamespaces(); err != nil {
		return emptyResp, err
	}

	if req.SandboxPidns {
		if err := a.sandbox.setupSharedPidNs(); err != nil {
			return emptyResp, err
		}
	}

	mountList, err := addStorages(req.Storages, a.sandbox)
	if err != nil {
		return emptyResp, err
	}

	a.sandbox.mounts = mountList

	if err := setupDNS(a.sandbox.network.dns); err != nil {
		return emptyResp, err
	}

	return emptyResp, nil
}

func (a *agentGRPC) DestroySandbox(ctx context.Context, req *pb.DestroySandboxRequest) (*gpb.Empty, error) {
	if a.sandbox.running == false {
		agentLog.Info("Sandbox not started, this is a no-op")
		return emptyResp, nil
	}

	a.sandbox.Lock()

	for key, c := range a.sandbox.containers {
		if err := c.removeContainer(); err != nil {
			return emptyResp, err
		}

		// Find the sandbox storage used by this container
		for _, path := range c.mounts {
			if _, ok := a.sandbox.storages[path]; ok {
				if err := a.sandbox.unsetAndRemoveSandboxStorage(path); err != nil {
					return emptyResp, err
				}
			}
		}
		delete(a.sandbox.containers, key)
	}
	a.sandbox.Unlock()

	if err := a.sandbox.removeNetwork(); err != nil {
		return emptyResp, err
	}

	if err := removeMounts(a.sandbox.mounts); err != nil {
		return emptyResp, err
	}

	if err := a.sandbox.teardownSharedPidNs(); err != nil {
		return emptyResp, err
	}

	if err := a.sandbox.unmountSharedNamespaces(); err != nil {
		return emptyResp, err
	}

	a.sandbox.hostname = ""
	a.sandbox.id = ""
	a.sandbox.containers = make(map[string]*container)
	a.sandbox.running = false
	a.sandbox.network = network{}
	a.sandbox.mounts = []string{}
	a.sandbox.storages = make(map[string]*sandboxStorage)

	// Synchronize the caches on the system. This is needed to ensure
	// there is no pending transactions left before the VM is shut down.
	syscall.Sync()

	return emptyResp, nil
}

func (a *agentGRPC) AddInterface(ctx context.Context, req *pb.AddInterfaceRequest) (*types.Interface, error) {
	return a.sandbox.addInterface(nil, req.Interface)
}

func (a *agentGRPC) UpdateInterface(ctx context.Context, req *pb.UpdateInterfaceRequest) (*types.Interface, error) {
	return a.sandbox.updateInterface(nil, req.Interface)
}

func (a *agentGRPC) RemoveInterface(ctx context.Context, req *pb.RemoveInterfaceRequest) (*types.Interface, error) {
	return a.sandbox.removeInterface(nil, req.Interface)
}

func (a *agentGRPC) UpdateRoutes(ctx context.Context, req *pb.UpdateRoutesRequest) (*pb.Routes, error) {
	return a.sandbox.updateRoutes(nil, req.Routes)
}

func (a *agentGRPC) ListInterfaces(ctx context.Context, req *pb.ListInterfacesRequest) (*pb.Interfaces, error) {
	return a.sandbox.listInterfaces(nil)
}

func (a *agentGRPC) ListRoutes(ctx context.Context, req *pb.ListRoutesRequest) (*pb.Routes, error) {
	return a.sandbox.listRoutes(nil)
}

func (a *agentGRPC) OnlineCPUMem(ctx context.Context, req *pb.OnlineCPUMemRequest) (*gpb.Empty, error) {
	if !req.Wait {
		go a.onlineCPUMem(req)
		return emptyResp, nil
	}

	return emptyResp, a.onlineCPUMem(req)
}

func (a *agentGRPC) ReseedRandomDev(ctx context.Context, req *pb.ReseedRandomDevRequest) (*gpb.Empty, error) {
	return emptyResp, reseedRNG(req.Data)
}

func (a *agentGRPC) GetGuestDetails(ctx context.Context, req *pb.GuestDetailsRequest) (*pb.GuestDetailsResponse, error) {
	var details pb.GuestDetailsResponse
	if req.MemBlockSize {
		data, err := ioutil.ReadFile(sysfsMemoryBlockSizePath)
		if err != nil {
			if os.IsNotExist(err) {
				agentLog.WithField("sysfsMemoryBlockSizePath", sysfsMemoryBlockSizePath).Info("Guest kernel config doesn't support memory hotplug")
			} else {
				return nil, err
			}
		} else {
			if len(data) == 0 {
				return nil, fmt.Errorf("%v is empty", sysfsMemoryBlockSizePath)
			}
			details.MemBlockSizeBytes, err = strconv.ParseUint(string(data[:len(data)-1]), 16, 64)
			if err != nil {
				return nil, err
			}
		}
	}

	details.AgentDetails = a.getAgentDetails(ctx)

	return &details, nil
}

func (a *agentGRPC) haveSeccomp() bool {
	if seccompSupport == "yes" && seccomp.IsEnabled() {
		return true
	}

	return false
}

func (a *agentGRPC) getAgentDetails(ctx context.Context) *pb.AgentDetails {
	details := pb.AgentDetails{
		Version:         version,
		InitDaemon:      os.Getpid() == 1,
		SupportsSeccomp: a.haveSeccomp(),
	}

	for handler := range deviceHandlerList {
		details.DeviceHandlers = append(details.DeviceHandlers, handler)
	}

	for handler := range storageHandlerList {
		details.StorageHandlers = append(details.StorageHandlers, handler)
	}

	return &details
}
