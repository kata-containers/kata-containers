// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package tests

import (
	"io/ioutil"
	"log"
	"os"
	"strings"

	"github.com/BurntSushi/toml"
)

// KataConfiguration is the runtime configuration
type KataConfiguration struct {
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
	Vsock                 bool   `toml:"use_vsock"`
	SharedFS              string `toml:"shared_fs"`
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

	// FirecrackerHypervisor is firecracker
	FirecrackerHypervisor = "firecracker"

	// CloudHypervisor is cloud-hypervisor
	CloudHypervisor = "clh"

	// DefaultProxy default proxy
	DefaultProxy = "kata"

	// DefaultAgent default agent
	DefaultAgent = "kata"

	// DefaultShim default shim
	DefaultShim = "kata"

	// DefaultKataConfigPath is the default path to the kata configuration file
	DefaultKataConfigPath = "/usr/share/defaults/kata-containers/configuration.toml"
)

// KataConfig is the runtime configuration
var KataConfig KataConfiguration

func init() {
	var err error
	kataConfigPath := DefaultKataConfigPath

	args := []string{"--kata-show-default-config-paths"}
	cmd := NewCommand(Runtime, args...)
	stdout, _, exitCode := cmd.Run()
	if exitCode == 0 && stdout != "" {
		for _, c := range strings.Split(stdout, "\n") {
			if _, err = os.Stat(c); err == nil {
				kataConfigPath = c
				break
			}
		}
	}

	KataConfig, err = loadKataConfiguration(kataConfigPath)
	if err != nil {
		log.Fatalf("failed to load kata configuration: %v\n", err)
	}
}

// loadKataConfiguration loads kata configuration
func loadKataConfiguration(configPath string) (KataConfiguration, error) {
	var config KataConfiguration
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
