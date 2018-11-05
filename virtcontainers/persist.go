// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	//"fmt"

	//"github.com/sirupsen/logrus"

	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

func (s *Sandbox) dumpState(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
	ss.SandboxContainer = s.id
	ss.GuestMemoryBlockSizeMB = s.state.GuestMemoryBlockSizeMB
	ss.State = string(s.state.State)

	for id, cont := range s.containers {
		cs[id] = persistapi.ContainerState{
			State: string(cont.state.State),
			Rootfs: persistapi.RootfsState{
				BlockDeviceID: cont.state.BlockDeviceID,
				FsType:        cont.state.Fstype,
			},
		}
	}
	return nil
}

func (s *Sandbox) dumpHypervisor(ss *persistapi.SandboxState, cs map[string]persistapi.ContainerState) error {
	ss.HypervisorState.BlockIndex = s.state.BlockIndex
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
