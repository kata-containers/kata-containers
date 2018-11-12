// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"
	"runtime"
	"syscall"

	deviceApi "github.com/kata-containers/runtime/virtcontainers/device/api"
	deviceConfig "github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

func init() {
	runtime.LockOSThread()
}

var virtLog = logrus.WithField("source", "virtcontainers")

// trace creates a new tracing span based on the specified name and parent
// context.
func trace(parent context.Context, name string) (opentracing.Span, context.Context) {
	span, ctx := opentracing.StartSpanFromContext(parent, name)

	// Should not need to be changed (again).
	span.SetTag("source", "virtcontainers")
	span.SetTag("component", "virtcontainers")

	// Should be reset as new subsystems are entered.
	span.SetTag("subsystem", "api")

	return span, ctx
}

// SetLogger sets the logger for virtcontainers package.
func SetLogger(ctx context.Context, logger *logrus.Entry) {
	fields := virtLog.Data
	virtLog = logger.WithFields(fields)

	deviceApi.SetLogger(virtLog)
}

// CreateSandbox is the virtcontainers sandbox creation entry point.
// CreateSandbox creates a sandbox and its containers. It does not start them.
func CreateSandbox(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (VCSandbox, error) {
	span, ctx := trace(ctx, "CreateSandbox")
	defer span.Finish()

	s, err := createSandboxFromConfig(ctx, sandboxConfig, factory)
	if err == nil {
		s.releaseStatelessSandbox()
	}

	return s, err
}

func createSandboxFromConfig(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (*Sandbox, error) {
	span, ctx := trace(ctx, "createSandboxFromConfig")
	defer span.Finish()

	var err error

	// Create the sandbox.
	s, err := createSandbox(ctx, sandboxConfig, factory)
	if err != nil {
		return nil, err
	}

	// Create the sandbox network
	if err = s.createNetwork(); err != nil {
		return nil, err
	}

	// network rollback
	defer func() {
		if err != nil && s.networkNS.NetNsCreated {
			s.removeNetwork()
		}
	}()

	// Start the VM
	if err = s.startVM(); err != nil {
		return nil, err
	}

	// rollback to stop VM if error occurs
	defer func() {
		if err != nil {
			s.stopVM()
		}
	}()

	// Once startVM is done, we want to guarantee
	// that the sandbox is manageable. For that we need
	// to start the sandbox inside the VM.
	if err = s.agent.startSandbox(s); err != nil {
		return nil, err
	}

	// rollback to stop sandbox in VM
	defer func() {
		if err != nil {
			s.agent.stopSandbox(s)
		}
	}()

	if err := s.getAndStoreGuestDetails(); err != nil {
		return nil, err
	}

	// Create Containers
	if err = s.createContainers(); err != nil {
		return nil, err
	}

	// The sandbox is completely created now, we can store it.
	if err = s.storeSandbox(); err != nil {
		return nil, err
	}

	// Setup host cgroups
	if err := s.setupCgroups(); err != nil {
		return nil, err
	}

	return s, nil
}

// DeleteSandbox is the virtcontainers sandbox deletion entry point.
// DeleteSandbox will stop an already running container and then delete it.
func DeleteSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	span, ctx := trace(ctx, "DeleteSandbox")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	// Delete it.
	if err := s.Delete(); err != nil {
		return nil, err
	}

	return s, nil
}

// FetchSandbox is the virtcontainers sandbox fetching entry point.
// FetchSandbox will find out and connect to an existing sandbox and
// return the sandbox structure. The caller is responsible of calling
// VCSandbox.Release() after done with it.
func FetchSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	span, ctx := trace(ctx, "FetchSandbox")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}

	// If the proxy is KataBuiltInProxyType type, it needs to restart the proxy to watch the
	// guest console if it hadn't been watched.
	if isProxyBuiltIn(s.config.ProxyType) {
		err = s.startProxy()
		if err != nil {
			s.Release()
			return nil, err
		}
	}

	return s, nil
}

// StartSandbox is the virtcontainers sandbox starting entry point.
// StartSandbox will talk to the given hypervisor to start an existing
// sandbox and all its containers.
// It returns the sandbox ID.
func StartSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	span, ctx := trace(ctx, "StartSandbox")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return startSandbox(s)
}

func startSandbox(s *Sandbox) (*Sandbox, error) {
	// Start it
	err := s.Start()
	if err != nil {
		return nil, err
	}

	return s, nil
}

// StopSandbox is the virtcontainers sandbox stopping entry point.
// StopSandbox will talk to the given agent to stop an existing sandbox and destroy all containers within that sandbox.
func StopSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	span, ctx := trace(ctx, "StopSandbox")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandbox
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	// Stop it.
	err = s.Stop()
	if err != nil {
		return nil, err
	}

	return s, nil
}

// RunSandbox is the virtcontainers sandbox running entry point.
// RunSandbox creates a sandbox and its containers and then it starts them.
func RunSandbox(ctx context.Context, sandboxConfig SandboxConfig, factory Factory) (VCSandbox, error) {
	span, ctx := trace(ctx, "RunSandbox")
	defer span.Finish()

	s, err := createSandboxFromConfig(ctx, sandboxConfig, factory)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	lockFile, err := rwLockSandbox(s.id)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	return startSandbox(s)
}

// ListSandbox is the virtcontainers sandbox listing entry point.
func ListSandbox(ctx context.Context) ([]SandboxStatus, error) {
	span, ctx := trace(ctx, "ListSandbox")
	defer span.Finish()

	dir, err := os.Open(configStoragePath)
	if err != nil {
		if os.IsNotExist(err) {
			// No sandbox directory is not an error
			return []SandboxStatus{}, nil
		}
		return []SandboxStatus{}, err
	}

	defer dir.Close()

	sandboxesID, err := dir.Readdirnames(0)
	if err != nil {
		return []SandboxStatus{}, err
	}

	var sandboxStatusList []SandboxStatus

	for _, sandboxID := range sandboxesID {
		sandboxStatus, err := StatusSandbox(ctx, sandboxID)
		if err != nil {
			continue
		}

		sandboxStatusList = append(sandboxStatusList, sandboxStatus)
	}

	return sandboxStatusList, nil
}

// StatusSandbox is the virtcontainers sandbox status entry point.
func StatusSandbox(ctx context.Context, sandboxID string) (SandboxStatus, error) {
	span, ctx := trace(ctx, "StatusSandbox")
	defer span.Finish()

	if sandboxID == "" {
		return SandboxStatus{}, errNeedSandboxID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return SandboxStatus{}, err
	}

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		unlockSandbox(lockFile)
		return SandboxStatus{}, err
	}
	defer s.releaseStatelessSandbox()

	// We need to potentially wait for a separate container.stop() routine
	// that needs to be terminated before we return from this function.
	// Deferring the synchronization here is very important since we want
	// to avoid a deadlock. Indeed, the goroutine started by statusContainer
	// will need to lock an exclusive lock, meaning that all other locks have
	// to be released to let this happen. This call ensures this will be the
	// last operation executed by this function.
	defer s.wg.Wait()
	defer unlockSandbox(lockFile)

	var contStatusList []ContainerStatus
	for _, container := range s.containers {
		contStatus, err := statusContainer(s, container.id)
		if err != nil {
			return SandboxStatus{}, err
		}

		contStatusList = append(contStatusList, contStatus)
	}

	sandboxStatus := SandboxStatus{
		ID:               s.id,
		State:            s.state,
		Hypervisor:       s.config.HypervisorType,
		HypervisorConfig: s.config.HypervisorConfig,
		Agent:            s.config.AgentType,
		ContainersStatus: contStatusList,
		Annotations:      s.config.Annotations,
	}

	return sandboxStatus, nil
}

// CreateContainer is the virtcontainers container creation entry point.
// CreateContainer creates a container on a given sandbox.
func CreateContainer(ctx context.Context, sandboxID string, containerConfig ContainerConfig) (VCSandbox, VCContainer, error) {
	span, ctx := trace(ctx, "CreateContainer")
	defer span.Finish()

	if sandboxID == "" {
		return nil, nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, nil, err
	}
	defer s.releaseStatelessSandbox()

	c, err := s.CreateContainer(containerConfig)
	if err != nil {
		return nil, nil, err
	}

	return s, c, nil
}

// DeleteContainer is the virtcontainers container deletion entry point.
// DeleteContainer deletes a Container from a Sandbox. If the container is running,
// it needs to be stopped first.
func DeleteContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error) {
	span, ctx := trace(ctx, "DeleteContainer")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.DeleteContainer(containerID)
}

// StartContainer is the virtcontainers container starting entry point.
// StartContainer starts an already created container.
func StartContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error) {
	span, ctx := trace(ctx, "StartContainer")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.StartContainer(containerID)
}

// StopContainer is the virtcontainers container stopping entry point.
// StopContainer stops an already running container.
func StopContainer(ctx context.Context, sandboxID, containerID string) (VCContainer, error) {
	span, ctx := trace(ctx, "StopContainer")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.StopContainer(containerID)
}

// EnterContainer is the virtcontainers container command execution entry point.
// EnterContainer enters an already running container and runs a given command.
func EnterContainer(ctx context.Context, sandboxID, containerID string, cmd Cmd) (VCSandbox, VCContainer, *Process, error) {
	span, ctx := trace(ctx, "EnterContainer")
	defer span.Finish()

	if sandboxID == "" {
		return nil, nil, nil, errNeedSandboxID
	}

	if containerID == "" {
		return nil, nil, nil, errNeedContainerID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return nil, nil, nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, nil, nil, err
	}
	defer s.releaseStatelessSandbox()

	c, process, err := s.EnterContainer(containerID, cmd)
	if err != nil {
		return nil, nil, nil, err
	}

	return s, c, process, nil
}

// StatusContainer is the virtcontainers container status entry point.
// StatusContainer returns a detailed container status.
func StatusContainer(ctx context.Context, sandboxID, containerID string) (ContainerStatus, error) {
	span, ctx := trace(ctx, "StatusContainer")
	defer span.Finish()

	if sandboxID == "" {
		return ContainerStatus{}, errNeedSandboxID
	}

	if containerID == "" {
		return ContainerStatus{}, errNeedContainerID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return ContainerStatus{}, err
	}

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		unlockSandbox(lockFile)
		return ContainerStatus{}, err
	}
	defer s.releaseStatelessSandbox()

	// We need to potentially wait for a separate container.stop() routine
	// that needs to be terminated before we return from this function.
	// Deferring the synchronization here is very important since we want
	// to avoid a deadlock. Indeed, the goroutine started by statusContainer
	// will need to lock an exclusive lock, meaning that all other locks have
	// to be released to let this happen. This call ensures this will be the
	// last operation executed by this function.
	defer s.wg.Wait()
	defer unlockSandbox(lockFile)

	return statusContainer(s, containerID)
}

// This function is going to spawn a goroutine and it needs to be waited for
// by the caller.
func statusContainer(sandbox *Sandbox, containerID string) (ContainerStatus, error) {
	for _, container := range sandbox.containers {
		if container.id == containerID {
			// We have to check for the process state to make sure
			// we update the status in case the process is supposed
			// to be running but has been killed or terminated.
			if (container.state.State == StateReady ||
				container.state.State == StateRunning ||
				container.state.State == StatePaused) &&
				container.process.Pid > 0 {

				running, err := isShimRunning(container.process.Pid)
				if err != nil {
					return ContainerStatus{}, err
				}

				if !running {
					sandbox.wg.Add(1)
					go func() {
						defer sandbox.wg.Done()
						lockFile, err := rwLockSandbox(sandbox.id)
						if err != nil {
							return
						}
						defer unlockSandbox(lockFile)

						if err := container.stop(); err != nil {
							return
						}
					}()
				}
			}

			return ContainerStatus{
				ID:          container.id,
				State:       container.state,
				PID:         container.process.Pid,
				StartTime:   container.process.StartTime,
				RootFs:      container.config.RootFs,
				Annotations: container.config.Annotations,
			}, nil
		}
	}

	// No matching containers in the sandbox
	return ContainerStatus{}, nil
}

// KillContainer is the virtcontainers entry point to send a signal
// to a container running inside a sandbox. If all is true, all processes in
// the container will be sent the signal.
func KillContainer(ctx context.Context, sandboxID, containerID string, signal syscall.Signal, all bool) error {
	span, ctx := trace(ctx, "KillContainer")
	defer span.Finish()

	if sandboxID == "" {
		return errNeedSandboxID
	}

	if containerID == "" {
		return errNeedContainerID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return err
	}
	defer s.releaseStatelessSandbox()

	return s.KillContainer(containerID, signal, all)
}

// PauseSandbox is the virtcontainers pausing entry point which pauses an
// already running sandbox.
func PauseSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	span, ctx := trace(ctx, "PauseSandbox")
	defer span.Finish()

	return togglePauseSandbox(ctx, sandboxID, true)
}

// ResumeSandbox is the virtcontainers resuming entry point which resumes
// (or unpauses) and already paused sandbox.
func ResumeSandbox(ctx context.Context, sandboxID string) (VCSandbox, error) {
	span, ctx := trace(ctx, "ResumeSandbox")
	defer span.Finish()

	return togglePauseSandbox(ctx, sandboxID, false)
}

// ProcessListContainer is the virtcontainers entry point to list
// processes running inside a container
func ProcessListContainer(ctx context.Context, sandboxID, containerID string, options ProcessListOptions) (ProcessList, error) {
	span, ctx := trace(ctx, "ProcessListContainer")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.ProcessListContainer(containerID, options)
}

// UpdateContainer is the virtcontainers entry point to update
// container's resources.
func UpdateContainer(ctx context.Context, sandboxID, containerID string, resources specs.LinuxResources) error {
	span, ctx := trace(ctx, "UpdateContainer")
	defer span.Finish()

	if sandboxID == "" {
		return errNeedSandboxID
	}

	if containerID == "" {
		return errNeedContainerID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return err
	}
	defer s.releaseStatelessSandbox()

	return s.UpdateContainer(containerID, resources)
}

// StatsContainer is the virtcontainers container stats entry point.
// StatsContainer returns a detailed container stats.
func StatsContainer(ctx context.Context, sandboxID, containerID string) (ContainerStats, error) {
	span, ctx := trace(ctx, "StatsContainer")
	defer span.Finish()

	if sandboxID == "" {
		return ContainerStats{}, errNeedSandboxID
	}

	if containerID == "" {
		return ContainerStats{}, errNeedContainerID
	}
	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return ContainerStats{}, err
	}

	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return ContainerStats{}, err
	}
	defer s.releaseStatelessSandbox()

	return s.StatsContainer(containerID)
}

func togglePauseContainer(ctx context.Context, sandboxID, containerID string, pause bool) error {
	if sandboxID == "" {
		return errNeedSandboxID
	}

	if containerID == "" {
		return errNeedContainerID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return err
	}
	defer s.releaseStatelessSandbox()

	if pause {
		return s.PauseContainer(containerID)
	}

	return s.ResumeContainer(containerID)
}

// PauseContainer is the virtcontainers container pause entry point.
func PauseContainer(ctx context.Context, sandboxID, containerID string) error {
	span, ctx := trace(ctx, "PauseContainer")
	defer span.Finish()

	return togglePauseContainer(ctx, sandboxID, containerID, true)
}

// ResumeContainer is the virtcontainers container resume entry point.
func ResumeContainer(ctx context.Context, sandboxID, containerID string) error {
	span, ctx := trace(ctx, "ResumeContainer")
	defer span.Finish()

	return togglePauseContainer(ctx, sandboxID, containerID, false)
}

// AddDevice will add a device to sandbox
func AddDevice(ctx context.Context, sandboxID string, info deviceConfig.DeviceInfo) (deviceApi.Device, error) {
	span, ctx := trace(ctx, "AddDevice")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.AddDevice(info)
}

func toggleInterface(ctx context.Context, sandboxID string, inf *types.Interface, add bool) (*types.Interface, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	if add {
		return s.AddInterface(inf)
	}

	return s.RemoveInterface(inf)
}

// AddInterface is the virtcontainers add interface entry point.
func AddInterface(ctx context.Context, sandboxID string, inf *types.Interface) (*types.Interface, error) {
	span, ctx := trace(ctx, "AddInterface")
	defer span.Finish()

	return toggleInterface(ctx, sandboxID, inf, true)
}

// RemoveInterface is the virtcontainers remove interface entry point.
func RemoveInterface(ctx context.Context, sandboxID string, inf *types.Interface) (*types.Interface, error) {
	span, ctx := trace(ctx, "RemoveInterface")
	defer span.Finish()

	return toggleInterface(ctx, sandboxID, inf, false)
}

// ListInterfaces is the virtcontainers list interfaces entry point.
func ListInterfaces(ctx context.Context, sandboxID string) ([]*types.Interface, error) {
	span, ctx := trace(ctx, "ListInterfaces")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.ListInterfaces()
}

// UpdateRoutes is the virtcontainers update routes entry point.
func UpdateRoutes(ctx context.Context, sandboxID string, routes []*types.Route) ([]*types.Route, error) {
	span, ctx := trace(ctx, "UpdateRoutes")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.UpdateRoutes(routes)
}

// ListRoutes is the virtcontainers list routes entry point.
func ListRoutes(ctx context.Context, sandboxID string) ([]*types.Route, error) {
	span, ctx := trace(ctx, "ListRoutes")
	defer span.Finish()

	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(ctx, sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.ListRoutes()
}
