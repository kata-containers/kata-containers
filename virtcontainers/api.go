// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os"
	"runtime"
	"syscall"

	"github.com/sirupsen/logrus"
)

func init() {
	runtime.LockOSThread()
}

var virtLog = logrus.FieldLogger(logrus.New())

// SetLogger sets the logger for virtcontainers package.
func SetLogger(logger logrus.FieldLogger) {
	fields := logrus.Fields{
		"source": "virtcontainers",
		"arch":   runtime.GOARCH,
	}

	virtLog = logger.WithFields(fields)
}

// CreateSandbox is the virtcontainers sandbox creation entry point.
// CreateSandbox creates a sandbox and its containers. It does not start them.
func CreateSandbox(sandboxConfig SandboxConfig) (VCSandbox, error) {
	return createSandboxFromConfig(sandboxConfig)
}

func createSandboxFromConfig(sandboxConfig SandboxConfig) (*Sandbox, error) {
	// Create the sandbox.
	s, err := createSandbox(sandboxConfig)
	if err != nil {
		return nil, err
	}

	// Create the sandbox network
	if err := s.createNetwork(); err != nil {
		return nil, err
	}

	// Start the VM
	if err := s.startVM(); err != nil {
		return nil, err
	}

	// Create Containers
	if err := s.createContainers(); err != nil {
		return nil, err
	}

	// The sandbox is completely created now, we can store it.
	if err := s.storeSandbox(); err != nil {
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
	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	// Delete it.
	if err := p.Delete(); err != nil {
		return nil, err
	}

	return p, nil
}

// FetchSandbox is the virtcontainers sandbox fetching entry point.
// FetchSandbox will find out and connect to an existing sandbox and
// return the sandbox structure.
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
	return fetchSandbox(sandboxID)
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
	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	return startSandbox(p)
}

func startSandbox(p *Sandbox) (*Sandbox, error) {
	// Start it
	err := p.start()
	if err != nil {
		return nil, err
	}

	// Execute poststart hooks.
	if err := p.config.Hooks.postStartHooks(); err != nil {
		return nil, err
	}

	return p, nil
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
	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	// Stop it.
	err = p.stop()
	if err != nil {
		return nil, err
	}

	// Remove the network.
	if err := p.removeNetwork(); err != nil {
		return nil, err
	}

	// Execute poststop hooks.
	if err := p.config.Hooks.postStopHooks(); err != nil {
		return nil, err
	}

	return p, nil
}

// RunSandbox is the virtcontainers sandbox running entry point.
// RunSandbox creates a sandbox and its containers and then it starts them.
func RunSandbox(sandboxConfig SandboxConfig) (VCSandbox, error) {
	p, err := createSandboxFromConfig(sandboxConfig)
	if err != nil {
		return nil, err
	}

	lockFile, err := rwLockSandbox(p.id)
	if err != nil {
		return nil, err
	}
	defer unlockSandbox(lockFile)

	return startSandbox(p)
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

	sandbox, err := fetchSandbox(sandboxID)
	if err != nil {
		unlockSandbox(lockFile)
		return SandboxStatus{}, err
	}

	// We need to potentially wait for a separate container.stop() routine
	// that needs to be terminated before we return from this function.
	// Deferring the synchronization here is very important since we want
	// to avoid a deadlock. Indeed, the goroutine started by statusContainer
	// will need to lock an exclusive lock, meaning that all other locks have
	// to be released to let this happen. This call ensures this will be the
	// last operation executed by this function.
	defer sandbox.wg.Wait()
	defer unlockSandbox(lockFile)

	var contStatusList []ContainerStatus
	for _, container := range sandbox.containers {
		contStatus, err := statusContainer(sandbox, container.id)
		if err != nil {
			return SandboxStatus{}, err
		}

		contStatusList = append(contStatusList, contStatus)
	}

	sandboxStatus := SandboxStatus{
		ID:               sandbox.id,
		State:            sandbox.state,
		Hypervisor:       sandbox.config.HypervisorType,
		HypervisorConfig: sandbox.config.HypervisorConfig,
		Agent:            sandbox.config.AgentType,
		ContainersStatus: contStatusList,
		Annotations:      sandbox.config.Annotations,
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

	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, nil, err
	}

	// Create the container.
	c, err := createContainer(p, containerConfig)
	if err != nil {
		return nil, nil, err
	}

	// Add the container to the containers list in the sandbox.
	if err := p.addContainer(c); err != nil {
		return nil, nil, err
	}

	// Store it.
	err = c.storeContainer()
	if err != nil {
		return nil, nil, err
	}

	// Update sandbox config.
	p.config.Containers = append(p.config.Containers, containerConfig)
	err = p.storage.storeSandboxResource(sandboxID, configFileType, *(p.config))
	if err != nil {
		return nil, nil, err
	}

	return p, c, nil
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

	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	// Fetch the container.
	c, err := p.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Delete it.
	err = c.delete()
	if err != nil {
		return nil, err
	}

	// Update sandbox config
	for idx, contConfig := range p.config.Containers {
		if contConfig.ID == containerID {
			p.config.Containers = append(p.config.Containers[:idx], p.config.Containers[idx+1:]...)
			break
		}
	}
	err = p.storage.storeSandboxResource(sandboxID, configFileType, *(p.config))
	if err != nil {
		return nil, err
	}

	return c, nil
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

	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	// Fetch the container.
	c, err := p.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	// Start it.
	err = c.start()
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

	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	// Fetch the container.
	c, err := p.findContainer(containerID)
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

	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, nil, nil, err
	}

	// Fetch the container.
	c, err := p.findContainer(containerID)
	if err != nil {
		return nil, nil, nil, err
	}

	// Enter it.
	process, err := c.enter(cmd)
	if err != nil {
		return nil, nil, nil, err
	}

	return p, c, process, nil
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

	sandbox, err := fetchSandbox(sandboxID)
	if err != nil {
		unlockSandbox(lockFile)
		return ContainerStatus{}, err
	}

	// We need to potentially wait for a separate container.stop() routine
	// that needs to be terminated before we return from this function.
	// Deferring the synchronization here is very important since we want
	// to avoid a deadlock. Indeed, the goroutine started by statusContainer
	// will need to lock an exclusive lock, meaning that all other locks have
	// to be released to let this happen. This call ensures this will be the
	// last operation executed by this function.
	defer sandbox.wg.Wait()
	defer unlockSandbox(lockFile)

	return statusContainer(sandbox, containerID)
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

	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return err
	}

	// Fetch the container.
	c, err := p.findContainer(containerID)
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

	p, err := fetchSandbox(sandboxID)
	if err != nil {
		return nil, err
	}

	// Fetch the container.
	c, err := p.findContainer(containerID)
	if err != nil {
		return nil, err
	}

	return c.processList(options)
}
