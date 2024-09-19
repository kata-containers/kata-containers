// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"errors"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/api"
	devconfig "github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
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
	ss.SandboxCgroupPath = s.state.SandboxCgroupPath
	ss.OverheadCgroupPath = s.state.OverheadCgroupPath

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

func (s *Sandbox) dumpHypervisor(ss *persistapi.SandboxState) {
	ss.HypervisorState = s.hypervisor.Save()
	// BlockIndexMap will be moved from sandbox state to hypervisor state later
	ss.HypervisorState.BlockIndexMap = s.state.BlockIndexMap
}

func deviceToDeviceState(devices []api.Device) (dss []devconfig.DeviceState) {
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

func (s *Sandbox) dumpAgent(ss *persistapi.SandboxState) {
	if s.agent != nil {
		ss.AgentState = s.agent.save()
	}
}

func (s *Sandbox) dumpNetwork(ss *persistapi.SandboxState) {
	ss.Network = persistapi.NetworkInfo{
		NetworkID:      s.network.NetworkID(),
		NetworkCreated: s.network.NetworkCreated(),
	}
	for _, e := range s.network.Endpoints() {
		ss.Network.Endpoints = append(ss.Network.Endpoints, e.save())
	}
}

func (s *Sandbox) dumpConfig(ss *persistapi.SandboxState) {
	sconfig := s.config
	ss.Config = persistapi.SandboxConfig{
		HypervisorType: string(sconfig.HypervisorType),
		NetworkConfig: persistapi.NetworkConfig{
			NetworkID:         sconfig.NetworkConfig.NetworkID,
			NetworkCreated:    sconfig.NetworkConfig.NetworkCreated,
			DisableNewNetwork: sconfig.NetworkConfig.DisableNewNetwork,
			InterworkingModel: int(sconfig.NetworkConfig.InterworkingModel),
		},

		ShmSize:             sconfig.ShmSize,
		SharePidNs:          sconfig.SharePidNs,
		SystemdCgroup:       sconfig.SystemdCgroup,
		SandboxCgroupOnly:   sconfig.SandboxCgroupOnly,
		DisableGuestSeccomp: sconfig.DisableGuestSeccomp,
		EnableVCPUsPinning:  sconfig.EnableVCPUsPinning,
		GuestSeLinuxLabel:   sconfig.GuestSeLinuxLabel,
	}

	ss.Config.SandboxBindMounts = append(ss.Config.SandboxBindMounts, sconfig.SandboxBindMounts...)

	for _, e := range sconfig.Experimental {
		ss.Config.Experimental = append(ss.Config.Experimental, e.Name)
	}

	ss.Config.HypervisorConfig = persistapi.HypervisorConfig{
		NumVCPUsF:               sconfig.HypervisorConfig.NumVCPUsF,
		DefaultMaxVCPUs:         sconfig.HypervisorConfig.DefaultMaxVCPUs,
		MemorySize:              sconfig.HypervisorConfig.MemorySize,
		DefaultBridges:          sconfig.HypervisorConfig.DefaultBridges,
		Msize9p:                 sconfig.HypervisorConfig.Msize9p,
		MemSlots:                sconfig.HypervisorConfig.MemSlots,
		MemOffset:               sconfig.HypervisorConfig.MemOffset,
		VirtioMem:               sconfig.HypervisorConfig.VirtioMem,
		VirtioFSCacheSize:       sconfig.HypervisorConfig.VirtioFSCacheSize,
		KernelPath:              sconfig.HypervisorConfig.KernelPath,
		ImagePath:               sconfig.HypervisorConfig.ImagePath,
		InitrdPath:              sconfig.HypervisorConfig.InitrdPath,
		FirmwarePath:            sconfig.HypervisorConfig.FirmwarePath,
		MachineAccelerators:     sconfig.HypervisorConfig.MachineAccelerators,
		CPUFeatures:             sconfig.HypervisorConfig.CPUFeatures,
		HypervisorPath:          sconfig.HypervisorConfig.HypervisorPath,
		HypervisorPathList:      sconfig.HypervisorConfig.HypervisorPathList,
		JailerPath:              sconfig.HypervisorConfig.JailerPath,
		JailerPathList:          sconfig.HypervisorConfig.JailerPathList,
		BlockDeviceDriver:       sconfig.HypervisorConfig.BlockDeviceDriver,
		HypervisorMachineType:   sconfig.HypervisorConfig.HypervisorMachineType,
		MemoryPath:              sconfig.HypervisorConfig.MemoryPath,
		DevicesStatePath:        sconfig.HypervisorConfig.DevicesStatePath,
		EntropySource:           sconfig.HypervisorConfig.EntropySource,
		EntropySourceList:       sconfig.HypervisorConfig.EntropySourceList,
		SharedFS:                sconfig.HypervisorConfig.SharedFS,
		VirtioFSDaemon:          sconfig.HypervisorConfig.VirtioFSDaemon,
		VirtioFSDaemonList:      sconfig.HypervisorConfig.VirtioFSDaemonList,
		VirtioFSCache:           sconfig.HypervisorConfig.VirtioFSCache,
		VirtioFSExtraArgs:       sconfig.HypervisorConfig.VirtioFSExtraArgs[:],
		BlockDeviceCacheSet:     sconfig.HypervisorConfig.BlockDeviceCacheSet,
		BlockDeviceCacheDirect:  sconfig.HypervisorConfig.BlockDeviceCacheDirect,
		BlockDeviceCacheNoflush: sconfig.HypervisorConfig.BlockDeviceCacheNoflush,
		DisableBlockDeviceUse:   sconfig.HypervisorConfig.DisableBlockDeviceUse,
		EnableIOThreads:         sconfig.HypervisorConfig.EnableIOThreads,
		Debug:                   sconfig.HypervisorConfig.Debug,
		MemPrealloc:             sconfig.HypervisorConfig.MemPrealloc,
		HugePages:               sconfig.HypervisorConfig.HugePages,
		FileBackedMemRootDir:    sconfig.HypervisorConfig.FileBackedMemRootDir,
		FileBackedMemRootList:   sconfig.HypervisorConfig.FileBackedMemRootList,
		DisableNestingChecks:    sconfig.HypervisorConfig.DisableNestingChecks,
		DisableImageNvdimm:      sconfig.HypervisorConfig.DisableImageNvdimm,
		BootToBeTemplate:        sconfig.HypervisorConfig.BootToBeTemplate,
		BootFromTemplate:        sconfig.HypervisorConfig.BootFromTemplate,
		DisableVhostNet:         sconfig.HypervisorConfig.DisableVhostNet,
		EnableVhostUserStore:    sconfig.HypervisorConfig.EnableVhostUserStore,
		SeccompSandbox:          sconfig.HypervisorConfig.SeccompSandbox,
		VhostUserStorePath:      sconfig.HypervisorConfig.VhostUserStorePath,
		VhostUserStorePathList:  sconfig.HypervisorConfig.VhostUserStorePathList,
		GuestHookPath:           sconfig.HypervisorConfig.GuestHookPath,
		VMid:                    sconfig.HypervisorConfig.VMid,
		RxRateLimiterMaxRate:    sconfig.HypervisorConfig.RxRateLimiterMaxRate,
		TxRateLimiterMaxRate:    sconfig.HypervisorConfig.TxRateLimiterMaxRate,
		SGXEPCSize:              sconfig.HypervisorConfig.SGXEPCSize,
		EnableAnnotations:       sconfig.HypervisorConfig.EnableAnnotations,
	}

	ss.Config.KataAgentConfig = &persistapi.KataAgentConfig{
		LongLiveConn: sconfig.AgentConfig.LongLiveConn,
	}

	for _, contConf := range sconfig.Containers {
		ss.Config.ContainerConfigs = append(ss.Config.ContainerConfigs, persistapi.ContainerConfig{
			ID:          contConf.ID,
			Annotations: contConf.Annotations,
			RootFs:      contConf.RootFs.Target,
			Resources:   contConf.Resources,
		})
	}
}

func (s *Sandbox) Save() error {
	var (
		ss = persistapi.SandboxState{}
		cs = make(map[string]persistapi.ContainerState)
	)

	s.dumpVersion(&ss)
	s.dumpState(&ss, cs)
	s.dumpHypervisor(&ss)
	s.dumpDevices(&ss, cs)
	s.dumpProcess(cs)
	s.dumpMounts(cs)
	s.dumpAgent(&ss)
	s.dumpNetwork(&ss)
	s.dumpConfig(&ss)

	if err := s.store.ToDisk(ss, cs); err != nil {
		return err
	}

	return nil
}

func (s *Sandbox) loadState(ss persistapi.SandboxState) {
	s.state.PersistVersion = ss.PersistVersion
	s.state.GuestMemoryBlockSizeMB = ss.GuestMemoryBlockSizeMB
	s.state.BlockIndexMap = ss.HypervisorState.BlockIndexMap
	s.state.State = types.StateString(ss.State)
	s.state.SandboxCgroupPath = ss.SandboxCgroupPath
	s.state.OverheadCgroupPath = ss.OverheadCgroupPath
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

func (s *Sandbox) loadHypervisor(hs hv.HypervisorState) {
	s.hypervisor.Load(hs)
}

func (s *Sandbox) loadAgent(as persistapi.AgentState) {
	if s.agent != nil {
		s.agent.load(as)
	}
}

func (s *Sandbox) loadDevices(devStates []devconfig.DeviceState) {
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

func (s *Sandbox) loadNetwork(netInfo persistapi.NetworkInfo) {
	s.network = LoadNetwork(netInfo)
}

// Restore will restore sandbox data from persist file on disk
func (s *Sandbox) Restore() error {
	ss, _, err := s.store.FromDisk(s.id)
	if err != nil {
		return err
	}

	s.loadState(ss)
	s.loadHypervisor(ss.HypervisorState)
	s.loadDevices(ss.Devices)
	s.loadAgent(ss.AgentState)
	s.loadNetwork(ss.Network)
	return nil
}

// Restore will restore container data from persist file on disk
func (c *Container) Restore() error {
	_, css, err := c.sandbox.store.FromDisk(c.sandbox.id)
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

func loadSandboxConfig(id string) (*SandboxConfig, error) {
	store, err := persist.GetDriver()
	if err != nil || store == nil {
		return nil, errors.New("failed to get fs persist driver")
	}

	ss, _, err := store.FromDisk(id)
	if err != nil {
		return nil, err
	}

	savedConf := ss.Config
	sconfig := &SandboxConfig{
		ID:             id,
		HypervisorType: HypervisorType(savedConf.HypervisorType),
		NetworkConfig: NetworkConfig{
			NetworkID:         savedConf.NetworkConfig.NetworkID,
			NetworkCreated:    savedConf.NetworkConfig.NetworkCreated,
			DisableNewNetwork: savedConf.NetworkConfig.DisableNewNetwork,
			InterworkingModel: NetInterworkingModel(savedConf.NetworkConfig.InterworkingModel),
		},

		ShmSize:             savedConf.ShmSize,
		SharePidNs:          savedConf.SharePidNs,
		SystemdCgroup:       savedConf.SystemdCgroup,
		SandboxCgroupOnly:   savedConf.SandboxCgroupOnly,
		DisableGuestSeccomp: savedConf.DisableGuestSeccomp,
		EnableVCPUsPinning:  savedConf.EnableVCPUsPinning,
		GuestSeLinuxLabel:   savedConf.GuestSeLinuxLabel,
	}
	sconfig.SandboxBindMounts = append(sconfig.SandboxBindMounts, savedConf.SandboxBindMounts...)

	for _, name := range savedConf.Experimental {
		sconfig.Experimental = append(sconfig.Experimental, *exp.Get(name))
	}

	hconf := savedConf.HypervisorConfig
	sconfig.HypervisorConfig = HypervisorConfig{
		NumVCPUsF:               hconf.NumVCPUsF,
		DefaultMaxVCPUs:         hconf.DefaultMaxVCPUs,
		MemorySize:              hconf.MemorySize,
		DefaultBridges:          hconf.DefaultBridges,
		Msize9p:                 hconf.Msize9p,
		MemSlots:                hconf.MemSlots,
		MemOffset:               hconf.MemOffset,
		VirtioMem:               hconf.VirtioMem,
		VirtioFSCacheSize:       hconf.VirtioFSCacheSize,
		KernelPath:              hconf.KernelPath,
		ImagePath:               hconf.ImagePath,
		InitrdPath:              hconf.InitrdPath,
		FirmwarePath:            hconf.FirmwarePath,
		MachineAccelerators:     hconf.MachineAccelerators,
		CPUFeatures:             hconf.CPUFeatures,
		HypervisorPath:          hconf.HypervisorPath,
		HypervisorPathList:      hconf.HypervisorPathList,
		JailerPath:              hconf.JailerPath,
		JailerPathList:          hconf.JailerPathList,
		BlockDeviceDriver:       hconf.BlockDeviceDriver,
		HypervisorMachineType:   hconf.HypervisorMachineType,
		MemoryPath:              hconf.MemoryPath,
		DevicesStatePath:        hconf.DevicesStatePath,
		EntropySource:           hconf.EntropySource,
		EntropySourceList:       hconf.EntropySourceList,
		SharedFS:                hconf.SharedFS,
		VirtioFSDaemon:          hconf.VirtioFSDaemon,
		VirtioFSDaemonList:      hconf.VirtioFSDaemonList,
		VirtioFSCache:           hconf.VirtioFSCache,
		VirtioFSExtraArgs:       hconf.VirtioFSExtraArgs[:],
		BlockDeviceCacheSet:     hconf.BlockDeviceCacheSet,
		BlockDeviceCacheDirect:  hconf.BlockDeviceCacheDirect,
		BlockDeviceCacheNoflush: hconf.BlockDeviceCacheNoflush,
		DisableBlockDeviceUse:   hconf.DisableBlockDeviceUse,
		EnableIOThreads:         hconf.EnableIOThreads,
		Debug:                   hconf.Debug,
		MemPrealloc:             hconf.MemPrealloc,
		HugePages:               hconf.HugePages,
		FileBackedMemRootDir:    hconf.FileBackedMemRootDir,
		FileBackedMemRootList:   hconf.FileBackedMemRootList,
		DisableNestingChecks:    hconf.DisableNestingChecks,
		DisableImageNvdimm:      hconf.DisableImageNvdimm,
		HotPlugVFIO:             hconf.HotPlugVFIO,
		ColdPlugVFIO:            hconf.ColdPlugVFIO,
		PCIeRootPort:            hconf.PCIeRootPort,
		PCIeSwitchPort:          hconf.PCIeSwitchPort,
		BootToBeTemplate:        hconf.BootToBeTemplate,
		BootFromTemplate:        hconf.BootFromTemplate,
		DisableVhostNet:         hconf.DisableVhostNet,
		EnableVhostUserStore:    hconf.EnableVhostUserStore,
		VhostUserStorePath:      hconf.VhostUserStorePath,
		VhostUserStorePathList:  hconf.VhostUserStorePathList,
		GuestHookPath:           hconf.GuestHookPath,
		VMid:                    hconf.VMid,
		RxRateLimiterMaxRate:    hconf.RxRateLimiterMaxRate,
		TxRateLimiterMaxRate:    hconf.TxRateLimiterMaxRate,
		SGXEPCSize:              hconf.SGXEPCSize,
		EnableAnnotations:       hconf.EnableAnnotations,
	}

	sconfig.AgentConfig = KataAgentConfig{
		LongLiveConn: savedConf.KataAgentConfig.LongLiveConn,
	}

	for _, contConf := range savedConf.ContainerConfigs {
		sconfig.Containers = append(sconfig.Containers, ContainerConfig{
			ID:          contConf.ID,
			Annotations: contConf.Annotations,
			Resources:   contConf.Resources,
			RootFs: RootFs{
				Target: contConf.RootFs,
			},
		})
	}
	return sconfig, nil
}
