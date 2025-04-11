// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2021 Adobe Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package oci

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"regexp"
	goruntime "runtime"
	"strconv"
	"strings"
	"syscall"

	ctrAnnotations "github.com/containerd/containerd/pkg/cri/annotations"
	podmanAnnotations "github.com/containers/podman/v4/pkg/annotations"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"
	"k8s.io/apimachinery/pkg/api/resource"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/govmm"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	vcAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations"
	dockershimAnnotations "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/annotations/dockershim"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	vcutils "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
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
	CRIContainerTypeKeyList = []string{ctrAnnotations.ContainerType, podmanAnnotations.ContainerType, dockershimAnnotations.ContainerTypeLabelKey}

	// CRISandboxNameKeyList lists all the CRI keys that could define
	// the sandbox ID (sandbox ID) from annotations in the config.json.
	CRISandboxNameKeyList = []string{ctrAnnotations.SandboxID, podmanAnnotations.SandboxID, dockershimAnnotations.SandboxIDLabelKey}

	// CRIContainerTypeList lists all the maps from CRI ContainerTypes annotations
	// to a virtcontainers ContainerType.
	CRIContainerTypeList = []annotationContainerType{
		{podmanAnnotations.ContainerTypeSandbox, vc.PodSandbox},
		{podmanAnnotations.ContainerTypeContainer, vc.PodContainer},
		{ctrAnnotations.ContainerTypeSandbox, vc.PodSandbox},
		{ctrAnnotations.ContainerTypeContainer, vc.PodContainer},
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
	// TemplatePath specifies the path of template.
	TemplatePath string

	// VMCacheEndpoint specifies the endpoint of transport VM from the VM cache server to runtime.
	VMCacheEndpoint string

	// VMCacheNumber specifies the the number of caches of VMCache.
	VMCacheNumber uint

	// Template enables VM templating support in VM factory.
	Template bool
}

// RuntimeConfig aggregates all runtime specific settings
// nolint: govet
type RuntimeConfig struct {
	//Paths to be bindmounted RO into the guest.
	SandboxBindMounts []string

	//Experimental features enabled
	Experimental []exp.Feature

	JaegerEndpoint string
	JaegerUser     string
	JaegerPassword string
	HypervisorType vc.HypervisorType

	FactoryConfig    FactoryConfig
	HypervisorConfig vc.HypervisorConfig
	AgentConfig      vc.KataAgentConfig

	//Determines how the VM should be connected to the
	//the container network interface
	InterNetworkModel vc.NetInterworkingModel

	//Determines how VFIO devices should be presented to the
	//container
	VfioMode config.VFIOModeType

	Debug bool
	Trace bool

	//Determines if seccomp should be applied inside guest
	DisableGuestSeccomp bool

	// EnableVCPUsPinning controls whether each vCPU thread should be scheduled to a fixed CPU
	EnableVCPUsPinning bool

	//SELinux security context applied to the container process inside guest.
	GuestSeLinuxLabel string

	// Sandbox sizing information which, if provided, indicates the size of
	// the sandbox needed for the workload(s)
	SandboxCPUs  float32
	SandboxMemMB uint32

	// Determines if we should attempt to size the VM at boot time and skip
	// any later resource updates.
	StaticSandboxResourceMgmt bool

	// Determines if create a netns for hypervisor process
	DisableNewNetNs bool

	//Determines kata processes are managed only in sandbox cgroup
	SandboxCgroupOnly bool

	// Determines if enable pprof
	EnablePprof bool

	// Determines if Kata creates emptyDir on the guest
	DisableGuestEmptyDir bool

	// CreateContainer timeout which, if provided, indicates the createcontainer request timeout
	// needed for the workload ( Mostly used for pulling images in the guest )
	CreateContainerTimeout uint64

	// Base directory of directly attachable network config
	DanConfig string
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
	readonly := false
	bind := false
	for _, flag := range m.Options {
		switch flag {
		case "rbind", "bind":
			bind = true
		case "ro":
			readonly = true
		}
	}

	// normal bind mounts, set type to bind.
	// https://github.com/opencontainers/runc/blob/v1.1.3/libcontainer/specconv/spec_linux.go#L512-L520
	mountType := m.Type
	if mountType != vc.KataEphemeralDevType && mountType != vc.KataLocalDevType && bind {
		mountType = "bind"
	}

	return vc.Mount{
		Source:      m.Source,
		Destination: m.Destination,
		Type:        mountType,
		Options:     m.Options,
		ReadOnly:    readonly,
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

func contains(strings []string, toFind string) bool {
	for _, candidate := range strings {
		if candidate == toFind {
			return true
		}
	}
	return false
}

func regexpContains(regexps []string, toMatch string) bool {
	for _, candidate := range regexps {
		if matched, _ := regexp.MatchString(candidate, toMatch); matched {
			return true
		}
	}
	return false
}

func checkPathIsInGlobs(globs []string, path string) bool {
	for _, glob := range globs {
		filenames, _ := filepath.Glob(glob)
		for _, a := range filenames {
			if path == a {
				return true
			}
		}
	}

	return false
}

// Check if an annotation name either belongs to another prefix, matches regexp list
func checkAnnotationNameIsValid(list []string, name string, prefix string) bool {
	if strings.HasPrefix(name, prefix) {
		return regexpContains(list, strings.TrimPrefix(name, prefix))
	}

	return true
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

func getDanConfigPath(danConfigDir string, sandboxID string) string {
	return filepath.Join(danConfigDir, sandboxID+".json")
}

func networkConfig(ocispec specs.Spec, sandboxID string, config RuntimeConfig) (vc.NetworkConfig, error) {
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
			netConf.NetworkID = n.Path
		}
	}
	netConf.InterworkingModel = config.InterNetworkModel
	netConf.DisableNewNetwork = config.DisableNewNetNs

	// if dan config exits, it will be used to config network in guest VM
	danConfig := getDanConfigPath(config.DanConfig, sandboxID)
	if _, err := os.Stat(danConfig); err == nil {
		netConf.DanConfigPath = danConfig
	}

	return netConf, nil
}

// ContainerType returns the type of container and if the container type was
// found from CRI server's annotations in the container spec.
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

	return vc.SingleContainer, nil
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

func addAnnotations(ocispec specs.Spec, config *vc.SandboxConfig, runtime RuntimeConfig) error {
	for key := range ocispec.Annotations {
		if !checkAnnotationNameIsValid(runtime.HypervisorConfig.EnableAnnotations, key, vcAnnotations.KataAnnotationHypervisorPrefix) {
			return fmt.Errorf("annotation %v is not enabled", key)
		}
	}

	err := addAssetAnnotations(ocispec, config)
	if err != nil {
		return err
	}

	if err := addHypervisorConfigOverrides(ocispec, config, runtime); err != nil {
		return err
	}

	if err := addRuntimeConfigOverrides(ocispec, config, runtime); err != nil {
		return err
	}

	if err := addAgentConfigOverrides(ocispec, config); err != nil {
		return err
	}
	return nil
}

func addAssetAnnotations(ocispec specs.Spec, config *vc.SandboxConfig) error {
	assetAnnotations, err := types.AssetAnnotations()
	if err != nil {
		return err
	}

	for _, a := range assetAnnotations {
		value, ok := ocispec.Annotations[a]
		if ok {
			config.Annotations[a] = value
		}
	}

	return nil
}

func addHypervisorConfigOverrides(ocispec specs.Spec, config *vc.SandboxConfig, runtime RuntimeConfig) error {
	if err := addHypervisorCPUOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addHypervisorMemoryOverrides(ocispec, config, runtime); err != nil {
		return err
	}

	if err := addHypervisorBlockOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addHypervisorVirtioFsOverrides(ocispec, config, runtime); err != nil {
		return err
	}

	if err := addHypervisorNetworkOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addHypervisorPathOverrides(ocispec, config, runtime); err != nil {
		return err
	}

	if err := addHypervisorHotColdPlugVfioOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addHypervisorPCIeRootPortOverrides(ocispec, config); err != nil {
		return err
	}

	if err := addHypervisorPCIeSwitchPortOverrides(ocispec, config); err != nil {
		return err
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

	if value, ok := ocispec.Annotations[vcAnnotations.VhostUserStorePath]; ok {
		if !checkPathIsInGlobs(runtime.HypervisorConfig.VhostUserStorePathList, value) {
			return fmt.Errorf("vhost store path %v required from annotation is not valid", value)
		}
		config.HypervisorConfig.VhostUserStorePath = value
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.EnableVhostUserStore).setBool(func(enable bool) {
		config.HypervisorConfig.EnableVhostUserStore = enable
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.VhostUserDeviceReconnect).setUint(func(reconnect uint64) {
		config.HypervisorConfig.VhostUserDeviceReconnect = uint32(reconnect)
	}); err != nil {
		return err
	}

	if value, ok := ocispec.Annotations[vcAnnotations.GuestHookPath]; ok {
		if value != "" {
			config.HypervisorConfig.GuestHookPath = value
		}
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.DisableImageNvdimm).setBool(func(disableNvdimm bool) {
		config.HypervisorConfig.DisableImageNvdimm = disableNvdimm
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.UseLegacySerial).setBool(func(useLegacySerial bool) {
		config.HypervisorConfig.LegacySerial = useLegacySerial
	}); err != nil {
		return err
	}

	if value, ok := ocispec.Annotations[vcAnnotations.EntropySource]; ok {
		if !checkPathIsInGlobs(runtime.HypervisorConfig.EntropySourceList, value) {
			return fmt.Errorf("entropy source %v required from annotation is not valid", value)
		}
		if value != "" {
			config.HypervisorConfig.EntropySource = value
		}
	}
	if epcSize, ok := ocispec.Annotations[vcAnnotations.SGXEPC]; ok {
		quantity, err := resource.ParseQuantity(epcSize)
		if err != nil {
			return fmt.Errorf("Couldn't parse EPC '%v': %v", err, epcSize)
		}

		if quantity.Format != resource.BinarySI {
			return fmt.Errorf("Unsupported EPC format '%v': use Ki | Mi | Gi | Ti | Pi | Ei as suffix", epcSize)
		}

		size, _ := quantity.AsInt64()

		config.HypervisorConfig.SGXEPCSize = size
	}
	if initdata, ok := ocispec.Annotations[vcAnnotations.Initdata]; ok {
		config.HypervisorConfig.Initdata = initdata
	}

	if err := addHypervisorGPUOverrides(ocispec, config); err != nil {
		return err
	}

	return nil
}

func addHypervisorPathOverrides(ocispec specs.Spec, config *vc.SandboxConfig, runtime RuntimeConfig) error {
	if value, ok := ocispec.Annotations[vcAnnotations.HypervisorPath]; ok {
		if !checkPathIsInGlobs(runtime.HypervisorConfig.HypervisorPathList, value) {
			return fmt.Errorf("hypervisor %v required from annotation is not valid", value)
		}
		config.HypervisorConfig.HypervisorPath = value
	}

	if value, ok := ocispec.Annotations[vcAnnotations.JailerPath]; ok {
		if !checkPathIsInGlobs(runtime.HypervisorConfig.JailerPathList, value) {
			return fmt.Errorf("jailer %v required from annotation is not valid", value)
		}
		config.HypervisorConfig.JailerPath = value
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

	return nil
}

func addHypervisorPCIePortOverride(value string) (config.PCIePort, error) {
	if value == "" {
		return config.NoPort, nil
	}
	port := config.PCIePort(value)
	if port.Invalid() {
		return config.InvalidPort, fmt.Errorf("Invalid PCIe port \"%v\" specified in annotation", value)
	}
	return port, nil
}

func addHypervisorHotColdPlugVfioOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {

	var err error
	if value, ok := ocispec.Annotations[vcAnnotations.HotPlugVFIO]; ok {
		if sbConfig.HypervisorConfig.HotPlugVFIO, err = addHypervisorPCIePortOverride(value); err != nil {
			return err
		}
		// If hot-plug is specified disable cold-plug and vice versa
		sbConfig.HypervisorConfig.ColdPlugVFIO = config.NoPort
	}
	if value, ok := ocispec.Annotations[vcAnnotations.ColdPlugVFIO]; ok {
		if sbConfig.HypervisorConfig.ColdPlugVFIO, err = addHypervisorPCIePortOverride(value); err != nil {
			return err
		}
		// If cold-plug is specified disable hot-plug and vice versa
		sbConfig.HypervisorConfig.HotPlugVFIO = config.NoPort
	}
	return nil
}

func addHypervisorPCIeRootPortOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.PCIeRootPort).setUint(func(pcieRootPort uint64) {
		if pcieRootPort > 0 {
			sbConfig.HypervisorConfig.PCIeRootPort = uint32(pcieRootPort)
		}
	}); err != nil {
		return err
	}
	return nil
}

func addHypervisorPCIeSwitchPortOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	if err := newAnnotationConfiguration(ocispec, vcAnnotations.PCIeSwitchPort).setUint(func(pcieSwitchPort uint64) {
		if pcieSwitchPort > 0 {
			sbConfig.HypervisorConfig.PCIeSwitchPort = uint32(pcieSwitchPort)
		}
	}); err != nil {
		return err
	}
	return nil
}

func addHypervisorMemoryOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig, runtime RuntimeConfig) error {

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.DefaultMemory).setUintWithCheck(func(memorySz uint64) error {
		if memorySz < vc.MinHypervisorMemory && sbConfig.HypervisorType != vc.RemoteHypervisor {
			return fmt.Errorf("Memory specified in annotation %s is less than minimum required %d, please specify a larger value", vcAnnotations.DefaultMemory, vc.MinHypervisorMemory)
		}
		sbConfig.HypervisorConfig.MemorySize = uint32(memorySz)
		return nil
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.MemSlots).setUint(func(mslots uint64) {
		if mslots > 0 {
			sbConfig.HypervisorConfig.MemSlots = uint32(mslots)
		}
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.MemOffset).setUint(func(moffset uint64) {
		if moffset > 0 {
			sbConfig.HypervisorConfig.MemOffset = moffset
		}
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.VirtioMem).setBool(func(virtioMem bool) {
		sbConfig.HypervisorConfig.VirtioMem = virtioMem
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.MemPrealloc).setBool(func(memPrealloc bool) {
		sbConfig.HypervisorConfig.MemPrealloc = memPrealloc
	}); err != nil {
		return err
	}

	if value, ok := ocispec.Annotations[vcAnnotations.FileBackedMemRootDir]; ok {
		if !checkPathIsInGlobs(runtime.HypervisorConfig.FileBackedMemRootList, value) {
			return fmt.Errorf("file_mem_backend value %v required from annotation is not valid", value)
		}
		sbConfig.HypervisorConfig.FileBackedMemRootDir = value
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.HugePages).setBool(func(hugePages bool) {
		sbConfig.HypervisorConfig.HugePages = hugePages
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.IOMMU).setBool(func(iommu bool) {
		sbConfig.HypervisorConfig.IOMMU = iommu
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.IOMMUPlatform).setBool(func(deviceIOMMU bool) {
		sbConfig.HypervisorConfig.IOMMUPlatform = deviceIOMMU
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.EnableGuestSwap).setBool(func(enableGuestSwap bool) {
		sbConfig.HypervisorConfig.GuestSwap = enableGuestSwap
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.EnableRootlessHypervisor).setBool(func(enableRootlessHypervisor bool) {
		sbConfig.HypervisorConfig.Rootless = enableRootlessHypervisor
	}); err != nil {
		return err
	}

	return nil
}

func addHypervisorCPUOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	numCPUs := goruntime.NumCPU()

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.DefaultVCPUs).setFloat32WithCheck(func(vcpus float32) error {
		if vcpus > float32(numCPUs) && sbConfig.HypervisorType != vc.RemoteHypervisor {
			return fmt.Errorf("Number of cpus %f specified in annotation default_vcpus is greater than the number of CPUs %d on the system", vcpus, numCPUs)
		}
		sbConfig.HypervisorConfig.NumVCPUsF = float32(vcpus)
		return nil
	}); err != nil {
		return err
	}

	return newAnnotationConfiguration(ocispec, vcAnnotations.DefaultMaxVCPUs).setUintWithCheck(func(maxVCPUs uint64) error {
		max := uint32(maxVCPUs)

		if max > uint32(numCPUs) && sbConfig.HypervisorType != vc.RemoteHypervisor {
			return fmt.Errorf("Number of cpus %d in annotation default_maxvcpus is greater than the number of CPUs %d on the system", max, numCPUs)
		}

		if sbConfig.HypervisorType == vc.QemuHypervisor && max > govmm.MaxVCPUs() && sbConfig.HypervisorType != vc.RemoteHypervisor {
			return fmt.Errorf("Number of cpus %d in annotation default_maxvcpus is greater than max no of CPUs %d supported for qemu", max, govmm.MaxVCPUs())
		}
		sbConfig.HypervisorConfig.DefaultMaxVCPUs = max
		return nil
	})
}

func addHypervisorGPUOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	if sbConfig.HypervisorType != vc.RemoteHypervisor {
		return nil
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.DefaultGPUs).setUint(func(gpus uint64) {
		sbConfig.HypervisorConfig.DefaultGPUs = uint32(gpus)
	}); err != nil {
		return err
	}

	if value, ok := ocispec.Annotations[vcAnnotations.DefaultGPUModel]; ok {
		if value != "" {
			sbConfig.HypervisorConfig.DefaultGPUModel = value
		}
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

	if value, ok := ocispec.Annotations[vcAnnotations.BlockDeviceAIO]; ok {
		supportedAIO := []string{config.AIONative, config.AIOThreads, config.AIOIOUring}

		valid := false
		for _, b := range supportedAIO {
			if b == value {
				sbConfig.HypervisorConfig.BlockDeviceAIO = value
				valid = true
			}
		}

		if !valid {
			return fmt.Errorf("Invalid AIO mechanism  %v specified in annotation (supported IO mechanism : %v)", value, supportedAIO)
		}
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.DisableBlockDeviceUse).setBool(func(disableBlockDeviceUse bool) {
		sbConfig.HypervisorConfig.DisableBlockDeviceUse = disableBlockDeviceUse
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.EnableIOThreads).setBool(func(enableIOThreads bool) {
		sbConfig.HypervisorConfig.EnableIOThreads = enableIOThreads
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.BlockDeviceCacheSet).setBool(func(blockDeviceCacheSet bool) {
		sbConfig.HypervisorConfig.BlockDeviceCacheSet = blockDeviceCacheSet
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.BlockDeviceCacheDirect).setBool(func(blockDeviceCacheDirect bool) {
		sbConfig.HypervisorConfig.BlockDeviceCacheDirect = blockDeviceCacheDirect
	}); err != nil {
		return err
	}

	return newAnnotationConfiguration(ocispec, vcAnnotations.BlockDeviceCacheNoflush).setBool(func(blockDeviceCacheNoflush bool) {
		sbConfig.HypervisorConfig.BlockDeviceCacheNoflush = blockDeviceCacheNoflush
	})
}

func addHypervisorVirtioFsOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig, runtime RuntimeConfig) error {
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
		if !checkPathIsInGlobs(runtime.HypervisorConfig.VirtioFSDaemonList, value) {
			return fmt.Errorf("virtiofs daemon %v required from annotation is not valid", value)
		}
		sbConfig.HypervisorConfig.VirtioFSDaemon = value
	}

	if value, ok := ocispec.Annotations[vcAnnotations.VirtioFSExtraArgs]; ok {
		var parsedValue []string
		err := json.Unmarshal([]byte(value), &parsedValue)
		if err != nil {
			return fmt.Errorf("Error parsing virtiofsd extra arguments: %v", err)
		}
		sbConfig.HypervisorConfig.VirtioFSExtraArgs = append(sbConfig.HypervisorConfig.VirtioFSExtraArgs, parsedValue...)
	}

	if sbConfig.HypervisorConfig.SharedFS == config.VirtioFS && sbConfig.HypervisorConfig.VirtioFSDaemon == "" {
		return fmt.Errorf("cannot enable virtio-fs without daemon path")
	}

	if value, ok := ocispec.Annotations[vcAnnotations.VirtioFSCache]; ok {
		sbConfig.HypervisorConfig.VirtioFSCache = value
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.VirtioFSCacheSize).setUint(func(cacheSize uint64) {
		sbConfig.HypervisorConfig.VirtioFSCacheSize = uint32(cacheSize)
	}); err != nil {
		return err
	}

	return newAnnotationConfiguration(ocispec, vcAnnotations.Msize9p).setUintWithCheck(func(msize9p uint64) error {
		if msize9p == 0 {
			return fmt.Errorf("Error parsing annotation for msize_9p, please specify positive numeric value")
		}
		sbConfig.HypervisorConfig.Msize9p = uint32(msize9p)
		return nil
	})
}

func addHypervisorNetworkOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig) error {
	if value, ok := ocispec.Annotations[vcAnnotations.CPUFeatures]; ok {
		if value != "" {
			sbConfig.HypervisorConfig.CPUFeatures = value
		}
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.DisableVhostNet).setBool(func(disableVhostNet bool) {
		sbConfig.HypervisorConfig.DisableVhostNet = disableVhostNet
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.RxRateLimiterMaxRate).setUint(func(rxRateLimiterMaxRate uint64) {
		sbConfig.HypervisorConfig.RxRateLimiterMaxRate = rxRateLimiterMaxRate
	}); err != nil {
		return err
	}

	return newAnnotationConfiguration(ocispec, vcAnnotations.TxRateLimiterMaxRate).setUint(func(txRateLimiterMaxRate uint64) {
		sbConfig.HypervisorConfig.TxRateLimiterMaxRate = txRateLimiterMaxRate
	})
}

func addRuntimeConfigOverrides(ocispec specs.Spec, sbConfig *vc.SandboxConfig, runtime RuntimeConfig) error {

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.DisableGuestSeccomp).setBool(func(disableGuestSeccomp bool) {
		sbConfig.DisableGuestSeccomp = disableGuestSeccomp
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.SandboxCgroupOnly).setBool(func(sandboxCgroupOnly bool) {
		sbConfig.SandboxCgroupOnly = sandboxCgroupOnly
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.CreateContainerTimeout).setUint(func(createContainerTimeout uint64) {
		sbConfig.CreateContainerTimeout = createContainerTimeout
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.EnableVCPUsPinning).setBool(func(enableVCPUsPinning bool) {
		sbConfig.EnableVCPUsPinning = enableVCPUsPinning
	}); err != nil {
		return err
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

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.DisableNewNetNs).setBool(func(disableNewNetNs bool) {
		sbConfig.NetworkConfig.DisableNewNetwork = disableNewNetNs
	}); err != nil {
		return err
	}

	if value, ok := ocispec.Annotations[vcAnnotations.InterNetworkModel]; ok {
		runtimeConfig := RuntimeConfig{}
		if err := runtimeConfig.InterNetworkModel.SetModel(value); err != nil {
			return fmt.Errorf("Unknown network model specified in annotation %s", vcAnnotations.InterNetworkModel)
		}

		sbConfig.NetworkConfig.InterworkingModel = runtimeConfig.InterNetworkModel
	}

	if value, ok := ocispec.Annotations[vcAnnotations.VfioMode]; ok {
		if err := sbConfig.VfioMode.VFIOSetMode(value); err != nil {
			return fmt.Errorf("Unknown VFIO mode \"%s\" in annotation %s",
				value, vcAnnotations.VfioMode)
		}
	}

	return nil
}

func addAgentConfigOverrides(ocispec specs.Spec, config *vc.SandboxConfig) error {
	c := config.AgentConfig
	updateConfig := false

	if value, ok := ocispec.Annotations[vcAnnotations.KernelModules]; ok {
		modules := strings.Split(value, KernelModulesSeparator)
		c.KernelModules = modules
		updateConfig = true
	}

	if value, ok := ocispec.Annotations[vcAnnotations.Policy]; ok {
		if decoded_rules, err := base64.StdEncoding.DecodeString(value); err == nil {
			c.Policy = string(decoded_rules)
			updateConfig = true
		} else {
			return err
		}
	}

	if updateConfig {
		config.AgentConfig = c
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.AgentTrace).setBool(func(trace bool) {
		c.Trace = trace
	}); err != nil {
		return err
	}

	if err := newAnnotationConfiguration(ocispec, vcAnnotations.AgentContainerPipeSize).setUint(func(containerPipeSize uint64) {
		c.ContainerPipeSize = uint32(containerPipeSize)
	}); err != nil {
		return err
	}

	config.AgentConfig = c

	return nil
}

// SandboxConfig converts an OCI compatible runtime configuration file
// to a virtcontainers sandbox configuration structure.
func SandboxConfig(ocispec specs.Spec, runtime RuntimeConfig, bundlePath, cid string, detach, systemdCgroup bool) (vc.SandboxConfig, error) {
	containerConfig, err := ContainerConfig(ocispec, bundlePath, cid, detach)
	if err != nil {
		return vc.SandboxConfig{}, err
	}

	shmSize, err := getShmSize(containerConfig)
	if err != nil {
		return vc.SandboxConfig{}, err
	}

	networkConfig, err := networkConfig(ocispec, cid, runtime)
	if err != nil {
		return vc.SandboxConfig{}, err
	}

	sandboxConfig := vc.SandboxConfig{
		ID: cid,

		Hostname: ocispec.Hostname,

		HypervisorType:   runtime.HypervisorType,
		HypervisorConfig: runtime.HypervisorConfig,

		AgentConfig: runtime.AgentConfig,

		NetworkConfig: networkConfig,

		Containers: []vc.ContainerConfig{containerConfig},

		Annotations: map[string]string{
			vcAnnotations.BundlePathKey: bundlePath,
		},

		SandboxResources: vc.SandboxResourceSizing{
			WorkloadCPUs:  runtime.SandboxCPUs,
			WorkloadMemMB: runtime.SandboxMemMB,
		},

		StaticResourceMgmt: runtime.StaticSandboxResourceMgmt,

		ShmSize: shmSize,

		VfioMode: runtime.VfioMode,

		SystemdCgroup: systemdCgroup,

		SandboxCgroupOnly: runtime.SandboxCgroupOnly,
		SandboxBindMounts: runtime.SandboxBindMounts,

		DisableGuestSeccomp: runtime.DisableGuestSeccomp,

		EnableVCPUsPinning: runtime.EnableVCPUsPinning,

		GuestSeLinuxLabel: runtime.GuestSeLinuxLabel,

		Experimental: runtime.Experimental,

		CreateContainerTimeout: runtime.CreateContainerTimeout,
	}

	if err := addAnnotations(ocispec, &sandboxConfig, runtime); err != nil {
		return vc.SandboxConfig{}, err
	}

	// If we are utilizing static resource management for the sandbox, ensure that the hypervisor is started
	// with the base number of CPU/memory (which is equal to the default CPU/memory specified for the runtime
	// configuration or annotations) as well as any specified workload resources.
	if sandboxConfig.StaticResourceMgmt {
		sandboxConfig.SandboxResources.BaseCPUs = sandboxConfig.HypervisorConfig.NumVCPUsF
		sandboxConfig.SandboxResources.BaseMemMB = sandboxConfig.HypervisorConfig.MemorySize

		sandboxConfig.HypervisorConfig.NumVCPUsF += sandboxConfig.SandboxResources.WorkloadCPUs
		sandboxConfig.HypervisorConfig.MemorySize += sandboxConfig.SandboxResources.WorkloadMemMB

		sandboxConfig.HypervisorConfig.DefaultMaxVCPUs = sandboxConfig.HypervisorConfig.NumVCPUs()

		ociLog.WithFields(logrus.Fields{
			"workload cpu":       sandboxConfig.SandboxResources.WorkloadCPUs,
			"default cpu":        sandboxConfig.SandboxResources.BaseCPUs,
			"workload mem in MB": sandboxConfig.SandboxResources.WorkloadMemMB,
			"default mem":        sandboxConfig.SandboxResources.BaseMemMB,
		}).Debugf("static resources set")

	}

	return sandboxConfig, nil
}

// ContainerConfig converts an OCI compatible runtime configuration
// file to a virtcontainers container configuration structure.
func ContainerConfig(ocispec specs.Spec, bundlePath, cid string, detach bool) (vc.ContainerConfig, error) {
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
		Annotations:    ocispec.Annotations,
		Mounts:         containerMounts(ocispec),
		DeviceInfos:    deviceInfos,
		Resources:      *ocispec.Linux.Resources,

		// This is a custom OCI spec modified at SetEphemeralStorageType()
		// to support ephemeral storage and k8s empty dir.
		CustomSpec: &ocispec,
	}
	if containerConfig.Annotations == nil {
		containerConfig.Annotations = map[string]string{
			vcAnnotations.BundlePathKey: bundlePath,
		}
	} else {
		containerConfig.Annotations[vcAnnotations.BundlePathKey] = bundlePath
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

// IsCRIOContainerManager check if a Pod is created from CRI-O
func IsCRIOContainerManager(spec *specs.Spec) bool {
	if val, ok := spec.Annotations[podmanAnnotations.ContainerType]; ok {
		if val == podmanAnnotations.ContainerTypeSandbox || val == podmanAnnotations.ContainerTypeContainer {
			return true
		}
	}
	return false
}

const (
	errAnnotationPositiveNumericKey = "Error parsing annotation for %s: Please specify positive numeric value"
	errAnnotationBoolKey            = "Error parsing annotation for %s: Please specify boolean value 'true|false'"
	errAnnotationNumericKeyIsTooBig = "Error parsing annotation for %s: The number exceeds the maximum allowed for its type"
)

type annotationConfiguration struct {
	ocispec specs.Spec
	key     string
}

func newAnnotationConfiguration(ocispec specs.Spec, key string) *annotationConfiguration {
	return &annotationConfiguration{
		ocispec: ocispec,
		key:     key,
	}
}

func (a *annotationConfiguration) setBool(f func(bool)) error {
	if value, ok := a.ocispec.Annotations[a.key]; ok {
		boolValue, err := strconv.ParseBool(value)
		if err != nil {
			return fmt.Errorf(errAnnotationBoolKey, a.key)
		}
		f(boolValue)
	}
	return nil
}

func (a *annotationConfiguration) setUint(f func(uint64)) error {
	return a.setUintWithCheck(func(v uint64) error {
		f(v)
		return nil
	})
}

func (a *annotationConfiguration) setUintWithCheck(f func(uint64) error) error {
	if value, ok := a.ocispec.Annotations[a.key]; ok {
		uintValue, err := strconv.ParseUint(value, 10, 64)
		if err != nil {
			return fmt.Errorf(errAnnotationPositiveNumericKey, a.key)
		}
		return f(uintValue)
	}
	return nil
}

func (a *annotationConfiguration) setFloat32WithCheck(f func(float32) error) error {
	if value, ok := a.ocispec.Annotations[a.key]; ok {
		float64Value, err := strconv.ParseFloat(value, 32)
		if err != nil || float64Value < 0 {
			return fmt.Errorf(errAnnotationPositiveNumericKey, a.key)
		}
		if float64Value > math.MaxFloat32 {
			return fmt.Errorf(errAnnotationNumericKeyIsTooBig, a.key)
		}
		float32Value := float32(float64Value)
		return f(float32Value)
	}
	return nil
}

// CalculateSandboxSizing will calculate the number of CPUs and amount of Memory that should
// be added to the VM if sandbox annotations are provided with this sizing details
func CalculateSandboxSizing(spec *specs.Spec) (numCPU float32, memSizeMB uint32) {
	var memory, quota int64
	var period uint64
	var err error

	if spec == nil || spec.Annotations == nil {
		return 0, 0
	}

	// For each annotation, if it isn't defined, or if there's an error in parsing, we'll log
	// a warning and continue the calculation with 0 value. We expect values like,
	//  Annotations[SandboxMem] = "1048576"
	//  Annotations[SandboxCPUPeriod] = "100000"
	//  Annotations[SandboxCPUQuota] = "220000"
	// ... to result in VM resources of 1 (MB) for memory, and 3 for CPU (2200 mCPU rounded up to 3).
	annotation, ok := spec.Annotations[ctrAnnotations.SandboxCPUPeriod]
	if ok {
		period, err = strconv.ParseUint(annotation, 10, 64)
		if err != nil {
			ociLog.Warningf("sandbox-sizing: failure to parse SandboxCPUPeriod: %s", annotation)
			period = 0
		}
	}

	annotation, ok = spec.Annotations[ctrAnnotations.SandboxCPUQuota]
	if ok {
		quota, err = strconv.ParseInt(annotation, 10, 64)
		if err != nil {
			ociLog.Warningf("sandbox-sizing: failure to parse SandboxCPUQuota: %s", annotation)
			quota = 0
		}
	}

	annotation, ok = spec.Annotations[ctrAnnotations.SandboxMem]
	if ok {
		memory, err = strconv.ParseInt(annotation, 10, 64)
		if err != nil {
			ociLog.Warningf("sandbox-sizing: failure to parse SandboxMem: %s", annotation)
			memory = 0
		}
	}

	return calculateVMResources(period, quota, memory)
}

// CalculateContainerSizing will calculate the number of CPUs and amount of memory that is needed
// based on the provided LinuxResources
func CalculateContainerSizing(spec *specs.Spec) (numCPU float32, memSizeMB uint32) {
	var memory, quota int64
	var period uint64

	if spec == nil || spec.Linux == nil || spec.Linux.Resources == nil {
		return 0, 0
	}

	resources := spec.Linux.Resources

	if resources.CPU != nil && resources.CPU.Quota != nil && resources.CPU.Period != nil {
		quota = *resources.CPU.Quota
		period = *resources.CPU.Period
	}

	if resources.Memory != nil && resources.Memory.Limit != nil {
		memory = *resources.Memory.Limit
	}

	return calculateVMResources(period, quota, memory)
}

func calculateVMResources(period uint64, quota int64, memory int64) (numCPU float32, memSizeMB uint32) {
	numCPU = vcutils.CalculateCPUsF(quota, period)

	if memory < 0 {
		// While spec allows for a negative value to indicate unconstrained, we don't
		// see this in practice. Since we rely only on default memory if the workload
		// is unconstrained, we will treat as 0 for VM resource accounting.
		ociLog.Infof("memory limit provided < 0, treating as 0 MB for VM sizing: %d", memory)
		memSizeMB = 0
	} else {
		memSizeMB = uint32(memory / 1024 / 1024)
	}
	return numCPU, memSizeMB
}
