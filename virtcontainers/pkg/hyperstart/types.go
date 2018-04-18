// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package hyperstart

import (
	"syscall"
)

// Defines all available commands to communicate with hyperstart agent.
const (
	VersionCode = iota
	StartSandboxCode
	GetSandboxDeprecatedCode
	StopSandboxDeprecatedCode
	DestroySandboxCode
	RestartContainerDeprecatedCode
	ExecCmdCode
	FinishCmdDeprecatedCode
	ReadyCode
	AckCode
	ErrorCode
	WinsizeCode
	PingCode
	FinishSandboxDeprecatedCode
	NextCode
	WriteFileCode
	ReadFileCode
	NewContainerCode
	KillContainerCode
	OnlineCPUMemCode
	SetupInterfaceCode
	SetupRouteCode
	RemoveContainerCode
	PsContainerCode
	ProcessAsyncEventCode
)

// FileCommand is the structure corresponding to the format expected by
// hyperstart to interact with files.
type FileCommand struct {
	Container string `json:"container"`
	File      string `json:"file"`
}

// KillCommand is the structure corresponding to the format expected by
// hyperstart to kill a container on the guest.
type KillCommand struct {
	Container    string         `json:"container"`
	Signal       syscall.Signal `json:"signal"`
	AllProcesses bool           `json:"allProcesses"`
}

// ExecCommand is the structure corresponding to the format expected by
// hyperstart to execute a command on the guest.
type ExecCommand struct {
	Container string  `json:"container,omitempty"`
	Process   Process `json:"process"`
}

// RemoveCommand is the structure corresponding to the format expected by
// hyperstart to remove a container on the guest.
type RemoveCommand struct {
	Container string `json:"container"`
}

// PsCommand is the structure corresponding to the format expected by
// hyperstart to list processes of a container on the guest.
type PsCommand struct {
	Container string   `json:"container"`
	Format    string   `json:"format"`
	PsArgs    []string `json:"psargs"`
}

// PAECommand is the structure hyperstart can expects to
// receive after a process has been started/executed on a container.
type PAECommand struct {
	Container string `json:"container"`
	Process   string `json:"process"`
	Event     string `json:"event"`
	Info      string `json:"info,omitempty"`
	Status    int    `json:"status,omitempty"`
}

// DecodedMessage is the structure holding messages coming from CTL channel.
type DecodedMessage struct {
	Code    uint32
	Message []byte
}

// TtyMessage is the structure holding messages coming from TTY channel.
type TtyMessage struct {
	Session uint64
	Message []byte
}

// WindowSizeMessage is the structure corresponding to the format expected by
// hyperstart to resize a container's window.
type WindowSizeMessage struct {
	Container string `json:"container"`
	Process   string `json:"process"`
	Row       uint16 `json:"row"`
	Column    uint16 `json:"column"`
}

// VolumeDescriptor describes a volume related to a container.
type VolumeDescriptor struct {
	Device       string `json:"device"`
	Addr         string `json:"addr,omitempty"`
	Mount        string `json:"mount"`
	Fstype       string `json:"fstype,omitempty"`
	ReadOnly     bool   `json:"readOnly"`
	DockerVolume bool   `json:"dockerVolume"`
}

// FsmapDescriptor describes a filesystem map related to a container.
type FsmapDescriptor struct {
	Source       string `json:"source"`
	Path         string `json:"path"`
	ReadOnly     bool   `json:"readOnly"`
	DockerVolume bool   `json:"dockerVolume"`
	AbsolutePath bool   `json:"absolutePath"`
	SCSIAddr     string `json:"scsiAddr"`
}

// EnvironmentVar holds an environment variable and its value.
type EnvironmentVar struct {
	Env   string `json:"env"`
	Value string `json:"value"`
}

// Rlimit describes a resource limit.
type Rlimit struct {
	// Type of the rlimit to set
	Type string `json:"type"`
	// Hard is the hard limit for the specified type
	Hard uint64 `json:"hard"`
	// Soft is the soft limit for the specified type
	Soft uint64 `json:"soft"`
}

// Capabilities specify the capabilities to keep when executing the process inside the container.
type Capabilities struct {
	// Bounding is the set of capabilities checked by the kernel.
	Bounding []string `json:"bounding"`
	// Effective is the set of capabilities checked by the kernel.
	Effective []string `json:"effective"`
	// Inheritable is the capabilities preserved across execve.
	Inheritable []string `json:"inheritable"`
	// Permitted is the limiting superset for effective capabilities.
	Permitted []string `json:"permitted"`
	// Ambient is the ambient set of capabilities that are kept.
	Ambient []string `json:"ambient"`
}

// Process describes a process running on a container inside a sandbox.
type Process struct {
	// Args specifies the binary and arguments for the application to execute.
	Args []string `json:"args"`

	// Rlimits specifies rlimit options to apply to the process.
	Rlimits []Rlimit `json:"rlimits,omitempty"`

	// Envs populates the process environment for the process.
	Envs []EnvironmentVar `json:"envs,omitempty"`

	AdditionalGroups []string `json:"additionalGroups,omitempty"`

	// Workdir is the current working directory for the process and must be
	// relative to the container's root.
	Workdir string `json:"workdir"`

	User  string `json:"user,omitempty"`
	Group string `json:"group,omitempty"`
	// Sequeue number for stdin and stdout
	Stdio uint64 `json:"stdio,omitempty"`
	// Sequeue number for stderr if it is not shared with stdout
	Stderr uint64 `json:"stderr,omitempty"`
	// NoNewPrivileges indicates that the process should not gain any additional privileges
	Capabilities Capabilities `json:"capabilities"`

	NoNewPrivileges bool `json:"noNewPrivileges"`
	// Capabilities specifies the sets of capabilities for the process(es) inside the container.
	// Terminal creates an interactive terminal for the process.
	Terminal bool `json:"terminal"`
}

// SystemMountsInfo describes additional information for system mounts that the agent
// needs to handle
type SystemMountsInfo struct {
	// Indicates if /dev has been passed as a bind mount for the host /dev
	BindMountDev bool `json:"bindMountDev"`

	// Size of /dev/shm assigned on the host.
	DevShmSize int `json:"devShmSize"`
}

// Constraints describes the constrains for a container
type Constraints struct {
	// CPUQuota specifies the total amount of time in microseconds
	// The number of microseconds per CPUPeriod that the container is guaranteed CPU access
	CPUQuota int64

	// CPUPeriod specifies the CPU CFS scheduler period of time in microseconds
	CPUPeriod uint64

	// CPUShares specifies container's weight vs. other containers
	CPUShares uint64
}

// Container describes a container running on a sandbox.
type Container struct {
	ID               string              `json:"id"`
	Rootfs           string              `json:"rootfs"`
	Fstype           string              `json:"fstype,omitempty"`
	Image            string              `json:"image"`
	SCSIAddr         string              `json:"scsiAddr,omitempty"`
	Volumes          []*VolumeDescriptor `json:"volumes,omitempty"`
	Fsmap            []*FsmapDescriptor  `json:"fsmap,omitempty"`
	Sysctl           map[string]string   `json:"sysctl,omitempty"`
	Process          *Process            `json:"process"`
	RestartPolicy    string              `json:"restartPolicy"`
	Initialize       bool                `json:"initialize"`
	SystemMountsInfo SystemMountsInfo    `json:"systemMountsInfo"`
	Constraints      Constraints         `json:"constraints"`
}

// IPAddress describes an IP address and its network mask.
type IPAddress struct {
	IPAddress string `json:"ipAddress"`
	NetMask   string `json:"netMask"`
}

// NetworkIface describes a network interface to setup on the host.
type NetworkIface struct {
	Device      string      `json:"device,omitempty"`
	NewDevice   string      `json:"newDeviceName,omitempty"`
	IPAddresses []IPAddress `json:"ipAddresses"`
	MTU         int         `json:"mtu"`
	MACAddr     string      `json:"macAddr"`
}

// Route describes a route to setup on the host.
type Route struct {
	Dest    string `json:"dest"`
	Gateway string `json:"gateway,omitempty"`
	Device  string `json:"device,omitempty"`
}

// Sandbox describes the sandbox configuration to start inside the VM.
type Sandbox struct {
	Hostname   string         `json:"hostname"`
	Containers []Container    `json:"containers,omitempty"`
	Interfaces []NetworkIface `json:"interfaces,omitempty"`
	DNS        []string       `json:"dns,omitempty"`
	Routes     []Route        `json:"routes,omitempty"`
	ShareDir   string         `json:"shareDir"`
}
