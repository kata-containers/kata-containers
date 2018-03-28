//
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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

// CreatePod is the virtcontainers pod creation entry point.
// CreatePod creates a pod and its containers. It does not start them.
func CreatePod(podConfig PodConfig) (VCPod, error) {
	return createPodFromConfig(podConfig)
}

func createPodFromConfig(podConfig PodConfig) (*Pod, error) {
	// Create the pod.
	p, err := createPod(podConfig)
	if err != nil {
		return nil, err
	}

	// Create the pod network
	if err := p.createNetwork(); err != nil {
		return nil, err
	}

	// Start the VM
	if err := p.startVM(); err != nil {
		return nil, err
	}

	// Create Containers
	if err := p.createContainers(); err != nil {
		return nil, err
	}

	// The pod is completely created now, we can store it.
	if err := p.storePod(); err != nil {
		return nil, err
	}

	return p, nil
}

// DeletePod is the virtcontainers pod deletion entry point.
// DeletePod will stop an already running container and then delete it.
func DeletePod(podID string) (VCPod, error) {
	if podID == "" {
		return nil, errNeedPodID
	}

	lockFile, err := rwLockPod(podID)
	if err != nil {
		return nil, err
	}
	defer unlockPod(lockFile)

	// Fetch the pod from storage and create it.
	p, err := fetchPod(podID)
	if err != nil {
		return nil, err
	}

	// Delete it.
	if err := p.delete(); err != nil {
		return nil, err
	}

	return p, nil
}

// StartPod is the virtcontainers pod starting entry point.
// StartPod will talk to the given hypervisor to start an existing
// pod and all its containers.
// It returns the pod ID.
func StartPod(podID string) (VCPod, error) {
	if podID == "" {
		return nil, errNeedPodID
	}

	lockFile, err := rwLockPod(podID)
	if err != nil {
		return nil, err
	}
	defer unlockPod(lockFile)

	// Fetch the pod from storage and create it.
	p, err := fetchPod(podID)
	if err != nil {
		return nil, err
	}

	return startPod(p)
}

func startPod(p *Pod) (*Pod, error) {
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

// StopPod is the virtcontainers pod stopping entry point.
// StopPod will talk to the given agent to stop an existing pod and destroy all containers within that pod.
func StopPod(podID string) (VCPod, error) {
	if podID == "" {
		return nil, errNeedPod
	}

	lockFile, err := rwLockPod(podID)
	if err != nil {
		return nil, err
	}
	defer unlockPod(lockFile)

	// Fetch the pod from storage and create it.
	p, err := fetchPod(podID)
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

// RunPod is the virtcontainers pod running entry point.
// RunPod creates a pod and its containers and then it starts them.
func RunPod(podConfig PodConfig) (VCPod, error) {
	p, err := createPodFromConfig(podConfig)
	if err != nil {
		return nil, err
	}

	lockFile, err := rwLockPod(p.id)
	if err != nil {
		return nil, err
	}
	defer unlockPod(lockFile)

	return startPod(p)
}

// ListPod is the virtcontainers pod listing entry point.
func ListPod() ([]PodStatus, error) {
	dir, err := os.Open(configStoragePath)
	if err != nil {
		if os.IsNotExist(err) {
			// No pod directory is not an error
			return []PodStatus{}, nil
		}
		return []PodStatus{}, err
	}

	defer dir.Close()

	podsID, err := dir.Readdirnames(0)
	if err != nil {
		return []PodStatus{}, err
	}

	var podStatusList []PodStatus

	for _, podID := range podsID {
		podStatus, err := StatusPod(podID)
		if err != nil {
			continue
		}

		podStatusList = append(podStatusList, podStatus)
	}

	return podStatusList, nil
}

// StatusPod is the virtcontainers pod status entry point.
func StatusPod(podID string) (PodStatus, error) {
	if podID == "" {
		return PodStatus{}, errNeedPodID
	}

	lockFile, err := rLockPod(podID)
	if err != nil {
		return PodStatus{}, err
	}

	pod, err := fetchPod(podID)
	if err != nil {
		unlockPod(lockFile)
		return PodStatus{}, err
	}

	// We need to potentially wait for a separate container.stop() routine
	// that needs to be terminated before we return from this function.
	// Deferring the synchronization here is very important since we want
	// to avoid a deadlock. Indeed, the goroutine started by statusContainer
	// will need to lock an exclusive lock, meaning that all other locks have
	// to be released to let this happen. This call ensures this will be the
	// last operation executed by this function.
	defer pod.wg.Wait()
	defer unlockPod(lockFile)

	var contStatusList []ContainerStatus
	for _, container := range pod.containers {
		contStatus, err := statusContainer(pod, container.id)
		if err != nil {
			return PodStatus{}, err
		}

		contStatusList = append(contStatusList, contStatus)
	}

	podStatus := PodStatus{
		ID:               pod.id,
		State:            pod.state,
		Hypervisor:       pod.config.HypervisorType,
		HypervisorConfig: pod.config.HypervisorConfig,
		Agent:            pod.config.AgentType,
		ContainersStatus: contStatusList,
		Annotations:      pod.config.Annotations,
	}

	return podStatus, nil
}

// CreateContainer is the virtcontainers container creation entry point.
// CreateContainer creates a container on a given pod.
func CreateContainer(podID string, containerConfig ContainerConfig) (VCPod, VCContainer, error) {
	if podID == "" {
		return nil, nil, errNeedPodID
	}

	lockFile, err := rwLockPod(podID)
	if err != nil {
		return nil, nil, err
	}
	defer unlockPod(lockFile)

	p, err := fetchPod(podID)
	if err != nil {
		return nil, nil, err
	}

	// Create the container.
	c, err := createContainer(p, containerConfig)
	if err != nil {
		return nil, nil, err
	}

	// Add the container to the containers list in the pod.
	if err := p.addContainer(c); err != nil {
		return nil, nil, err
	}

	// Store it.
	err = c.storeContainer()
	if err != nil {
		return nil, nil, err
	}

	// Update pod config.
	p.config.Containers = append(p.config.Containers, containerConfig)
	err = p.storage.storePodResource(podID, configFileType, *(p.config))
	if err != nil {
		return nil, nil, err
	}

	return p, c, nil
}

// DeleteContainer is the virtcontainers container deletion entry point.
// DeleteContainer deletes a Container from a Pod. If the container is running,
// it needs to be stopped first.
func DeleteContainer(podID, containerID string) (VCContainer, error) {
	if podID == "" {
		return nil, errNeedPodID
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	lockFile, err := rwLockPod(podID)
	if err != nil {
		return nil, err
	}
	defer unlockPod(lockFile)

	p, err := fetchPod(podID)
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

	// Update pod config
	for idx, contConfig := range p.config.Containers {
		if contConfig.ID == containerID {
			p.config.Containers = append(p.config.Containers[:idx], p.config.Containers[idx+1:]...)
			break
		}
	}
	err = p.storage.storePodResource(podID, configFileType, *(p.config))
	if err != nil {
		return nil, err
	}

	return c, nil
}

// StartContainer is the virtcontainers container starting entry point.
// StartContainer starts an already created container.
func StartContainer(podID, containerID string) (VCContainer, error) {
	if podID == "" {
		return nil, errNeedPodID
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	lockFile, err := rwLockPod(podID)
	if err != nil {
		return nil, err
	}
	defer unlockPod(lockFile)

	p, err := fetchPod(podID)
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
func StopContainer(podID, containerID string) (VCContainer, error) {
	if podID == "" {
		return nil, errNeedPodID
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	lockFile, err := rwLockPod(podID)
	if err != nil {
		return nil, err
	}
	defer unlockPod(lockFile)

	p, err := fetchPod(podID)
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
func EnterContainer(podID, containerID string, cmd Cmd) (VCPod, VCContainer, *Process, error) {
	if podID == "" {
		return nil, nil, nil, errNeedPodID
	}

	if containerID == "" {
		return nil, nil, nil, errNeedContainerID
	}

	lockFile, err := rLockPod(podID)
	if err != nil {
		return nil, nil, nil, err
	}
	defer unlockPod(lockFile)

	p, err := fetchPod(podID)
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
func StatusContainer(podID, containerID string) (ContainerStatus, error) {
	if podID == "" {
		return ContainerStatus{}, errNeedPodID
	}

	if containerID == "" {
		return ContainerStatus{}, errNeedContainerID
	}

	lockFile, err := rLockPod(podID)
	if err != nil {
		return ContainerStatus{}, err
	}

	pod, err := fetchPod(podID)
	if err != nil {
		unlockPod(lockFile)
		return ContainerStatus{}, err
	}

	// We need to potentially wait for a separate container.stop() routine
	// that needs to be terminated before we return from this function.
	// Deferring the synchronization here is very important since we want
	// to avoid a deadlock. Indeed, the goroutine started by statusContainer
	// will need to lock an exclusive lock, meaning that all other locks have
	// to be released to let this happen. This call ensures this will be the
	// last operation executed by this function.
	defer pod.wg.Wait()
	defer unlockPod(lockFile)

	return statusContainer(pod, containerID)
}

// This function is going to spawn a goroutine and it needs to be waited for
// by the caller.
func statusContainer(pod *Pod, containerID string) (ContainerStatus, error) {
	for _, container := range pod.containers {
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
					pod.wg.Add(1)
					go func() {
						defer pod.wg.Done()
						lockFile, err := rwLockPod(pod.id)
						if err != nil {
							return
						}
						defer unlockPod(lockFile)

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

	// No matching containers in the pod
	return ContainerStatus{}, nil
}

// KillContainer is the virtcontainers entry point to send a signal
// to a container running inside a pod. If all is true, all processes in
// the container will be sent the signal.
func KillContainer(podID, containerID string, signal syscall.Signal, all bool) error {
	if podID == "" {
		return errNeedPodID
	}

	if containerID == "" {
		return errNeedContainerID
	}

	lockFile, err := rwLockPod(podID)
	if err != nil {
		return err
	}
	defer unlockPod(lockFile)

	p, err := fetchPod(podID)
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

// PausePod is the virtcontainers pausing entry point which pauses an
// already running pod.
func PausePod(podID string) (VCPod, error) {
	return togglePausePod(podID, true)
}

// ResumePod is the virtcontainers resuming entry point which resumes
// (or unpauses) and already paused pod.
func ResumePod(podID string) (VCPod, error) {
	return togglePausePod(podID, false)
}

// ProcessListContainer is the virtcontainers entry point to list
// processes running inside a container
func ProcessListContainer(podID, containerID string, options ProcessListOptions) (ProcessList, error) {
	if podID == "" {
		return nil, errNeedPodID
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	lockFile, err := rLockPod(podID)
	if err != nil {
		return nil, err
	}
	defer unlockPod(lockFile)

	p, err := fetchPod(podID)
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
