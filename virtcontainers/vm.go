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

	proxy    proxy
	proxyPid int
	proxyURL string

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

	ProxyType   ProxyType
	ProxyConfig ProxyConfig
}

// Valid check VMConfig validity.
func (c *VMConfig) Valid() error {
	return c.HypervisorConfig.valid()
}

func setupProxy(h hypervisor, agent agent, config VMConfig, id string) (int, string, proxy, error) {
	consoleURL, err := h.getSandboxConsole(id)
	if err != nil {
		return -1, "", nil, err
	}
	agentURL, err := agent.getAgentURL()
	if err != nil {
		return -1, "", nil, err
	}

	// default to kata builtin proxy
	proxyType := config.ProxyType
	if len(proxyType.String()) == 0 {
		proxyType = KataBuiltInProxyType
	}
	proxy, err := newProxy(proxyType)
	if err != nil {
		return -1, "", nil, err
	}

	proxyParams := proxyParams{
		id:         id,
		path:       config.ProxyConfig.Path,
		agentURL:   agentURL,
		consoleURL: consoleURL,
		logger:     virtLog.WithField("vm", id),
		debug:      config.ProxyConfig.Debug,
	}
	pid, url, err := proxy.start(proxyParams)
	if err != nil {
		virtLog.WithFields(logrus.Fields{
			"vm":         id,
			"proxy type": config.ProxyType,
			"params":     proxyParams,
		}).WithError(err).Error("failed to start proxy")
		return -1, "", nil, err
	}

	return pid, url, proxy, nil
}

// NewVM creates a new VM based on provided VMConfig.
func NewVM(ctx context.Context, config VMConfig) (*VM, error) {
	var (
		proxy proxy
		pid   int
		url   string
	)

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

	defer func() {
		if err != nil {
			virtLog.WithField("vm", id).WithError(err).Error("failed to create new vm")
		}
	}()

	if err = hypervisor.init(ctx, id, &config.HypervisorConfig, &filesystem{}); err != nil {
		return nil, err
	}

	if err = hypervisor.createSandbox(); err != nil {
		return nil, err
	}

	// 2. setup agent
	agent := newAgent(config.AgentType)
	vmSharePath := buildVMSharePath(id)
	err = agent.configure(hypervisor, id, vmSharePath, isProxyBuiltIn(config.ProxyType), config.AgentConfig)
	if err != nil {
		return nil, err
	}

	// 3. boot up guest vm
	if err = hypervisor.startSandbox(); err != nil {
		return nil, err
	}
	if err = hypervisor.waitSandbox(vmStartTimeout); err != nil {
		return nil, err
	}

	defer func() {
		if err != nil {
			virtLog.WithField("vm", id).WithError(err).Info("clean up vm")
			hypervisor.stopSandbox()
		}
	}()

	// 4. setup proxy
	pid, url, proxy, err = setupProxy(hypervisor, agent, config, id)
	if err != nil {
		return nil, err
	}
	defer func() {
		if err != nil {
			virtLog.WithField("vm", id).WithError(err).Info("clean up proxy")
			proxy.stop(pid)
		}
	}()
	if err = agent.setProxy(nil, proxy, pid, url); err != nil {
		return nil, err
	}

	// 5. check agent aliveness
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
		proxy:      proxy,
		proxyPid:   pid,
		proxyURL:   url,
		cpu:        config.HypervisorConfig.NumVCPUs,
		memory:     config.HypervisorConfig.MemorySize,
	}, nil
}

func buildVMSharePath(id string) string {
	return filepath.Join(RunVMStoragePath, id, "shared")
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
	return v.hypervisor.startSandbox()
}

// Disconnect agent and proxy connections to a VM
func (v *VM) Disconnect() error {
	v.logger().Info("kill vm")

	if err := v.agent.disconnect(); err != nil {
		v.logger().WithError(err).Error("failed to disconnect agent")
	}
	if err := v.proxy.stop(v.proxyPid); err != nil {
		v.logger().WithError(err).Error("failed to stop proxy")
	}

	return nil
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
		"proxy-pid":   v.proxyPid,
		"proxy-url":   v.proxyURL,
	}).Infof("assign vm to sandbox %s", s.id)

	if err := s.agent.setProxy(s, v.proxy, v.proxyPid, v.proxyURL); err != nil {
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

	return nil
}
