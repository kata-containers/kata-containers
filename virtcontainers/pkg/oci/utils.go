// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package oci

import (
	"context"
	"errors"
	"fmt"
	"path/filepath"
	goruntime "runtime"
	"strconv"
	"strings"
	"syscall"

	criContainerdAnnotations "github.com/containerd/cri-containerd/pkg/annotations"
	crioAnnotations "github.com/cri-o/cri-o/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	exp "github.com/kata-containers/runtime/virtcontainers/experimental"
	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	dockershimAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations/dockershim"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

type annotationContainerType struct {
	annotation    string
	containerType vc.ContainerType
}

var (
	// ErrNoLinux is an error for missing Linux sections in the OCI configuration file.
	ErrNoLinux = errors.New("missing Linux section")

	// CRIContainerTypeKeyList lists all the CRI keys that could define
	// the container type from annotations in the config.json.
	CRIContainerTypeKeyList = []string{criContainerdAnnotations.ContainerType, crioAnnotations.ContainerType, dockershimAnnotations.ContainerTypeLabelKey}

	// CRISandboxNameKeyList lists all the CRI keys that could define
	// the sandbox ID (sandbox ID) from annotations in the config.json.
	CRISandboxNameKeyList = []string{criContainerdAnnotations.SandboxID, crioAnnotations.SandboxID, dockershimAnnotations.SandboxIDLabelKey}

	// CRIContainerTypeList lists all the maps from CRI ContainerTypes annotations
	// to a virtcontainers ContainerType.
	CRIContainerTypeList = []annotationContainerType{
		{crioAnnotations.ContainerTypeSandbox, vc.PodSandbox},
		{crioAnnotations.ContainerTypeContainer, vc.PodContainer},
		{criContainerdAnnotations.ContainerTypeSandbox, vc.PodSandbox},
		{criContainerdAnnotations.ContainerTypeContainer, vc.PodContainer},
		{dockershimAnnotations.ContainerTypeLabelSandbox, vc.PodSandbox},
		{dockershimAnnotations.ContainerTypeLabelContainer, vc.PodContainer},
	}
)

const (
	// StateCreated represents a container that has been created and is
	// ready to be run.
	StateCreated = "created"

	// StateRunning represents a container that's currently running.
	StateRunning = "running"

	// StateStopped represents a container that has been stopped.
	StateStopped = "stopped"

	// StatePaused represents a container that has been paused.
	StatePaused = "paused"
)

const KernelModulesSeparator = ";"

// FactoryConfig is a structure to set the VM factory configuration.
type FactoryConfig struct {
	// Template enables VM templating support in VM factory.
	Template bool

	// TemplatePath specifies the path of template.
	TemplatePath string

	// VMCacheNumber specifies the the number of caches of VMCache.
	VMCacheNumber uint

	// VMCacheEndpoint specifies the endpoint of transport VM from the VM cache server to runtime.
	VMCacheEndpoint string
}

// RuntimeConfig aggregates all runtime specific settings
type RuntimeConfig struct {
	HypervisorType   vc.HypervisorType
	HypervisorConfig vc.HypervisorConfig

	NetmonConfig vc.NetmonConfig

	AgentType   vc.AgentType
	AgentConfig interface{}

	ProxyType   vc.ProxyType
	ProxyConfig vc.ProxyConfig

	ShimType   vc.ShimType
	ShimConfig interface{}

	Console string

	//Determines how the VM should be connected to the
	//the container network interface
	InterNetworkModel vc.NetInterworkingModel
	FactoryConfig     FactoryConfig
	Debug             bool
	Trace             bool

	//Determines if seccomp should be applied inside guest
	DisableGuestSeccomp bool

	//Determines if create a netns for hypervisor process
	DisableNewNetNs bool

	//Determines kata processes are managed only in sandbox cgroup
	SandboxCgroupOnly bool

	//Experimental features enabled
	Experimental []exp.Feature
}

// AddKernelParam allows the addition of new kernel parameters to an existing
// hypervisor configuration stored inside the current runtime configuration.
func (config *RuntimeConfig) AddKernelParam(p vc.Param) error {
	return config.HypervisorConfig.AddKernelParam(p)
}

var ociLog = logrus.WithFields(logrus.Fields{
	"source":    "virtcontainers",
	"subsystem": "oci",
})

// SetLogger sets the logger for oci package.
func SetLogger(ctx context.Context, logger *logrus.Entry) {
	fields := ociLog.Data
	ociLog = logger.WithFields(fields)
}

func cmdEnvs(spec specs.Spec, envs []types.EnvVar) []types.EnvVar {
	for _, env := range spec.Process.Env {
		kv := strings.Split(env, "=")
		if len(kv) < 2 {
			continue
		}

		envs = append(envs,
			types.EnvVar{
				Var:   kv[0],
				Value: kv[1],
			})
	}

	return envs
}

func newMount(m specs.Mount) vc.Mount {
	return vc.Mount{
		Source:      m.Source,
		Destination: m.Destination,
		Type:        m.Type,
		Options:     m.Options,
	}
}

func containerMounts(spec specs.Spec) []vc.Mount {
	ociMounts := spec.Mounts

	if ociMounts == nil {
		return []vc.Mount{}
	}

	var mnts []vc.Mount
	for _, m := range ociMounts {
		mnts = append(mnts, newMount(m))
	}

	return mnts
}

func contains(s []string, e string) bool {
	for _, a := range s {
		if a == e {
			return true
		}
	}
	return false
}

func newLinuxDeviceInfo(d specs.LinuxDevice) (*config.DeviceInfo, error) {
	allowedDeviceTypes := []string{"c", "b", "u", "p"}

	if !contains(allowedDeviceTypes, d.Type) {
		return nil, fmt.Errorf("Unexpected Device Type %s for device %s", d.Type, d.Path)
	}

	if d.Path == "" {
		return nil, fmt.Errorf("Path cannot be empty for device")
	}

	deviceInfo := config.DeviceInfo{
		ContainerPath: d.Path,
		DevType:       d.Type,
		Major:         d.Major,
		Minor:         d.Minor,
	}
	if d.UID != nil {
		deviceInfo.UID = *d.UID
	}

	if d.GID != nil {
		deviceInfo.GID = *d.GID
	}

	if d.FileMode != nil {
		deviceInfo.FileMode = *d.FileMode
	}

	return &deviceInfo, nil
}

func containerDeviceInfos(spec specs.Spec) ([]config.DeviceInfo, error) {
	ociLinuxDevices := spec.Linux.Devices

	if ociLinuxDevices == nil {
		return []config.DeviceInfo{}, nil
	}

	var devices []config.DeviceInfo
	for _, d := range ociLinuxDevices {
		linuxDeviceInfo, err := newLinuxDeviceInfo(d)
		if err != nil {
			return []config.DeviceInfo{}, err
		}

		devices = append(devices, *linuxDeviceInfo)
	}

	return devices, nil
}

func networkConfig(ocispec specs.Spec, config RuntimeConfig) (vc.NetworkConfig, error) {
	linux := ocispec.Linux
	if linux == nil {
		return vc.NetworkConfig{}, ErrNoLinux
	}

	var netConf vc.NetworkConfig

	for _, n := range linux.Namespaces {
		if n.Type != specs.NetworkNamespace {
			continue
		}

		if n.Path != "" {
			netConf.NetNSPath = n.Path
		}
	}
	netConf.InterworkingModel = config.InterNetworkModel
	netConf.DisableNewNetNs = config.DisableNewNetNs

	netConf.NetmonConfig = vc.NetmonConfig{
		Path:   config.NetmonConfig.Path,
		Debug:  config.NetmonConfig.Debug,
		Enable: config.NetmonConfig.Enable,
	}

	return netConf, nil
}

// GetContainerType determines which type of container matches the annotations
// table provided.
func GetContainerType(annotations map[string]string) (vc.ContainerType, error) {
	if containerType, ok := annotations[vcAnnotations.ContainerTypeKey]; ok {
		return vc.ContainerType(containerType), nil
	}

	ociLog.Errorf("Annotations[%s] not found, cannot determine the container type",
		vcAnnotations.ContainerTypeKey)
	return vc.UnknownContainerType, fmt.Errorf("Could not find container type")
}

// ContainerType returns the type of container and if the container type was
// found from CRI servers annotations.
func ContainerType(spec specs.Spec) (vc.ContainerType, error) {
	for _, key := range CRIContainerTypeKeyList {
		containerTypeVal, ok := spec.Annotations[key]
		if !ok {
			continue
		}

		for _, t := range CRIContainerTypeList {
			if t.annotation == containerTypeVal {
				return t.containerType, nil
			}

		}

		return vc.UnknownContainerType, fmt.Errorf("Unknown container type %s", containerTypeVal)
	}

	return vc.PodSandbox, nil
}

func GetSandboxConfigPath(annotations map[string]string) string {
	return annotations[vcAnnotations.SandboxConfigPathKey]
}

// SandboxID determines the sandbox ID related to an OCI configuration. This function
// is expected to be called only when the container type is "PodContainer".
func SandboxID(spec specs.Spec) (string, error) {
	for _, key := range CRISandboxNameKeyList {
		sandboxID, ok := spec.Annotations[key]
		if ok {
			return sandboxID, nil
		}
	}

	return "", fmt.Errorf("Could not find sandbox ID")
}

func addAnnotations(ocispec specs.Spec, config *vc.SandboxConfig) error {
	addAssetAnnotations(ocispec, config)
	if err := addHypervisorConfigOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addRuntimeConfigOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addAgentConfigOverrides(ocispec, config); err != nil {
		return err
	}
	return nil
}

func addAssetAnnotations(ocispec specs.Spec, config *vc.SandboxConfig) {
	assetAnnotations := []string{
		vcAnnotations.KernelPath,
		vcAnnotations.ImagePath,
		vcAnnotations.InitrdPath,
		vcAnnotations.FirmwarePath,
		vcAnnotations.KernelHash,
		vcAnnotations.ImageHash,
		vcAnnotations.InitrdHash,
		vcAnnotations.FirmwareHash,
		vcAnnotations.AssetHashType,
	}

	for _, a := range assetAnnotations {
		value, ok := ocispec.Annotations[a]
		if !ok {
			continue
		}

		config.Annotations[a] = value
	}
}

func addHypervisorConfigOverrides(ocispec specs.Spec, config *vc.SandboxConfig) error {
	if err := addHypervisorCPUOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addHypervisorMemoryOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addHypervisorBlockOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addHypervisporVirtioFsOverrides(ocispec, config); err != nil {
		return err
	}

	if value, ok := ocispec.Annotations[vcAnnotations.KernelParams]; ok {
		if value != "" {
			params := vc.DeserializeParams(strings.Fields(value))
			for _, param := range params {
				if err := config.HypervisorConfig.AddKernelParam(param); err != nil {
					return fmt.Errorf("Error adding kernel parameters in annotation kernel_params : %v", err)
				}
			}
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.MachineType]; ok {
		if value != "" {
			config.HypervisorConfig.HypervisorMachineType = value
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.MachineAccelerators]; ok {
		if value != "" {
			config.HypervisorConfig.MachineAccelerators = value
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.DisableVhostNet]; ok {
		disableVhostNet, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for disable_vhost_net: Please specify boolean value 'true|false'")
		}

		config.HypervisorConfig.DisableVhostNet = disableVhostNet
	}

	if value, ok := ocispec.Annotations[vcAnnotations.GuestHookPath]; ok {
		if value != "" {
			config.HypervisorConfig.GuestHookPath = value
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.UseVSock]; ok {
		useVsock, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for use_vsock: Please specify boolean value 'true|false'")
		}

		config.HypervisorConfig.UseVSock = useVsock
	}

	if value, ok := ocispec.Annotations[vcAnnotations.DisableImageNvdimm]; ok {
		disableNvdimm, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for use_nvdimm: Please specify boolean value 'true|false'")
		}

		config.HypervisorConfig.DisableImageNvdimm = disableNvdimm
	}

	if value, ok := ocispec.Annotations[vcAnnotations.HotplugVFIOOnRootBus]; ok {
		hotplugVFIOOnRootBus, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for hotplug_vfio_on_root_bus: Please specify boolean value 'true|false'")
		}

		config.HypervisorConfig.HotplugVFIOOnRootBus = hotplugVFIOOnRootBus
	}

	if value, ok := ocispec.Annotations[vcAnnotations.EntropySource]; ok {
		if value != "" {
			config.HypervisorConfig.EntropySource = value
		}
	}

	return nil
}

func addHypervisorMemoryOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	if value, ok := ocispec.Annotations[vcAnnotations.DefaultMemory]; ok {
		memorySz, err := strconv.ParseUint(value, 10, 32)
		if err != nil {
			return fmt.Errorf("Error encountered parsing annotation for default_memory: %v, please specify positive numeric value greater than 8", err)
		}

		if memorySz < vc.MinHypervisorMemory {
			return fmt.Errorf("Memory specified in annotation %s is less than minimum required %d, please specify a larger value", vcAnnotations.DefaultMemory, vc.MinHypervisorMemory)
		}

		sbConfig.HypervisorConfig.MemorySize = uint32(memorySz)
	}

	if value, ok := ocispec.Annotations[vcAnnotations.MemSlots]; ok {
		mslots, err := strconv.ParseUint(value, 10, 32)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for memory_slots: %v, please specify positive numeric value", err)
		}

		if mslots > 0 {
			sbConfig.HypervisorConfig.MemSlots = uint32(mslots)
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.MemOffset]; ok {
		moffset, err := strconv.ParseUint(value, 10, 32)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for memory_offset: %v, please specify positive numeric value", err)
		}

		if moffset > 0 {
			sbConfig.HypervisorConfig.MemOffset = uint32(moffset)
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.VirtioMem]; ok {
		virtioMem, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for enable_virtio_mem: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.VirtioMem = virtioMem
	}

	if value, ok := ocispec.Annotations[vcAnnotations.MemPrealloc]; ok {
		memPrealloc, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for enable_mem_prealloc: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.MemPrealloc = memPrealloc
	}

	if value, ok := ocispec.Annotations[vcAnnotations.EnableSwap]; ok {
		enableSwap, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for enable_swap: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.Mlock = !enableSwap
	}

	if value, ok := ocispec.Annotations[vcAnnotations.FileBackedMemRootDir]; ok {
		sbConfig.HypervisorConfig.FileBackedMemRootDir = value
	}

	if value, ok := ocispec.Annotations[vcAnnotations.HugePages]; ok {
		hugePages, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for enable_hugepages: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.HugePages = hugePages
	}
	return nil
}

func addHypervisorCPUOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	if value, ok := ocispec.Annotations[vcAnnotations.DefaultVCPUs]; ok {
		vcpus, err := strconv.ParseUint(value, 10, 32)
		if err != nil {
			return fmt.Errorf("Error encountered parsing annotation default_vcpus: %v, please specify numeric value", err)
		}

		numCPUs := goruntime.NumCPU()

		if uint32(vcpus) > uint32(numCPUs) {
			return fmt.Errorf("Number of cpus %d specified in annotation default_vcpus is greater than the number of CPUs %d on the system", vcpus, numCPUs)
		}

		sbConfig.HypervisorConfig.NumVCPUs = uint32(vcpus)
	}

	if value, ok := ocispec.Annotations[vcAnnotations.DefaultMaxVCPUs]; ok {
		maxVCPUs, err := strconv.ParseUint(value, 10, 32)
		if err != nil {
			return fmt.Errorf("Error encountered parsing annotation for default_maxvcpus: %v, please specify positive numeric value", err)
		}

		numCPUs := goruntime.NumCPU()
		max := uint32(maxVCPUs)

		if max > uint32(numCPUs) {
			return fmt.Errorf("Number of cpus %d in annotation default_maxvcpus is greater than the number of CPUs %d on the system", max, numCPUs)
		}

		if sbConfig.HypervisorType == vc.QemuHypervisor && max > vc.MaxQemuVCPUs() {
			return fmt.Errorf("Number of cpus %d in annotation default_maxvcpus is greater than max no of CPUs %d supported for qemu", max, vc.MaxQemuVCPUs())
		}

		sbConfig.HypervisorConfig.DefaultMaxVCPUs = max
	}

	return nil
}

func addHypervisorBlockOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	if value, ok := ocispec.Annotations[vcAnnotations.BlockDeviceDriver]; ok {
		supportedBlockDrivers := []string{config.VirtioSCSI, config.VirtioBlock, config.VirtioMmio, config.Nvdimm, config.VirtioBlockCCW}

		valid := false
		for _, b := range supportedBlockDrivers {
			if b == value {
				sbConfig.HypervisorConfig.BlockDeviceDriver = value
				valid = true
			}
		}

		if !valid {
			return fmt.Errorf("Invalid hypervisor block storage driver %v specified in annotation (supported drivers: %v)", value, supportedBlockDrivers)
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.DisableBlockDeviceUse]; ok {
		disableBlockDeviceUse, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for disable_block_device_use: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.DisableBlockDeviceUse = disableBlockDeviceUse
	}

	if value, ok := ocispec.Annotations[vcAnnotations.EnableIOThreads]; ok {
		enableIOThreads, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for enable_iothreads: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.EnableIOThreads = enableIOThreads
	}

	if value, ok := ocispec.Annotations[vcAnnotations.BlockDeviceCacheSet]; ok {
		blockDeviceCacheSet, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for block_device_cache_set: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.BlockDeviceCacheSet = blockDeviceCacheSet
	}

	if value, ok := ocispec.Annotations[vcAnnotations.BlockDeviceCacheDirect]; ok {
		blockDeviceCacheDirect, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for block_device_cache_direct: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.BlockDeviceCacheDirect = blockDeviceCacheDirect
	}

	if value, ok := ocispec.Annotations[vcAnnotations.BlockDeviceCacheNoflush]; ok {
		blockDeviceCacheNoflush, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for block_device_cache_noflush: Please specify boolean value 'true|false'")
		}

		sbConfig.HypervisorConfig.BlockDeviceCacheNoflush = blockDeviceCacheNoflush
	}

	return nil
}

func addHypervisporVirtioFsOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	if value, ok := ocispec.Annotations[vcAnnotations.SharedFS]; ok {
		supportedSharedFS := []string{config.Virtio9P, config.VirtioFS}
		valid := false
		for _, fs := range supportedSharedFS {
			if fs == value {
				sbConfig.HypervisorConfig.SharedFS = value
				valid = true
			}
		}

		if !valid {
			return fmt.Errorf("Invalid hypervisor shared file system %v specified for annotation shared_fs, (supported file systems: %v)", value, supportedSharedFS)
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.VirtioFSDaemon]; ok {
		sbConfig.HypervisorConfig.VirtioFSDaemon = value
	}

	if sbConfig.HypervisorConfig.SharedFS == config.VirtioFS && sbConfig.HypervisorConfig.VirtioFSDaemon == "" {
		return fmt.Errorf("cannot enable virtio-fs without daemon path")
	}

	if value, ok := ocispec.Annotations[vcAnnotations.VirtioFSCache]; ok {
		sbConfig.HypervisorConfig.VirtioFSCache = value
	}

	if value, ok := ocispec.Annotations[vcAnnotations.VirtioFSCacheSize]; ok {
		cacheSize, err := strconv.ParseUint(value, 10, 32)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for virtio_fs_cache_size: %v, please specify positive numeric value", err)
		}

		sbConfig.HypervisorConfig.VirtioFSCacheSize = uint32(cacheSize)
	}

	if value, ok := ocispec.Annotations[vcAnnotations.Msize9p]; ok {
		msize9p, err := strconv.ParseUint(value, 10, 32)
		if err != nil || msize9p == 0 {
			return fmt.Errorf("Error parsing annotation for msize_9p, please specify positive numeric value")
		}

		sbConfig.HypervisorConfig.Msize9p = uint32(msize9p)
	}

	return nil
}

func addRuntimeConfigOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	if value, ok := ocispec.Annotations[vcAnnotations.DisableGuestSeccomp]; ok {
		disableGuestSeccomp, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for disable_guest_seccomp: Please specify boolean value 'true|false'")
		}

		sbConfig.DisableGuestSeccomp = disableGuestSeccomp
	}

	if value, ok := ocispec.Annotations[vcAnnotations.SandboxCgroupOnly]; ok {
		sandboxCgroupOnly, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for sandbox_cgroup_only: Please specify boolean value 'true|false'")
		}

		sbConfig.SandboxCgroupOnly = sandboxCgroupOnly
	}

	if value, ok := ocispec.Annotations[vcAnnotations.Experimental]; ok {
		features := strings.Split(value, " ")
		sbConfig.Experimental = []exp.Feature{}

		for _, f := range features {
			feature := exp.Get(f)
			if feature == nil {
				return fmt.Errorf("Unsupported experimental feature %s specified in annotation %v", f, vcAnnotations.Experimental)
			}
			sbConfig.Experimental = append(sbConfig.Experimental, *feature)
		}
	}

	if value, ok := ocispec.Annotations[vcAnnotations.DisableNewNetNs]; ok {
		disableNewNetNs, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for experimental: Please specify boolean value 'true|false'")
		}
		sbConfig.NetworkConfig.DisableNewNetNs = disableNewNetNs
	}

	if value, ok := ocispec.Annotations[vcAnnotations.InterNetworkModel]; ok {
		runtimeConfig := RuntimeConfig{}
		if err := runtimeConfig.InterNetworkModel.SetModel(value); err != nil {
			return fmt.Errorf("Unknown network model specified in annotation %s", vcAnnotations.InterNetworkModel)
		}

		sbConfig.NetworkConfig.InterworkingModel = runtimeConfig.InterNetworkModel
	}

	return nil
}

func addAgentConfigOverrides(ocispec specs.Spec, config *vc.SandboxConfig) error {
	c, ok := config.AgentConfig.(vc.KataAgentConfig)
	if !ok {
		return nil
	}

	if value, ok := ocispec.Annotations[vcAnnotations.KernelModules]; ok {
		modules := strings.Split(value, KernelModulesSeparator)
		c.KernelModules = modules
		config.AgentConfig = c
	}

	if value, ok := ocispec.Annotations[vcAnnotations.AgentTrace]; ok {
		trace, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf("Error parsing annotation for agent.trace: Please specify boolean value 'true|false'")
		}
		c.Trace = trace
	}

	if value, ok := ocispec.Annotations[vcAnnotations.AgentTraceMode]; ok {
		c.TraceMode = value
	}

	if value, ok := ocispec.Annotations[vcAnnotations.AgentTraceType]; ok {
		c.TraceType = value
	}

	config.AgentConfig = c

	return nil
}

// SandboxConfig converts an OCI compatible runtime configuration file
// to a virtcontainers sandbox configuration structure.
func SandboxConfig(ocispec specs.Spec, runtime RuntimeConfig, bundlePath, cid, console string, detach, systemdCgroup bool) (vc.SandboxConfig, error) {
	containerConfig, err := ContainerConfig(ocispec, bundlePath, cid, console, detach)
	if err != nil {
		return vc.SandboxConfig{}, err
	}

	shmSize, err := getShmSize(containerConfig)
	if err != nil {
		return vc.SandboxConfig{}, err
	}

	networkConfig, err := networkConfig(ocispec, runtime)
	if err != nil {
		return vc.SandboxConfig{}, err
	}

	sandboxConfig := vc.SandboxConfig{
		ID: cid,

		Hostname: ocispec.Hostname,

		HypervisorType:   runtime.HypervisorType,
		HypervisorConfig: runtime.HypervisorConfig,

		AgentType:   runtime.AgentType,
		AgentConfig: runtime.AgentConfig,

		ProxyType:   runtime.ProxyType,
		ProxyConfig: runtime.ProxyConfig,

		ShimType:   runtime.ShimType,
		ShimConfig: runtime.ShimConfig,

		NetworkConfig: networkConfig,

		Containers: []vc.ContainerConfig{containerConfig},

		Annotations: map[string]string{
			vcAnnotations.BundlePathKey: bundlePath,
		},

		ShmSize: shmSize,

		SystemdCgroup: systemdCgroup,

		SandboxCgroupOnly: runtime.SandboxCgroupOnly,

		DisableGuestSeccomp: runtime.DisableGuestSeccomp,

		// Q: Is this really necessary? @weizhang555
		// Spec: &ocispec,

		Experimental: runtime.Experimental,
	}

	if err := addAnnotations(ocispec, &sandboxConfig); err != nil {
		return vc.SandboxConfig{}, err
	}

	return sandboxConfig, nil
}

// ContainerConfig converts an OCI compatible runtime configuration
// file to a virtcontainers container configuration structure.
func ContainerConfig(ocispec specs.Spec, bundlePath, cid, console string, detach bool) (vc.ContainerConfig, error) {
	rootfs := vc.RootFs{Target: ocispec.Root.Path, Mounted: true}
	if !filepath.IsAbs(rootfs.Target) {
		rootfs.Target = filepath.Join(bundlePath, ocispec.Root.Path)
	}

	ociLog.Debugf("container rootfs: %s", rootfs.Target)

	cmd := types.Cmd{
		Args:            ocispec.Process.Args,
		Envs:            cmdEnvs(ocispec, []types.EnvVar{}),
		WorkDir:         ocispec.Process.Cwd,
		User:            strconv.FormatUint(uint64(ocispec.Process.User.UID), 10),
		PrimaryGroup:    strconv.FormatUint(uint64(ocispec.Process.User.GID), 10),
		Interactive:     ocispec.Process.Terminal,
		Console:         console,
		Detach:          detach,
		NoNewPrivileges: ocispec.Process.NoNewPrivileges,
	}

	cmd.SupplementaryGroups = []string{}
	for _, gid := range ocispec.Process.User.AdditionalGids {
		cmd.SupplementaryGroups = append(cmd.SupplementaryGroups, strconv.FormatUint(uint64(gid), 10))
	}

	deviceInfos, err := containerDeviceInfos(ocispec)
	if err != nil {
		return vc.ContainerConfig{}, err
	}

	if ocispec.Process != nil {
		cmd.Capabilities = ocispec.Process.Capabilities
	}

	containerConfig := vc.ContainerConfig{
		ID:             cid,
		RootFs:         rootfs,
		ReadonlyRootfs: ocispec.Root.Readonly,
		Cmd:            cmd,
		Annotations: map[string]string{
			vcAnnotations.BundlePathKey: bundlePath,
		},
		Mounts:      containerMounts(ocispec),
		DeviceInfos: deviceInfos,
		Resources:   *ocispec.Linux.Resources,

		// This is a custom OCI spec modified at SetEphemeralStorageType()
		// to support ephemeral storage and k8s empty dir.
		CustomSpec: &ocispec,
	}

	cType, err := ContainerType(ocispec)
	if err != nil {
		return vc.ContainerConfig{}, err
	}

	containerConfig.Annotations[vcAnnotations.ContainerTypeKey] = string(cType)

	return containerConfig, nil
}

func getShmSize(c vc.ContainerConfig) (uint64, error) {
	var shmSize uint64

	for _, m := range c.Mounts {
		if m.Destination != "/dev/shm" {
			continue
		}

		shmSize = vc.DefaultShmSize

		if m.Type == "bind" && m.Source != "/dev/shm" {
			var s syscall.Statfs_t

			if err := syscall.Statfs(m.Source, &s); err != nil {
				return 0, err
			}
			shmSize = uint64(s.Bsize) * s.Blocks
		}
		break
	}

	ociLog.Infof("shm-size detected: %d", shmSize)

	return shmSize, nil
}

// StatusToOCIState translates a virtcontainers container status into an OCI state.
func StatusToOCIState(status vc.ContainerStatus) specs.State {
	return specs.State{
		Version:     specs.Version,
		ID:          status.ID,
		Status:      StateToOCIState(status.State.State),
		Pid:         status.PID,
		Bundle:      status.Annotations[vcAnnotations.BundlePathKey],
		Annotations: status.Annotations,
	}
}

// StateToOCIState translates a virtcontainers container state into an OCI one.
func StateToOCIState(state types.StateString) string {
	switch state {
	case types.StateReady:
		return StateCreated
	case types.StateRunning:
		return StateRunning
	case types.StateStopped:
		return StateStopped
	case types.StatePaused:
		return StatePaused
	default:
		return ""
	}
}

// EnvVars converts an OCI process environment variables slice
// into a virtcontainers EnvVar slice.
func EnvVars(envs []string) ([]types.EnvVar, error) {
	var envVars []types.EnvVar

	envDelimiter := "="
	expectedEnvLen := 2

	for _, env := range envs {
		envSlice := strings.SplitN(env, envDelimiter, expectedEnvLen)

		if len(envSlice) < expectedEnvLen {
			return []types.EnvVar{}, fmt.Errorf("Wrong string format: %s, expecting only %v parameters separated with %q",
				env, expectedEnvLen, envDelimiter)
		}

		if envSlice[0] == "" {
			return []types.EnvVar{}, fmt.Errorf("Environment variable cannot be empty")
		}

		envSlice[1] = strings.Trim(envSlice[1], "' ")

		envVar := types.EnvVar{
			Var:   envSlice[0],
			Value: envSlice[1],
		}

		envVars = append(envVars, envVar)
	}

	return envVars, nil
}

// GetOCIConfig returns an OCI spec configuration from the annotation
// stored into the container status.
func GetOCIConfig(status vc.ContainerStatus) (specs.Spec, error) {
	if status.Spec == nil {
		return specs.Spec{}, fmt.Errorf("missing OCI spec for container")
	}

	return *status.Spec, nil
}
