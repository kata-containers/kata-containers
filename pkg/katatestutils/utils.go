// Copyright (c) 2018-2019 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katatestutils

import "strconv"

type RuntimeConfigOptions struct {
	Hypervisor           string
	HypervisorPath       string
	DefaultVCPUCount     uint32
	DefaultMaxVCPUCount  uint32
	DefaultMemSize       uint32
	DefaultMsize9p       uint32
	DefaultGuestHookPath string
	KernelPath           string
	ImagePath            string
	KernelParams         string
	MachineType          string
	ShimPath             string
	ProxyPath            string
	NetmonPath           string
	LogPath              string
	BlockDeviceDriver    string
	AgentTraceMode       string
	AgentTraceType       string
	SharedFS             string
	DisableBlock         bool
	EnableIOThreads      bool
	HotplugVFIOOnRootBus bool
	DisableNewNetNs      bool
	HypervisorDebug      bool
	RuntimeDebug         bool
	RuntimeTrace         bool
	ProxyDebug           bool
	ShimDebug            bool
	NetmonDebug          bool
	AgentDebug           bool
	AgentTrace           bool
}

func MakeRuntimeConfigFileData(config RuntimeConfigOptions) string {
	return `
	# Runtime configuration file

	[hypervisor.` + config.Hypervisor + `]
	path = "` + config.HypervisorPath + `"
	kernel = "` + config.KernelPath + `"
	block_device_driver =  "` + config.BlockDeviceDriver + `"
	kernel_params = "` + config.KernelParams + `"
	image = "` + config.ImagePath + `"
	machine_type = "` + config.MachineType + `"
	default_vcpus = ` + strconv.FormatUint(uint64(config.DefaultVCPUCount), 10) + `
	default_maxvcpus = ` + strconv.FormatUint(uint64(config.DefaultMaxVCPUCount), 10) + `
	default_memory = ` + strconv.FormatUint(uint64(config.DefaultMemSize), 10) + `
	disable_block_device_use =  ` + strconv.FormatBool(config.DisableBlock) + `
	enable_iothreads =  ` + strconv.FormatBool(config.EnableIOThreads) + `
	hotplug_vfio_on_root_bus =  ` + strconv.FormatBool(config.HotplugVFIOOnRootBus) + `
	msize_9p = ` + strconv.FormatUint(uint64(config.DefaultMsize9p), 10) + `
	enable_debug = ` + strconv.FormatBool(config.HypervisorDebug) + `
	guest_hook_path = "` + config.DefaultGuestHookPath + `"
	shared_fs = "` + config.SharedFS + `"
	virtio_fs_daemon = "/path/to/virtiofsd"

	[proxy.kata]
	enable_debug = ` + strconv.FormatBool(config.ProxyDebug) + `
	path = "` + config.ProxyPath + `"

	[shim.kata]
	path = "` + config.ShimPath + `"
	enable_debug = ` + strconv.FormatBool(config.ShimDebug) + `

	[agent.kata]
	enable_debug = ` + strconv.FormatBool(config.AgentDebug) + `
	enable_tracing = ` + strconv.FormatBool(config.AgentTrace) + `
	trace_mode = "` + config.AgentTraceMode + `"` + `
	trace_type = "` + config.AgentTraceType + `"` + `

	[netmon]
	path = "` + config.NetmonPath + `"
	enable_debug = ` + strconv.FormatBool(config.NetmonDebug) + `

	[runtime]
	enable_debug = ` + strconv.FormatBool(config.RuntimeDebug) + `
	enable_tracing = ` + strconv.FormatBool(config.RuntimeTrace) + `
	disable_new_netns= ` + strconv.FormatBool(config.DisableNewNetNs)
}
