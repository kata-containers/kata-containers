// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	//"fmt"

	//"github.com/sirupsen/logrus"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

func (s *Sandbox) dumpState(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
	ss.SandboxContainer = s.id
	ss.GuestMemoryBlockSizeMB = s.state.GuestMemoryBlockSizeMB
	ss.State = string(s.state.State)

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

// Restore will restore data from persist disk on disk
func (s *Sandbox) Restore() error {
	if err := s.newStore.Restore(s.id); err != nil {
		return err
	}

	ss, _, err := s.newStore.GetStates()
	if err != nil {
		return err
	}

	/*
		// TODO: need more modifications, restoring containers
		// will make sandbox.addContainer failing
		if s.containers == nil {
			s.containers = make(map[string]*Container)
		}

		for id, cont := range cs {
			s.containers[id] = &Container{
				state: State{
					State:         stateString(cont.State),
					BlockDeviceID: cont.Rootfs.BlockDeviceID,
					Fstype:        cont.Rootfs.FsType,
					Pid:           cont.ShimPid,
				},
			}
		}

		sbxCont, ok := s.containers[ss.SandboxContainer]
		if !ok {
			return fmt.Errorf("failed to get sandbox container state")
		}
	*/
	s.state.GuestMemoryBlockSizeMB = ss.GuestMemoryBlockSizeMB
	s.state.BlockIndex = ss.HypervisorState.BlockIndex
	s.state.State = types.StateString(ss.State)

	return nil
}
