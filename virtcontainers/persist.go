// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	exp "github.com/kata-containers/runtime/virtcontainers/experimental"
	"github.com/kata-containers/runtime/virtcontainers/persist"
	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

var (
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
		state.Rootfs = persistapi.Mount{
			BlockDeviceID: cont.rootFs.BlockDeviceID,
			Type:          cont.rootFs.Type,
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

// verSaveCallback set persist data version to current version in runtime
func (s *Sandbox) verSaveCallback() {
	s.newStore.AddSaveCallback("version", func(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
		ss.PersistVersion = persistapi.CurPersistVersion
		return nil
	})
}

// stateSaveCallback register hook to set sandbox and container state to persist
func (s *Sandbox) stateSaveCallback() {
	s.newStore.AddSaveCallback("state", s.dumpState)
}

// hvStateSaveCallback register hook to save hypervisor state to persist data
func (s *Sandbox) hvStateSaveCallback() {
	s.newStore.AddSaveCallback("hypervisor", s.dumpHypervisor)
}

// PersistDevices register hook to save device informations
func (s *Sandbox) devicesSaveCallback() {
	s.newStore.AddSaveCallback("devices", s.dumpDevices)
}

func (s *Sandbox) getSbxAndCntStates() (*persistapi.SandboxState, map[string]persistapi.ContainerState, error) {
	if err := s.newStore.FromDisk(s.id); err != nil {
		return nil, nil, err
	}

	return s.newStore.GetStates()
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
		State:      types.StateString(cs[c.id].State),
		CgroupPath: cs[c.id].CgroupPath,
	}

	return nil
}

func (s *Sandbox) supportNewStore() bool {
	for _, f := range s.config.Experimental {
		if f == persist.NewStoreFeature && exp.Get("newstore") != nil {
			return true
		}
	}
	return false
}
