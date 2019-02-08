// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

var (
	errSandboxPersistNotExist   = errors.New("sandbox doesn't exist in persist data")
	errContainerPersistNotExist = errors.New("container doesn't exist in persist data")
)

func (s *Sandbox) dumpState(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
	ss.SandboxContainer = s.id
	ss.GuestMemoryBlockSizeMB = s.state.GuestMemoryBlockSizeMB
	ss.State = string(s.state.State)
	ss.CgroupPath = s.state.CgroupPath

	for id, cont := range s.containers {
		state := persistapi.ContainerState{}
		if v, ok := cs[id]; ok {
			state = v
		}
		state.State = string(cont.state.State)
		state.Rootfs = persistapi.RootfsState{
			BlockDeviceID: cont.state.BlockDeviceID,
			FsType:        cont.state.Fstype,
		}
		state.CgroupPath = cont.state.CgroupPath
		cs[id] = state
	}

	// delete removed containers
	for id := range cs {
		if _, ok := s.containers[id]; !ok {
			delete(cs, id)
		}
	}

	return nil
}

func (s *Sandbox) dumpHypervisor(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
	ss.HypervisorState.BlockIndex = s.state.BlockIndex
	return nil
}

func deviceToDeviceState(devices []api.Device) (dss []persistapi.DeviceState) {
	for _, dev := range devices {
		dss = append(dss, dev.Dump())
	}
	return
}

func (s *Sandbox) dumpDevices(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
	ss.Devices = deviceToDeviceState(s.devManager.GetAllDevices())

	for id, cont := range s.containers {
		state := persistapi.ContainerState{}
		if v, ok := cs[id]; ok {
			state = v
		}

		state.DeviceMaps = nil
		for _, dev := range cont.devices {
			state.DeviceMaps = append(state.DeviceMaps, persistapi.DeviceMap{
				ID:            dev.ID,
				ContainerPath: dev.ContainerPath,
				FileMode:      dev.FileMode,
				UID:           dev.UID,
				GID:           dev.GID,
			})
		}

		cs[id] = state
	}

	// delete removed containers
	for id := range cs {
		if _, ok := s.containers[id]; !ok {
			delete(cs, id)
		}
	}

	return nil
}

// PersistVersion set persist data version to current version in runtime
func (s *Sandbox) persistVersion() {
	s.newStore.RegisterHook("version", func(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
		ss.PersistVersion = persistapi.CurPersistVersion
		return nil
	})
}

// PersistState register hook to set sandbox and container state to persist
func (s *Sandbox) persistState() {
	s.newStore.RegisterHook("state", s.dumpState)
}

// PersistHvState register hook to save hypervisor state to persist data
func (s *Sandbox) persistHvState() {
	s.newStore.RegisterHook("hypervisor", s.dumpHypervisor)
}

// PersistDevices register hook to save device informations
func (s *Sandbox) persistDevices() {
	s.newStore.RegisterHook("devices", s.dumpDevices)
}

func (s *Sandbox) getSbxAndCntStates() (*persistapi.SandboxState, map[string]persistapi.ContainerState, error) {
	ss, cs, err := s.newStore.GetStates()
	if err != nil {
		return nil, nil, err
	}

	if len(cs) == 0 {
		if err := s.newStore.Restore(s.id); err != nil {
			return nil, nil, err
		}

		ss, cs, err = s.newStore.GetStates()
		if err != nil {
			return nil, nil, err
		}

		if len(cs) == 0 {
			return nil, nil, errSandboxPersistNotExist
		}
	}
	return ss, cs, nil
}

// Restore will restore sandbox data from persist file on disk
func (s *Sandbox) Restore() error {
	ss, _, err := s.getSbxAndCntStates()
	if err != nil {
		return err
	}

	s.state.GuestMemoryBlockSizeMB = ss.GuestMemoryBlockSizeMB
	s.state.BlockIndex = ss.HypervisorState.BlockIndex
	s.state.State = types.StateString(ss.State)
	s.state.CgroupPath = ss.CgroupPath

	return nil
}

// Restore will restore container data from persist file on disk
func (c *Container) Restore() error {
	_, cs, err := c.sandbox.getSbxAndCntStates()
	if err != nil {
		return err
	}

	if _, ok := cs[c.id]; !ok {
		return errContainerPersistNotExist
	}

	c.state = types.ContainerState{
		State:         types.StateString(cs[c.id].State),
		BlockDeviceID: cs[c.id].Rootfs.BlockDeviceID,
		Fstype:        cs[c.id].Rootfs.FsType,
		CgroupPath:    cs[c.id].CgroupPath,
	}

	return nil
}
