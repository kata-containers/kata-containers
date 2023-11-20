// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package types

import (
	"fmt"
	"net"
	"os"
	"strings"

	"github.com/opencontainers/runtime-spec/specs-go"
)

// StateString is a string representing a sandbox state.
type StateString string

const (
	// StateReady represents a sandbox/container that's ready to be run
	StateReady StateString = "ready"

	// StateRunning represents a sandbox/container that's currently running.
	StateRunning StateString = "running"

	// StatePaused represents a sandbox/container that has been paused.
	StatePaused StateString = "paused"

	// StateStopped represents a sandbox/container that has been stopped.
	StateStopped StateString = "stopped"

	// StateCreating represents a sandbox/container that's in creating.
	StateCreating StateString = "creating"
)

const (
	HybridVSockScheme     = "hvsock"
	MockHybridVSockScheme = "mock"
	VSockScheme           = "vsock"
	RemoteSockScheme      = "remote"
)

// SandboxState is a sandbox state structure
type SandboxState struct {
	// Index map of the block device passed to hypervisor.
	BlockIndexMap map[int]struct{} `json:"blockIndexMap"`

	// Path to all the cgroups setup for a container. Key is cgroup subsystem name
	// with the value as the path.
	CgroupPaths map[string]string `json:"cgroupPaths"`

	State StateString `json:"state"`

	// SandboxCgroupPath is the cgroup path for all the sandbox processes,
	// when sandbox_cgroup_only is set. When it's not set, part of those
	// processes will be living under the overhead cgroup.
	SandboxCgroupPath string `json:"sandboxCgroupPath,omitempty"`

	// OverheadCgroupPath is the path to the optional overhead cgroup
	// path holding processes that should not be part of the sandbox
	// cgroup.
	OverheadCgroupPath string `json:"overheadCgroupPath,omitempty"`

	// PersistVersion indicates current storage api version.
	// It's also known as ABI version of kata-runtime.
	// Note: it won't be written to disk
	PersistVersion uint `json:"-"`

	// GuestMemoryBlockSizeMB is the size of memory block of guestos
	GuestMemoryBlockSizeMB uint32 `json:"guestMemoryBlockSize"`

	// GuestMemoryHotplugProbe determines whether guest kernel supports memory hotplug probe interface
	GuestMemoryHotplugProbe bool `json:"guestMemoryHotplugProbe"`
}

// Valid checks that the sandbox state is valid.
func (state *SandboxState) Valid() bool {
	return state.State.valid()
}

// ValidTransition returns an error if we want to move to
// an unreachable state.
func (state *SandboxState) ValidTransition(oldState StateString, newState StateString) error {
	return state.State.validTransition(oldState, newState)
}

func (state *StateString) valid() bool {
	for _, validState := range []StateString{StateReady, StateRunning, StatePaused, StateStopped} {
		if *state == validState {
			return true
		}
	}

	return false
}

func (state *StateString) validTransition(oldState StateString, newState StateString) error {
	if *state != oldState {
		return fmt.Errorf("Invalid state %v (Expecting %v)", state, oldState)
	}

	switch *state {
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

	return fmt.Errorf("Can not move from %v to %v",
		state, newState)
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

// VSock defines a virtio-socket to communicate between
// the host and any process inside the VM.
// This kind of socket is not supported in all hypervisors.
type VSock struct {
	VhostFd   *os.File
	ContextID uint64
	Port      uint32
}

func (s *VSock) String() string {
	return fmt.Sprintf("%s://%d:%d", VSockScheme, s.ContextID, s.Port)
}

// HybridVSock defines a hybrid vsocket to communicate between
// the host and any process inside the VM.
// This is a virtio-vsock implementation based on AF_VSOCK on the
// guest side and multiple AF_UNIX sockets on the host side.
// This kind of socket is not supported in all hypervisors.
// Firecracker supports it.
type HybridVSock struct {
	UdsPath   string
	ContextID uint64
	Port      uint32
}

func (s *HybridVSock) String() string {
	return fmt.Sprintf("%s://%s:%d", HybridVSockScheme, s.UdsPath, s.Port)
}

type RemoteSock struct {
	Conn             net.Conn
	SandboxID        string
	TunnelSocketPath string
}

func (s *RemoteSock) String() string {
	return fmt.Sprintf("%s://%s", RemoteSockScheme, s.TunnelSocketPath)
}

// MockHybridVSock defines a mock hybrid vsocket for tests only.
type MockHybridVSock struct {
	UdsPath string
}

func (s *MockHybridVSock) String() string {
	return fmt.Sprintf("%s://%s", MockHybridVSockScheme, s.UdsPath)
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

// EnvVar is a key/value structure representing a command
// environment variable.
type EnvVar struct {
	Var   string
	Value string
}

// Cmd represents a command to execute in a running container.
type Cmd struct {
	Capabilities *specs.LinuxCapabilities

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

	Args                []string
	Envs                []EnvVar
	SupplementaryGroups []string

	Interactive     bool
	Detach          bool
	NoNewPrivileges bool
}

// Resources describes VM resources configuration.
type Resources struct {
	// Memory is the amount of available memory in MiB.
	Memory      uint
	MemorySlots uint8
}
