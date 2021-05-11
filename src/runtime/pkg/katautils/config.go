// Copyright (c) 2018-2021 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"errors"
	"fmt"
	"io/ioutil"
	"path/filepath"
	goruntime "runtime"
	"strings"

	"github.com/BurntSushi/toml"
	govmmQemu "github.com/kata-containers/govmm/qemu"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
)

const (
	defaultHypervisor = vc.QemuHypervisor
)

var (
	// if true, enable opentracing support.
	tracing = false
)

// The TOML configuration file contains a number of sections (or
// tables). The names of these tables are in dotted ("nested table")
// form:
//
//   [<component>.<type>]
//
// The components are hypervisor, and agent. For example,
//
//   [agent.kata]
//
// The currently supported types are listed below:
const (
	// supported hypervisor component types
	firecrackerHypervisorTableType = "firecracker"
	clhHypervisorTableType         = "clh"
	qemuHypervisorTableType        = "qemu"
	acrnHypervisorTableType        = "acrn"

	// the maximum amount of PCI bridges that can be cold plugged in a VM
	maxPCIBridges uint32 = 5
)

type tomlConfig struct {
	Hypervisor map[string]hypervisor
	Agent      map[string]agent
	Runtime    runtime
	Factory    factory
	Netmon     netmon
	Image      image
}

type image struct {
	ServiceOffload bool   `toml:"service_offload"`
	Provision      string `toml:"provision"`
}

type factory struct {
	Template        bool   `toml:"enable_template"`
	TemplatePath    string `toml:"template_path"`
	VMCacheNumber   uint   `toml:"vm_cache_number"`
	VMCacheEndpoint string `toml:"vm_cache_endpoint"`
}

type hypervisor struct {
	Path                    string   `toml:"path"`
	JailerPath              string   `toml:"jailer_path"`
	Kernel                  string   `toml:"kernel"`
	CtlPath                 string   `toml:"ctlpath"`
	Initrd                  string   `toml:"initrd"`
	Image                   string   `toml:"image"`
	Firmware                string   `toml:"firmware"`
	MachineAccelerators     string   `toml:"machine_accelerators"`
	CPUFeatures             string   `toml:"cpu_features"`
	KernelParams            string   `toml:"kernel_params"`
	MachineType             string   `toml:"machine_type"`
	BlockDeviceDriver       string   `toml:"block_device_driver"`
	EntropySource           string   `toml:"entropy_source"`
	SharedFS                string   `toml:"shared_fs"`
	VirtioFSDaemon          string   `toml:"virtio_fs_daemon"`
	VirtioFSCache           string   `toml:"virtio_fs_cache"`
	VhostUserStorePath      string   `toml:"vhost_user_store_path"`
	FileBackedMemRootDir    string   `toml:"file_mem_backend"`
	GuestHookPath           string   `toml:"guest_hook_path"`
	GuestMemoryDumpPath     string   `toml:"guest_memory_dump_path"`
	HypervisorPathList      []string `toml:"valid_hypervisor_paths"`
	JailerPathList          []string `toml:"valid_jailer_paths"`
	CtlPathList             []string `toml:"valid_ctlpaths"`
	VirtioFSDaemonList      []string `toml:"valid_virtio_fs_daemon_paths"`
	VirtioFSExtraArgs       []string `toml:"virtio_fs_extra_args"`
	PFlashList              []string `toml:"pflashes"`
	VhostUserStorePathList  []string `toml:"valid_vhost_user_store_paths"`
	FileBackedMemRootList   []string `toml:"valid_file_mem_backends"`
	EntropySourceList       []string `toml:"valid_entropy_sources"`
	EnableAnnotations       []string `toml:"enable_annotations"`
	RxRateLimiterMaxRate    uint64   `toml:"rx_rate_limiter_max_rate"`
	TxRateLimiterMaxRate    uint64   `toml:"tx_rate_limiter_max_rate"`
	VirtioFSCacheSize       uint32   `toml:"virtio_fs_cache_size"`
	NumVCPUs                int32    `toml:"default_vcpus"`
	DefaultMaxVCPUs         uint32   `toml:"default_maxvcpus"`
	MemorySize              uint32   `toml:"default_memory"`
	MemSlots                uint32   `toml:"memory_slots"`
	MemOffset               uint32   `toml:"memory_offset"`
	DefaultBridges          uint32   `toml:"default_bridges"`
	Msize9p                 uint32   `toml:"msize_9p"`
	PCIeRootPort            uint32   `toml:"pcie_root_port"`
	BlockDeviceCacheSet     bool     `toml:"block_device_cache_set"`
	BlockDeviceCacheDirect  bool     `toml:"block_device_cache_direct"`
	BlockDeviceCacheNoflush bool     `toml:"block_device_cache_noflush"`
	EnableVhostUserStore    bool     `toml:"enable_vhost_user_store"`
	DisableBlockDeviceUse   bool     `toml:"disable_block_device_use"`
	MemPrealloc             bool     `toml:"enable_mem_prealloc"`
	HugePages               bool     `toml:"enable_hugepages"`
	VirtioMem               bool     `toml:"enable_virtio_mem"`
	IOMMU                   bool     `toml:"enable_iommu"`
	IOMMUPlatform           bool     `toml:"enable_iommu_platform"`
	Swap                    bool     `toml:"enable_swap"`
	Debug                   bool     `toml:"enable_debug"`
	DisableNestingChecks    bool     `toml:"disable_nesting_checks"`
	EnableIOThreads         bool     `toml:"enable_iothreads"`
	DisableImageNvdimm      bool     `toml:"disable_image_nvdimm"`
	HotplugVFIOOnRootBus    bool     `toml:"hotplug_vfio_on_root_bus"`
	DisableVhostNet         bool     `toml:"disable_vhost_net"`
	GuestMemoryDumpPaging   bool     `toml:"guest_memory_dump_paging"`
	ConfidentialGuest       bool     `toml:"confidential_guest"`
}

type runtime struct {
	InterNetworkModel   string   `toml:"internetworking_model"`
	JaegerEndpoint      string   `toml:"jaeger_endpoint"`
	JaegerUser          string   `toml:"jaeger_user"`
	JaegerPassword      string   `toml:"jaeger_password"`
	SandboxBindMounts   []string `toml:"sandbox_bind_mounts"`
	Experimental        []string `toml:"experimental"`
	Debug               bool     `toml:"enable_debug"`
	Tracing             bool     `toml:"enable_tracing"`
	DisableNewNetNs     bool     `toml:"disable_new_netns"`
	DisableGuestSeccomp bool     `toml:"disable_guest_seccomp"`
	SandboxCgroupOnly   bool     `toml:"sandbox_cgroup_only"`
	EnablePprof         bool     `toml:"enable_pprof"`
}

type agent struct {
	TraceMode           string   `toml:"trace_mode"`
	TraceType           string   `toml:"trace_type"`
	KernelModules       []string `toml:"kernel_modules"`
	Debug               bool     `toml:"enable_debug"`
	Tracing             bool     `toml:"enable_tracing"`
	DebugConsoleEnabled bool     `toml:"debug_console_enabled"`
	DialTimeout         uint32   `toml:"dial_timeout"`
}

type netmon struct {
	Path   string `toml:"path"`
	Debug  bool   `toml:"enable_debug"`
	Enable bool   `toml:"enable_netmon"`
}

func (h hypervisor) path() (string, error) {
	p := h.Path

	if h.Path == "" {
		p = defaultHypervisorPath
	}

	return ResolvePath(p)
}

func (h hypervisor) ctlpath() (string, error) {
	p := h.CtlPath

	if h.CtlPath == "" {
		p = defaultHypervisorCtlPath
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
		return "", errors.New("initrd is not set")
	}

	return ResolvePath(p)
}

func (h hypervisor) image() (string, error) {
	p := h.Image

	if p == "" {
		return "", errors.New("image is not set")
	}

	return ResolvePath(p)
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

func (h hypervisor) GetEntropySource() string {
	if h.EntropySource == "" {
		return defaultEntropySource
	}

	return h.EntropySource
}

func (h hypervisor) defaultVCPUs() uint32 {
	numCPUs := goruntime.NumCPU()

	if h.NumVCPUs < 0 || h.NumVCPUs > int32(numCPUs) {
		return uint32(numCPUs)
	}
	if h.NumVCPUs == 0 { // or unspecified
		return defaultVCPUCount
	}

	return uint32(h.NumVCPUs)
}

func (h hypervisor) defaultMaxVCPUs() uint32 {
	numcpus := uint32(goruntime.NumCPU())
	maxvcpus := vc.MaxQemuVCPUs()
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

func (h hypervisor) defaultMemOffset() uint32 {
	offset := h.MemOffset
	if offset == 0 {
		offset = defaultMemOffset
	}

	return offset
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

func (h hypervisor) sharedFS() (string, error) {
	supportedSharedFS := []string{config.Virtio9P, config.VirtioFS}

	if h.SharedFS == "" {
		return config.Virtio9P, nil
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

func (h hypervisor) getInitrdAndImage() (initrd string, image string, err error) {
	initrd, errInitrd := h.initrd()

	image, errImage := h.image()

	if image != "" && initrd != "" {
		return "", "", errors.New("having both an image and an initrd defined in the configuration file is not supported")
	}

	if errInitrd != nil && errImage != nil {
		return "", "", fmt.Errorf("Either initrd or image must be set to a valid path (initrd: %v) (image: %v)", errInitrd, errImage)
	}

	return
}

func (h hypervisor) getRxRateLimiterCfg() uint64 {
	return h.RxRateLimiterMaxRate
}

func (h hypervisor) getTxRateLimiterCfg() uint64 {
	return h.TxRateLimiterMaxRate
}

func (h hypervisor) getIOMMUPlatform() bool {
	if h.IOMMUPlatform {
		kataUtilsLogger.Info("IOMMUPlatform is enabled by default.")
	} else {
		kataUtilsLogger.Info("IOMMUPlatform is disabled by default.")
	}
	return h.IOMMUPlatform
}

func (a agent) debugConsoleEnabled() bool {
	return a.DebugConsoleEnabled
}

func (a agent) dialTimout() uint32 {
	return a.DialTimeout
}

func (a agent) debug() bool {
	return a.Debug
}

func (a agent) trace() bool {
	return a.Tracing
}

func (a agent) traceMode() string {
	return a.TraceMode
}

func (a agent) traceType() string {
	return a.TraceType
}

func (a agent) kernelModules() []string {
	return a.KernelModules
}

func (n netmon) enable() bool {
	return n.Enable
}

func (n netmon) path() string {
	if n.Path == "" {
		return defaultNetmonPath
	}

	return n.Path
}

func (n netmon) debug() bool {
	return n.Debug
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

	initrd, image, err := h.getInitrdAndImage()
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
		FirmwarePath:          firmware,
		KernelParams:          vc.DeserializeParams(strings.Fields(kernelParams)),
		NumVCPUs:              h.defaultVCPUs(),
		DefaultMaxVCPUs:       h.defaultMaxVCPUs(),
		MemorySize:            h.defaultMemSz(),
		MemSlots:              h.defaultMemSlots(),
		EntropySource:         h.GetEntropySource(),
		EntropySourceList:     h.EntropySourceList,
		DefaultBridges:        h.defaultBridges(),
		DisableBlockDeviceUse: h.DisableBlockDeviceUse,
		HugePages:             h.HugePages,
		Mlock:                 !h.Swap,
		Debug:                 h.Debug,
		DisableNestingChecks:  h.DisableNestingChecks,
		BlockDeviceDriver:     blockDriver,
		EnableIOThreads:       h.EnableIOThreads,
		DisableVhostNet:       true, // vhost-net backend is not supported in Firecracker
		GuestHookPath:         h.guestHookPath(),
		RxRateLimiterMaxRate:  rxRateLimiterMaxRate,
		TxRateLimiterMaxRate:  txRateLimiterMaxRate,
		EnableAnnotations:     h.EnableAnnotations,
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

	initrd, image, err := h.getInitrdAndImage()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	pflashes, err := h.PFlash()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if image != "" && initrd != "" {
		return vc.HypervisorConfig{},
			errors.New("having both an image and an initrd defined in the configuration file is not supported")
	}

	if image == "" && initrd == "" {
		return vc.HypervisorConfig{},
			errors.New("either image or initrd must be defined in the configuration file")
	}

	firmware, err := h.firmware()
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

	blockDriver, err := h.blockDeviceDriver()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	sharedFS, err := h.sharedFS()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if sharedFS == config.VirtioFS && h.VirtioFSDaemon == "" {
		return vc.HypervisorConfig{},
			errors.New("cannot enable virtio-fs without daemon path in configuration file")
	}

	if vSock, err := utils.SupportsVsocks(); !vSock {
		return vc.HypervisorConfig{}, err
	}

	rxRateLimiterMaxRate := h.getRxRateLimiterCfg()
	txRateLimiterMaxRate := h.getTxRateLimiterCfg()

	return vc.HypervisorConfig{
		HypervisorPath:          hypervisor,
		HypervisorPathList:      h.HypervisorPathList,
		KernelPath:              kernel,
		InitrdPath:              initrd,
		ImagePath:               image,
		FirmwarePath:            firmware,
		PFlash:                  pflashes,
		MachineAccelerators:     machineAccelerators,
		CPUFeatures:             cpuFeatures,
		KernelParams:            vc.DeserializeParams(strings.Fields(kernelParams)),
		HypervisorMachineType:   machineType,
		NumVCPUs:                h.defaultVCPUs(),
		DefaultMaxVCPUs:         h.defaultMaxVCPUs(),
		MemorySize:              h.defaultMemSz(),
		MemSlots:                h.defaultMemSlots(),
		MemOffset:               h.defaultMemOffset(),
		VirtioMem:               h.VirtioMem,
		EntropySource:           h.GetEntropySource(),
		EntropySourceList:       h.EntropySourceList,
		DefaultBridges:          h.defaultBridges(),
		DisableBlockDeviceUse:   h.DisableBlockDeviceUse,
		SharedFS:                sharedFS,
		VirtioFSDaemon:          h.VirtioFSDaemon,
		VirtioFSDaemonList:      h.VirtioFSDaemonList,
		VirtioFSCacheSize:       h.VirtioFSCacheSize,
		VirtioFSCache:           h.defaultVirtioFSCache(),
		VirtioFSExtraArgs:       h.VirtioFSExtraArgs,
		MemPrealloc:             h.MemPrealloc,
		HugePages:               h.HugePages,
		IOMMU:                   h.IOMMU,
		IOMMUPlatform:           h.getIOMMUPlatform(),
		FileBackedMemRootDir:    h.FileBackedMemRootDir,
		FileBackedMemRootList:   h.FileBackedMemRootList,
		Mlock:                   !h.Swap,
		Debug:                   h.Debug,
		DisableNestingChecks:    h.DisableNestingChecks,
		BlockDeviceDriver:       blockDriver,
		BlockDeviceCacheSet:     h.BlockDeviceCacheSet,
		BlockDeviceCacheDirect:  h.BlockDeviceCacheDirect,
		BlockDeviceCacheNoflush: h.BlockDeviceCacheNoflush,
		EnableIOThreads:         h.EnableIOThreads,
		Msize9p:                 h.msize9p(),
		DisableImageNvdimm:      h.DisableImageNvdimm,
		HotplugVFIOOnRootBus:    h.HotplugVFIOOnRootBus,
		PCIeRootPort:            h.PCIeRootPort,
		DisableVhostNet:         h.DisableVhostNet,
		EnableVhostUserStore:    h.EnableVhostUserStore,
		VhostUserStorePath:      h.vhostUserStorePath(),
		VhostUserStorePathList:  h.VhostUserStorePathList,
		GuestHookPath:           h.guestHookPath(),
		RxRateLimiterMaxRate:    rxRateLimiterMaxRate,
		TxRateLimiterMaxRate:    txRateLimiterMaxRate,
		EnableAnnotations:       h.EnableAnnotations,
		GuestMemoryDumpPath:     h.GuestMemoryDumpPath,
		GuestMemoryDumpPaging:   h.GuestMemoryDumpPaging,
		ConfidentialGuest:       h.ConfidentialGuest,
	}, nil
}

func newAcrnHypervisorConfig(h hypervisor) (vc.HypervisorConfig, error) {
	hypervisor, err := h.path()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	hypervisorctl, err := h.ctlpath()
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

	if image == "" {
		return vc.HypervisorConfig{},
			errors.New("image must be defined in the configuration file")
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

	return vc.HypervisorConfig{
		HypervisorPath:        hypervisor,
		HypervisorPathList:    h.HypervisorPathList,
		KernelPath:            kernel,
		ImagePath:             image,
		HypervisorCtlPath:     hypervisorctl,
		HypervisorCtlPathList: h.CtlPathList,
		FirmwarePath:          firmware,
		KernelParams:          vc.DeserializeParams(strings.Fields(kernelParams)),
		NumVCPUs:              h.defaultVCPUs(),
		DefaultMaxVCPUs:       h.defaultMaxVCPUs(),
		MemorySize:            h.defaultMemSz(),
		MemSlots:              h.defaultMemSlots(),
		EntropySource:         h.GetEntropySource(),
		EntropySourceList:     h.EntropySourceList,
		DefaultBridges:        h.defaultBridges(),
		HugePages:             h.HugePages,
		Mlock:                 !h.Swap,
		Debug:                 h.Debug,
		DisableNestingChecks:  h.DisableNestingChecks,
		BlockDeviceDriver:     blockDriver,
		DisableVhostNet:       h.DisableVhostNet,
		GuestHookPath:         h.guestHookPath(),
		EnableAnnotations:     h.EnableAnnotations,
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

	initrd, image, err := h.getInitrdAndImage()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	if initrd != "" {
		return vc.HypervisorConfig{},
			errors.New("having an initrd defined in the configuration file is not supported")
	}

	if image == "" {
		return vc.HypervisorConfig{},
			errors.New("image must be defined in the configuration file")
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

	sharedFS := config.VirtioFS

	if h.VirtioFSDaemon == "" {
		return vc.HypervisorConfig{},
			errors.New("virtio-fs daemon path is missing in configuration file")
	}

	return vc.HypervisorConfig{
		HypervisorPath:          hypervisor,
		HypervisorPathList:      h.HypervisorPathList,
		KernelPath:              kernel,
		InitrdPath:              initrd,
		ImagePath:               image,
		FirmwarePath:            firmware,
		MachineAccelerators:     machineAccelerators,
		KernelParams:            vc.DeserializeParams(strings.Fields(kernelParams)),
		HypervisorMachineType:   machineType,
		NumVCPUs:                h.defaultVCPUs(),
		DefaultMaxVCPUs:         h.defaultMaxVCPUs(),
		MemorySize:              h.defaultMemSz(),
		MemSlots:                h.defaultMemSlots(),
		MemOffset:               h.defaultMemOffset(),
		VirtioMem:               h.VirtioMem,
		EntropySource:           h.GetEntropySource(),
		EntropySourceList:       h.EntropySourceList,
		DefaultBridges:          h.defaultBridges(),
		DisableBlockDeviceUse:   h.DisableBlockDeviceUse,
		SharedFS:                sharedFS,
		VirtioFSDaemon:          h.VirtioFSDaemon,
		VirtioFSDaemonList:      h.VirtioFSDaemonList,
		VirtioFSCacheSize:       h.VirtioFSCacheSize,
		VirtioFSCache:           h.VirtioFSCache,
		MemPrealloc:             h.MemPrealloc,
		HugePages:               h.HugePages,
		FileBackedMemRootDir:    h.FileBackedMemRootDir,
		FileBackedMemRootList:   h.FileBackedMemRootList,
		Mlock:                   !h.Swap,
		Debug:                   h.Debug,
		DisableNestingChecks:    h.DisableNestingChecks,
		BlockDeviceDriver:       blockDriver,
		BlockDeviceCacheSet:     h.BlockDeviceCacheSet,
		BlockDeviceCacheDirect:  h.BlockDeviceCacheDirect,
		BlockDeviceCacheNoflush: h.BlockDeviceCacheNoflush,
		EnableIOThreads:         h.EnableIOThreads,
		Msize9p:                 h.msize9p(),
		HotplugVFIOOnRootBus:    h.HotplugVFIOOnRootBus,
		PCIeRootPort:            h.PCIeRootPort,
		DisableVhostNet:         true,
		GuestHookPath:           h.guestHookPath(),
		VirtioFSExtraArgs:       h.VirtioFSExtraArgs,
		SGXEPCSize:              defaultSGXEPCSize,
		EnableAnnotations:       h.EnableAnnotations,
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
		case acrnHypervisorTableType:
			config.HypervisorType = vc.AcrnHypervisor
			hConfig, err = newAcrnHypervisorConfig(hypervisor)
		case clhHypervisorTableType:
			config.HypervisorType = vc.ClhHypervisor
			hConfig, err = newClhHypervisorConfig(hypervisor)
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
			TraceMode:          agent.traceMode(),
			TraceType:          agent.traceType(),
			KernelModules:      agent.kernelModules(),
			EnableDebugConsole: agent.debugConsoleEnabled(),
			DialTimeout:        agent.dialTimout(),
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
	err := vc.KataAgentSetDefaultTraceConfigOptions(&runtimeConfig.AgentConfig)
	if err != nil {
		return err
	}

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

	config.NetmonConfig = vc.NetmonConfig{
		Path:   tomlConf.Netmon.path(),
		Debug:  tomlConf.Netmon.debug(),
		Enable: tomlConf.Netmon.enable(),
	}

	err = SetKernelParams(config)
	if err != nil {
		return err
	}

	return nil
}

func GetDefaultHypervisorConfig() vc.HypervisorConfig {
	return vc.HypervisorConfig{
		HypervisorPath:          defaultHypervisorPath,
		JailerPath:              defaultJailerPath,
		KernelPath:              defaultKernelPath,
		ImagePath:               defaultImagePath,
		InitrdPath:              defaultInitrdPath,
		FirmwarePath:            defaultFirmwarePath,
		MachineAccelerators:     defaultMachineAccelerators,
		CPUFeatures:             defaultCPUFeatures,
		HypervisorMachineType:   defaultMachineType,
		NumVCPUs:                defaultVCPUCount,
		DefaultMaxVCPUs:         defaultMaxVCPUCount,
		MemorySize:              defaultMemSize,
		MemOffset:               defaultMemOffset,
		VirtioMem:               defaultVirtioMem,
		DisableBlockDeviceUse:   defaultDisableBlockDeviceUse,
		DefaultBridges:          defaultBridgesCount,
		MemPrealloc:             defaultEnableMemPrealloc,
		HugePages:               defaultEnableHugePages,
		IOMMU:                   defaultEnableIOMMU,
		IOMMUPlatform:           defaultEnableIOMMUPlatform,
		FileBackedMemRootDir:    defaultFileBackedMemRootDir,
		Mlock:                   !defaultEnableSwap,
		Debug:                   defaultEnableDebug,
		DisableNestingChecks:    defaultDisableNestingChecks,
		BlockDeviceDriver:       defaultBlockDeviceDriver,
		BlockDeviceCacheSet:     defaultBlockDeviceCacheSet,
		BlockDeviceCacheDirect:  defaultBlockDeviceCacheDirect,
		BlockDeviceCacheNoflush: defaultBlockDeviceCacheNoflush,
		EnableIOThreads:         defaultEnableIOThreads,
		Msize9p:                 defaultMsize9p,
		HotplugVFIOOnRootBus:    defaultHotplugVFIOOnRootBus,
		PCIeRootPort:            defaultPCIeRootPort,
		GuestHookPath:           defaultGuestHookPath,
		VhostUserStorePath:      defaultVhostUserStorePath,
		VirtioFSCache:           defaultVirtioFSCacheMode,
		DisableImageNvdimm:      defaultDisableImageNvdimm,
		RxRateLimiterMaxRate:    defaultRxRateLimiterMaxRate,
		TxRateLimiterMaxRate:    defaultTxRateLimiterMaxRate,
		SGXEPCSize:              defaultSGXEPCSize,
		ConfidentialGuest:       defaultConfidentialGuest,
	}
}

func initConfig() (config oci.RuntimeConfig, err error) {
	err = config.InterNetworkModel.SetModel(defaultInterNetworkingModel)
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
	tracing = config.Trace

	if tomlConf.Runtime.InterNetworkModel != "" {
		err = config.InterNetworkModel.SetModel(tomlConf.Runtime.InterNetworkModel)
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

	config.SandboxCgroupOnly = tomlConf.Runtime.SandboxCgroupOnly
	config.DisableNewNetNs = tomlConf.Runtime.DisableNewNetNs
	config.EnablePprof = tomlConf.Runtime.EnablePprof
	config.JaegerEndpoint = tomlConf.Runtime.JaegerEndpoint
	config.JaegerUser = tomlConf.Runtime.JaegerUser
	config.JaegerPassword = tomlConf.Runtime.JaegerPassword
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

	configData, err := ioutil.ReadFile(resolved)
	if err != nil {
		return tomlConf, resolved, err
	}

	_, err = toml.Decode(string(configData), &tomlConf)
	if err != nil {
		return tomlConf, resolved, err
	}

	return tomlConf, resolved, nil
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

	return nil
}

// checkNetNsConfig performs sanity checks on disable_new_netns config.
// Because it is an expert option and conflicts with some other common configs.
func checkNetNsConfig(config oci.RuntimeConfig) error {
	if config.DisableNewNetNs {
		if config.NetmonConfig.Enable {
			return fmt.Errorf("config disable_new_netns conflicts with enable_netmon")
		}
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
		name = n
	}

	if runtimeConfig != "" {
		defaultRuntimeConfiguration = runtimeConfig
	}

	if sysRuntimeConfig != "" {
		defaultSysConfRuntimeConfiguration = sysRuntimeConfig
	}
}
