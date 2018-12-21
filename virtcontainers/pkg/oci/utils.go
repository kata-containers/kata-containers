// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package oci

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io/ioutil"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"

	criContainerdAnnotations "github.com/containerd/cri-containerd/pkg/annotations"
	crioAnnotations "github.com/kubernetes-incubator/cri-o/pkg/annotations"
	spec "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/sirupsen/logrus"

	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
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

// CompatOCIProcess is a structure inheriting from spec.Process defined
// in runtime-spec/specs-go package. The goal is to be compatible with
// both v1.0.0-rc4 and v1.0.0-rc5 since the latter introduced a change
// about the type of the Capabilities field.
// Refer to: https://github.com/opencontainers/runtime-spec/commit/37391fb
type CompatOCIProcess struct {
	spec.Process
	Capabilities interface{} `json:"capabilities,omitempty" platform:"linux"`
}

// CompatOCISpec is a structure inheriting from spec.Spec defined
// in runtime-spec/specs-go package. It relies on the CompatOCIProcess
// structure declared above, in order to be compatible with both
// v1.0.0-rc4 and v1.0.0-rc5.
// Refer to: https://github.com/opencontainers/runtime-spec/commit/37391fb
type CompatOCISpec struct {
	spec.Spec
	Process *CompatOCIProcess `json:"process,omitempty"`
}

// FactoryConfig is a structure to set the VM factory configuration.
type FactoryConfig struct {
	// Template enables VM templating support in VM factory.
	Template bool
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

func cmdEnvs(spec CompatOCISpec, envs []types.EnvVar) []types.EnvVar {
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

func newMount(m spec.Mount) vc.Mount {
	return vc.Mount{
		Source:      m.Source,
		Destination: m.Destination,
		Type:        m.Type,
		Options:     m.Options,
	}
}

func containerMounts(spec CompatOCISpec) []vc.Mount {
	ociMounts := spec.Spec.Mounts

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

func newLinuxDeviceInfo(d spec.LinuxDevice) (*config.DeviceInfo, error) {
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

func containerDeviceInfos(spec CompatOCISpec) ([]config.DeviceInfo, error) {
	ociLinuxDevices := spec.Spec.Linux.Devices

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

func containerCapabilities(s CompatOCISpec) (types.LinuxCapabilities, error) {
	capabilities := s.Process.Capabilities
	var c types.LinuxCapabilities

	// In spec v1.0.0-rc4, capabilities was a list of strings. This was changed
	// to an object with v1.0.0-rc5.
	// Check for the interface type to support both the versions.
	switch caps := capabilities.(type) {
	case map[string]interface{}:
		for key, value := range caps {
			switch val := value.(type) {
			case []interface{}:
				var list []string

				for _, str := range val {
					list = append(list, str.(string))
				}

				switch key {
				case "bounding":
					c.Bounding = list
				case "effective":
					c.Effective = list
				case "inheritable":
					c.Inheritable = list
				case "ambient":
					c.Ambient = list
				case "permitted":
					c.Permitted = list
				}

			default:
				return c, fmt.Errorf("Unexpected format for capabilities: %v", caps)
			}
		}
	case []interface{}:
		var list []string
		for _, str := range caps {
			list = append(list, str.(string))
		}

		c = types.LinuxCapabilities{
			Bounding:    list,
			Effective:   list,
			Inheritable: list,
			Ambient:     list,
			Permitted:   list,
		}
	case nil:
		ociLog.Debug("Empty capabilities have been passed")
		return c, nil
	default:
		return c, fmt.Errorf("Unexpected format for capabilities: %v", caps)
	}

	return c, nil
}

// ContainerCapabilities return a LinuxCapabilities for virtcontainer
func ContainerCapabilities(s CompatOCISpec) (types.LinuxCapabilities, error) {
	if s.Process == nil {
		return types.LinuxCapabilities{}, fmt.Errorf("ContainerCapabilities, Process is nil")
	}
	return containerCapabilities(s)
}

func networkConfig(ocispec CompatOCISpec, config RuntimeConfig) (vc.NetworkConfig, error) {
	linux := ocispec.Linux
	if linux == nil {
		return vc.NetworkConfig{}, ErrNoLinux
	}

	var netConf vc.NetworkConfig

	for _, n := range linux.Namespaces {
		if n.Type != spec.NetworkNamespace {
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

// getConfigPath returns the full config path from the bundle
// path provided.
func getConfigPath(bundlePath string) string {
	return filepath.Join(bundlePath, "config.json")
}

// ParseConfigJSON unmarshals the config.json file.
func ParseConfigJSON(bundlePath string) (CompatOCISpec, error) {
	configPath := getConfigPath(bundlePath)
	ociLog.Debugf("converting %s", configPath)

	configByte, err := ioutil.ReadFile(configPath)
	if err != nil {
		return CompatOCISpec{}, err
	}

	var ocispec CompatOCISpec
	if err := json.Unmarshal(configByte, &ocispec); err != nil {
		return CompatOCISpec{}, err
	}
	caps, err := ContainerCapabilities(ocispec)
	if err != nil {
		return CompatOCISpec{}, err
	}
	ocispec.Process.Capabilities = caps

	return ocispec, nil
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
func (spec *CompatOCISpec) ContainerType() (vc.ContainerType, error) {
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

// SandboxID determines the sandbox ID related to an OCI configuration. This function
// is expected to be called only when the container type is "PodContainer".
func (spec *CompatOCISpec) SandboxID() (string, error) {
	for _, key := range CRISandboxNameKeyList {
		sandboxID, ok := spec.Annotations[key]
		if ok {
			return sandboxID, nil
		}
	}

	return "", fmt.Errorf("Could not find sandbox ID")
}

func addAssetAnnotations(ocispec CompatOCISpec, config *vc.SandboxConfig) {
	assetAnnotations := []string{
		vcAnnotations.KernelPath,
		vcAnnotations.ImagePath,
		vcAnnotations.InitrdPath,
		vcAnnotations.KernelHash,
		vcAnnotations.ImageHash,
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

// SandboxConfig converts an OCI compatible runtime configuration file
// to a virtcontainers sandbox configuration structure.
func SandboxConfig(ocispec CompatOCISpec, runtime RuntimeConfig, bundlePath, cid, console string, detach, systemdCgroup bool) (vc.SandboxConfig, error) {
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

	ociSpecJSON, err := json.Marshal(ocispec)
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

		NetworkModel:  vc.DefaultNetworkModel,
		NetworkConfig: networkConfig,

		Containers: []vc.ContainerConfig{containerConfig},

		Annotations: map[string]string{
			vcAnnotations.ConfigJSONKey: string(ociSpecJSON),
			vcAnnotations.BundlePathKey: bundlePath,
		},

		ShmSize: shmSize,

		SystemdCgroup: systemdCgroup,

		DisableGuestSeccomp: runtime.DisableGuestSeccomp,
	}

	addAssetAnnotations(ocispec, &sandboxConfig)

	return sandboxConfig, nil
}

// ContainerConfig converts an OCI compatible runtime configuration
// file to a virtcontainers container configuration structure.
func ContainerConfig(ocispec CompatOCISpec, bundlePath, cid, console string, detach bool) (vc.ContainerConfig, error) {

	ociSpecJSON, err := json.Marshal(ocispec)
	if err != nil {
		return vc.ContainerConfig{}, err
	}

	rootfs := ocispec.Root.Path
	if !filepath.IsAbs(rootfs) {
		rootfs = filepath.Join(bundlePath, ocispec.Root.Path)
	}
	ociLog.Debugf("container rootfs: %s", rootfs)

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
		caps, ok := ocispec.Process.Capabilities.(types.LinuxCapabilities)
		if !ok {
			return vc.ContainerConfig{}, fmt.Errorf("Unexpected format for capabilities: %v", ocispec.Process.Capabilities)
		}
		cmd.Capabilities = caps
	}

	containerConfig := vc.ContainerConfig{
		ID:             cid,
		RootFs:         rootfs,
		ReadonlyRootfs: ocispec.Spec.Root.Readonly,
		Cmd:            cmd,
		Annotations: map[string]string{
			vcAnnotations.ConfigJSONKey: string(ociSpecJSON),
			vcAnnotations.BundlePathKey: bundlePath,
		},
		Mounts:      containerMounts(ocispec),
		DeviceInfos: deviceInfos,
		Resources:   *ocispec.Linux.Resources,
	}

	cType, err := ocispec.ContainerType()
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
func StatusToOCIState(status vc.ContainerStatus) spec.State {
	return spec.State{
		Version:     spec.Version,
		ID:          status.ID,
		Status:      StateToOCIState(status.State),
		Pid:         status.PID,
		Bundle:      status.Annotations[vcAnnotations.BundlePathKey],
		Annotations: status.Annotations,
	}
}

// StateToOCIState translates a virtcontainers container state into an OCI one.
func StateToOCIState(state types.State) string {
	switch state.State {
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
func GetOCIConfig(status vc.ContainerStatus) (CompatOCISpec, error) {
	ociConfigStr, ok := status.Annotations[vcAnnotations.ConfigJSONKey]
	if !ok {
		return CompatOCISpec{}, fmt.Errorf("Annotation[%s] not found", vcAnnotations.ConfigJSONKey)
	}

	var ociSpec CompatOCISpec
	if err := json.Unmarshal([]byte(ociConfigStr), &ociSpec); err != nil {
		return CompatOCISpec{}, err
	}

	return ociSpec, nil
}
