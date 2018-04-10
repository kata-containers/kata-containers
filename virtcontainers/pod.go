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
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"syscall"

	"github.com/sirupsen/logrus"
)

// controlSocket is the pod control socket.
// It is an hypervisor resource, and for example qemu's control
// socket is the QMP one.
const controlSocket = "ctl"

// monitorSocket is the pod monitoring socket.
// It is an hypervisor resource, and is a qmp socket in the qemu case.
// This is a socket that any monitoring entity will listen to in order
// to understand if the VM is still alive or not.
const monitorSocket = "mon"

// vmStartTimeout represents the time in seconds a pod can wait before
// to consider the VM starting operation failed.
const vmStartTimeout = 10

// stateString is a string representing a pod state.
type stateString string

const (
	// StateReady represents a pod/container that's ready to be run
	StateReady stateString = "ready"

	// StateRunning represents a pod/container that's currently running.
	StateRunning stateString = "running"

	// StatePaused represents a pod/container that has been paused.
	StatePaused stateString = "paused"

	// StateStopped represents a pod/container that has been stopped.
	StateStopped stateString = "stopped"
)

// State is a pod state structure.
type State struct {
	State stateString `json:"state"`

	// Index of the block device passed to hypervisor.
	BlockIndex int `json:"blockIndex"`

	// File system of the rootfs incase it is block device
	Fstype string `json:"fstype"`

	// Bool to indicate if the drive for a container was hotplugged.
	HotpluggedDrive bool `json:"hotpluggedDrive"`
}

// valid checks that the pod state is valid.
func (state *State) valid() bool {
	for _, validState := range []stateString{StateReady, StateRunning, StatePaused, StateStopped} {
		if state.State == validState {
			return true
		}
	}

	return false
}

// validTransition returns an error if we want to move to
// an unreachable state.
func (state *State) validTransition(oldState stateString, newState stateString) error {
	if state.State != oldState {
		return fmt.Errorf("Invalid state %s (Expecting %s)", state.State, oldState)
	}

	switch state.State {
	case StateReady:
		if newState == StateRunning || newState == StateStopped {
			return nil
		}

	case StateRunning:
		if newState == StatePaused || newState == StateStopped {
			return nil
		}

	case StatePaused:
		if newState == StateRunning || newState == StateStopped {
			return nil
		}

	case StateStopped:
		if newState == StateRunning {
			return nil
		}
	}

	return fmt.Errorf("Can not move from %s to %s",
		state.State, newState)
}

// Volume is a shared volume between the host and the VM,
// defined by its mount tag and its host path.
type Volume struct {
	// MountTag is a label used as a hint to the guest.
	MountTag string

	// HostPath is the host filesystem path for this volume.
	HostPath string
}

// Volumes is a Volume list.
type Volumes []Volume

// Set assigns volume values from string to a Volume.
func (v *Volumes) Set(volStr string) error {
	if volStr == "" {
		return fmt.Errorf("volStr cannot be empty")
	}

	volSlice := strings.Split(volStr, " ")
	const expectedVolLen = 2
	const volDelimiter = ":"

	for _, vol := range volSlice {
		volArgs := strings.Split(vol, volDelimiter)

		if len(volArgs) != expectedVolLen {
			return fmt.Errorf("Wrong string format: %s, expecting only %v parameters separated with %q",
				vol, expectedVolLen, volDelimiter)
		}

		if volArgs[0] == "" || volArgs[1] == "" {
			return fmt.Errorf("Volume parameters cannot be empty")
		}

		volume := Volume{
			MountTag: volArgs[0],
			HostPath: volArgs[1],
		}

		*v = append(*v, volume)
	}

	return nil
}

// String converts a Volume to a string.
func (v *Volumes) String() string {
	var volSlice []string

	for _, volume := range *v {
		volSlice = append(volSlice, fmt.Sprintf("%s:%s", volume.MountTag, volume.HostPath))
	}

	return strings.Join(volSlice, " ")
}

// Socket defines a socket to communicate between
// the host and any process inside the VM.
type Socket struct {
	DeviceID string
	ID       string
	HostPath string
	Name     string
}

// Sockets is a Socket list.
type Sockets []Socket

// Set assigns socket values from string to a Socket.
func (s *Sockets) Set(sockStr string) error {
	if sockStr == "" {
		return fmt.Errorf("sockStr cannot be empty")
	}

	sockSlice := strings.Split(sockStr, " ")
	const expectedSockCount = 4
	const sockDelimiter = ":"

	for _, sock := range sockSlice {
		sockArgs := strings.Split(sock, sockDelimiter)

		if len(sockArgs) != expectedSockCount {
			return fmt.Errorf("Wrong string format: %s, expecting only %v parameters separated with %q", sock, expectedSockCount, sockDelimiter)
		}

		for _, a := range sockArgs {
			if a == "" {
				return fmt.Errorf("Socket parameters cannot be empty")
			}
		}

		socket := Socket{
			DeviceID: sockArgs[0],
			ID:       sockArgs[1],
			HostPath: sockArgs[2],
			Name:     sockArgs[3],
		}

		*s = append(*s, socket)
	}

	return nil
}

// String converts a Socket to a string.
func (s *Sockets) String() string {
	var sockSlice []string

	for _, sock := range *s {
		sockSlice = append(sockSlice, fmt.Sprintf("%s:%s:%s:%s", sock.DeviceID, sock.ID, sock.HostPath, sock.Name))
	}

	return strings.Join(sockSlice, " ")
}

// Drive represents a block storage drive which may be used in case the storage
// driver has an underlying block storage device.
type Drive struct {

	// Path to the disk-image/device which will be used with this drive
	File string

	// Format of the drive
	Format string

	// ID is used to identify this drive in the hypervisor options.
	ID string

	// Index assigned to the drive. In case of virtio-scsi, this is used as SCSI LUN index
	Index int
}

// EnvVar is a key/value structure representing a command
// environment variable.
type EnvVar struct {
	Var   string
	Value string
}

// LinuxCapabilities specify the capabilities to keep when executing
// the process inside the container.
type LinuxCapabilities struct {
	// Bounding is the set of capabilities checked by the kernel.
	Bounding []string
	// Effective is the set of capabilities checked by the kernel.
	Effective []string
	// Inheritable is the capabilities preserved across execve.
	Inheritable []string
	// Permitted is the limiting superset for effective capabilities.
	Permitted []string
	// Ambient is the ambient set of capabilities that are kept.
	Ambient []string
}

// Cmd represents a command to execute in a running container.
type Cmd struct {
	Args                []string
	Envs                []EnvVar
	SupplementaryGroups []string

	// Note that these fields *MUST* remain as strings.
	//
	// The reason being that we want runtimes to be able to support CLI
	// operations like "exec --user=". That option allows the
	// specification of a user (either as a string username or a numeric
	// UID), and may optionally also include a group (groupame or GID).
	//
	// Since this type is the interface to allow the runtime to specify
	// the user and group the workload can run as, these user and group
	// fields cannot be encoded as integer values since that would imply
	// the runtime itself would need to perform a UID/GID lookup on the
	// user-specified username/groupname. But that isn't practically
	// possible given that to do so would require the runtime to access
	// the image to allow it to interrogate the appropriate databases to
	// convert the username/groupnames to UID/GID values.
	//
	// Note that this argument applies solely to the _runtime_ supporting
	// a "--user=" option when running in a "standalone mode" - there is
	// no issue when the runtime is called by a container manager since
	// all the user and group mapping is handled by the container manager
	// and specified to the runtime in terms of UID/GID's in the
	// configuration file generated by the container manager.
	User         string
	PrimaryGroup string
	WorkDir      string
	Console      string
	Capabilities LinuxCapabilities

	Interactive     bool
	Detach          bool
	NoNewPrivileges bool
}

// Resources describes VM resources configuration.
type Resources struct {
	// Memory is the amount of available memory in MiB.
	Memory uint
}

// PodStatus describes a pod status.
type PodStatus struct {
	ID               string
	State            State
	Hypervisor       HypervisorType
	HypervisorConfig HypervisorConfig
	Agent            AgentType
	ContainersStatus []ContainerStatus

	// Annotations allow clients to store arbitrary values,
	// for example to add additional status values required
	// to support particular specifications.
	Annotations map[string]string
}

// PodConfig is a Pod configuration.
type PodConfig struct {
	ID string

	Hostname string

	// Field specific to OCI specs, needed to setup all the hooks
	Hooks Hooks

	// VMConfig is the VM configuration to set for this pod.
	VMConfig Resources

	HypervisorType   HypervisorType
	HypervisorConfig HypervisorConfig

	AgentType   AgentType
	AgentConfig interface{}

	ProxyType   ProxyType
	ProxyConfig ProxyConfig

	ShimType   ShimType
	ShimConfig interface{}

	NetworkModel  NetworkModel
	NetworkConfig NetworkConfig

	// Volumes is a list of shared volumes between the host and the Pod.
	Volumes []Volume

	// Containers describe the list of containers within a Pod.
	// This list can be empty and populated by adding containers
	// to the Pod a posteriori.
	Containers []ContainerConfig

	// Annotations keys must be unique strings and must be name-spaced
	// with e.g. reverse domain notation (org.clearlinux.key).
	Annotations map[string]string
}

// valid checks that the pod configuration is valid.
func (podConfig *PodConfig) valid() bool {
	if podConfig.ID == "" {
		return false
	}

	if _, err := newHypervisor(podConfig.HypervisorType); err != nil {
		podConfig.HypervisorType = QemuHypervisor
	}

	return true
}

const (
	// R/W lock
	exclusiveLock = syscall.LOCK_EX

	// Read only lock
	sharedLock = syscall.LOCK_SH
)

// rLockPod locks the pod with a shared lock.
func rLockPod(podID string) (*os.File, error) {
	return lockPod(podID, sharedLock)
}

// rwLockPod locks the pod with an exclusive lock.
func rwLockPod(podID string) (*os.File, error) {
	return lockPod(podID, exclusiveLock)
}

// lock locks any pod to prevent it from being accessed by other processes.
func lockPod(podID string, lockType int) (*os.File, error) {
	if podID == "" {
		return nil, errNeedPodID
	}

	fs := filesystem{}
	podlockFile, _, err := fs.podURI(podID, lockFileType)
	if err != nil {
		return nil, err
	}

	lockFile, err := os.Open(podlockFile)
	if err != nil {
		return nil, err
	}

	if err := syscall.Flock(int(lockFile.Fd()), lockType); err != nil {
		return nil, err
	}

	return lockFile, nil
}

// unlock unlocks any pod to allow it being accessed by other processes.
func unlockPod(lockFile *os.File) error {
	if lockFile == nil {
		return fmt.Errorf("lockFile cannot be empty")
	}

	err := syscall.Flock(int(lockFile.Fd()), syscall.LOCK_UN)
	if err != nil {
		return err
	}

	lockFile.Close()

	return nil
}

// Pod is composed of a set of containers and a runtime environment.
// A Pod can be created, deleted, started, paused, stopped, listed, entered, and restored.
type Pod struct {
	id string

	hypervisor hypervisor
	agent      agent
	storage    resourceStorage
	network    network

	config *PodConfig

	volumes []Volume

	containers []*Container

	runPath    string
	configPath string

	state State

	networkNS NetworkNamespace

	annotationsLock *sync.RWMutex

	wg *sync.WaitGroup
}

// ID returns the pod identifier string.
func (p *Pod) ID() string {
	return p.id
}

// Logger returns a logrus logger appropriate for logging Pod messages
func (p *Pod) Logger() *logrus.Entry {
	return virtLog.WithFields(logrus.Fields{
		"subsystem": "pod",
		"pod-id":    p.id,
	})
}

// Annotations returns any annotation that a user could have stored through the pod.
func (p *Pod) Annotations(key string) (string, error) {
	value, exist := p.config.Annotations[key]
	if exist == false {
		return "", fmt.Errorf("Annotations key %s does not exist", key)
	}

	return value, nil
}

// SetAnnotations sets or adds an annotations
func (p *Pod) SetAnnotations(annotations map[string]string) error {
	p.annotationsLock.Lock()
	defer p.annotationsLock.Unlock()

	for k, v := range annotations {
		p.config.Annotations[k] = v
	}

	err := p.storage.storePodResource(p.id, configFileType, *(p.config))
	if err != nil {
		return err
	}

	return nil
}

// GetAnnotations returns pod's annotations
func (p *Pod) GetAnnotations() map[string]string {
	p.annotationsLock.RLock()
	defer p.annotationsLock.RUnlock()

	return p.config.Annotations
}

// GetAllContainers returns all containers.
func (p *Pod) GetAllContainers() []VCContainer {
	ifa := make([]VCContainer, len(p.containers))

	for i, v := range p.containers {
		ifa[i] = v
	}

	return ifa
}

// GetContainer returns the container named by the containerID.
func (p *Pod) GetContainer(containerID string) VCContainer {
	for _, c := range p.containers {
		if c.id == containerID {
			return c
		}
	}
	return nil
}

func createAssets(podConfig *PodConfig) error {
	kernel, err := newAsset(podConfig, kernelAsset)
	if err != nil {
		return err
	}

	image, err := newAsset(podConfig, imageAsset)
	if err != nil {
		return err
	}

	initrd, err := newAsset(podConfig, initrdAsset)
	if err != nil {
		return err
	}

	if image != nil && initrd != nil {
		return fmt.Errorf("%s and %s cannot be both set", imageAsset, initrdAsset)
	}

	for _, a := range []*asset{kernel, image, initrd} {
		if err := podConfig.HypervisorConfig.addCustomAsset(a); err != nil {
			return err
		}
	}

	return nil
}

// createPod creates a pod from a pod description, the containers list, the hypervisor
// and the agent passed through the Config structure.
// It will create and store the pod structure, and then ask the hypervisor
// to physically create that pod i.e. starts a VM for that pod to eventually
// be started.
func createPod(podConfig PodConfig) (*Pod, error) {
	if err := createAssets(&podConfig); err != nil {
		return nil, err
	}

	p, err := newPod(podConfig)
	if err != nil {
		return nil, err
	}

	// Fetch pod network to be able to access it from the pod structure.
	networkNS, err := p.storage.fetchPodNetwork(p.id)
	if err == nil {
		p.networkNS = networkNS
	}

	// We first try to fetch the pod state from storage.
	// If it exists, this means this is a re-creation, i.e.
	// we don't need to talk to the guest's agent, but only
	// want to create the pod and its containers in memory.
	state, err := p.storage.fetchPodState(p.id)
	if err == nil && state.State != "" {
		p.state = state
		return p, nil
	}

	// Below code path is called only during create, because of earlier check.
	if err := p.agent.createPod(p); err != nil {
		return nil, err
	}

	// Set pod state
	if err := p.setPodState(StateReady); err != nil {
		return nil, err
	}

	return p, nil
}

func newPod(podConfig PodConfig) (*Pod, error) {
	if podConfig.valid() == false {
		return nil, fmt.Errorf("Invalid pod configuration")
	}

	agent := newAgent(podConfig.AgentType)

	hypervisor, err := newHypervisor(podConfig.HypervisorType)
	if err != nil {
		return nil, err
	}

	network := newNetwork(podConfig.NetworkModel)

	p := &Pod{
		id:              podConfig.ID,
		hypervisor:      hypervisor,
		agent:           agent,
		storage:         &filesystem{},
		network:         network,
		config:          &podConfig,
		volumes:         podConfig.Volumes,
		runPath:         filepath.Join(runStoragePath, podConfig.ID),
		configPath:      filepath.Join(configStoragePath, podConfig.ID),
		state:           State{},
		annotationsLock: &sync.RWMutex{},
		wg:              &sync.WaitGroup{},
	}

	if err = globalPodList.addPod(p); err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			p.Logger().WithError(err).WithField("podid", p.id).Error("Create new pod failed")
			globalPodList.removePod(p.id)
		}
	}()

	if err = p.storage.createAllResources(*p); err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			p.storage.deletePodResources(p.id, nil)
		}
	}()

	if err = p.hypervisor.init(p); err != nil {
		return nil, err
	}

	if err = p.hypervisor.createPod(podConfig); err != nil {
		return nil, err
	}

	agentConfig := newAgentConfig(podConfig)
	if err = p.agent.init(p, agentConfig); err != nil {
		return nil, err
	}

	return p, nil
}

// storePod stores a pod config.
func (p *Pod) storePod() error {
	err := p.storage.storePodResource(p.id, configFileType, *(p.config))
	if err != nil {
		return err
	}

	for _, container := range p.containers {
		err = p.storage.storeContainerResource(p.id, container.id, configFileType, *(container.config))
		if err != nil {
			return err
		}
	}

	return nil
}

// fetchPod fetches a pod config from a pod ID and returns a pod.
func fetchPod(podID string) (pod *Pod, err error) {
	if podID == "" {
		return nil, errNeedPodID
	}

	pod, err = globalPodList.lookupPod(podID)
	if pod != nil && err == nil {
		return pod, err
	}

	fs := filesystem{}
	config, err := fs.fetchPodConfig(podID)
	if err != nil {
		return nil, err
	}

	pod, err = createPod(config)
	if err != nil {
		return nil, fmt.Errorf("failed to create pod with config %+v: %v", config, err)
	}

	// This pod already exists, we don't need to recreate the containers in the guest.
	// We only need to fetch the containers from storage and create the container structs.
	if err := pod.newContainers(); err != nil {
		return nil, err
	}

	return pod, nil
}

// findContainer returns a container from the containers list held by the
// pod structure, based on a container ID.
func (p *Pod) findContainer(containerID string) (*Container, error) {
	if p == nil {
		return nil, errNeedPod
	}

	if containerID == "" {
		return nil, errNeedContainerID
	}

	for _, c := range p.containers {
		if containerID == c.id {
			return c, nil
		}
	}

	return nil, fmt.Errorf("Could not find the container %q from the pod %q containers list",
		containerID, p.id)
}

// removeContainer removes a container from the containers list held by the
// pod structure, based on a container ID.
func (p *Pod) removeContainer(containerID string) error {
	if p == nil {
		return errNeedPod
	}

	if containerID == "" {
		return errNeedContainerID
	}

	for idx, c := range p.containers {
		if containerID == c.id {
			p.containers = append(p.containers[:idx], p.containers[idx+1:]...)
			return nil
		}
	}

	return fmt.Errorf("Could not remove the container %q from the pod %q containers list",
		containerID, p.id)
}

// delete deletes an already created pod.
// The VM in which the pod is running will be shut down.
func (p *Pod) delete() error {
	if p.state.State != StateReady &&
		p.state.State != StatePaused &&
		p.state.State != StateStopped {
		return fmt.Errorf("Pod not ready, paused or stopped, impossible to delete")
	}

	for _, c := range p.containers {
		if err := c.delete(); err != nil {
			return err
		}
	}

	globalPodList.removePod(p.id)

	return p.storage.deletePodResources(p.id, nil)
}

func (p *Pod) createNetwork() error {
	// Initialize the network.
	netNsPath, netNsCreated, err := p.network.init(p.config.NetworkConfig)
	if err != nil {
		return err
	}

	// Execute prestart hooks inside netns
	if err := p.network.run(netNsPath, func() error {
		return p.config.Hooks.preStartHooks()
	}); err != nil {
		return err
	}

	// Add the network
	networkNS, err := p.network.add(*p, p.config.NetworkConfig, netNsPath, netNsCreated)
	if err != nil {
		return err
	}
	p.networkNS = networkNS

	// Store the network
	return p.storage.storePodNetwork(p.id, networkNS)
}

func (p *Pod) removeNetwork() error {
	if p.networkNS.NetNsCreated {
		return p.network.remove(*p, p.networkNS)
	}

	return nil
}

// startVM starts the VM.
func (p *Pod) startVM() error {
	p.Logger().Info("Starting VM")

	if err := p.network.run(p.networkNS.NetNsPath, func() error {
		return p.hypervisor.startPod()
	}); err != nil {
		return err
	}

	if err := p.hypervisor.waitPod(vmStartTimeout); err != nil {
		return err
	}

	p.Logger().Info("VM started")

	// Once startVM is done, we want to guarantee
	// that the pod is manageable. For that we need
	// to start the pod inside the VM.
	return p.agent.startPod(*p)
}

func (p *Pod) addContainer(c *Container) error {
	p.containers = append(p.containers, c)

	return nil
}

// newContainers creates new containers structure and
// adds them to the pod. It does not create the containers
// in the guest. This should only be used when fetching a
// pod that already exists.
func (p *Pod) newContainers() error {
	for _, contConfig := range p.config.Containers {
		c, err := newContainer(p, contConfig)
		if err != nil {
			return err
		}

		if err := p.addContainer(c); err != nil {
			return err
		}
	}

	return nil
}

// createContainers registers all containers to the proxy, create the
// containers in the guest and starts one shim per container.
func (p *Pod) createContainers() error {
	for _, contConfig := range p.config.Containers {
		newContainer, err := createContainer(p, contConfig)
		if err != nil {
			return err
		}

		if err := p.addContainer(newContainer); err != nil {
			return err
		}
	}

	return nil
}

// start starts a pod. The containers that are making the pod
// will be started.
func (p *Pod) start() error {
	if err := p.state.validTransition(p.state.State, StateRunning); err != nil {
		return err
	}

	if err := p.setPodState(StateRunning); err != nil {
		return err
	}

	for _, c := range p.containers {
		if err := c.start(); err != nil {
			return err
		}
	}

	p.Logger().Info("Pod is started")

	return nil
}

// stop stops a pod. The containers that are making the pod
// will be destroyed.
func (p *Pod) stop() error {
	if err := p.state.validTransition(p.state.State, StateStopped); err != nil {
		return err
	}

	for _, c := range p.containers {
		if err := c.stop(); err != nil {
			return err
		}
	}

	if err := p.agent.stopPod(*p); err != nil {
		return err
	}

	p.Logger().Info("Stopping VM")
	if err := p.hypervisor.stopPod(); err != nil {
		return err
	}

	return p.setPodState(StateStopped)
}

func (p *Pod) pause() error {
	if err := p.hypervisor.pausePod(); err != nil {
		return err
	}

	return p.pauseSetStates()
}

func (p *Pod) resume() error {
	if err := p.hypervisor.resumePod(); err != nil {
		return err
	}

	return p.resumeSetStates()
}

// list lists all pod running on the host.
func (p *Pod) list() ([]Pod, error) {
	return nil, nil
}

// enter runs an executable within a pod.
func (p *Pod) enter(args []string) error {
	return nil
}

// setPodState sets both the in-memory and on-disk state of the
// pod.
func (p *Pod) setPodState(state stateString) error {
	if state == "" {
		return errNeedState
	}

	// update in-memory state
	p.state.State = state

	// update on-disk state
	return p.storage.storePodResource(p.id, stateFileType, p.state)
}

func (p *Pod) pauseSetStates() error {
	// XXX: When a pod is paused, all its containers are forcibly
	// paused too.
	if err := p.setContainersState(StatePaused); err != nil {
		return err
	}

	return p.setPodState(StatePaused)
}

func (p *Pod) resumeSetStates() error {
	// XXX: Resuming a paused pod puts all containers back into the
	// running state.
	if err := p.setContainersState(StateRunning); err != nil {
		return err
	}

	return p.setPodState(StateRunning)
}

// getAndSetPodBlockIndex retrieves pod block index and increments it for
// subsequent accesses. This index is used to maintain the index at which a
// block device is assigned to a container in the pod.
func (p *Pod) getAndSetPodBlockIndex() (int, error) {
	currentIndex := p.state.BlockIndex

	// Increment so that container gets incremented block index
	p.state.BlockIndex++

	// update on-disk state
	err := p.storage.storePodResource(p.id, stateFileType, p.state)
	if err != nil {
		return -1, err
	}

	return currentIndex, nil
}

// decrementPodBlockIndex decrements the current pod block index.
// This is used to recover from failure while adding a block device.
func (p *Pod) decrementPodBlockIndex() error {
	p.state.BlockIndex--

	// update on-disk state
	err := p.storage.storePodResource(p.id, stateFileType, p.state)
	if err != nil {
		return err
	}

	return nil
}

func (p *Pod) setContainersState(state stateString) error {
	if state == "" {
		return errNeedState
	}

	for _, c := range p.containers {
		if err := c.setContainerState(state); err != nil {
			return err
		}
	}

	return nil
}

func (p *Pod) deleteContainerState(containerID string) error {
	if containerID == "" {
		return errNeedContainerID
	}

	err := p.storage.deleteContainerResources(p.id, containerID, []podResource{stateFileType})
	if err != nil {
		return err
	}

	return nil
}

func (p *Pod) deleteContainersState() error {
	for _, container := range p.config.Containers {
		err := p.deleteContainerState(container.ID)
		if err != nil {
			return err
		}
	}

	return nil
}

// togglePausePod pauses a pod if pause is set to true, else it resumes
// it.
func togglePausePod(podID string, pause bool) (*Pod, error) {
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

	if pause {
		err = p.pause()
	} else {
		err = p.resume()
	}

	if err != nil {
		return nil, err
	}

	return p, nil
}
