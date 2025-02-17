// Copyright (c) 2018-2022 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
// Copyright (c) 2021 Adobe Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	goruntime "runtime"
	"strings"

	"github.com/BurntSushi/toml"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/govmm"
	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils/katatrace"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/pbnjay/memory"
	"github.com/sirupsen/logrus"
)

const (
	defaultHypervisor = vc.QemuHypervisor
)

// The TOML configuration file contains a number of sections (or
// tables). The names of these tables are in dotted ("nested table")
// form:
//
//	[<component>.<type>]
//
// The components are hypervisor, and agent. For example,
//
//	[agent.kata]
//
// The currently supported types are listed below:
const (
	// supported hypervisor component types
	firecrackerHypervisorTableType = "firecracker"
	clhHypervisorTableType         = "clh"
	qemuHypervisorTableType        = "qemu"
	dragonballHypervisorTableType  = "dragonball"
	stratovirtHypervisorTableType  = "stratovirt"
	remoteHypervisorTableType      = "remote"

	// the maximum amount of PCI bridges that can be cold plugged in a VM
	maxPCIBridges uint32 = 5
	// For more info on why these values were chosen, see:
	// https://github.com/kata-containers/kata-containers/blob/main/docs/design/kata-vra.md#hypervisor-resource-limits
	maxPCIeRootPorts   uint32 = 16
	maxPCIeSwitchPorts uint32 = 16

	// the maximum valid loglevel for the hypervisor
	maxHypervisorLoglevel uint32 = 3

	errInvalidHypervisorPrefix = "configuration file contains invalid hypervisor section"
)

type tomlConfig struct {
	Hypervisor map[string]hypervisor
	Agent      map[string]agent
	Factory    factory
	Runtime    runtime
}

type factory struct {
	TemplatePath    string `toml:"template_path"`
	VMCacheEndpoint string `toml:"vm_cache_endpoint"`
	VMCacheNumber   uint   `toml:"vm_cache_number"`
	Template        bool   `toml:"enable_template"`
}

type hypervisor struct {
	Path                           string                    `toml:"path"`
	JailerPath                     string                    `toml:"jailer_path"`
	Kernel                         string                    `toml:"kernel"`
	Initrd                         string                    `toml:"initrd"`
	Image                          string                    `toml:"image"`
	RootfsType                     string                    `toml:"rootfs_type"`
	Firmware                       string                    `toml:"firmware"`
	FirmwareVolume                 string                    `toml:"firmware_volume"`
	MachineAccelerators            string                    `toml:"machine_accelerators"`
	CPUFeatures                    string                    `toml:"cpu_features"`
	KernelParams                   string                    `toml:"kernel_params"`
	MachineType                    string                    `toml:"machine_type"`
	QgsPort                        uint32                    `toml:"tdx_quote_generation_service_socket_port"`
	BlockDeviceDriver              string                    `toml:"block_device_driver"`
	EntropySource                  string                    `toml:"entropy_source"`
	SharedFS                       string                    `toml:"shared_fs"`
	VirtioFSDaemon                 string                    `toml:"virtio_fs_daemon"`
	VirtioFSCache                  string                    `toml:"virtio_fs_cache"`
	VhostUserStorePath             string                    `toml:"vhost_user_store_path"`
	FileBackedMemRootDir           string                    `toml:"file_mem_backend"`
	GuestHookPath                  string                    `toml:"guest_hook_path"`
	GuestMemoryDumpPath            string                    `toml:"guest_memory_dump_path"`
	SeccompSandbox                 string                    `toml:"seccompsandbox"`
	BlockDeviceAIO                 string                    `toml:"block_device_aio"`
	RemoteHypervisorSocket         string                    `toml:"remote_hypervisor_socket"`
	HypervisorPathList             []string                  `toml:"valid_hypervisor_paths"`
	JailerPathList                 []string                  `toml:"valid_jailer_paths"`
	VirtioFSDaemonList             []string                  `toml:"valid_virtio_fs_daemon_paths"`
	VirtioFSExtraArgs              []string                  `toml:"virtio_fs_extra_args"`
	PFlashList                     []string                  `toml:"pflashes"`
	VhostUserStorePathList         []string                  `toml:"valid_vhost_user_store_paths"`
	FileBackedMemRootList          []string                  `toml:"valid_file_mem_backends"`
	EntropySourceList              []string                  `toml:"valid_entropy_sources"`
	EnableAnnotations              []string                  `toml:"enable_annotations"`
	RxRateLimiterMaxRate           uint64                    `toml:"rx_rate_limiter_max_rate"`
	TxRateLimiterMaxRate           uint64                    `toml:"tx_rate_limiter_max_rate"`
	MemOffset                      uint64                    `toml:"memory_offset"`
	DefaultMaxMemorySize           uint64                    `toml:"default_maxmemory"`
	DiskRateLimiterBwMaxRate       int64                     `toml:"disk_rate_limiter_bw_max_rate"`
	DiskRateLimiterBwOneTimeBurst  int64                     `toml:"disk_rate_limiter_bw_one_time_burst"`
	DiskRateLimiterOpsMaxRate      int64                     `toml:"disk_rate_limiter_ops_max_rate"`
	DiskRateLimiterOpsOneTimeBurst int64                     `toml:"disk_rate_limiter_ops_one_time_burst"`
	NetRateLimiterBwMaxRate        int64                     `toml:"net_rate_limiter_bw_max_rate"`
	NetRateLimiterBwOneTimeBurst   int64                     `toml:"net_rate_limiter_bw_one_time_burst"`
	NetRateLimiterOpsMaxRate       int64                     `toml:"net_rate_limiter_ops_max_rate"`
	NetRateLimiterOpsOneTimeBurst  int64                     `toml:"net_rate_limiter_ops_one_time_burst"`
	HypervisorLoglevel             uint32                    `toml:"hypervisor_loglevel"`
	VirtioFSCacheSize              uint32                    `toml:"virtio_fs_cache_size"`
	VirtioFSQueueSize              uint32                    `toml:"virtio_fs_queue_size"`
	DefaultMaxVCPUs                uint32                    `toml:"default_maxvcpus"`
	MemorySize                     uint32                    `toml:"default_memory"`
	MemSlots                       uint32                    `toml:"memory_slots"`
	DefaultBridges                 uint32                    `toml:"default_bridges"`
	Msize9p                        uint32                    `toml:"msize_9p"`
	RemoteHypervisorTimeout        uint32                    `toml:"remote_hypervisor_timeout"`
	NumVCPUs                       float32                   `toml:"default_vcpus"`
	BlockDeviceCacheSet            bool                      `toml:"block_device_cache_set"`
	BlockDeviceCacheDirect         bool                      `toml:"block_device_cache_direct"`
	BlockDeviceCacheNoflush        bool                      `toml:"block_device_cache_noflush"`
	EnableVhostUserStore           bool                      `toml:"enable_vhost_user_store"`
	VhostUserDeviceReconnect       uint32                    `toml:"vhost_user_reconnect_timeout_sec"`
	DisableBlockDeviceUse          bool                      `toml:"disable_block_device_use"`
	MemPrealloc                    bool                      `toml:"enable_mem_prealloc"`
	HugePages                      bool                      `toml:"enable_hugepages"`
	VirtioMem                      bool                      `toml:"enable_virtio_mem"`
	IOMMU                          bool                      `toml:"enable_iommu"`
	IOMMUPlatform                  bool                      `toml:"enable_iommu_platform"`
	Debug                          bool                      `toml:"enable_debug"`
	DisableNestingChecks           bool                      `toml:"disable_nesting_checks"`
	EnableIOThreads                bool                      `toml:"enable_iothreads"`
	DisableImageNvdimm             bool                      `toml:"disable_image_nvdimm"`
	HotPlugVFIO                    config.PCIePort           `toml:"hot_plug_vfio"`
	ColdPlugVFIO                   config.PCIePort           `toml:"cold_plug_vfio"`
	PCIeRootPort                   uint32                    `toml:"pcie_root_port"`
	PCIeSwitchPort                 uint32                    `toml:"pcie_switch_port"`
	DisableVhostNet                bool                      `toml:"disable_vhost_net"`
	GuestMemoryDumpPaging          bool                      `toml:"guest_memory_dump_paging"`
	ConfidentialGuest              bool                      `toml:"confidential_guest"`
	SevSnpGuest                    bool                      `toml:"sev_snp_guest"`
	GuestSwap                      bool                      `toml:"enable_guest_swap"`
	Rootless                       bool                      `toml:"rootless"`
	DisableSeccomp                 bool                      `toml:"disable_seccomp"`
	DisableSeLinux                 bool                      `toml:"disable_selinux"`
	DisableGuestSeLinux            bool                      `toml:"disable_guest_selinux"`
	LegacySerial                   bool                      `toml:"use_legacy_serial"`
	ExtraMonitorSocket             govmmQemu.MonitorProtocol `toml:"extra_monitor_socket"`
}

type runtime struct {
	InterNetworkModel         string   `toml:"internetworking_model"`
	JaegerEndpoint            string   `toml:"jaeger_endpoint"`
	JaegerUser                string   `toml:"jaeger_user"`
	JaegerPassword            string   `toml:"jaeger_password"`
	VfioMode                  string   `toml:"vfio_mode"`
	GuestSeLinuxLabel         string   `toml:"guest_selinux_label"`
	SandboxBindMounts         []string `toml:"sandbox_bind_mounts"`
	Experimental              []string `toml:"experimental"`
	Tracing                   bool     `toml:"enable_tracing"`
	DisableNewNetNs           bool     `toml:"disable_new_netns"`
	DisableGuestSeccomp       bool     `toml:"disable_guest_seccomp"`
	EnableVCPUsPinning        bool     `toml:"enable_vcpus_pinning"`
	Debug                     bool     `toml:"enable_debug"`
	SandboxCgroupOnly         bool     `toml:"sandbox_cgroup_only"`
	StaticSandboxResourceMgmt bool     `toml:"static_sandbox_resource_mgmt"`
	EnablePprof               bool     `toml:"enable_pprof"`
	DisableGuestEmptyDir      bool     `toml:"disable_guest_empty_dir"`
	CreateContainerTimeout    uint64   `toml:"create_container_timeout"`
	DanConf                   string   `toml:"dan_conf"`
}

type agent struct {
	KernelModules       []string `toml:"kernel_modules"`
	Debug               bool     `toml:"enable_debug"`
	Tracing             bool     `toml:"enable_tracing"`
	DebugConsoleEnabled bool     `toml:"debug_console_enabled"`
	DialTimeout         uint32   `toml:"dial_timeout"`
	CdhApiTimeout       uint32   `toml:"cdh_api_timeout"`
}

func (orig *tomlConfig) Clone() tomlConfig {
	clone := *orig
	clone.Hypervisor = make(map[string]hypervisor)
	clone.Agent = make(map[string]agent)

	for key, value := range orig.Hypervisor {
		clone.Hypervisor[key] = value
	}
	for key, value := range orig.Agent {
		clone.Agent[key] = value
	}
	return clone
}

func (h hypervisor) path() (string, error) {
	p := h.Path

	if h.Path == "" {
		p = defaultHypervisorPath
	}

	return ResolvePath(p)
}

func (h hypervisor) jailerPath() (string, error) {
	p := h.JailerPath

	if h.JailerPath == "" {
		return "", nil
	}

	return ResolvePath(p)
}

func (h hypervisor) kernel() (string, error) {
	p := h.Kernel

	if p == "" {
		p = defaultKernelPath
	}

	return ResolvePath(p)
}

func (h hypervisor) initrd() (string, error) {
	p := h.Initrd

	if p == "" {
		return "", nil
	}

	return ResolvePath(p)
}

func (h hypervisor) image() (string, error) {
	p := h.Image

	if p == "" {
		return "", nil
	}

	return ResolvePath(p)
}

func (h hypervisor) rootfsType() (string, error) {
	p := h.RootfsType

	if p == "" {
		p = "ext4"
	}

	return p, nil
}

func (h hypervisor) firmware() (string, error) {
	p := h.Firmware

	if p == "" {
		if defaultFirmwarePath == "" {
			return "", nil
		}
		p = defaultFirmwarePath
	}

	return ResolvePath(p)
}

func (h hypervisor) coldPlugVFIO() config.PCIePort {
	if h.ColdPlugVFIO == "" {
		return defaultColdPlugVFIO
	}
	return h.ColdPlugVFIO
}
func (h hypervisor) hotPlugVFIO() config.PCIePort {
	if h.HotPlugVFIO == "" {
		return defaultHotPlugVFIO
	}
	return h.HotPlugVFIO
}

func (h hypervisor) pcieRootPort() uint32 {
	if h.PCIeRootPort > maxPCIeRootPorts {
		return maxPCIeRootPorts
	}
	return h.PCIeRootPort
}

func (h hypervisor) pcieSwitchPort() uint32 {
	if h.PCIeSwitchPort > maxPCIeSwitchPorts {
		return maxPCIeSwitchPorts
	}
	return h.PCIeSwitchPort
}

func (h hypervisor) firmwareVolume() (string, error) {
	p := h.FirmwareVolume

	if p == "" {
		if defaultFirmwareVolumePath == "" {
			return "", nil
		}
		p = defaultFirmwareVolumePath
	}

	return ResolvePath(p)
}

func (h hypervisor) PFlash() ([]string, error) {
	pflashes := h.PFlashList

	if len(pflashes) == 0 {
		return []string{}, nil
	}

	for _, pflash := range pflashes {
		_, err := ResolvePath(pflash)
		if err != nil {
			return []string{}, fmt.Errorf("failed to resolve path: %s: %v", pflash, err)
		}
	}

	return pflashes, nil
}

func (h hypervisor) machineAccelerators() string {
	var machineAccelerators string
	for _, accelerator := range strings.Split(h.MachineAccelerators, ",") {
		if accelerator != "" {
			machineAccelerators += strings.TrimSpace(accelerator) + ","
		}
	}

	machineAccelerators = strings.Trim(machineAccelerators, ",")

	return machineAccelerators
}

func (h hypervisor) cpuFeatures() string {
	var cpuFeatures string
	for _, feature := range strings.Split(h.CPUFeatures, ",") {
		if feature != "" {
			cpuFeatures += strings.TrimSpace(feature) + ","
		}
	}

	cpuFeatures = strings.Trim(cpuFeatures, ",")

	return cpuFeatures
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

func (h hypervisor) qgsPort() uint32 {
	if h.QgsPort == 0 {
		return defaultQgsPort
	}

	return h.QgsPort
}

func (h hypervisor) GetEntropySource() string {
	if h.EntropySource == "" {
		return defaultEntropySource
	}

	return h.EntropySource
}

var procCPUInfo = "/proc/cpuinfo"

func getHostCPUs() uint32 {
	cpuInfo, err := os.ReadFile(procCPUInfo)
	if err != nil {
		kataUtilsLogger.Warn("unable to read /proc/cpuinfo to determine cpu count - using go runtime value instead")
		return uint32(goruntime.NumCPU())
	}

	cores := 0
	lines := strings.Split(string(cpuInfo), "\n")
	for _, line := range lines {
		if strings.HasPrefix(line, "processor") {
			cores++
		}
	}

	return uint32(cores)
}

// Current cpu number should not larger than defaultMaxVCPUs()
func getCurrentCpuNum() uint32 {
	var cpu uint32
	h := hypervisor{}

	cpu = getHostCPUs()
	if cpu > h.defaultMaxVCPUs() {
		cpu = h.defaultMaxVCPUs()
	}

	return cpu
}

func (h hypervisor) defaultVCPUs() float32 {
	numCPUs := float32(getCurrentCpuNum())

	if h.NumVCPUs < 0 || h.NumVCPUs > numCPUs {
		return numCPUs
	}
	if h.NumVCPUs == 0 { // or unspecified
		return float32(defaultVCPUCount)
	}

	return h.NumVCPUs
}

func (h hypervisor) defaultMaxVCPUs() uint32 {
	numcpus := getHostCPUs()
	maxvcpus := govmm.MaxVCPUs()
	reqVCPUs := h.DefaultMaxVCPUs

	//don't exceed the number of physical CPUs. If a default is not provided, use the
	// numbers of physical CPUs
	if reqVCPUs >= numcpus || reqVCPUs == 0 {
		reqVCPUs = numcpus
	}

	// Don't exceed the maximum number of vCPUs supported by hypervisor
	if reqVCPUs > maxvcpus {
		return maxvcpus
	}

	return reqVCPUs
}

func (h hypervisor) defaultMemSz() uint32 {
	if h.MemorySize < vc.MinHypervisorMemory {
		return defaultMemSize // MiB
	}

	return h.MemorySize
}

func (h hypervisor) defaultMemSlots() uint32 {
	slots := h.MemSlots
	if slots == 0 {
		slots = defaultMemSlots
	}

	return slots
}

func (h hypervisor) defaultMemOffset() uint64 {
	offset := h.MemOffset
	if offset == 0 {
		offset = defaultMemOffset
	}

	return offset
}

func (h hypervisor) defaultMaxMemSz() uint64 {
	hostMemory := memory.TotalMemory() / 1024 / 1024 //MiB

	if h.DefaultMaxMemorySize == 0 {
		return hostMemory
	}

	if h.DefaultMaxMemorySize > hostMemory {
		return hostMemory
	}

	return h.DefaultMaxMemorySize
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

func (h hypervisor) defaultHypervisorLoglevel() uint32 {
	if h.HypervisorLoglevel > maxHypervisorLoglevel {
		return maxHypervisorLoglevel
	}

	return h.HypervisorLoglevel
}

func (h hypervisor) defaultVirtioFSCache() string {
	if h.VirtioFSCache == "" {
		return defaultVirtioFSCacheMode
	}

	return h.VirtioFSCache
}

func (h hypervisor) blockDeviceDriver() (string, error) {
	supportedBlockDrivers := []string{config.VirtioSCSI, config.VirtioBlock, config.VirtioMmio, config.Nvdimm, config.VirtioBlockCCW}

	if h.BlockDeviceDriver == "" {
		return defaultBlockDeviceDriver, nil
	}

	for _, b := range supportedBlockDrivers {
		if b == h.BlockDeviceDriver {
			return h.BlockDeviceDriver, nil
		}
	}

	return "", fmt.Errorf("Invalid hypervisor block storage driver %v specified (supported drivers: %v)", h.BlockDeviceDriver, supportedBlockDrivers)
}

func (h hypervisor) blockDeviceAIO() (string, error) {
	supportedBlockAIO := []string{config.AIOIOUring, config.AIONative, config.AIOThreads}

	if h.BlockDeviceAIO == "" {
		return defaultBlockDeviceAIO, nil
	}

	for _, b := range supportedBlockAIO {
		if b == h.BlockDeviceAIO {
			return h.BlockDeviceAIO, nil
		}
	}

	return "", fmt.Errorf("Invalid hypervisor block storage I/O mechanism  %v specified (supported AIO: %v)", h.BlockDeviceAIO, supportedBlockAIO)
}

func (h hypervisor) extraMonitorSocket() (govmmQemu.MonitorProtocol, error) {
	supportedExtraMonitor := []govmmQemu.MonitorProtocol{govmmQemu.Hmp, govmmQemu.Qmp, govmmQemu.QmpPretty}

	if h.ExtraMonitorSocket == "" {
		return "", nil
	}

	for _, extra := range supportedExtraMonitor {
		if extra == h.ExtraMonitorSocket {
			return extra, nil
		}
	}

	return "", fmt.Errorf("Invalid hypervisor extra monitor socket %v specified (supported values: %v)", h.ExtraMonitorSocket, supportedExtraMonitor)
}

func (h hypervisor) sharedFS() (string, error) {
	supportedSharedFS := []string{config.Virtio9P, config.VirtioFS, config.VirtioFSNydus, config.NoSharedFS}

	if h.SharedFS == "" {
		return config.VirtioFS, nil
	}

	for _, fs := range supportedSharedFS {
		if fs == h.SharedFS {
			return h.SharedFS, nil
		}
	}

	return "", fmt.Errorf("Invalid hypervisor shared file system %v specified (supported file systems: %v)", h.SharedFS, supportedSharedFS)
}

func (h hypervisor) msize9p() uint32 {
	if h.Msize9p == 0 {
		return defaultMsize9p
	}

	return h.Msize9p
}

func (h hypervisor) guestHookPath() string {
	if h.GuestHookPath == "" {
		return defaultGuestHookPath
	}
	return h.GuestHookPath
}

func (h hypervisor) vhostUserStorePath() string {
	if h.VhostUserStorePath == "" {
		return defaultVhostUserStorePath
	}
	return h.VhostUserStorePath
}

func (h hypervisor) getDiskRateLimiterBwMaxRate() int64 {
	return h.DiskRateLimiterBwMaxRate
}

func (h hypervisor) getDiskRateLimiterBwOneTimeBurst() int64 {
	if h.DiskRateLimiterBwOneTimeBurst != 0 && h.getDiskRateLimiterBwMaxRate() == 0 {
		kataUtilsLogger.Warn("The DiskRateLimiterBwOneTimeBurst is set but DiskRateLimiterBwMaxRate is not set, this option will be ignored.")

		h.DiskRateLimiterBwOneTimeBurst = 0
	}

	return h.DiskRateLimiterBwOneTimeBurst
}

func (h hypervisor) getDiskRateLimiterOpsMaxRate() int64 {
	return h.DiskRateLimiterOpsMaxRate
}

func (h hypervisor) getDiskRateLimiterOpsOneTimeBurst() int64 {
	if h.DiskRateLimiterOpsOneTimeBurst != 0 && h.getDiskRateLimiterOpsMaxRate() == 0 {
		kataUtilsLogger.Warn("The DiskRateLimiterOpsOneTimeBurst is set but DiskRateLimiterOpsMaxRate is not set, this option will be ignored.")

		h.DiskRateLimiterOpsOneTimeBurst = 0
	}

	return h.DiskRateLimiterOpsOneTimeBurst
}

func (h hypervisor) getRxRateLimiterCfg() uint64 {
	return h.RxRateLimiterMaxRate
}

func (h hypervisor) getTxRateLimiterCfg() uint64 {
	return h.TxRateLimiterMaxRate
}

func (h hypervisor) getNetRateLimiterBwMaxRate() int64 {
	return h.NetRateLimiterBwMaxRate
}

func (h hypervisor) getNetRateLimiterBwOneTimeBurst() int64 {
	if h.NetRateLimiterBwOneTimeBurst != 0 && h.getNetRateLimiterBwMaxRate() == 0 {
		kataUtilsLogger.Warn("The NetRateLimiterBwOneTimeBurst is set but NetRateLimiterBwMaxRate is not set, this option will be ignored.")

		h.NetRateLimiterBwOneTimeBurst = 0
	}

	return h.NetRateLimiterBwOneTimeBurst
}

func (h hypervisor) getNetRateLimiterOpsMaxRate() int64 {
	return h.NetRateLimiterOpsMaxRate
}

func (h hypervisor) getNetRateLimiterOpsOneTimeBurst() int64 {
	if h.NetRateLimiterOpsOneTimeBurst != 0 && h.getNetRateLimiterOpsMaxRate() == 0 {
		kataUtilsLogger.Warn("The NetRateLimiterOpsOneTimeBurst is set but NetRateLimiterOpsMaxRate is not set, this option will be ignored.")

		h.NetRateLimiterOpsOneTimeBurst = 0
	}

	return h.NetRateLimiterOpsOneTimeBurst
}

func (h hypervisor) getIOMMUPlatform() bool {
	if h.IOMMUPlatform {
		kataUtilsLogger.Info("IOMMUPlatform is enabled by default.")
	} else {
		kataUtilsLogger.Info("IOMMUPlatform is disabled by default.")
	}
	return h.IOMMUPlatform
}

func (h hypervisor) getRemoteHypervisorSocket() string {
	if h.RemoteHypervisorSocket == "" {
		return defaultRemoteHypervisorSocket
	}
	return h.RemoteHypervisorSocket
}

func (h hypervisor) getRemoteHypervisorTimeout() uint32 {
	if h.RemoteHypervisorTimeout == 0 {
		return defaultRemoteHypervisorTimeout
	}
	return h.RemoteHypervisorTimeout
}

func (a agent) debugConsoleEnabled() bool {
	return a.DebugConsoleEnabled
}

func (a agent) dialTimout() uint32 {
	return a.DialTimeout
}

func (a agent) cdhApiTimout() uint32 {
	return a.CdhApiTimeout
}

func (a agent) debug() bool {
	return a.Debug
}

func (a agent) trace() bool {
	return a.Tracing
}

func (a agent) kernelModules() []string {
	return a.KernelModules
}

func newFirecrackerHypervisorConfig(h hypervisor) (vc.HypervisorConfig, error) {
	hypervisor, err := h.path()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	jailer, err := h.jailerPath()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	kernel, err := h.kernel()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	initrd, err := h.initrd()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	image, err := h.image()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	rootfsType, err := h.rootfsType()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	firmware, err := h.firmware()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	kernelParams := h.kernelParams()

	blockDriver, err := h.blockDeviceDriver()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	rxRateLimiterMaxRate := h.getRxRateLimiterCfg()
	txRateLimiterMaxRate := h.getTxRateLimiterCfg()

	return vc.HypervisorConfig{
		HypervisorPath:        hypervisor,
		HypervisorPathList:    h.HypervisorPathList,
		JailerPath:            jailer,
		JailerPathList:        h.JailerPathList,
		KernelPath:            kernel,
		InitrdPath:            initrd,
		ImagePath:             image,
		RootfsType:            rootfsType,
		FirmwarePath:          firmware,
		KernelParams:          vc.DeserializeParams(vc.KernelParamFields(kernelParams)),
		NumVCPUsF:             h.defaultVCPUs(),
		DefaultMaxVCPUs:       h.defaultMaxVCPUs(),
		MemorySize:            h.defaultMemSz(),
		MemSlots:              h.defaultMemSlots(),
		DefaultMaxMemorySize:  h.defaultMaxMemSz(),
		EntropySource:         h.GetEntropySource(),
		EntropySourceList:     h.EntropySourceList,
		DefaultBridges:        h.defaultBridges(),
		DisableBlockDeviceUse: false, // shared fs is not supported in Firecracker,
		HugePages:             h.HugePages,
		Debug:                 h.Debug,
		DisableNestingChecks:  h.DisableNestingChecks,
		BlockDeviceDriver:     blockDriver,
		EnableIOThreads:       h.EnableIOThreads,
		DisableVhostNet:       true, // vhost-net backend is not supported in Firecracker
		GuestHookPath:         h.guestHookPath(),
		RxRateLimiterMaxRate:  rxRateLimiterMaxRate,
		TxRateLimiterMaxRate:  txRateLimiterMaxRate,
		EnableAnnotations:     h.EnableAnnotations,
		DisableSeLinux:        h.DisableSeLinux,
		DisableGuestSeLinux:   true, // Guest SELinux is not supported in Firecracker
	}, nil
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

	initrd, err := h.initrd()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	image, err := h.image()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	rootfsType, err := h.rootfsType()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	pflashes, err := h.PFlash()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	firmware, err := h.firmware()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	firmwareVolume, err := h.firmwareVolume()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	machineAccelerators := h.machineAccelerators()
	cpuFeatures := h.cpuFeatures()
	kernelParams := h.kernelParams()
	machineType := h.machineType()

	// The "microvm" machine type doesn't support NVDIMM so override the
	// config setting to explicitly disable it (i.e. don't require the
	// user to add 'disable_image_nvdimm = true' in the .toml file).
	if machineType == govmmQemu.MachineTypeMicrovm && !h.DisableImageNvdimm {
		h.DisableImageNvdimm = true
		kataUtilsLogger.Info("Setting 'disable_image_nvdimm = true' as microvm does not support NVDIMM")
	}

	// Nvdimm can only be support when UEFI/ACPI is enabled on arm64, otherwise disable it.
	if goruntime.GOARCH == "arm64" && firmware == "" {
		if p, err := h.PFlash(); err == nil {
			if len(p) == 0 {
				h.DisableImageNvdimm = true
				kataUtilsLogger.Info("Setting 'disable_image_nvdimm = true' if there is no firmware specified")
			}
		}
	}

	blockDriver, err := h.blockDeviceDriver()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	blockAIO, err := h.blockDeviceAIO()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	sharedFS, err := h.sharedFS()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if (sharedFS == config.VirtioFS || sharedFS == config.VirtioFSNydus) && h.VirtioFSDaemon == "" {
		return vc.HypervisorConfig{},
			fmt.Errorf("cannot enable %s without daemon path in configuration file", sharedFS)
	}

	if vSock, err := utils.SupportsVsocks(); !vSock {
		return vc.HypervisorConfig{}, err
	}

	rxRateLimiterMaxRate := h.getRxRateLimiterCfg()
	txRateLimiterMaxRate := h.getTxRateLimiterCfg()

	extraMonitorSocket, err := h.extraMonitorSocket()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	return vc.HypervisorConfig{
		HypervisorPath:           hypervisor,
		HypervisorPathList:       h.HypervisorPathList,
		KernelPath:               kernel,
		InitrdPath:               initrd,
		ImagePath:                image,
		RootfsType:               rootfsType,
		FirmwarePath:             firmware,
		FirmwareVolumePath:       firmwareVolume,
		PFlash:                   pflashes,
		MachineAccelerators:      machineAccelerators,
		CPUFeatures:              cpuFeatures,
		KernelParams:             vc.DeserializeParams(vc.KernelParamFields(kernelParams)),
		HypervisorMachineType:    machineType,
		QgsPort:                  h.qgsPort(),
		NumVCPUsF:                h.defaultVCPUs(),
		DefaultMaxVCPUs:          h.defaultMaxVCPUs(),
		MemorySize:               h.defaultMemSz(),
		MemSlots:                 h.defaultMemSlots(),
		MemOffset:                h.defaultMemOffset(),
		DefaultMaxMemorySize:     h.defaultMaxMemSz(),
		VirtioMem:                h.VirtioMem,
		EntropySource:            h.GetEntropySource(),
		EntropySourceList:        h.EntropySourceList,
		DefaultBridges:           h.defaultBridges(),
		DisableBlockDeviceUse:    h.DisableBlockDeviceUse,
		SharedFS:                 sharedFS,
		VirtioFSDaemon:           h.VirtioFSDaemon,
		VirtioFSDaemonList:       h.VirtioFSDaemonList,
		HypervisorLoglevel:       h.defaultHypervisorLoglevel(),
		VirtioFSCacheSize:        h.VirtioFSCacheSize,
		VirtioFSCache:            h.defaultVirtioFSCache(),
		VirtioFSQueueSize:        h.VirtioFSQueueSize,
		VirtioFSExtraArgs:        h.VirtioFSExtraArgs,
		MemPrealloc:              h.MemPrealloc,
		HugePages:                h.HugePages,
		IOMMU:                    h.IOMMU,
		IOMMUPlatform:            h.getIOMMUPlatform(),
		FileBackedMemRootDir:     h.FileBackedMemRootDir,
		FileBackedMemRootList:    h.FileBackedMemRootList,
		Debug:                    h.Debug,
		DisableNestingChecks:     h.DisableNestingChecks,
		BlockDeviceDriver:        blockDriver,
		BlockDeviceAIO:           blockAIO,
		BlockDeviceCacheSet:      h.BlockDeviceCacheSet,
		BlockDeviceCacheDirect:   h.BlockDeviceCacheDirect,
		BlockDeviceCacheNoflush:  h.BlockDeviceCacheNoflush,
		EnableIOThreads:          h.EnableIOThreads,
		Msize9p:                  h.msize9p(),
		DisableImageNvdimm:       h.DisableImageNvdimm,
		HotPlugVFIO:              h.hotPlugVFIO(),
		ColdPlugVFIO:             h.coldPlugVFIO(),
		PCIeRootPort:             h.pcieRootPort(),
		PCIeSwitchPort:           h.pcieSwitchPort(),
		DisableVhostNet:          h.DisableVhostNet,
		EnableVhostUserStore:     h.EnableVhostUserStore,
		VhostUserStorePath:       h.vhostUserStorePath(),
		VhostUserStorePathList:   h.VhostUserStorePathList,
		VhostUserDeviceReconnect: h.VhostUserDeviceReconnect,
		SeccompSandbox:           h.SeccompSandbox,
		GuestHookPath:            h.guestHookPath(),
		RxRateLimiterMaxRate:     rxRateLimiterMaxRate,
		TxRateLimiterMaxRate:     txRateLimiterMaxRate,
		EnableAnnotations:        h.EnableAnnotations,
		GuestMemoryDumpPath:      h.GuestMemoryDumpPath,
		GuestMemoryDumpPaging:    h.GuestMemoryDumpPaging,
		ConfidentialGuest:        h.ConfidentialGuest,
		SevSnpGuest:              h.SevSnpGuest,
		GuestSwap:                h.GuestSwap,
		Rootless:                 h.Rootless,
		LegacySerial:             h.LegacySerial,
		DisableSeLinux:           h.DisableSeLinux,
		DisableGuestSeLinux:      h.DisableGuestSeLinux,
		ExtraMonitorSocket:       extraMonitorSocket,
	}, nil
}

func newClhHypervisorConfig(h hypervisor) (vc.HypervisorConfig, error) {
	hypervisor, err := h.path()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	kernel, err := h.kernel()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	initrd, err := h.initrd()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	image, err := h.image()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if image == "" && initrd == "" {
		return vc.HypervisorConfig{},
			errors.New("image or initrd must be defined in the configuration file")
	}

	rootfsType, err := h.rootfsType()
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

	sharedFS, err := h.sharedFS()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if sharedFS != config.VirtioFS && sharedFS != config.VirtioFSNydus && sharedFS != config.NoSharedFS {
		return vc.HypervisorConfig{},
			fmt.Errorf("Cloud Hypervisor does not support %s shared filesystem option", sharedFS)
	}

	if (sharedFS == config.VirtioFS || sharedFS == config.VirtioFSNydus) && h.VirtioFSDaemon == "" {
		return vc.HypervisorConfig{},
			fmt.Errorf("cannot enable %s without daemon path in configuration file", sharedFS)
	}

	return vc.HypervisorConfig{
		HypervisorPath:                 hypervisor,
		HypervisorPathList:             h.HypervisorPathList,
		KernelPath:                     kernel,
		InitrdPath:                     initrd,
		ImagePath:                      image,
		RootfsType:                     rootfsType,
		FirmwarePath:                   firmware,
		MachineAccelerators:            machineAccelerators,
		KernelParams:                   vc.DeserializeParams(vc.KernelParamFields(kernelParams)),
		HypervisorMachineType:          machineType,
		NumVCPUsF:                      h.defaultVCPUs(),
		DefaultMaxVCPUs:                h.defaultMaxVCPUs(),
		MemorySize:                     h.defaultMemSz(),
		MemSlots:                       h.defaultMemSlots(),
		MemOffset:                      h.defaultMemOffset(),
		DefaultMaxMemorySize:           h.defaultMaxMemSz(),
		VirtioMem:                      h.VirtioMem,
		EntropySource:                  h.GetEntropySource(),
		EntropySourceList:              h.EntropySourceList,
		DefaultBridges:                 h.defaultBridges(),
		DisableBlockDeviceUse:          h.DisableBlockDeviceUse,
		SharedFS:                       sharedFS,
		VirtioFSDaemon:                 h.VirtioFSDaemon,
		VirtioFSDaemonList:             h.VirtioFSDaemonList,
		HypervisorLoglevel:             h.defaultHypervisorLoglevel(),
		VirtioFSCacheSize:              h.VirtioFSCacheSize,
		VirtioFSCache:                  h.VirtioFSCache,
		MemPrealloc:                    h.MemPrealloc,
		HugePages:                      h.HugePages,
		FileBackedMemRootDir:           h.FileBackedMemRootDir,
		FileBackedMemRootList:          h.FileBackedMemRootList,
		Debug:                          h.Debug,
		DisableNestingChecks:           h.DisableNestingChecks,
		BlockDeviceDriver:              blockDriver,
		BlockDeviceCacheSet:            h.BlockDeviceCacheSet,
		BlockDeviceCacheDirect:         h.BlockDeviceCacheDirect,
		EnableIOThreads:                h.EnableIOThreads,
		Msize9p:                        h.msize9p(),
		ColdPlugVFIO:                   h.coldPlugVFIO(),
		HotPlugVFIO:                    h.hotPlugVFIO(),
		PCIeRootPort:                   h.pcieRootPort(),
		PCIeSwitchPort:                 h.pcieSwitchPort(),
		DisableVhostNet:                true,
		GuestHookPath:                  h.guestHookPath(),
		VirtioFSExtraArgs:              h.VirtioFSExtraArgs,
		SGXEPCSize:                     defaultSGXEPCSize,
		EnableAnnotations:              h.EnableAnnotations,
		DisableSeccomp:                 h.DisableSeccomp,
		ConfidentialGuest:              h.ConfidentialGuest,
		Rootless:                       h.Rootless,
		DisableSeLinux:                 h.DisableSeLinux,
		DisableGuestSeLinux:            h.DisableGuestSeLinux,
		NetRateLimiterBwMaxRate:        h.getNetRateLimiterBwMaxRate(),
		NetRateLimiterBwOneTimeBurst:   h.getNetRateLimiterBwOneTimeBurst(),
		NetRateLimiterOpsMaxRate:       h.getNetRateLimiterOpsMaxRate(),
		NetRateLimiterOpsOneTimeBurst:  h.getNetRateLimiterOpsOneTimeBurst(),
		DiskRateLimiterBwMaxRate:       h.getDiskRateLimiterBwMaxRate(),
		DiskRateLimiterBwOneTimeBurst:  h.getDiskRateLimiterBwOneTimeBurst(),
		DiskRateLimiterOpsMaxRate:      h.getDiskRateLimiterOpsMaxRate(),
		DiskRateLimiterOpsOneTimeBurst: h.getDiskRateLimiterOpsOneTimeBurst(),
	}, nil
}

func newDragonballHypervisorConfig(h hypervisor) (vc.HypervisorConfig, error) {
	kernel, err := h.kernel()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	image, err := h.image()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	rootfsType, err := h.rootfsType()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	kernelParams := h.kernelParams()

	return vc.HypervisorConfig{
		KernelPath:      kernel,
		ImagePath:       image,
		RootfsType:      rootfsType,
		KernelParams:    vc.DeserializeParams(vc.KernelParamFields(kernelParams)),
		NumVCPUsF:       h.defaultVCPUs(),
		DefaultMaxVCPUs: h.defaultMaxVCPUs(),
		MemorySize:      h.defaultMemSz(),
		MemSlots:        h.defaultMemSlots(),
		EntropySource:   h.GetEntropySource(),
		Debug:           h.Debug,
	}, nil
}

func newStratovirtHypervisorConfig(h hypervisor) (vc.HypervisorConfig, error) {
	hypervisor, err := h.path()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	kernel, err := h.kernel()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	initrd, err := h.initrd()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	image, err := h.image()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if image != "" && initrd != "" {
		return vc.HypervisorConfig{},
			errors.New("having both an image and an initrd defined in the configuration file is not supported")
	}

	if image == "" && initrd == "" {
		return vc.HypervisorConfig{},
			errors.New("image or initrd must be defined in the configuration file")
	}

	rootfsType, err := h.rootfsType()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	kernelParams := h.kernelParams()
	machineType := h.machineType()

	blockDriver, err := h.blockDeviceDriver()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if vSock, err := utils.SupportsVsocks(); !vSock {
		return vc.HypervisorConfig{}, err
	}

	sharedFS, err := h.sharedFS()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if sharedFS != config.VirtioFS && sharedFS != config.VirtioFSNydus && sharedFS != config.NoSharedFS {
		return vc.HypervisorConfig{},
			fmt.Errorf("Stratovirt Hypervisor does not support %s shared filesystem option", sharedFS)
	}

	if (sharedFS == config.VirtioFS || sharedFS == config.VirtioFSNydus) && h.VirtioFSDaemon == "" {
		return vc.HypervisorConfig{},
			fmt.Errorf("cannot enable %s without daemon path in configuration file", sharedFS)
	}

	return vc.HypervisorConfig{
		HypervisorPath:        hypervisor,
		HypervisorPathList:    h.HypervisorPathList,
		KernelPath:            kernel,
		InitrdPath:            initrd,
		ImagePath:             image,
		RootfsType:            rootfsType,
		KernelParams:          vc.DeserializeParams(strings.Fields(kernelParams)),
		HypervisorMachineType: machineType,
		NumVCPUsF:             h.defaultVCPUs(),
		DefaultMaxVCPUs:       h.defaultMaxVCPUs(),
		MemorySize:            h.defaultMemSz(),
		MemSlots:              h.defaultMemSlots(),
		MemOffset:             h.defaultMemOffset(),
		DefaultMaxMemorySize:  h.defaultMaxMemSz(),
		EntropySource:         h.GetEntropySource(),
		DefaultBridges:        h.defaultBridges(),
		DisableBlockDeviceUse: h.DisableBlockDeviceUse,
		SharedFS:              sharedFS,
		VirtioFSDaemon:        h.VirtioFSDaemon,
		VirtioFSDaemonList:    h.VirtioFSDaemonList,
		HypervisorLoglevel:    h.defaultHypervisorLoglevel(),
		VirtioFSCacheSize:     h.VirtioFSCacheSize,
		VirtioFSCache:         h.defaultVirtioFSCache(),
		VirtioFSExtraArgs:     h.VirtioFSExtraArgs,
		HugePages:             h.HugePages,
		Debug:                 h.Debug,
		DisableNestingChecks:  h.DisableNestingChecks,
		BlockDeviceDriver:     blockDriver,
		DisableVhostNet:       true,
		GuestHookPath:         h.guestHookPath(),
		EnableAnnotations:     h.EnableAnnotations,
		DisableSeccomp:        h.DisableSeccomp,
		DisableSeLinux:        h.DisableSeLinux,
		DisableGuestSeLinux:   h.DisableGuestSeLinux,
	}, nil
}

func newRemoteHypervisorConfig(h hypervisor) (vc.HypervisorConfig, error) {

	return vc.HypervisorConfig{
		RemoteHypervisorSocket:  h.getRemoteHypervisorSocket(),
		RemoteHypervisorTimeout: h.getRemoteHypervisorTimeout(),
		DisableGuestSeLinux:     true, // The remote hypervisor has a different guest, so Guest SELinux config doesn't work
		HypervisorMachineType:   h.MachineType,
		SharedFS:                config.NoSharedFS,

		// No valid value so avoid to append block device to list in kata_agent.appendDevices
		BlockDeviceDriver: "dummy",
		EnableAnnotations: h.EnableAnnotations,
		GuestHookPath:     h.guestHookPath(),
	}, nil
}

func newFactoryConfig(f factory) (oci.FactoryConfig, error) {
	if f.TemplatePath == "" {
		f.TemplatePath = defaultTemplatePath
	}
	if f.VMCacheEndpoint == "" {
		f.VMCacheEndpoint = defaultVMCacheEndpoint
	}
	return oci.FactoryConfig{
		Template:        f.Template,
		TemplatePath:    f.TemplatePath,
		VMCacheNumber:   f.VMCacheNumber,
		VMCacheEndpoint: f.VMCacheEndpoint,
	}, nil
}

func updateRuntimeConfigHypervisor(configPath string, tomlConf tomlConfig, config *oci.RuntimeConfig) error {
	for k, hypervisor := range tomlConf.Hypervisor {
		var err error
		var hConfig vc.HypervisorConfig

		switch k {
		case firecrackerHypervisorTableType:
			config.HypervisorType = vc.FirecrackerHypervisor
			hConfig, err = newFirecrackerHypervisorConfig(hypervisor)
		case qemuHypervisorTableType:
			config.HypervisorType = vc.QemuHypervisor
			hConfig, err = newQemuHypervisorConfig(hypervisor)
		case clhHypervisorTableType:
			config.HypervisorType = vc.ClhHypervisor
			hConfig, err = newClhHypervisorConfig(hypervisor)
		case dragonballHypervisorTableType:
			config.HypervisorType = vc.DragonballHypervisor
			hConfig, err = newDragonballHypervisorConfig(hypervisor)
		case stratovirtHypervisorTableType:
			config.HypervisorType = vc.StratovirtHypervisor
			hConfig, err = newStratovirtHypervisorConfig(hypervisor)
		case remoteHypervisorTableType:
			config.HypervisorType = vc.RemoteHypervisor
			hConfig, err = newRemoteHypervisorConfig(hypervisor)
		default:
			err = fmt.Errorf("%s: %+q", errInvalidHypervisorPrefix, k)
		}

		if err != nil {
			return fmt.Errorf("%v: %v", configPath, err)
		}

		config.HypervisorConfig = hConfig
	}

	return nil
}

func updateRuntimeConfigAgent(configPath string, tomlConf tomlConfig, config *oci.RuntimeConfig) error {
	for _, agent := range tomlConf.Agent {
		config.AgentConfig = vc.KataAgentConfig{
			LongLiveConn:       true,
			Debug:              agent.debug(),
			Trace:              agent.trace(),
			KernelModules:      agent.kernelModules(),
			EnableDebugConsole: agent.debugConsoleEnabled(),
			DialTimeout:        agent.dialTimout(),
			CdhApiTimeout:      agent.cdhApiTimout(),
		}
	}

	return nil
}

// SetKernelParams adds the user-specified kernel parameters (from the
// configuration file) to the defaults so that the former take priority.
func SetKernelParams(runtimeConfig *oci.RuntimeConfig) error {
	defaultKernelParams := GetKernelParamsFunc(needSystemd(runtimeConfig.HypervisorConfig), runtimeConfig.Trace)

	if runtimeConfig.HypervisorConfig.Debug {
		strParams := vc.SerializeParams(defaultKernelParams, "=")
		formatted := strings.Join(strParams, " ")
		kataUtilsLogger.WithField("default-kernel-parameters", formatted).Debug()
	}

	// retrieve the parameters specified in the config file
	userKernelParams := runtimeConfig.HypervisorConfig.KernelParams

	// reset
	runtimeConfig.HypervisorConfig.KernelParams = []vc.Param{}

	// first, add default values
	for _, p := range defaultKernelParams {
		if err := runtimeConfig.AddKernelParam(p); err != nil {
			return err
		}
	}

	// set the scsi scan mode to none for virtio-scsi
	if runtimeConfig.HypervisorConfig.BlockDeviceDriver == config.VirtioSCSI {
		p := vc.Param{
			Key:   "scsi_mod.scan",
			Value: "none",
		}
		if err := runtimeConfig.AddKernelParam(p); err != nil {
			return err
		}
	}

	// next, check for agent specific kernel params

	params := vc.KataAgentKernelParams(runtimeConfig.AgentConfig)

	for _, p := range params {
		if err := runtimeConfig.AddKernelParam(p); err != nil {
			return err
		}
	}

	// now re-add the user-specified values so that they take priority.
	for _, p := range userKernelParams {
		if err := runtimeConfig.AddKernelParam(p); err != nil {
			return err
		}
	}

	return nil
}

func updateRuntimeConfig(configPath string, tomlConf tomlConfig, config *oci.RuntimeConfig) error {
	if err := updateRuntimeConfigHypervisor(configPath, tomlConf, config); err != nil {
		return err
	}

	if err := updateRuntimeConfigAgent(configPath, tomlConf, config); err != nil {
		return err
	}

	fConfig, err := newFactoryConfig(tomlConf.Factory)
	if err != nil {
		return fmt.Errorf("%v: %v", configPath, err)
	}
	config.FactoryConfig = fConfig

	err = SetKernelParams(config)
	if err != nil {
		return err
	}

	return nil
}

func GetDefaultHypervisorConfig() vc.HypervisorConfig {
	return vc.HypervisorConfig{
		HypervisorPath:           defaultHypervisorPath,
		JailerPath:               defaultJailerPath,
		KernelPath:               defaultKernelPath,
		ImagePath:                defaultImagePath,
		InitrdPath:               defaultInitrdPath,
		RootfsType:               defaultRootfsType,
		FirmwarePath:             defaultFirmwarePath,
		FirmwareVolumePath:       defaultFirmwareVolumePath,
		MachineAccelerators:      defaultMachineAccelerators,
		CPUFeatures:              defaultCPUFeatures,
		HypervisorMachineType:    defaultMachineType,
		NumVCPUsF:                float32(defaultVCPUCount),
		DefaultMaxVCPUs:          defaultMaxVCPUCount,
		MemorySize:               defaultMemSize,
		MemOffset:                defaultMemOffset,
		VirtioMem:                defaultVirtioMem,
		DisableBlockDeviceUse:    defaultDisableBlockDeviceUse,
		DefaultBridges:           defaultBridgesCount,
		MemPrealloc:              defaultEnableMemPrealloc,
		HugePages:                defaultEnableHugePages,
		IOMMU:                    defaultEnableIOMMU,
		IOMMUPlatform:            defaultEnableIOMMUPlatform,
		FileBackedMemRootDir:     defaultFileBackedMemRootDir,
		Debug:                    defaultEnableDebug,
		ExtraMonitorSocket:       defaultExtraMonitorSocket,
		DisableNestingChecks:     defaultDisableNestingChecks,
		BlockDeviceDriver:        defaultBlockDeviceDriver,
		BlockDeviceAIO:           defaultBlockDeviceAIO,
		BlockDeviceCacheSet:      defaultBlockDeviceCacheSet,
		BlockDeviceCacheDirect:   defaultBlockDeviceCacheDirect,
		BlockDeviceCacheNoflush:  defaultBlockDeviceCacheNoflush,
		EnableIOThreads:          defaultEnableIOThreads,
		Msize9p:                  defaultMsize9p,
		ColdPlugVFIO:             defaultColdPlugVFIO,
		HotPlugVFIO:              defaultHotPlugVFIO,
		PCIeRootPort:             defaultPCIeRootPort,
		PCIeSwitchPort:           defaultPCIeSwitchPort,
		GuestHookPath:            defaultGuestHookPath,
		VhostUserStorePath:       defaultVhostUserStorePath,
		VhostUserDeviceReconnect: defaultVhostUserDeviceReconnect,
		HypervisorLoglevel:       defaultHypervisorLoglevel,
		VirtioFSCache:            defaultVirtioFSCacheMode,
		DisableImageNvdimm:       defaultDisableImageNvdimm,
		RxRateLimiterMaxRate:     defaultRxRateLimiterMaxRate,
		TxRateLimiterMaxRate:     defaultTxRateLimiterMaxRate,
		SGXEPCSize:               defaultSGXEPCSize,
		ConfidentialGuest:        defaultConfidentialGuest,
		SevSnpGuest:              defaultSevSnpGuest,
		GuestSwap:                defaultGuestSwap,
		Rootless:                 defaultRootlessHypervisor,
		DisableSeccomp:           defaultDisableSeccomp,
		DisableGuestSeLinux:      defaultDisableGuestSeLinux,
		LegacySerial:             defaultLegacySerial,
	}
}

func initConfig() (config oci.RuntimeConfig, err error) {
	err = config.InterNetworkModel.SetModel(defaultInterNetworkingModel)
	if err != nil {
		return oci.RuntimeConfig{}, err
	}

	err = config.VfioMode.VFIOSetMode(defaultVfioMode)
	if err != nil {
		return oci.RuntimeConfig{}, err
	}

	config = oci.RuntimeConfig{
		HypervisorType:   defaultHypervisor,
		HypervisorConfig: GetDefaultHypervisorConfig(),
		AgentConfig:      vc.KataAgentConfig{},
	}

	return config, nil
}

// LoadConfiguration loads the configuration file and converts it into a
// runtime configuration.
//
// If ignoreLogging is true, the system logger will not be initialised nor
// will this function make any log calls.
//
// All paths are resolved fully meaning if this function does not return an
// error, all paths are valid at the time of the call.
func LoadConfiguration(configPath string, ignoreLogging bool) (resolvedConfigPath string, config oci.RuntimeConfig, err error) {

	config, err = initConfig()
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	tomlConf, resolved, err := decodeConfig(configPath)
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	config.Debug = tomlConf.Runtime.Debug
	if !tomlConf.Runtime.Debug {
		// If debug is not required, switch back to the original
		// default log priority, otherwise continue in debug mode.
		kataUtilsLogger.Logger.Level = originalLoggerLevel
	}

	config.Trace = tomlConf.Runtime.Tracing
	katatrace.SetTracing(config.Trace)

	if tomlConf.Runtime.InterNetworkModel != "" {
		err = config.InterNetworkModel.SetModel(tomlConf.Runtime.InterNetworkModel)
		if err != nil {
			return "", config, err
		}
	}

	if tomlConf.Runtime.VfioMode != "" {
		err = config.VfioMode.VFIOSetMode(tomlConf.Runtime.VfioMode)

		if err != nil {
			return "", config, err
		}
	}

	if !ignoreLogging {
		err := handleSystemLog("", "")
		if err != nil {
			return "", config, err
		}

		kataUtilsLogger.WithFields(
			logrus.Fields{
				"format": "TOML",
				"file":   resolved,
			}).Info("loaded configuration")
	}

	if err := updateRuntimeConfig(resolved, tomlConf, &config); err != nil {
		return "", config, err
	}

	config.DisableGuestSeccomp = tomlConf.Runtime.DisableGuestSeccomp
	config.EnableVCPUsPinning = tomlConf.Runtime.EnableVCPUsPinning
	config.GuestSeLinuxLabel = tomlConf.Runtime.GuestSeLinuxLabel
	config.StaticSandboxResourceMgmt = tomlConf.Runtime.StaticSandboxResourceMgmt
	config.SandboxCgroupOnly = tomlConf.Runtime.SandboxCgroupOnly
	config.DisableNewNetNs = tomlConf.Runtime.DisableNewNetNs
	config.EnablePprof = tomlConf.Runtime.EnablePprof
	config.JaegerEndpoint = tomlConf.Runtime.JaegerEndpoint
	config.JaegerUser = tomlConf.Runtime.JaegerUser
	config.JaegerPassword = tomlConf.Runtime.JaegerPassword
	config.CreateContainerTimeout = tomlConf.Runtime.CreateContainerTimeout
	for _, f := range tomlConf.Runtime.Experimental {
		feature := exp.Get(f)
		if feature == nil {
			return "", config, fmt.Errorf("Unsupported experimental feature %q", f)
		}
		config.Experimental = append(config.Experimental, *feature)
	}

	if err = validateBindMounts(tomlConf.Runtime.SandboxBindMounts); err != nil {
		return "", config, err
	}
	config.SandboxBindMounts = tomlConf.Runtime.SandboxBindMounts

	config.DisableGuestEmptyDir = tomlConf.Runtime.DisableGuestEmptyDir

	config.DanConfig = tomlConf.Runtime.DanConf
	if err := checkConfig(config); err != nil {
		return "", config, err
	}

	return resolved, config, nil
}

// Verify that bind mounts exist
func validateBindMounts(mounts []string) error {
	if len(mounts) == 0 {
		return nil
	}

	bases := make(map[string]struct{})

	for _, m := range mounts {
		path, err := ResolvePath(m)
		if err != nil {
			return fmt.Errorf("sandbox-bindmounts: Failed to resolve path: %s: %v", m, err)
		}

		base := filepath.Base(path)
		// check to make sure the base does not already exists.
		if _, ok := bases[base]; !ok {
			bases[base] = struct{}{}
		} else {
			return fmt.Errorf("sandbox-bindmounts: File %s has base that matches already specified bindmount", path)
		}
	}
	return nil
}

func decodeConfig(configPath string) (tomlConfig, string, error) {
	var (
		resolved string
		tomlConf tomlConfig
		err      error
	)

	if configPath == "" {
		resolved, err = getDefaultConfigFile()
	} else {
		resolved, err = ResolvePath(configPath)
	}

	if err != nil {
		return tomlConf, "", fmt.Errorf("Cannot find usable config file (%v)", err)
	}

	configData, err := os.ReadFile(resolved)
	if err != nil {
		return tomlConf, resolved, err
	}

	_, err = toml.Decode(string(configData), &tomlConf)
	if err != nil {
		return tomlConf, resolved, err
	}

	err = decodeDropIns(resolved, &tomlConf)
	if err != nil {
		return tomlConf, resolved, err
	}

	return tomlConf, resolved, nil
}

func decodeDropIns(mainConfigPath string, tomlConf *tomlConfig) error {
	configDir := filepath.Dir(mainConfigPath)
	dropInDir := filepath.Join(configDir, "config.d")

	files, err := os.ReadDir(dropInDir)
	if err != nil {
		if !os.IsNotExist(err) {
			return fmt.Errorf("error reading %q directory: %s", dropInDir, err)
		} else {
			return nil
		}
	}

	for _, file := range files {
		dropInFpath := filepath.Join(dropInDir, file.Name())

		err = updateFromDropIn(dropInFpath, tomlConf)
		if err != nil {
			return err
		}
	}

	return nil
}

func updateFromDropIn(dropInFpath string, tomlConf *tomlConfig) error {
	configData, err := os.ReadFile(dropInFpath)
	if err != nil {
		return fmt.Errorf("error reading file %q: %s", dropInFpath, err)
	}

	// Ordinarily, BurntSushi only updates fields of tomlConfig that are
	// changed by the file and leaves the rest alone.  This doesn't apply
	// though to tomlConfig substructures that are stored in maps.  Their
	// previous contents are erased by toml.Decode() and only fields changed by
	// the file are set.  To work around this, a bit of juggling is needed to
	// preserve the previous contents and merge them manually with the incoming
	// changes afterwards, using reflection.
	tomlConfOrig := tomlConf.Clone()

	var md toml.MetaData
	md, err = toml.Decode(string(configData), &tomlConf)

	if err != nil {
		return fmt.Errorf("error decoding file %q: %s", dropInFpath, err)
	}

	if len(md.Undecoded()) > 0 {
		msg := fmt.Sprintf("warning: undecoded keys in %q: %+v", dropInFpath, md.Undecoded())
		kataUtilsLogger.Warn(msg)
	}

	for _, key := range md.Keys() {
		err = applyKey(*tomlConf, key, &tomlConfOrig)
		if err != nil {
			return fmt.Errorf("error applying key '%+v' from drop-in file %q: %s", key, dropInFpath, err)
		}
	}

	tomlConf.Hypervisor = tomlConfOrig.Hypervisor
	tomlConf.Agent = tomlConfOrig.Agent

	return nil
}

func applyKey(sourceConf tomlConfig, key []string, targetConf *tomlConfig) error {
	// Any key that might need treatment provided by this function has to have
	// (at least) three components: [ map_name map_key_name field_toml_tag ],
	// e.g. [agent kata enable_tracing] or [hypervisor qemu confidential_guest].
	if len(key) < 3 {
		return nil
	}
	switch key[0] {
	case "agent":
		return applyAgentKey(sourceConf, key[1:], targetConf)
	case "hypervisor":
		return applyHypervisorKey(sourceConf, key[1:], targetConf)
		// The table the 'key' is in is not stored in a map so no special handling
		// is needed.
	}
	return nil
}

// Both of the following functions copy the value of a 'sourceConf' field
// identified by the TOML tag in 'key' into the corresponding field in
// 'targetConf'.
func applyAgentKey(sourceConf tomlConfig, key []string, targetConf *tomlConfig) error {
	agentName := key[0]
	tomlKeyName := key[1]

	sourceAgentConf := sourceConf.Agent[agentName]
	targetAgentConf := targetConf.Agent[agentName]

	err := copyFieldValue(reflect.ValueOf(&sourceAgentConf).Elem(), tomlKeyName, reflect.ValueOf(&targetAgentConf).Elem())
	if err != nil {
		return err
	}

	targetConf.Agent[agentName] = targetAgentConf
	return nil
}

func applyHypervisorKey(sourceConf tomlConfig, key []string, targetConf *tomlConfig) error {
	hypervisorName := key[0]
	tomlKeyName := key[1]

	sourceHypervisorConf := sourceConf.Hypervisor[hypervisorName]
	targetHypervisorConf := targetConf.Hypervisor[hypervisorName]

	err := copyFieldValue(reflect.ValueOf(&sourceHypervisorConf).Elem(), tomlKeyName, reflect.ValueOf(&targetHypervisorConf).Elem())
	if err != nil {
		return err
	}

	targetConf.Hypervisor[hypervisorName] = targetHypervisorConf
	return nil
}

// Copies a TOML value of the source field identified by its TOML key to the
// corresponding field of the target.  Basically
// 'target[tomlKeyName] = source[tomlKeyNmae]'.
func copyFieldValue(source reflect.Value, tomlKeyName string, target reflect.Value) error {
	val, err := getValue(source, tomlKeyName)
	if err != nil {
		return fmt.Errorf("error getting key %q from a decoded drop-in conf file: %s", tomlKeyName, err)
	}
	err = setValue(target, tomlKeyName, val)
	if err != nil {
		return fmt.Errorf("error setting key %q to a new value '%v': %s", tomlKeyName, val.Interface(), err)
	}
	return nil
}

// The first argument is expected to be a reflect.Value of a tomlConfig
// substructure (hypervisor, agent), the second argument is a TOML key
// corresponding to the substructure field whose TOML value is queried.
// Return value corresponds to 'tomlConfStruct[tomlKey]'.
func getValue(tomlConfStruct reflect.Value, tomlKey string) (reflect.Value, error) {
	tomlConfStructType := tomlConfStruct.Type()
	for j := 0; j < tomlConfStruct.NumField(); j++ {
		fieldTomlTag := tomlConfStructType.Field(j).Tag.Get("toml")
		if fieldTomlTag == tomlKey {
			return tomlConfStruct.Field(j), nil
		}
	}
	return reflect.Value{}, fmt.Errorf("key %q not found", tomlKey)
}

// The first argument is expected to be a reflect.Value of a tomlConfig
// substructure (hypervisor, agent), the second argument is a TOML key
// corresponding to the substructure field whose TOML value is to be changed,
// the third argument is a reflect.Value representing the new TOML value.
// An equivalent of 'tomlConfStruct[tomlKey] = newVal'.
func setValue(tomlConfStruct reflect.Value, tomlKey string, newVal reflect.Value) error {
	tomlConfStructType := tomlConfStruct.Type()
	for j := 0; j < tomlConfStruct.NumField(); j++ {
		fieldTomlTag := tomlConfStructType.Field(j).Tag.Get("toml")
		if fieldTomlTag == tomlKey {
			tomlConfStruct.Field(j).Set(newVal)
			return nil
		}
	}
	return fmt.Errorf("key %q not found", tomlKey)
}

// checkConfig checks the validity of the specified config.
func checkConfig(config oci.RuntimeConfig) error {
	if err := checkNetNsConfig(config); err != nil {
		return err
	}

	if err := checkHypervisorConfig(config.HypervisorConfig); err != nil {
		return err
	}

	if err := checkFactoryConfig(config); err != nil {
		return err
	}

	hotPlugVFIO := config.HypervisorConfig.HotPlugVFIO
	coldPlugVFIO := config.HypervisorConfig.ColdPlugVFIO
	machineType := config.HypervisorConfig.HypervisorMachineType
	hypervisorType := config.HypervisorType
	if err := checkPCIeConfig(coldPlugVFIO, hotPlugVFIO, machineType, hypervisorType); err != nil {
		return err
	}

	return nil
}

// checkPCIeConfig ensures the PCIe configuration is valid.
// Only allow one of the following settings for cold-plug:
// no-port, root-port, switch-port
func checkPCIeConfig(coldPlug config.PCIePort, hotPlug config.PCIePort, machineType string, hypervisorType virtcontainers.HypervisorType) error {
	if hypervisorType != virtcontainers.QemuHypervisor && hypervisorType != virtcontainers.ClhHypervisor {
		kataUtilsLogger.Warn("Advanced PCIe Topology only available for QEMU/CLH hypervisor, ignoring hot(cold)_vfio_port setting")
		return nil
	}

	if coldPlug != config.NoPort && hotPlug != config.NoPort {
		return fmt.Errorf("invalid hot-plug=%s and cold-plug=%s settings, only one of them can be set", coldPlug, hotPlug)
	}
	if coldPlug == config.NoPort && hotPlug == config.NoPort {
		return nil
	}
	// Currently only QEMU q35,virt support advanced PCIe topologies
	// firecracker, dragonball do not have right now any PCIe support
	if machineType != "q35" && machineType != "virt" {
		return nil
	}
	if hypervisorType == virtcontainers.ClhHypervisor {
		if coldPlug != config.NoPort {
			return fmt.Errorf("cold-plug not supported on CLH")
		}
		if hotPlug != config.RootPort {
			return fmt.Errorf("only hot-plug=%s supported on CLH", config.RootPort)
		}
	}

	var port config.PCIePort
	if coldPlug != config.NoPort {
		port = coldPlug
	}
	if hotPlug != config.NoPort {
		port = hotPlug
	}
	if port == config.BridgePort || port == config.RootPort || port == config.SwitchPort {
		return nil
	}
	return fmt.Errorf("invalid vfio_port=%s setting, allowed values %s, %s, %s, %s",
		coldPlug, config.NoPort, config.BridgePort, config.RootPort, config.SwitchPort)
}

// checkNetNsConfig performs sanity checks on disable_new_netns config.
// Because it is an expert option and conflicts with some other common configs.
func checkNetNsConfig(config oci.RuntimeConfig) error {
	if config.DisableNewNetNs {
		if config.InterNetworkModel != vc.NetXConnectNoneModel {
			return fmt.Errorf("config disable_new_netns only works with 'none' internetworking_model")
		}
	}

	return nil
}

// checkFactoryConfig ensures the VM factory configuration is valid.
func checkFactoryConfig(config oci.RuntimeConfig) error {
	if config.FactoryConfig.Template {
		if config.HypervisorConfig.InitrdPath == "" {
			return errors.New("Factory option enable_template requires an initrd image")
		}
	}

	if config.FactoryConfig.VMCacheNumber > 0 {
		if config.HypervisorType != vc.QemuHypervisor {
			return errors.New("VM cache just support qemu")
		}
	}

	return nil
}

// checkHypervisorConfig performs basic "sanity checks" on the hypervisor
// config.
func checkHypervisorConfig(config vc.HypervisorConfig) error {

	if config.RemoteHypervisorSocket != "" {
		return nil
	}

	type image struct {
		path   string
		initrd bool
	}

	images := []image{
		{
			path:   config.ImagePath,
			initrd: false,
		},
		{
			path:   config.InitrdPath,
			initrd: true,
		},
	}

	memSizeMB := int64(config.MemorySize)

	if memSizeMB == 0 {
		return errors.New("VM memory cannot be zero")
	}

	mb := int64(1024 * 1024)

	for _, image := range images {
		if image.path == "" {
			continue
		}

		imageSizeBytes, err := fileSize(image.path)
		if err != nil {
			return err
		}

		if imageSizeBytes == 0 {
			return fmt.Errorf("image %q is empty", image.path)
		}

		if imageSizeBytes > mb {
			imageSizeMB := imageSizeBytes / mb

			msg := fmt.Sprintf("VM memory (%dMB) smaller than image %q size (%dMB)",
				memSizeMB, image.path, imageSizeMB)
			if imageSizeMB >= memSizeMB {
				if image.initrd {
					// Initrd's need to be fully read into memory
					return errors.New(msg)
				}

				// Images do not need to be fully read
				// into memory, but it would be highly
				// unusual to have an image larger
				// than the amount of memory assigned
				// to the VM.
				kataUtilsLogger.Warn(msg)
			}
		}
	}

	return nil
}

// GetDefaultConfigFilePaths returns a list of paths that will be
// considered as configuration files in priority order.
func GetDefaultConfigFilePaths() []string {
	return []string{
		// normally below "/etc"
		DEFAULTSYSCONFRUNTIMECONFIGURATION,

		// normally below "/usr/share"
		defaultRuntimeConfiguration,
	}
}

// getDefaultConfigFile looks in multiple default locations for a
// configuration file and returns the resolved path for the first file
// found, or an error if no config files can be found.
func getDefaultConfigFile() (string, error) {
	var errs []string

	for _, file := range GetDefaultConfigFilePaths() {
		resolved, err := ResolvePath(file)
		if err == nil {
			return resolved, nil
		}
		s := fmt.Sprintf("config file %q unresolvable: %v", file, err)
		errs = append(errs, s)
	}

	return "", errors.New(strings.Join(errs, ", "))
}

// SetConfigOptions will override some of the defaults settings.
func SetConfigOptions(n, runtimeConfig, sysRuntimeConfig string) {
	if n != "" {
		NAME = n
	}

	if runtimeConfig != "" {
		defaultRuntimeConfiguration = runtimeConfig
	}

	if sysRuntimeConfig != "" {
		DEFAULTSYSCONFRUNTIMECONFIGURATION = sysRuntimeConfig
	}
}
