// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package tests

import (
	"io/ioutil"

	"github.com/BurntSushi/toml"
)

// RuntimeConfig is the runtime configuration
type RuntimeConfig struct {
	Hypervisor map[string]hypervisor
	Proxy      map[string]proxy
	Shim       map[string]shim
	Agent      map[string]agent
	Runtime    runtime
}

type hypervisor struct {
	Path                  string `toml:"path"`
	Kernel                string `toml:"kernel"`
	Initrd                string `toml:"initrd"`
	Image                 string `toml:"image"`
	Firmware              string `toml:"firmware"`
	MachineAccelerators   string `toml:"machine_accelerators"`
	KernelParams          string `toml:"kernel_params"`
	MachineType           string `toml:"machine_type"`
	DefaultVCPUs          int32  `toml:"default_vcpus"`
	DefaultMaxVCPUs       uint32 `toml:"default_maxvcpus"`
	DefaultMemSz          uint32 `toml:"default_memory"`
	DefaultBridges        uint32 `toml:"default_bridges"`
	Msize9p               uint32 `toml:"msize_9p"`
	BlockDeviceDriver     string `toml:"block_device_driver"`
	DisableBlockDeviceUse bool   `toml:"disable_block_device_use"`
	MemPrealloc           bool   `toml:"enable_mem_prealloc"`
	HugePages             bool   `toml:"enable_hugepages"`
	Swap                  bool   `toml:"enable_swap"`
	Debug                 bool   `toml:"enable_debug"`
	DisableNestingChecks  bool   `toml:"disable_nesting_checks"`
	EnableIOThreads       bool   `toml:"enable_iothreads"`
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

const (
	// DefaultHypervisor default hypervisor
	DefaultHypervisor = "qemu"

	// DefaultProxy default proxy
	DefaultProxy = "kata"

	// DefaultAgent default agent
	DefaultAgent = "kata"

	// DefaultShim default shim
	DefaultShim = "kata"

	// DefaultRuntimeConfigPath is the default path to the runtime configuration file
	DefaultRuntimeConfigPath = "/usr/share/defaults/kata-containers/configuration.toml"
)

// LoadRuntimeConfiguration loads runtime configuration
func LoadRuntimeConfiguration(configPath string) (RuntimeConfig, error) {
	var config RuntimeConfig
	configData, err := ioutil.ReadFile(configPath)
	if err != nil {
		return config, err
	}

	_, err = toml.Decode(string(configData), &config)
	if err != nil {
		return config, err
	}

	return config, err
}
