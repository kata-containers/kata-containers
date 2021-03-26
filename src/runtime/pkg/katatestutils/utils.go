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
	NetmonPath           string
	LogPath              string
	BlockDeviceDriver    string
	AgentTraceMode       string
	AgentTraceType       string
	SharedFS             string
	VirtioFSDaemon       string
	PFlash               []string
	PCIeRootPort         uint32
	DisableBlock         bool
	EnableIOThreads      bool
	HotplugVFIOOnRootBus bool
	DisableNewNetNs      bool
	HypervisorDebug      bool
	RuntimeDebug         bool
	RuntimeTrace         bool
	ShimDebug            bool
	NetmonDebug          bool
	AgentDebug           bool
	AgentTrace           bool
	EnablePprof          bool
	JaegerEndpoint       string
	JaegerUser           string
	JaegerPassword       string
}

// ContainerIDTestDataType is a type used to test Container and Sandbox ID's.
type ContainerIDTestDataType struct {
	ID    string
	Valid bool
}

// ContainerIDTestData is a set of test data that lists valid and invalid Container IDs
var ContainerIDTestData = []ContainerIDTestDataType{
	{"", false},   // Cannot be blank
	{" ", false},  // Cannot be a space
	{".", false},  // Must start with an alphanumeric
	{"-", false},  // Must start with an alphanumeric
	{"_", false},  // Must start with an alphanumeric
	{" a", false}, // Must start with an alphanumeric
	{".a", false}, // Must start with an alphanumeric
	{"-a", false}, // Must start with an alphanumeric
	{"_a", false}, // Must start with an alphanumeric
	{"..", false}, // Must start with an alphanumeric
	{"a", false},  // Too short
	{"z", false},  // Too short
	{"A", false},  // Too short
	{"Z", false},  // Too short
	{"0", false},  // Too short
	{"9", false},  // Too short
	{"-1", false}, // Must start with an alphanumeric
	{"/", false},
	{"a/", false},
	{"a/../", false},
	{"../a", false},
	{"../../a", false},
	{"../../../a", false},
	{"foo/../bar", false},
	{"foo bar", false},
	{"a.", true},
	{"a..", true},
	{"aa", true},
	{"aa.", true},
	{"hello..world", true},
	{"hello/../world", false},
	{"aa1245124sadfasdfgasdga.", true},
	{"aAzZ0123456789_.-", true},
	{"abcdefghijklmnopqrstuvwxyz0123456789.-_", true},
	{"0123456789abcdefghijklmnopqrstuvwxyz.-_", true},
	{" abcdefghijklmnopqrstuvwxyz0123456789.-_", false},
	{".abcdefghijklmnopqrstuvwxyz0123456789.-_", false},
	{"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_", true},
	{"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ.-_", true},
	{" ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_", false},
	{".ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_", false},
	{"/a/b/c", false},
	{"a/b/c", false},
	{"foo/../../../etc/passwd", false},
	{"../../../../../../etc/motd", false},
	{"/etc/passwd", false},
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
	pcie_root_port = ` + strconv.FormatUint(uint64(config.PCIeRootPort), 10) + `
	msize_9p = ` + strconv.FormatUint(uint64(config.DefaultMsize9p), 10) + `
	enable_debug = ` + strconv.FormatBool(config.HypervisorDebug) + `
	guest_hook_path = "` + config.DefaultGuestHookPath + `"
	shared_fs = "` + config.SharedFS + `"
	virtio_fs_daemon = "` + config.VirtioFSDaemon + `"

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
	disable_new_netns= ` + strconv.FormatBool(config.DisableNewNetNs) + `
	enable_pprof= ` + strconv.FormatBool(config.EnablePprof) + `
	jaeger_endpoint= "` + config.JaegerEndpoint + `"
	jaeger_user= "` + config.JaegerUser + `"
	jaeger_password= "` + config.JaegerPassword + `"`
}
