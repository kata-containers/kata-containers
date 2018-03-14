// Copyright (c) 2017-2018 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package main

import (
	"errors"
	"os"
	"strings"

	"github.com/BurntSushi/toml"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/oci"
	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/urfave/cli"
)

// Semantic version for the output of the command.
//
// XXX: Increment for every change to the output format
// (meaning any change to the EnvInfo type).
const formatVersion = "1.0.9"

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

// ImageInfo stores root filesystem image details
type ImageInfo struct {
	Path string
}

// CPUInfo stores host CPU details
type CPUInfo struct {
	Vendor string
	Model  string
}

// RuntimeConfigInfo stores runtime config details.
type RuntimeConfigInfo struct {
	Path string
}

// RuntimeInfo stores runtime details.
type RuntimeInfo struct {
	Version RuntimeVersionInfo
	Config  RuntimeConfigInfo
	Debug   bool
}

// RuntimeVersionInfo stores details of the runtime version
type RuntimeVersionInfo struct {
	Semver string
	Commit string
	OCI    string
}

// HypervisorInfo stores hypervisor details
type HypervisorInfo struct {
	MachineType       string
	Version           string
	Path              string
	Debug             bool
	BlockDeviceDriver string
}

// ProxyInfo stores proxy details
type ProxyInfo struct {
	Type    string
	Version string
	Path    string
	Debug   bool
}

// ShimInfo stores shim details
type ShimInfo struct {
	Type    string
	Version string
	Path    string
	Debug   bool
}

// AgentInfo stores agent details
type AgentInfo struct {
	Type    string
	Version string
}

// DistroInfo stores host operating system distribution details.
type DistroInfo struct {
	Name    string
	Version string
}

// HostInfo stores host details
type HostInfo struct {
	Kernel             string
	Architecture       string
	Distro             DistroInfo
	CPU                CPUInfo
	VMContainerCapable bool
}

// EnvInfo collects all information that will be displayed by the
// env command.
//
// XXX: Any changes must be coupled with a change to formatVersion.
type EnvInfo struct {
	Meta       MetaInfo
	Runtime    RuntimeInfo
	Hypervisor HypervisorInfo
	Image      ImageInfo
	Kernel     KernelInfo
	Proxy      ProxyInfo
	Shim       ShimInfo
	Agent      AgentInfo
	Host       HostInfo
}

func getMetaInfo() MetaInfo {
	return MetaInfo{
		Version: formatVersion,
	}
}

func getRuntimeInfo(configFile string, config oci.RuntimeConfig) RuntimeInfo {
	runtimeVersion := RuntimeVersionInfo{
		Semver: version,
		Commit: commit,
		OCI:    specs.Version,
	}

	runtimeConfig := RuntimeConfigInfo{
		Path: configFile,
	}

	return RuntimeInfo{
		Version: runtimeVersion,
		Config:  runtimeConfig,
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
	}

	host := HostInfo{
		Kernel:             hostKernelVersion,
		Architecture:       arch,
		Distro:             hostDistro,
		CPU:                hostCPU,
		VMContainerCapable: hostVMContainerCapable,
	}

	return host, nil
}

func getProxyInfo(config oci.RuntimeConfig) (ProxyInfo, error) {
	version, err := getCommandVersion(defaultProxyPath)
	if err != nil {
		version = unknown
	}

	proxy := ProxyInfo{
		Type:    string(config.ProxyType),
		Version: version,
		Path:    config.ProxyConfig.Path,
		Debug:   config.ProxyConfig.Debug,
	}

	return proxy, nil
}

func getCommandVersion(cmd string) (string, error) {
	return runCommand([]string{cmd, "--version"})
}

func getShimInfo(config oci.RuntimeConfig) (ShimInfo, error) {
	shimConfig, ok := config.ShimConfig.(vc.ShimConfig)
	if !ok {
		return ShimInfo{}, errors.New("cannot determine shim config")
	}

	shimPath := shimConfig.Path

	version, err := getCommandVersion(shimPath)
	if err != nil {
		version = unknown
	}

	shim := ShimInfo{
		Type:    string(config.ShimType),
		Version: version,
		Path:    shimPath,
		Debug:   shimConfig.Debug,
	}

	return shim, nil
}

func getAgentInfo(config oci.RuntimeConfig) AgentInfo {
	agent := AgentInfo{
		Type:    string(config.AgentType),
		Version: unknown,
	}

	return agent
}

func getHypervisorInfo(config oci.RuntimeConfig) HypervisorInfo {
	hypervisorPath := config.HypervisorConfig.HypervisorPath

	version, err := getCommandVersion(hypervisorPath)
	if err != nil {
		version = unknown
	}

	return HypervisorInfo{
		MachineType:       config.HypervisorConfig.HypervisorMachineType,
		Version:           version,
		Path:              hypervisorPath,
		BlockDeviceDriver: config.HypervisorConfig.BlockDeviceDriver,
	}
}

func getEnvInfo(configFile string, config oci.RuntimeConfig) (env EnvInfo, err error) {
	meta := getMetaInfo()

	runtime := getRuntimeInfo(configFile, config)

	host, err := getHostInfo()
	if err != nil {
		return EnvInfo{}, err
	}

	proxy, _ := getProxyInfo(config)

	shim, err := getShimInfo(config)
	if err != nil {
		return EnvInfo{}, err
	}

	agent := getAgentInfo(config)

	hypervisor := getHypervisorInfo(config)

	image := ImageInfo{
		Path: config.HypervisorConfig.ImagePath,
	}

	kernel := KernelInfo{
		Path:       config.HypervisorConfig.KernelPath,
		Parameters: strings.Join(vc.SerializeParams(config.HypervisorConfig.KernelParams, "="), " "),
	}

	env = EnvInfo{
		Meta:       meta,
		Runtime:    runtime,
		Hypervisor: hypervisor,
		Image:      image,
		Kernel:     kernel,
		Proxy:      proxy,
		Shim:       shim,
		Agent:      agent,
		Host:       host,
	}

	return env, nil
}

func showSettings(env EnvInfo, file *os.File) error {
	encoder := toml.NewEncoder(file)

	err := encoder.Encode(env)
	if err != nil {
		return err
	}

	return nil
}

func handleSettings(file *os.File, metadata map[string]interface{}) error {
	if file == nil {
		return errors.New("Invalid output file specified")
	}

	configFile, ok := metadata["configFile"].(string)
	if !ok {
		return errors.New("cannot determine config file")
	}

	runtimeConfig, ok := metadata["runtimeConfig"].(oci.RuntimeConfig)
	if !ok {
		return errors.New("cannot determine runtime config")
	}

	env, err := getEnvInfo(configFile, runtimeConfig)
	if err != nil {
		return err
	}

	return showSettings(env, file)
}

var kataEnvCLICommand = cli.Command{
	Name:  envCmd,
	Usage: "display settings",
	Action: func(context *cli.Context) error {
		return handleSettings(defaultOutputFile, context.App.Metadata)
	},
}
