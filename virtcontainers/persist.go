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

func (s *Sandbox) dumpVersion(ss *persistapi.SandboxState) {
	// New created sandbox has a uninitialized `PersistVersion` which should be set to current version when do the first saving;
	// Old restored sandbox should keep its original version and shouldn't be modified any more after it's initialized.
	ss.PersistVersion = s.state.PersistVersion
	if ss.PersistVersion == 0 {
		ss.PersistVersion = persistapi.CurPersistVersion
	}
}

func (s *Sandbox) dumpState(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) {
	ss.SandboxContainer = s.id
	ss.GuestMemoryBlockSizeMB = s.state.GuestMemoryBlockSizeMB
	ss.GuestMemoryHotplugProbe = s.state.GuestMemoryHotplugProbe
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
}

func (s *Sandbox) dumpHypervisor(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) {
	ss.HypervisorState.BlockIndex = s.state.BlockIndex
}

func deviceToDeviceState(devices []api.Device) (dss []persistapi.DeviceState) {
	for _, dev := range devices {
		dss = append(dss, dev.Save())
	}
	return
}

func (s *Sandbox) dumpDevices(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) {
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
}

func (s *Sandbox) dumpProcess(cs map[string]persistapi.ContainerState) {
	for id, cont := range s.containers {
		state := persistapi.ContainerState{}
		if v, ok := cs[id]; ok {
			state = v
		}

		state.Process = persistapi.Process{
			Token:     cont.process.Token,
			Pid:       cont.process.Pid,
			StartTime: cont.process.StartTime,
		}

		cs[id] = state
	}

	// delete removed containers
	for id := range cs {
		if _, ok := s.containers[id]; !ok {
			delete(cs, id)
		}
	}
}

func (s *Sandbox) dumpMounts(cs map[string]persistapi.ContainerState) {
	for id, cont := range s.containers {
		state := persistapi.ContainerState{}
		if v, ok := cs[id]; ok {
			state = v
		}

		for _, m := range cont.mounts {
			state.Mounts = append(state.Mounts, persistapi.Mount{
				Source:        m.Source,
				Destination:   m.Destination,
				Options:       m.Options,
				HostPath:      m.HostPath,
				ReadOnly:      m.ReadOnly,
				BlockDeviceID: m.BlockDeviceID,
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
}

func (s *Sandbox) Save() error {
	var (
		ss = persistapi.SandboxState{}
		cs = make(map[string]persistapi.ContainerState)
	)

	s.dumpVersion(&ss)
	s.dumpState(&ss, cs)
	s.dumpHypervisor(&ss, cs)
	s.dumpDevices(&ss, cs)
	s.dumpProcess(cs)
	s.dumpMounts(cs)

	if err := s.newStore.ToDisk(ss, cs); err != nil {
		return err
	}

	return nil
}

func (s *Sandbox) loadState(ss persistapi.SandboxState) {
	s.state.PersistVersion = ss.PersistVersion
	s.state.GuestMemoryBlockSizeMB = ss.GuestMemoryBlockSizeMB
	s.state.BlockIndex = ss.HypervisorState.BlockIndex
	s.state.State = types.StateString(ss.State)
	s.state.CgroupPath = ss.CgroupPath
	s.state.GuestMemoryHotplugProbe = ss.GuestMemoryHotplugProbe
}

func (c *Container) loadContState(cs persistapi.ContainerState) {
	c.state = types.ContainerState{
		State:         types.StateString(cs.State),
		BlockDeviceID: cs.Rootfs.BlockDeviceID,
		Fstype:        cs.Rootfs.FsType,
		CgroupPath:    cs.CgroupPath,
	}
}

func (s *Sandbox) loadDevices(devStates []persistapi.DeviceState) {
	s.devManager.LoadDevices(devStates)
}

func (c *Container) loadContDevices(cs persistapi.ContainerState) {
	c.devices = nil
	for _, dev := range cs.DeviceMaps {
		c.devices = append(c.devices, ContainerDevice{
			ID:            dev.ID,
			ContainerPath: dev.ContainerPath,
			FileMode:      dev.FileMode,
			UID:           dev.UID,
			GID:           dev.GID,
		})
	}
}

func (c *Container) loadContMounts(cs persistapi.ContainerState) {
	c.mounts = nil
	for _, m := range cs.Mounts {
		c.mounts = append(c.mounts, Mount{
			Source:        m.Source,
			Destination:   m.Destination,
			Options:       m.Options,
			HostPath:      m.HostPath,
			ReadOnly:      m.ReadOnly,
			BlockDeviceID: m.BlockDeviceID,
		})
	}
}

func (c *Container) loadContProcess(cs persistapi.ContainerState) {
	c.process = Process{
		Token:     cs.Process.Token,
		Pid:       cs.Process.Pid,
		StartTime: cs.Process.StartTime,
	}
}

// Restore will restore sandbox data from persist file on disk
func (s *Sandbox) Restore() error {
	ss, _, err := s.newStore.FromDisk(s.id)
	if err != nil {
		return err
	}

	s.loadState(ss)
	s.loadDevices(ss.Devices)
	return nil
}

// Restore will restore container data from persist file on disk
func (c *Container) Restore() error {
	_, css, err := c.sandbox.newStore.FromDisk(c.sandbox.id)
	if err != nil {
		return err
	}

	cs, ok := css[c.id]
	if !ok {
		return errContainerPersistNotExist
	}

	c.loadContState(cs)
	c.loadContDevices(cs)
	c.loadContProcess(cs)
	c.loadContMounts(cs)
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
