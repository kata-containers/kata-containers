// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os"
	"runtime"
	"syscall"

	"github.com/kata-containers/agent/protocols/grpc"
	deviceApi "github.com/kata-containers/runtime/virtcontainers/device/api"
	deviceConfig "github.com/kata-containers/runtime/virtcontainers/device/config"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
)

func init() {
	runtime.LockOSThread()
}

var virtLog = logrus.WithField("source", "virtcontainers")

// SetLogger sets the logger for virtcontainers package.
func SetLogger(logger *logrus.Entry) {
	fields := virtLog.Data
	virtLog = logger.WithFields(fields)

	deviceApi.SetLogger(virtLog)
}

// CreateSandbox is the virtcontainers sandbox creation entry point.
// CreateSandbox creates a sandbox and its containers. It does not start them.
func CreateSandbox(sandboxConfig SandboxConfig, factory Factory) (VCSandbox, error) {
	s, err := createSandboxFromConfig(sandboxConfig, factory)
	if err == nil {
		s.releaseStatelessSandbox()
	}

	return s, err
}

func createSandboxFromConfig(sandboxConfig SandboxConfig, factory Factory) (*Sandbox, error) {
	var err error

	// Create the sandbox.
	s, err := createSandbox(sandboxConfig, factory)
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

	// Create Containers
	if err = s.createContainers(); err != nil {
		return nil, err
	}

	// The sandbox is completely created now, we can store it.
	if err = s.storeSandbox(); err != nil {
		return nil, err
	}

	return s, nil
}

// DeleteSandbox is the virtcontainers sandbox deletion entry point.
// DeleteSandbox will stop an already running container and then delete it.
func DeleteSandbox(sandboxID string) (VCSandbox, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(sandboxID)
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
func FetchSandbox(sandboxID string) (VCSandbox, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(sandboxID)
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
func StartSandbox(sandboxID string) (VCSandbox, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return startSandbox(s)
}

func startSandbox(s *Sandbox) (*Sandbox, error) {
	// Start it
	err := s.start()
	if err != nil {
		return nil, err
	}

	// Execute poststart hooks.
	if err := s.config.Hooks.postStartHooks(s); err != nil {
		return nil, err
	}

	return s, nil
}

// StopSandbox is the virtcontainers sandbox stopping entry point.
// StopSandbox will talk to the given agent to stop an existing sandbox and destroy all containers within that sandbox.
func StopSandbox(sandboxID string) (VCSandbox, error) {
	if sandboxID == "" {
		return nil, errNeedSandbox
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	// Fetch the sandbox from storage and create it.
	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	// Stop it.
	err = s.stop()
	if err != nil {
		return nil, err
	}

	// Remove the network.
	if err := s.removeNetwork(); err != nil {
		return nil, err
	}

	// Execute poststop hooks.
	if err := s.config.Hooks.postStopHooks(s); err != nil {
		return nil, err
	}

	return s, nil
}

// RunSandbox is the virtcontainers sandbox running entry point.
// RunSandbox creates a sandbox and its containers and then it starts them.
func RunSandbox(sandboxConfig SandboxConfig, factory Factory) (VCSandbox, error) {
	s, err := createSandboxFromConfig(sandboxConfig, factory)
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
func ListSandbox() ([]SandboxStatus, error) {
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
		sandboxStatus, err := StatusSandbox(sandboxID)
		if err != nil {
			continue
		}

		sandboxStatusList = append(sandboxStatusList, sandboxStatus)
	}

	return sandboxStatusList, nil
}

// StatusSandbox is the virtcontainers sandbox status entry point.
func StatusSandbox(sandboxID string) (SandboxStatus, error) {
	if sandboxID == "" {
		return SandboxStatus{}, errNeedSandboxID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return SandboxStatus{}, err
	}

	s, err := fetchSandbox(sandboxID)
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
func CreateContainer(sandboxID string, containerConfig ContainerConfig) (VCSandbox, VCContainer, error) {
	if sandboxID == "" {
		return nil, nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(sandboxID)
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
func DeleteContainer(sandboxID, containerID string) (VCContainer, error) {
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

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.DeleteContainer(containerID)
}

// StartContainer is the virtcontainers container starting entry point.
// StartContainer starts an already created container.
func StartContainer(sandboxID, containerID string) (VCContainer, error) {
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

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	c, err := s.StartContainer(containerID)
	if err != nil {
		return nil, err
	}

	return c, nil
}

// StopContainer is the virtcontainers container stopping entry point.
// StopContainer stops an already running container.
func StopContainer(sandboxID, containerID string) (VCContainer, error) {
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

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Stop it.
	err = c.stop()
	if err != nil {
		return nil, err
	}

	return c, nil
}

// EnterContainer is the virtcontainers container command execution entry point.
// EnterContainer enters an already running container and runs a given command.
func EnterContainer(sandboxID, containerID string, cmd Cmd) (VCSandbox, VCContainer, *Process, error) {
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

	s, err := fetchSandbox(sandboxID)
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
func StatusContainer(sandboxID, containerID string) (ContainerStatus, error) {
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

	s, err := fetchSandbox(sandboxID)
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
func KillContainer(sandboxID, containerID string, signal syscall.Signal, all bool) error {
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

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return err
	}
	defer s.releaseStatelessSandbox()

	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	// Send a signal to the process.
	err = c.kill(signal, all)
	if err != nil {
		return err
	}

	return nil
}

// PauseSandbox is the virtcontainers pausing entry point which pauses an
// already running sandbox.
func PauseSandbox(sandboxID string) (VCSandbox, error) {
	return togglePauseSandbox(sandboxID, true)
}

// ResumeSandbox is the virtcontainers resuming entry point which resumes
// (or unpauses) and already paused sandbox.
func ResumeSandbox(sandboxID string) (VCSandbox, error) {
	return togglePauseSandbox(sandboxID, false)
}

// ProcessListContainer is the virtcontainers entry point to list
// processes running inside a container
func ProcessListContainer(sandboxID, containerID string, options ProcessListOptions) (ProcessList, error) {
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

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	return c.processList(options)
}

// UpdateContainer is the virtcontainers entry point to update
// container's resources.
func UpdateContainer(sandboxID, containerID string, resources specs.LinuxResources) error {
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

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return err
	}
	defer s.releaseStatelessSandbox()

	return s.UpdateContainer(containerID, resources)
}

// StatsContainer is the virtcontainers container stats entry point.
// StatsContainer returns a detailed container stats.
func StatsContainer(sandboxID, containerID string) (ContainerStats, error) {
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

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return ContainerStats{}, err
	}
	defer s.releaseStatelessSandbox()

	return s.StatsContainer(containerID)
}

func togglePauseContainer(sandboxID, containerID string, pause bool) error {
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

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return err
	}
	defer s.releaseStatelessSandbox()

	// Fetch the container.
	c, err := s.findContainer(containerID)
	if err != nil {
		return err
	}

	if pause {
		return c.pause()
	}

	return c.resume()
}

// PauseContainer is the virtcontainers container pause entry point.
func PauseContainer(sandboxID, containerID string) error {
	return togglePauseContainer(sandboxID, containerID, true)
}

// ResumeContainer is the virtcontainers container resume entry point.
func ResumeContainer(sandboxID, containerID string) error {
	return togglePauseContainer(sandboxID, containerID, false)
}

// AddDevice will add a device to sandbox
func AddDevice(sandboxID string, info deviceConfig.DeviceInfo) (deviceApi.Device, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer s.releaseStatelessSandbox()

	return s.AddDevice(info)
}

func toggleInterface(sandboxID string, inf *grpc.Interface, add bool) (*grpc.Interface, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	if add {
		return s.AddInterface(inf)
	}
	return s.RemoveInterface(inf)
}

// AddInterface is the virtcontainers add interface entry point.
func AddInterface(sandboxID string, inf *grpc.Interface) (*grpc.Interface, error) {
	return toggleInterface(sandboxID, inf, true)
}

// RemoveInterface is the virtcontainers remove interface entry point.
func RemoveInterface(sandboxID string, inf *grpc.Interface) (*grpc.Interface, error) {
	return toggleInterface(sandboxID, inf, false)
}

// ListInterfaces is the virtcontainers list interfaces entry point.
func ListInterfaces(sandboxID string) ([]*grpc.Interface, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	return s.ListInterfaces()
}

// UpdateRoutes is the virtcontainers update routes entry point.
func UpdateRoutes(sandboxID string, routes []*grpc.Route) ([]*grpc.Route, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rwLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	return s.UpdateRoutes(routes)
}

// ListRoutes is the virtcontainers list routes entry point.
func ListRoutes(sandboxID string) ([]*grpc.Route, error) {
	if sandboxID == "" {
		return nil, errNeedSandboxID
	}

	lockFile, err := rLockSandbox(sandboxID)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	s, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	return s.ListRoutes()
}
