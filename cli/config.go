// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"errors"
	"fmt"
	"io/ioutil"
	goruntime "runtime"
	"strings"

	"github.com/BurntSushi/toml"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/sirupsen/logrus"
)

const (
	defaultHypervisor = vc.QemuHypervisor
	defaultProxy      = vc.KataProxyType
	defaultShim       = vc.KataShimType
	defaultAgent      = vc.KataContainersAgent
)

// The TOML configuration file contains a number of sections (or
// tables). The names of these tables are in dotted ("nested table")
// form:
//
//   [<component>.<type>]
//
// The components are hypervisor, proxy, shim and agent. For example,
//
//   [proxy.kata]
//
// The currently supported types are listed below:
const (
	// supported hypervisor component types
	qemuHypervisorTableType = "qemu"

	// supported proxy component types
	ccProxyTableType   = "cc"
	kataProxyTableType = "kata"

	// supported shim component types
	ccShimTableType   = "cc"
	kataShimTableType = "kata"

	// supported agent component types
	hyperstartAgentTableType = "hyperstart"
	kataAgentTableType       = "kata"

	// the maximum amount of PCI bridges that can be cold plugged in a VM
	maxPCIBridges uint32 = 5
)

type tomlConfig struct {
	Hypervisor map[string]hypervisor
	Proxy      map[string]proxy
	Shim       map[string]shim
	Agent      map[string]agent
	Runtime    runtime
}

type hypervisor struct {
	Path                  string `toml:"path"`
	Kernel                string `toml:"kernel"`
	Image                 string `toml:"image"`
	Firmware              string `toml:"firmware"`
	MachineAccelerators   string `toml:"machine_accelerators"`
	KernelParams          string `toml:"kernel_params"`
	MachineType           string `toml:"machine_type"`
	DefaultVCPUs          int32  `toml:"default_vcpus"`
	DefaultMemSz          uint32 `toml:"default_memory"`
	DefaultBridges        uint32 `toml:"default_bridges"`
	DisableBlockDeviceUse bool   `toml:"disable_block_device_use"`
	BlockDeviceDriver     string `toml:"block_device_driver"`
	MemPrealloc           bool   `toml:"enable_mem_prealloc"`
	HugePages             bool   `toml:"enable_hugepages"`
	Swap                  bool   `toml:"enable_swap"`
	Debug                 bool   `toml:"enable_debug"`
	DisableNestingChecks  bool   `toml:"disable_nesting_checks"`
}

type proxy struct {
	Path  string `toml:"path"`
	Debug bool   `toml:"enable_debug"`
}

type runtime struct {
	Debug             bool   `toml:"enable_debug"`
	InterNetworkModel string `toml:"internetworking_model"`
}

type shim struct {
	Path  string `toml:"path"`
	Debug bool   `toml:"enable_debug"`
}

type agent struct {
}

func (h hypervisor) path() (string, error) {
	p := h.Path

	if h.Path == "" {
		p = defaultHypervisorPath
	}

	return resolvePath(p)
}

func (h hypervisor) kernel() (string, error) {
	p := h.Kernel

	if p == "" {
		p = defaultKernelPath
	}

	return resolvePath(p)
}

func (h hypervisor) image() (string, error) {
	p := h.Image

	if p == "" {
		p = defaultImagePath
	}

	return resolvePath(p)
}

func (h hypervisor) firmware() (string, error) {
	p := h.Firmware

	if p == "" {
		if defaultFirmwarePath == "" {
			return "", nil
		}
		p = defaultFirmwarePath
	}

	return resolvePath(p)
}

func (h hypervisor) machineAccelerators() string {
	var machineAccelerators string
	accelerators := strings.Split(h.MachineAccelerators, ",")
	acceleratorsLen := len(accelerators)
	for i := 0; i < acceleratorsLen; i++ {
		if accelerators[i] != "" {
			machineAccelerators += strings.Trim(accelerators[i], "\r\t\n ") + ","
		}
	}

	machineAccelerators = strings.Trim(machineAccelerators, ",")

	return machineAccelerators
}

func (h hypervisor) kernelParams() string {
	if h.KernelParams == "" {
		return defaultKernelParams
	}

	return h.KernelParams
}

func (h hypervisor) machineType() string {
	if h.MachineType == "" {
		return defaultMachineType
	}

	return h.MachineType
}

func (h hypervisor) defaultVCPUs() uint32 {
	numCPUs := goruntime.NumCPU()

	if h.DefaultVCPUs < 0 || h.DefaultVCPUs > int32(numCPUs) {
		return uint32(numCPUs)
	}
	if h.DefaultVCPUs == 0 { // or unspecified
		return defaultVCPUCount
	}

	return uint32(h.DefaultVCPUs)
}

func (h hypervisor) defaultMemSz() uint32 {
	if h.DefaultMemSz < 8 {
		return defaultMemSize // MiB
	}

	return h.DefaultMemSz
}

func (h hypervisor) defaultBridges() uint32 {
	if h.DefaultBridges == 0 {
		return defaultBridgesCount
	}

	if h.DefaultBridges > maxPCIBridges {
		return maxPCIBridges
	}

	return h.DefaultBridges
}

func (h hypervisor) blockDeviceDriver() (string, error) {
	if h.BlockDeviceDriver == "" {
		return defaultBlockDeviceDriver, nil
	}

	if h.BlockDeviceDriver != vc.VirtioSCSI && h.BlockDeviceDriver != vc.VirtioBlock {
		return "", fmt.Errorf("Invalid value %s provided for hypervisor block storage driver, can be either %s or %s", h.BlockDeviceDriver, vc.VirtioSCSI, vc.VirtioBlock)
	}

	return h.BlockDeviceDriver, nil
}

func (p proxy) path() string {
	if p.Path == "" {
		return defaultProxyPath
	}

	return p.Path
}

func (p proxy) debug() bool {
	return p.Debug
}

func (s shim) path() (string, error) {
	p := s.Path

	if p == "" {
		p = defaultShimPath
	}

	return resolvePath(p)
}

func (s shim) debug() bool {
	return s.Debug
}

func newQemuHypervisorConfig(h hypervisor) (vc.HypervisorConfig, error) {
	hypervisor, err := h.path()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	kernel, err := h.kernel()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	image, err := h.image()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	firmware, err := h.firmware()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	machineAccelerators := h.machineAccelerators()
	kernelParams := h.kernelParams()
	machineType := h.machineType()

	blockDriver, err := h.blockDeviceDriver()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	return vc.HypervisorConfig{
		HypervisorPath:        hypervisor,
		KernelPath:            kernel,
		ImagePath:             image,
		FirmwarePath:          firmware,
		MachineAccelerators:   machineAccelerators,
		KernelParams:          vc.DeserializeParams(strings.Fields(kernelParams)),
		HypervisorMachineType: machineType,
		DefaultVCPUs:          h.defaultVCPUs(),
		DefaultMemSz:          h.defaultMemSz(),
		DefaultBridges:        h.defaultBridges(),
		DisableBlockDeviceUse: h.DisableBlockDeviceUse,
		MemPrealloc:           h.MemPrealloc,
		HugePages:             h.HugePages,
		Mlock:                 !h.Swap,
		Debug:                 h.Debug,
		DisableNestingChecks:  h.DisableNestingChecks,
		BlockDeviceDriver:     blockDriver,
	}, nil
}

func newShimConfig(s shim) (vc.ShimConfig, error) {
	path, err := s.path()
	if err != nil {
		return vc.ShimConfig{}, err
	}

	return vc.ShimConfig{
		Path:  path,
		Debug: s.debug(),
	}, nil
}

func updateRuntimeConfig(configPath string, tomlConf tomlConfig, config *oci.RuntimeConfig) error {
	for k, hypervisor := range tomlConf.Hypervisor {
		switch k {
		case qemuHypervisorTableType:
			hConfig, err := newQemuHypervisorConfig(hypervisor)
			if err != nil {
				return fmt.Errorf("%v: %v", configPath, err)
			}

			config.VMConfig.Memory = uint(hConfig.DefaultMemSz)

			config.HypervisorConfig = hConfig
		}
	}

	for k, proxy := range tomlConf.Proxy {
		switch k {
		case ccProxyTableType:
			config.ProxyType = vc.CCProxyType
		case kataProxyTableType:
			config.ProxyType = vc.KataProxyType
		}

		config.ProxyConfig = vc.ProxyConfig{
			Path:  proxy.path(),
			Debug: proxy.debug(),
		}
	}

	for k := range tomlConf.Agent {
		switch k {
		case hyperstartAgentTableType:
			config.AgentType = hyperstartAgentTableType
			config.AgentConfig = vc.HyperConfig{}

		case kataAgentTableType:
			config.AgentType = kataAgentTableType
			config.AgentConfig = vc.KataAgentConfig{}

		}
	}

	for k, shim := range tomlConf.Shim {
		switch k {
		case ccShimTableType:
			config.ShimType = vc.CCShimType
		case kataShimTableType:
			config.ShimType = vc.KataShimType
		}

		shConfig, err := newShimConfig(shim)
		if err != nil {
			return fmt.Errorf("%v: %v", configPath, err)
		}

		config.ShimConfig = shConfig
	}

	return nil
}

// loadConfiguration loads the configuration file and converts it into a
// runtime configuration.
//
// If ignoreLogging is true, the system logger will not be initialised nor
// will this function make any log calls.
//
// All paths are resolved fully meaning if this function does not return an
// error, all paths are valid at the time of the call.
func loadConfiguration(configPath string, ignoreLogging bool) (resolvedConfigPath string, config oci.RuntimeConfig, err error) {
	defaultHypervisorConfig := vc.HypervisorConfig{
		HypervisorPath:        defaultHypervisorPath,
		KernelPath:            defaultKernelPath,
		ImagePath:             defaultImagePath,
		FirmwarePath:          defaultFirmwarePath,
		MachineAccelerators:   defaultMachineAccelerators,
		HypervisorMachineType: defaultMachineType,
		DefaultVCPUs:          defaultVCPUCount,
		DefaultMemSz:          defaultMemSize,
		DefaultBridges:        defaultBridgesCount,
		MemPrealloc:           defaultEnableMemPrealloc,
		HugePages:             defaultEnableHugePages,
		Mlock:                 !defaultEnableSwap,
		Debug:                 defaultEnableDebug,
		DisableNestingChecks:  defaultDisableNestingChecks,
		BlockDeviceDriver:     defaultBlockDeviceDriver,
	}

	err = config.InterNetworkModel.SetModel(defaultInterNetworkingModel)
	if err != nil {
		return "", config, err
	}

	defaultAgentConfig := vc.HyperConfig{}

	config = oci.RuntimeConfig{
		HypervisorType:   defaultHypervisor,
		HypervisorConfig: defaultHypervisorConfig,
		AgentType:        defaultAgent,
		AgentConfig:      defaultAgentConfig,
		ProxyType:        defaultProxy,
		ShimType:         defaultShim,
	}

	var resolved string

	if configPath == "" {
		resolved, err = getDefaultConfigFile()
	} else {
		resolved, err = resolvePath(configPath)
	}

	if err != nil {
		return "", config, fmt.Errorf("Cannot find usable config file (%v)", err)
	}

	configData, err := ioutil.ReadFile(resolved)
	if err != nil {
		return "", config, err
	}

	var tomlConf tomlConfig
	_, err = toml.Decode(string(configData), &tomlConf)
	if err != nil {
		return "", config, err
	}

	if tomlConf.Runtime.Debug {
		crashOnError = true
	} else {
		// If debug is not required, switch back to the original
		// default log priority, otherwise continue in debug mode.
		kataLog.Logger.Level = originalLoggerLevel
	}

	if tomlConf.Runtime.InterNetworkModel != "" {
		err = config.InterNetworkModel.SetModel(tomlConf.Runtime.InterNetworkModel)
		if err != nil {
			return "", config, err
		}
	}

	if !ignoreLogging {
		err = handleSystemLog("", "")
		if err != nil {
			return "", config, err
		}

		kataLog.WithFields(
			logrus.Fields{
				"format": "TOML",
			}).Debugf("loaded configuration")
	}

	if err := updateRuntimeConfig(resolved, tomlConf, &config); err != nil {
		return "", config, err
	}

	return resolved, config, nil
}

// getDefaultConfigFilePaths returns a list of paths that will be
// considered as configuration files in priority order.
func getDefaultConfigFilePaths() []string {
	return []string{
		// normally below "/etc"
		defaultSysConfRuntimeConfiguration,

		// normally below "/usr/share"
		defaultRuntimeConfiguration,
	}
}

// getDefaultConfigFile looks in multiple default locations for a
// configuration file and returns the resolved path for the first file
// found, or an error if no config files can be found.
func getDefaultConfigFile() (string, error) {
	var errs []string

	for _, file := range getDefaultConfigFilePaths() {
		resolved, err := resolvePath(file)
		if err == nil {
			return resolved, nil
		}
		s := fmt.Sprintf("config file %q unresolvable: %v", file, err)
		errs = append(errs, s)
	}

	return "", errors.New(strings.Join(errs, ", "))
}
