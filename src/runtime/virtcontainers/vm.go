// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"time"

	pb "github.com/kata-containers/kata-containers/src/runtime/protocols/cache"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist"
	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/uuid"
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

	store persistapi.PersistDriver
}

// VMConfig is a collection of all info that a new blackbox VM needs.
type VMConfig struct {
	HypervisorType   HypervisorType
	HypervisorConfig HypervisorConfig

	AgentConfig KataAgentConfig
}

// Valid check VMConfig validity.
func (c *VMConfig) Valid() error {
	return c.HypervisorConfig.valid()
}

// ToGrpc convert VMConfig struct to grpc format pb.GrpcVMConfig.
func (c *VMConfig) ToGrpc() (*pb.GrpcVMConfig, error) {
	data, err := json.Marshal(&c)
	if err != nil {
		return nil, err
	}

	agentConfig, err := json.Marshal(&c.AgentConfig)
	if err != nil {
		return nil, err
	}

	return &pb.GrpcVMConfig{
		Data:        data,
		AgentConfig: agentConfig,
	}, nil
}

// GrpcToVMConfig convert grpc format pb.GrpcVMConfig to VMConfig struct.
func GrpcToVMConfig(j *pb.GrpcVMConfig) (*VMConfig, error) {
	var config VMConfig
	err := json.Unmarshal(j.Data, &config)
	if err != nil {
		return nil, err
	}

	var kataConfig KataAgentConfig
	err = json.Unmarshal(j.AgentConfig, &kataConfig)
	if err == nil {
		config.AgentConfig = kataConfig
	}

	return &config, nil
}

// NewVM creates a new VM based on provided VMConfig.
func NewVM(ctx context.Context, config VMConfig) (*VM, error) {
	// 1. setup hypervisor
	hypervisor, err := newHypervisor(config.HypervisorType)
	if err != nil {
		return nil, err
	}

	if err = config.Valid(); err != nil {
		return nil, err
	}

	id := uuid.Generate().String()

	virtLog.WithField("vm", id).WithField("config", config).Info("create new vm")

	store, err := persist.GetDriver()
	if err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			virtLog.WithField("vm", id).WithError(err).Error("failed to create new vm")
			virtLog.WithField("vm", id).Errorf("Deleting store for %s", id)
			store.Destroy(id)
		}
	}()

	if err = hypervisor.createSandbox(ctx, id, NetworkNamespace{}, &config.HypervisorConfig); err != nil {
		return nil, err
	}

	// 2. setup agent
	newAagentFunc := getNewAgentFunc(ctx)
	agent := newAagentFunc()

	vmSharePath := buildVMSharePath(id, store.RunVMStoragePath())
	err = agent.configure(hypervisor, id, vmSharePath, config.AgentConfig)
	if err != nil {
		return nil, err
	}
	err = agent.setAgentURL()
	if err != nil {
		return nil, err
	}

	// 3. boot up guest vm
	if err = hypervisor.startSandbox(vmStartTimeout); err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			virtLog.WithField("vm", id).WithError(err).Info("clean up vm")
			hypervisor.stopSandbox()
		}
	}()

	// 4. check agent aliveness
	// VMs booted from template are paused, do not check
	if !config.HypervisorConfig.BootFromTemplate {
		virtLog.WithField("vm", id).Info("check agent status")
		err = agent.check()
		if err != nil {
			return nil, err
		}
	}

	return &VM{
		id:         id,
		hypervisor: hypervisor,
		agent:      agent,
		cpu:        config.HypervisorConfig.NumVCPUs,
		memory:     config.HypervisorConfig.MemorySize,
		store:      store,
	}, nil
}

// NewVMFromGrpc creates a new VM based on provided pb.GrpcVM and VMConfig.
func NewVMFromGrpc(ctx context.Context, v *pb.GrpcVM, config VMConfig) (*VM, error) {
	virtLog.WithField("GrpcVM", v).WithField("config", config).Info("create new vm from Grpc")

	hypervisor, err := newHypervisor(config.HypervisorType)
	if err != nil {
		return nil, err
	}

	store, err := persist.GetDriver()
	if err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			virtLog.WithField("vm", v.Id).WithError(err).Error("failed to create new vm from Grpc")
			virtLog.WithField("vm", v.Id).Errorf("Deleting store for %s", v.Id)
			store.Destroy(v.Id)
		}
	}()

	err = hypervisor.fromGrpc(ctx, &config.HypervisorConfig, v.Hypervisor)
	if err != nil {
		return nil, err
	}

	// create agent instance
	newAagentFunc := getNewAgentFunc(ctx)
	agent := newAagentFunc()
	agent.configureFromGrpc(hypervisor, v.Id, config.AgentConfig)

	return &VM{
		id:         v.Id,
		hypervisor: hypervisor,
		agent:      agent,
		cpu:        v.Cpu,
		memory:     v.Memory,
		cpuDelta:   v.CpuDelta,
		store:      store,
	}, nil
}

func buildVMSharePath(id string, vmStoragePath string) string {
	return filepath.Join(vmStoragePath, id, "shared")
}

func (v *VM) logger() logrus.FieldLogger {
	return virtLog.WithField("vm", v.id)
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
	return v.hypervisor.startSandbox(vmStartTimeout)
}

// Disconnect agent connections to a VM
func (v *VM) Disconnect() error {
	v.logger().Info("kill vm")

	if err := v.agent.disconnect(); err != nil {
		v.logger().WithError(err).Error("failed to disconnect agent")
	}

	return nil
}

// Stop stops a VM process.
func (v *VM) Stop() error {
	v.logger().Info("stop vm")

	if err := v.hypervisor.stopSandbox(); err != nil {
		return err
	}

	return v.store.Destroy(v.id)
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
		dev := &memoryDevice{1, int(numMB), 0, false}
		if _, err := v.hypervisor.hotplugAddDevice(dev, memoryDev); err != nil {
			return err
		}
	}

	return nil
}

// OnlineCPUMemory puts the hotplugged CPU and memory online.
func (v *VM) OnlineCPUMemory() error {
	v.logger().Infof("online CPU %d and memory", v.cpuDelta)
	err := v.agent.onlineCPUMem(v.cpuDelta, false)
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
		v.logger().WithError(err).Warnf("fail to open %s", urandomDev)
		return err
	}
	defer f.Close()
	if _, err = f.Read(data); err != nil {
		v.logger().WithError(err).Warnf("fail to read %s", urandomDev)
		return err
	}

	return v.agent.reseedRNG(data)
}

// SyncTime syncs guest time with host time.
func (v *VM) SyncTime() error {
	now := time.Now()
	v.logger().WithField("time", now).Infof("sync guest time")
	return v.agent.setGuestDateTime(now)
}

func (v *VM) assignSandbox(s *Sandbox) error {
	// add vm symlinks
	// - link vm socket from sandbox dir (/run/vc/vm/sbid/<kata.sock>) to vm dir (/run/vc/vm/vmid/<kata.sock>)
	// - link 9pfs share path from sandbox dir (/run/kata-containers/shared/sandboxes/sbid/) to vm dir (/run/vc/vm/vmid/shared/)

	vmSharePath := buildVMSharePath(v.id, v.store.RunVMStoragePath())
	vmSockDir := filepath.Join(v.store.RunVMStoragePath(), v.id)
	sbSharePath := getMountPath(s.id)
	sbSockDir := filepath.Join(v.store.RunVMStoragePath(), s.id)

	v.logger().WithFields(logrus.Fields{
		"vmSharePath": vmSharePath,
		"vmSockDir":   vmSockDir,
		"sbSharePath": sbSharePath,
		"sbSockDir":   sbSockDir,
	}).Infof("assign vm to sandbox %s", s.id)

	if err := s.agent.reuseAgent(v.agent); err != nil {
		return err
	}

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
	s.config.HypervisorConfig.VMid = v.id

	return nil
}

// ToGrpc convert VM struct to Grpc format pb.GrpcVM.
func (v *VM) ToGrpc(config VMConfig) (*pb.GrpcVM, error) {
	hJSON, err := v.hypervisor.toGrpc()
	if err != nil {
		return nil, err
	}

	return &pb.GrpcVM{
		Id:         v.id,
		Hypervisor: hJSON,

		Cpu:      v.cpu,
		Memory:   v.memory,
		CpuDelta: v.cpuDelta,
	}, nil
}

func (v *VM) GetVMStatus() *pb.GrpcVMStatus {
	return &pb.GrpcVMStatus{
		Pid:    int64(getHypervisorPid(v.hypervisor)),
		Cpu:    v.cpu,
		Memory: v.memory,
	}
}
