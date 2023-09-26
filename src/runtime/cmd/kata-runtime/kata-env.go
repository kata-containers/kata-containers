// Copyright (c) 2017-2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"encoding/json"
	"errors"
	"os"
	"runtime"
	"strings"

	"github.com/BurntSushi/toml"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/prometheus/procfs"
	"github.com/urfave/cli"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/oci"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	exp "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/experimental"
	vcUtils "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// Semantic version for the output of the command.
//
// XXX: Increment for every change to the output format
// (meaning any change to the EnvInfo type).
const formatVersion = "1.0.27"

// MetaInfo stores information on the format of the output itself
type MetaInfo struct {
	// output format version
	Version string
}

// KernelInfo stores kernel details
type KernelInfo struct {
	Path       string
	Parameters string
}

// InitrdInfo stores initrd image details
type InitrdInfo struct {
	Path string
}

// ImageInfo stores root filesystem image details
type ImageInfo struct {
	Path string
}

// CPUInfo stores host CPU details
type CPUInfo struct {
	Vendor string
	Model  string
	CPUs   int
}

// MemoryInfo stores host memory details
type MemoryInfo struct {
	Total     uint64
	Free      uint64
	Available uint64
}

// RuntimeConfigInfo stores runtime config details.
type RuntimeConfigInfo struct {
	Path string
}

// RuntimeInfo stores runtime details.
type RuntimeInfo struct {
	Config              RuntimeConfigInfo
	Path                string
	GuestSeLinuxLabel   string
	Experimental        []exp.Feature
	Version             RuntimeVersionInfo
	Debug               bool
	Trace               bool
	DisableGuestSeccomp bool
	DisableNewNetNs     bool
	SandboxCgroupOnly   bool
}

type VersionInfo struct {
	Semver string
	Commit string
	Major  uint64
	Minor  uint64
	Patch  uint64
}

// RuntimeVersionInfo stores details of the runtime version
type RuntimeVersionInfo struct {
	OCI     string
	Version VersionInfo
}

type SecurityInfo struct {
	Rootless          bool
	DisableSeccomp    bool
	GuestHookPath     string
	EnableAnnotations []string
	ConfidentialGuest bool
}

// HypervisorInfo stores hypervisor details
type HypervisorInfo struct {
	MachineType       string
	Version           string
	Path              string
	BlockDeviceDriver string
	EntropySource     string
	SharedFS          string
	VirtioFSDaemon    string
	SocketPath        string
	Msize9p           uint32
	MemorySlots       uint32
	HotPlugVFIO       config.PCIePort
	ColdPlugVFIO      config.PCIePort
	PCIeRootPort      uint32
	PCIeSwitchPort    uint32
	Debug             bool
	SecurityInfo      SecurityInfo
}

// AgentInfo stores agent details
type AgentInfo struct {
	Debug bool
	Trace bool
}

// DistroInfo stores host operating system distribution details.
type DistroInfo struct {
	Name    string
	Version string
}

// HostInfo stores host details
type HostInfo struct {
	AvailableGuestProtections []string
	Kernel                    string
	Architecture              string
	Distro                    DistroInfo
	CPU                       CPUInfo
	Memory                    MemoryInfo
	VMContainerCapable        bool
	SupportVSocks             bool
}

// EnvInfo collects all information that will be displayed by the
// env command.
//
// XXX: Any changes must be coupled with a change to formatVersion.
type EnvInfo struct {
	Kernel     KernelInfo
	Meta       MetaInfo
	Image      ImageInfo
	Initrd     InitrdInfo
	Hypervisor HypervisorInfo
	Runtime    RuntimeInfo
	Host       HostInfo
	Agent      AgentInfo
}

func getMetaInfo() MetaInfo {
	return MetaInfo{
		Version: formatVersion,
	}
}

func getRuntimeInfo(configFile string, config oci.RuntimeConfig) RuntimeInfo {
	runtimeVersionInfo := constructVersionInfo(katautils.VERSION)
	runtimeVersionInfo.Commit = katautils.COMMIT

	runtimeVersion := RuntimeVersionInfo{
		Version: runtimeVersionInfo,
		OCI:     specs.Version,
	}

	runtimeConfig := RuntimeConfigInfo{
		Path: configFile,
	}

	runtimePath, _ := os.Executable()

	return RuntimeInfo{
		Debug:               config.Debug,
		Trace:               config.Trace,
		Version:             runtimeVersion,
		Config:              runtimeConfig,
		Path:                runtimePath,
		DisableNewNetNs:     config.DisableNewNetNs,
		SandboxCgroupOnly:   config.SandboxCgroupOnly,
		Experimental:        config.Experimental,
		DisableGuestSeccomp: config.DisableGuestSeccomp,
		GuestSeLinuxLabel:   config.GuestSeLinuxLabel,
	}
}

func getHostInfo() (HostInfo, error) {
	hostKernelVersion, err := getKernelVersion()
	if err != nil {
		return HostInfo{}, err
	}

	hostDistroName, hostDistroVersion, err := getDistroDetails()
	if err != nil {
		return HostInfo{}, err
	}

	cpuVendor, cpuModel, err := getCPUDetails()
	if err != nil {
		return HostInfo{}, err
	}

	hostVMContainerCapable := true

	details := vmContainerCapableDetails{
		cpuInfoFile:           procCPUInfo,
		requiredCPUFlags:      archRequiredCPUFlags,
		requiredCPUAttribs:    archRequiredCPUAttribs,
		requiredKernelModules: archRequiredKernelModules,
	}

	if err = hostIsVMContainerCapable(details); err != nil {
		hostVMContainerCapable = false
	}

	hostDistro := DistroInfo{
		Name:    hostDistroName,
		Version: hostDistroVersion,
	}

	hostCPU := CPUInfo{
		Vendor: cpuVendor,
		Model:  cpuModel,
		CPUs:   runtime.NumCPU(),
	}

	supportVSocks, _ := vcUtils.SupportsVsocks()

	memoryInfo := getMemoryInfo()

	availableGuestProtection := vc.AvailableGuestProtections()

	host := HostInfo{
		Kernel:                    hostKernelVersion,
		Architecture:              arch,
		Distro:                    hostDistro,
		CPU:                       hostCPU,
		Memory:                    memoryInfo,
		AvailableGuestProtections: availableGuestProtection,
		VMContainerCapable:        hostVMContainerCapable,
		SupportVSocks:             supportVSocks,
	}

	return host, nil
}

func getMemoryInfo() MemoryInfo {
	fs, err := procfs.NewDefaultFS()
	if err != nil {
		return MemoryInfo{}
	}

	mi, err := fs.Meminfo()
	if err != nil {
		return MemoryInfo{}
	}

	return MemoryInfo{
		Total:     *mi.MemTotal,
		Free:      *mi.MemFree,
		Available: *mi.MemAvailable,
	}
}

func getCommandVersion(cmd string) (string, error) {
	return utils.RunCommand([]string{cmd, "--version"})
}

func getAgentInfo(config oci.RuntimeConfig) (AgentInfo, error) {
	agent := AgentInfo{}

	agentConfig := config.AgentConfig
	agent.Debug = agentConfig.Debug
	agent.Trace = agentConfig.Trace

	return agent, nil
}

func getSecurityInfo(config vc.HypervisorConfig) SecurityInfo {
	return SecurityInfo{
		Rootless:          config.Rootless,
		DisableSeccomp:    config.DisableSeccomp,
		GuestHookPath:     config.GuestHookPath,
		EnableAnnotations: config.EnableAnnotations,
		ConfidentialGuest: config.ConfidentialGuest,
	}
}

func getHypervisorInfo(config oci.RuntimeConfig) (HypervisorInfo, error) {
	hypervisorPath := config.HypervisorConfig.HypervisorPath

	version, err := getCommandVersion(hypervisorPath)
	if err != nil {
		version = unknown
	}

	hypervisorType := config.HypervisorType

	socketPath := unknown

	// It is only reliable to make this call as root since a
	// non-privileged user may not have access to /dev/vhost-vsock.
	if os.Geteuid() == 0 {
		socketPath, err = vc.GetHypervisorSocketTemplate(hypervisorType, &config.HypervisorConfig)
		if err != nil {
			return HypervisorInfo{}, err
		}
	}

	securityInfo := getSecurityInfo(config.HypervisorConfig)

	return HypervisorInfo{
		Debug:             config.HypervisorConfig.Debug,
		MachineType:       config.HypervisorConfig.HypervisorMachineType,
		Version:           version,
		Path:              hypervisorPath,
		BlockDeviceDriver: config.HypervisorConfig.BlockDeviceDriver,
		Msize9p:           config.HypervisorConfig.Msize9p,
		MemorySlots:       config.HypervisorConfig.MemSlots,
		EntropySource:     config.HypervisorConfig.EntropySource,
		SharedFS:          config.HypervisorConfig.SharedFS,
		VirtioFSDaemon:    config.HypervisorConfig.VirtioFSDaemon,
		HotPlugVFIO:       config.HypervisorConfig.HotPlugVFIO,
		ColdPlugVFIO:      config.HypervisorConfig.ColdPlugVFIO,
		PCIeRootPort:      config.HypervisorConfig.PCIeRootPort,
		PCIeSwitchPort:    config.HypervisorConfig.PCIeSwitchPort,
		SocketPath:        socketPath,
		SecurityInfo:      securityInfo,
	}, nil
}

func getEnvInfo(configFile string, config oci.RuntimeConfig) (env EnvInfo, err error) {
	err = setCPUtype(config.HypervisorType)
	if err != nil {
		return EnvInfo{}, err
	}

	meta := getMetaInfo()

	runtime := getRuntimeInfo(configFile, config)

	host, err := getHostInfo()
	if err != nil {
		return EnvInfo{}, err
	}

	agent, err := getAgentInfo(config)
	if err != nil {
		return EnvInfo{}, err
	}

	hypervisor, err := getHypervisorInfo(config)
	if err != nil {
		return EnvInfo{}, err
	}

	image := ImageInfo{
		Path: config.HypervisorConfig.ImagePath,
	}

	kernel := KernelInfo{
		Path:       config.HypervisorConfig.KernelPath,
		Parameters: strings.Join(vc.SerializeParams(config.HypervisorConfig.KernelParams, "="), " "),
	}

	initrd := InitrdInfo{
		Path: config.HypervisorConfig.InitrdPath,
	}

	env = EnvInfo{
		Meta:       meta,
		Runtime:    runtime,
		Hypervisor: hypervisor,
		Image:      image,
		Kernel:     kernel,
		Initrd:     initrd,
		Agent:      agent,
		Host:       host,
	}

	return env, nil
}

func handleSettings(file *os.File, c *cli.Context) error {
	if file == nil {
		return errors.New("Invalid output file specified")
	}

	configFile, ok := c.App.Metadata["configFile"].(string)
	if !ok {
		return errors.New("cannot determine config file")
	}

	runtimeConfig, ok := c.App.Metadata["runtimeConfig"].(oci.RuntimeConfig)
	if !ok {
		return errors.New("cannot determine runtime config")
	}

	env, err := getEnvInfo(configFile, runtimeConfig)
	if err != nil {
		return err
	}

	if c.Bool("json") {
		return writeJSONSettings(env, file)
	}

	return writeTOMLSettings(env, file)
}

func writeTOMLSettings(env EnvInfo, file *os.File) error {
	encoder := toml.NewEncoder(file)

	err := encoder.Encode(env)
	if err != nil {
		return err
	}

	return nil
}

func writeJSONSettings(env EnvInfo, file *os.File) error {
	encoder := json.NewEncoder(file)

	// Make it more human readable
	encoder.SetIndent("", "  ")

	err := encoder.Encode(env)
	if err != nil {
		return err
	}

	return nil
}

var kataEnvCLICommand = cli.Command{
	Name:    "env",
	Aliases: []string{"kata-env"},
	Usage:   "display settings. Default to TOML",
	Flags: []cli.Flag{
		cli.BoolFlag{
			Name:  "json",
			Usage: "Format output as JSON",
		},
	},
	Action: func(context *cli.Context) error {
		return handleSettings(defaultOutputFile, context)
	},
}
