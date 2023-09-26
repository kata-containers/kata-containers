// Copyright (c) 2018-2019 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katatestutils

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

const (
	testDirMode  = os.FileMode(0750)
	testFileMode = os.FileMode(0640)

	busyboxConfigJson = `
{
	"ociVersion": "1.0.1-dev",
	"process": {
		"terminal": true,
		"user": {
			"uid": 0,
			"gid": 0
		},
		"args": [
			"sh"
		],
		"env": [
			"PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
			"TERM=xterm"
		],
		"cwd": "/",
		"capabilities": {
			"bounding": [
				"CAP_AUDIT_WRITE",
				"CAP_KILL",
				"CAP_NET_BIND_SERVICE"
			],
			"effective": [
				"CAP_AUDIT_WRITE",
				"CAP_KILL",
				"CAP_NET_BIND_SERVICE"
			],
			"inheritable": [
				"CAP_AUDIT_WRITE",
				"CAP_KILL",
				"CAP_NET_BIND_SERVICE"
			],
			"permitted": [
				"CAP_AUDIT_WRITE",
				"CAP_KILL",
				"CAP_NET_BIND_SERVICE"
			],
			"ambient": [
				"CAP_AUDIT_WRITE",
				"CAP_KILL",
				"CAP_NET_BIND_SERVICE"
			]
		},
		"rlimits": [
			{
				"type": "RLIMIT_NOFILE",
				"hard": 1024,
				"soft": 1024
			}
		],
		"noNewPrivileges": true
	},
	"root": {
		"path": "rootfs",
		"readonly": true
	},
	"hostname": "runc",
	"mounts": [
		{
			"destination": "/proc",
			"type": "proc",
			"source": "proc"
		},
		{
			"destination": "/dev",
			"type": "tmpfs",
			"source": "tmpfs",
			"options": [
				"nosuid",
				"strictatime",
				"mode=755",
				"size=65536k"
			]
		},
		{
			"destination": "/dev/pts",
			"type": "devpts",
			"source": "devpts",
			"options": [
				"nosuid",
				"noexec",
				"newinstance",
				"ptmxmode=0666",
				"mode=0620",
				"gid=5"
			]
		},
		{
			"destination": "/dev/shm",
			"type": "tmpfs",
			"source": "shm",
			"options": [
				"nosuid",
				"noexec",
				"nodev",
				"mode=1777",
				"size=65536k"
			]
		},
		{
			"destination": "/dev/mqueue",
			"type": "mqueue",
			"source": "mqueue",
			"options": [
				"nosuid",
				"noexec",
				"nodev"
			]
		},
		{
			"destination": "/sys",
			"type": "sysfs",
			"source": "sysfs",
			"options": [
				"nosuid",
				"noexec",
				"nodev",
				"ro"
			]
		},
		{
			"destination": "/sys/fs/cgroup",
			"type": "cgroup",
			"source": "cgroup",
			"options": [
				"nosuid",
				"noexec",
				"nodev",
				"relatime",
				"ro"
			]
		}
	],
	"linux": {
		"resources": {
			"devices": [
				{
					"allow": false,
					"access": "rwm"
				}
			]
		},
		"namespaces": [
			{
				"type": "pid"
			},
			{
				"type": "network"
			},
			{
				"type": "ipc"
			},
			{
				"type": "uts"
			},
			{
				"type": "mount"
			}
		],
		"maskedPaths": [
			"/proc/acpi",
			"/proc/asound",
			"/proc/kcore",
			"/proc/keys",
			"/proc/latency_stats",
			"/proc/timer_list",
			"/proc/timer_stats",
			"/proc/sched_debug",
			"/sys/firmware",
			"/proc/scsi"
		],
		"readonlyPaths": [
			"/proc/bus",
			"/proc/fs",
			"/proc/irq",
			"/proc/sys",
			"/proc/sysrq-trigger"
		]
	}
}`
)

type RuntimeConfigOptions struct {
	Hypervisor           string
	HypervisorPath       string
	DefaultGuestHookPath string
	KernelPath           string
	ImagePath            string
	RootfsType           string
	KernelParams         string
	MachineType          string
	LogPath              string
	BlockDeviceDriver    string
	BlockDeviceAIO       string
	SharedFS             string
	VirtioFSDaemon       string
	JaegerEndpoint       string
	JaegerUser           string
	JaegerPassword       string
	PFlash               []string
	HotPlugVFIO          config.PCIePort
	ColdPlugVFIO         config.PCIePort
	PCIeRootPort         uint32
	PCIeSwitchPort       uint32
	DefaultVCPUCount     uint32
	DefaultMaxVCPUCount  uint32
	DefaultMemSize       uint32
	DefaultMaxMemorySize uint64
	DefaultMsize9p       uint32
	DisableBlock         bool
	EnableIOThreads      bool
	DisableNewNetNs      bool
	HypervisorDebug      bool
	RuntimeDebug         bool
	RuntimeTrace         bool
	AgentDebug           bool
	AgentTrace           bool
	EnablePprof          bool
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
	block_device_aio =  "` + config.BlockDeviceAIO + `"
	kernel_params = "` + config.KernelParams + `"
	image = "` + config.ImagePath + `"
	rootfs_type = "` + config.RootfsType + `"
	machine_type = "` + config.MachineType + `"
	default_vcpus = ` + strconv.FormatUint(uint64(config.DefaultVCPUCount), 10) + `
	default_maxvcpus = ` + strconv.FormatUint(uint64(config.DefaultMaxVCPUCount), 10) + `
	default_memory = ` + strconv.FormatUint(uint64(config.DefaultMemSize), 10) + `
	disable_block_device_use =  ` + strconv.FormatBool(config.DisableBlock) + `
	enable_iothreads =  ` + strconv.FormatBool(config.EnableIOThreads) + `
	cold_plug_vfio =  "` + config.ColdPlugVFIO.String() + `"
	hot_plug_vfio =   "` + config.HotPlugVFIO.String() + `"
	pcie_root_port = ` + strconv.FormatUint(uint64(config.PCIeRootPort), 10) + `
	pcie_switch_port = ` + strconv.FormatUint(uint64(config.PCIeSwitchPort), 10) + `
	msize_9p = ` + strconv.FormatUint(uint64(config.DefaultMsize9p), 10) + `
	enable_debug = ` + strconv.FormatBool(config.HypervisorDebug) + `
	guest_hook_path = "` + config.DefaultGuestHookPath + `"
	shared_fs = "` + config.SharedFS + `"
	virtio_fs_daemon = "` + config.VirtioFSDaemon + `"

	[agent.kata]
	enable_debug = ` + strconv.FormatBool(config.AgentDebug) + `
	enable_tracing = ` + strconv.FormatBool(config.AgentTrace) + `

	[runtime]
	enable_debug = ` + strconv.FormatBool(config.RuntimeDebug) + `
	enable_tracing = ` + strconv.FormatBool(config.RuntimeTrace) + `
	disable_new_netns= ` + strconv.FormatBool(config.DisableNewNetNs) + `
	enable_pprof= ` + strconv.FormatBool(config.EnablePprof) + `
	jaeger_endpoint= "` + config.JaegerEndpoint + `"
	jaeger_user= "` + config.JaegerUser + `"
	jaeger_password= "` + config.JaegerPassword + `"`
}

func IsInGitHubActions() bool {
	// https://docs.github.com/en/actions/reference/environment-variables#default-environment-variables
	return os.Getenv("GITHUB_ACTIONS") == "true"
}

func SetupOCIConfigFile(t *testing.T) (rootPath string, bundlePath, ociConfigFile string) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	bundlePath = filepath.Join(tmpdir, "bundle")
	err := os.MkdirAll(bundlePath, testDirMode)
	assert.NoError(err)

	ociConfigFile = filepath.Join(bundlePath, "config.json")
	err = os.WriteFile(ociConfigFile, []byte(busyboxConfigJson), testFileMode)
	assert.NoError(err)

	return tmpdir, bundlePath, ociConfigFile
}

// WriteOCIConfigFile using spec to update OCI config file by path configPath
func WriteOCIConfigFile(spec specs.Spec, configPath string) error {
	if configPath == "" {
		return errors.New("BUG: need config file path")
	}

	bytes, err := json.MarshalIndent(spec, "", "\t")
	if err != nil {
		return err
	}

	return os.WriteFile(configPath, bytes, testFileMode)
}
