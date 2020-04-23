// Copyright (c) 2019 hyper.sh
//
// SPDX-License-Identifier: Apache-2.0
//

package types

// ContainerState is a sandbox state structure.
type ContainerState struct {
	State StateString `json:"state"`

	BlockDeviceID string

	// File system of the rootfs incase it is block device
	Fstype string `json:"fstype"`

	// CgroupPath is the cgroup hierarchy where sandbox's processes
	// including the hypervisor are placed.
	CgroupPath string `json:"cgroupPath,omitempty"`
}

// Valid checks that the container state is valid.
func (state *ContainerState) Valid() bool {
	return state.State.valid()
}

// ValidTransition returns an error if we want to move to
// an unreachable state.
func (state *ContainerState) ValidTransition(oldState StateString, newState StateString) error {
	return state.State.validTransition(oldState, newState)
}
