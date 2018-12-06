// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"errors"
	"fmt"
	"io/ioutil"
	goruntime "runtime"
	"strings"

	"github.com/BurntSushi/toml"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	"github.com/kata-containers/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
)

const (
	defaultHypervisor = vc.QemuHypervisor
	defaultAgent      = vc.KataContainersAgent
)

var (
	defaultProxy = vc.KataProxyType
	defaultShim  = vc.KataShimType

	// if true, enable opentracing support.
	tracing = false
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
	Factory    factory
	Netmon     netmon
}

type factory struct {
	Template bool `toml:"enable_template"`
}

type hypervisor struct {
	Path                    string `toml:"path"`
	Kernel                  string `toml:"kernel"`
	Initrd                  string `toml:"initrd"`
	Image                   string `toml:"image"`
	Firmware                string `toml:"firmware"`
	MachineAccelerators     string `toml:"machine_accelerators"`
	KernelParams            string `toml:"kernel_params"`
	MachineType             string `toml:"machine_type"`
	BlockDeviceDriver       string `toml:"block_device_driver"`
	EntropySource           string `toml:"entropy_source"`
	BlockDeviceCacheSet     bool   `toml:"block_device_cache_set"`
	BlockDeviceCacheDirect  bool   `toml:"block_device_cache_direct"`
	BlockDeviceCacheNoflush bool   `toml:"block_device_cache_noflush"`
	NumVCPUs                int32  `toml:"default_vcpus"`
	DefaultMaxVCPUs         uint32 `toml:"default_maxvcpus"`
	MemorySize              uint32 `toml:"default_memory"`
	MemSlots                uint32 `toml:"memory_slots"`
	DefaultBridges          uint32 `toml:"default_bridges"`
	Msize9p                 uint32 `toml:"msize_9p"`
	DisableBlockDeviceUse   bool   `toml:"disable_block_device_use"`
	MemPrealloc             bool   `toml:"enable_mem_prealloc"`
	HugePages               bool   `toml:"enable_hugepages"`
	Swap                    bool   `toml:"enable_swap"`
	Debug                   bool   `toml:"enable_debug"`
	DisableNestingChecks    bool   `toml:"disable_nesting_checks"`
	EnableIOThreads         bool   `toml:"enable_iothreads"`
	UseVSock                bool   `toml:"use_vsock"`
	HotplugVFIOOnRootBus    bool   `toml:"hotplug_vfio_on_root_bus"`
	DisableVhostNet         bool   `toml:"disable_vhost_net"`
	GuestHookPath           string `toml:"guest_hook_path"`
}

type proxy struct {
	Path  string `toml:"path"`
	Debug bool   `toml:"enable_debug"`
}

type runtime struct {
	Debug             bool   `toml:"enable_debug"`
	Tracing           bool   `toml:"enable_tracing"`
	DisableNewNetNs   bool   `toml:"disable_new_netns"`
	InterNetworkModel string `toml:"internetworking_model"`
}

type shim struct {
	Path    string `toml:"path"`
	Debug   bool   `toml:"enable_debug"`
	Tracing bool   `toml:"enable_tracing"`
}

type agent struct {
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
	if h.MemorySize < 8 {
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
	supportedBlockDrivers := []string{config.VirtioSCSI, config.VirtioBlock, config.VirtioMmio}

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

func (h hypervisor) msize9p() uint32 {
	if h.Msize9p == 0 {
		return defaultMsize9p
	}

	return h.Msize9p
}

func (h hypervisor) useVSock() bool {
	return h.UseVSock
}

func (h hypervisor) guestHookPath() string {
	if h.GuestHookPath == "" {
		return defaultGuestHookPath
	}
	return h.GuestHookPath
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

	return ResolvePath(p)
}

func (s shim) debug() bool {
	return s.Debug
}

func (s shim) trace() bool {
	return s.Tracing
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
	kernelParams := h.kernelParams()
	machineType := h.machineType()

	blockDriver, err := h.blockDeviceDriver()
	if err != nil {
		return vc.HypervisorConfig{}, err
	}

	useVSock := false
	if h.useVSock() {
		if utils.SupportsVsocks() {
			kataUtilsLogger.Info("vsock supported")
			useVSock = true
		} else {
			kataUtilsLogger.Warn("No vsock support, falling back to legacy serial port")
		}
	}

	return vc.HypervisorConfig{
		HypervisorPath:          hypervisor,
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
		EntropySource:           h.GetEntropySource(),
		DefaultBridges:          h.defaultBridges(),
		DisableBlockDeviceUse:   h.DisableBlockDeviceUse,
		MemPrealloc:             h.MemPrealloc,
		HugePages:               h.HugePages,
		Mlock:                   !h.Swap,
		Debug:                   h.Debug,
		DisableNestingChecks:    h.DisableNestingChecks,
		BlockDeviceDriver:       blockDriver,
		BlockDeviceCacheSet:     h.BlockDeviceCacheSet,
		BlockDeviceCacheDirect:  h.BlockDeviceCacheDirect,
		BlockDeviceCacheNoflush: h.BlockDeviceCacheNoflush,
		EnableIOThreads:         h.EnableIOThreads,
		Msize9p:                 h.msize9p(),
		UseVSock:                useVSock,
		HotplugVFIOOnRootBus:    h.HotplugVFIOOnRootBus,
		DisableVhostNet:         h.DisableVhostNet,
		GuestHookPath:           h.guestHookPath(),
	}, nil
}

func newFactoryConfig(f factory) (oci.FactoryConfig, error) {
	return oci.FactoryConfig{Template: f.Template}, nil
}

func newShimConfig(s shim) (vc.ShimConfig, error) {
	path, err := s.path()
	if err != nil {
		return vc.ShimConfig{}, err
	}

	return vc.ShimConfig{
		Path:  path,
		Debug: s.debug(),
		Trace: s.trace(),
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
			config.AgentConfig = vc.KataAgentConfig{
				UseVSock: config.HypervisorConfig.UseVSock,
			}
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

	return nil
}

func initConfig() (config oci.RuntimeConfig, err error) {
	var defaultAgentConfig interface{}

	defaultHypervisorConfig := vc.HypervisorConfig{
		HypervisorPath:          defaultHypervisorPath,
		KernelPath:              defaultKernelPath,
		ImagePath:               defaultImagePath,
		InitrdPath:              defaultInitrdPath,
		FirmwarePath:            defaultFirmwarePath,
		MachineAccelerators:     defaultMachineAccelerators,
		HypervisorMachineType:   defaultMachineType,
		NumVCPUs:                defaultVCPUCount,
		DefaultMaxVCPUs:         defaultMaxVCPUCount,
		MemorySize:              defaultMemSize,
		DefaultBridges:          defaultBridgesCount,
		MemPrealloc:             defaultEnableMemPrealloc,
		HugePages:               defaultEnableHugePages,
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
		GuestHookPath:           defaultGuestHookPath,
	}

	err = config.InterNetworkModel.SetModel(defaultInterNetworkingModel)
	if err != nil {
		return oci.RuntimeConfig{}, err
	}

	defaultAgentConfig = vc.HyperConfig{}

	config = oci.RuntimeConfig{
		HypervisorType:   defaultHypervisor,
		HypervisorConfig: defaultHypervisorConfig,
		AgentType:        defaultAgent,
		AgentConfig:      defaultAgentConfig,
		ProxyType:        defaultProxy,
		ShimType:         defaultShim,
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
func LoadConfiguration(configPath string, ignoreLogging, builtIn bool) (resolvedConfigPath string, config oci.RuntimeConfig, err error) {
	var resolved string

	config, err = initConfig()
	if err != nil {
		return "", oci.RuntimeConfig{}, err
	}

	if configPath == "" {
		resolved, err = getDefaultConfigFile()
	} else {
		resolved, err = ResolvePath(configPath)
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

	if err := updateConfig(resolved, tomlConf, &config, builtIn); err != nil {
		return "", config, err
	}

	// use no proxy if HypervisorConfig.UseVSock is true
	if config.HypervisorConfig.UseVSock {
		kataUtilsLogger.Info("VSOCK supported, configure to not use proxy")
		config.ProxyType = vc.NoProxyType
		config.ProxyConfig = vc.ProxyConfig{}
	}

	config.DisableNewNetNs = tomlConf.Runtime.DisableNewNetNs

	if err := checkConfig(config); err != nil {
		return "", config, err
	}

	return resolved, config, nil
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

func updateConfig(configPath string, tomlConf tomlConfig, config *oci.RuntimeConfig, builtIn bool) error {

	if err := updateRuntimeConfig(configPath, tomlConf, config); err != nil {
		return err
	}

	if builtIn {
		config.ProxyType = vc.KataBuiltInProxyType
		config.ShimType = vc.KataBuiltInShimType
		config.AgentType = vc.KataContainersAgent
		config.AgentConfig = vc.KataAgentConfig{
			LongLiveConn: true,
			UseVSock:     config.HypervisorConfig.UseVSock,
		}
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
	} else if config.ShimConfig.(vc.ShimConfig).Trace {
		// Normally, the shim runs in a separate network namespace.
		// But when tracing, the shim process needs to be able to talk
		// to the Jaeger agent running in the host network namespace.
		return errors.New("Shim tracing requires disable_new_netns for Jaeger agent communication")
	}

	return nil
}

// checkFactoryConfig ensures the VM factory configuration is valid.
func checkFactoryConfig(config oci.RuntimeConfig) error {
	if config.FactoryConfig.Template && config.HypervisorConfig.InitrdPath == "" {
		return errors.New("Factory option enable_template requires an initrd image")
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
