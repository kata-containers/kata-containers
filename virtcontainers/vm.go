// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"os"
	"path/filepath"

	"github.com/kata-containers/runtime/virtcontainers/pkg/uuid"
	"github.com/sirupsen/logrus"
)

// VM is abstraction of a virtual machine.
type VM struct {
	id string

	hypervisor hypervisor
	agent      agent

	cpu    uint32
	memory uint32

	cpuDelta uint32
}

// VMConfig is a collection of all info that a new blackbox VM needs.
type VMConfig struct {
	HypervisorType   HypervisorType
	HypervisorConfig HypervisorConfig

	AgentType   AgentType
	AgentConfig interface{}
}

// Valid check VMConfig validity.
func (c *VMConfig) Valid() error {
	return c.HypervisorConfig.valid()
}

// NewVM creates a new VM based on provided VMConfig.
func NewVM(ctx context.Context, config VMConfig) (*VM, error) {
	hypervisor, err := newHypervisor(config.HypervisorType)
	if err != nil {
		return nil, err
	}

	if err = config.Valid(); err != nil {
		return nil, err
	}

	id := uuid.Generate().String()

	virtLog.WithField("vm id", id).WithField("config", config).Info("create new vm")

	defer func() {
		if err != nil {
			virtLog.WithField("vm id", id).WithError(err).Error("failed to create new vm")
		}
	}()

	if err = hypervisor.init(ctx, id, &config.HypervisorConfig, Resources{}, &filesystem{}); err != nil {
		return nil, err
	}

	if err = hypervisor.createSandbox(); err != nil {
		return nil, err
	}

	agent := newAgent(config.AgentType)
	agentConfig := newAgentConfig(config.AgentType, config.AgentConfig)
	// do not keep connection for temp agent
	if c, ok := agentConfig.(KataAgentConfig); ok {
		c.LongLiveConn = false
	}
	vmSharePath := buildVMSharePath(id)
	err = agent.configure(hypervisor, id, vmSharePath, true, agentConfig)
	if err != nil {
		return nil, err
	}

	if err = hypervisor.startSandbox(); err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			virtLog.WithField("vm id", id).WithError(err).Info("clean up vm")
			hypervisor.stopSandbox()
		}
	}()

	// VMs booted from template are paused, do not check
	if !config.HypervisorConfig.BootFromTemplate {
		err = hypervisor.waitSandbox(vmStartTimeout)
		if err != nil {
			return nil, err
		}

		virtLog.WithField("vm id", id).Info("check agent status")
		err = agent.check()
		if err != nil {
			return nil, err
		}
	}

	return &VM{
		id:         id,
		hypervisor: hypervisor,
		agent:      agent,
		cpu:        config.HypervisorConfig.DefaultVCPUs,
		memory:     config.HypervisorConfig.DefaultMemSz,
	}, nil
}

func buildVMSharePath(id string) string {
	return filepath.Join(RunVMStoragePath, id, "shared")
}

func (v *VM) logger() logrus.FieldLogger {
	return virtLog.WithField("vm id", v.id)
}

// Pause pauses a VM.
func (v *VM) Pause() error {
	v.logger().Info("pause vm")
	return v.hypervisor.pauseSandbox()
}

// Save saves a VM to persistent disk.
func (v *VM) Save() error {
	v.logger().Info("save vm")
	return v.hypervisor.saveSandbox()
}

// Resume resumes a paused VM.
func (v *VM) Resume() error {
	v.logger().Info("resume vm")
	return v.hypervisor.resumeSandbox()
}

// Start kicks off a configured VM.
func (v *VM) Start() error {
	v.logger().Info("start vm")
	return v.hypervisor.startSandbox()
}

// Stop stops a VM process.
func (v *VM) Stop() error {
	v.logger().Info("kill vm")
	return v.hypervisor.stopSandbox()
}

// AddCPUs adds num of CPUs to the VM.
func (v *VM) AddCPUs(num uint32) error {
	if num > 0 {
		v.logger().Infof("hot adding %d vCPUs", num)
		if _, err := v.hypervisor.hotplugAddDevice(num, cpuDev); err != nil {
			return err
		}
		v.cpuDelta += num
		v.cpu += num
	}

	return nil
}

// AddMemory adds numMB of memory to the VM.
func (v *VM) AddMemory(numMB uint32) error {
	if numMB > 0 {
		v.logger().Infof("hot adding %d MB memory", numMB)
		dev := &memoryDevice{1, int(numMB)}
		if _, err := v.hypervisor.hotplugAddDevice(dev, memoryDev); err != nil {
			return err
		}
	}

	return nil
}

// OnlineCPUMemory puts the hotplugged CPU and memory online.
func (v *VM) OnlineCPUMemory() error {
	v.logger().Infof("online CPU %d and memory", v.cpuDelta)
	err := v.agent.onlineCPUMem(v.cpuDelta)
	if err == nil {
		v.cpuDelta = 0
	}

	return err
}

// ReseedRNG adds random entropy to guest random number generator
// and reseeds it.
func (v *VM) ReseedRNG() error {
	v.logger().Infof("reseed guest random number generator")
	urandomDev := "/dev/urandom"
	data := make([]byte, 512)
	f, err := os.OpenFile(urandomDev, os.O_RDONLY, 0)
	if err != nil {
		v.logger().WithError(err).Warn("fail to open %s", urandomDev)
		return err
	}
	defer f.Close()
	if _, err = f.Read(data); err != nil {
		v.logger().WithError(err).Warn("fail to read %s", urandomDev)
		return err
	}

	return v.agent.reseedRNG(data)
}

func (v *VM) assignSandbox(s *Sandbox) error {
	// add vm symlinks
	// - link vm socket from sandbox dir (/run/vc/vm/sbid/<kata.sock>) to vm dir (/run/vc/vm/vmid/<kata.sock>)
	// - link 9pfs share path from sandbox dir (/run/kata-containers/shared/sandboxes/sbid/) to vm dir (/run/vc/vm/vmid/shared/)

	vmSharePath := buildVMSharePath(v.id)
	vmSockDir := v.agent.getVMPath(v.id)
	sbSharePath := s.agent.getSharePath(s.id)
	sbSockDir := s.agent.getVMPath(s.id)

	v.logger().WithFields(logrus.Fields{
		"vmSharePath": vmSharePath,
		"vmSockDir":   vmSockDir,
		"sbSharePath": sbSharePath,
		"sbSockDir":   sbSockDir,
	}).Infof("assign vm to sandbox %s", s.id)

	// First make sure the symlinks do not exist
	os.RemoveAll(sbSharePath)
	os.RemoveAll(sbSockDir)

	if err := os.Symlink(vmSharePath, sbSharePath); err != nil {
		return err
	}

	if err := os.Symlink(vmSockDir, sbSockDir); err != nil {
		os.Remove(sbSharePath)
		return err
	}

	s.hypervisor = v.hypervisor

	return nil
}
