// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"fmt"
	"os"
	"runtime"
	"strconv"
	"strings"
)

// HypervisorType describes an hypervisor type.
type HypervisorType string

const (
	// QemuHypervisor is the QEMU hypervisor.
	QemuHypervisor HypervisorType = "qemu"

	// MockHypervisor is a mock hypervisor for testing purposes
	MockHypervisor HypervisorType = "mock"
)

const (
	procMemInfo = "/proc/meminfo"
	procCPUInfo = "/proc/cpuinfo"
)

const (
	defaultVCPUs = 1
	// 2 GiB
	defaultMemSzMiB = 2048

	defaultBridges = 1

	defaultBlockDriver = VirtioSCSI
)

// In some architectures the maximum number of vCPUs depends on the number of physical cores.
var defaultMaxQemuVCPUs = MaxQemuVCPUs()

// deviceType describes a virtualized device type.
type deviceType int

const (
	// ImgDev is the image device type.
	imgDev deviceType = iota

	// FsDev is the filesystem device type.
	fsDev

	// NetDev is the network device type.
	netDev

	// SerialDev is the serial device type.
	serialDev // nolint: varcheck,unused

	// BlockDev is the block device type.
	blockDev

	// ConsoleDev is the console device type.
	consoleDev // nolint: varcheck,unused

	// SerialPortDev is the serial port device type.
	serialPortDev

	// VFIODevice is VFIO device type
	vfioDev

	// vhostuserDev is a Vhost-user device type
	vhostuserDev

	// CPUDevice is CPU device type
	cpuDev

	// memoryDevice is memory device type
	memoryDev
)

type memoryDevice struct {
	slot   int
	sizeMB int
}

// Set sets an hypervisor type based on the input string.
func (hType *HypervisorType) Set(value string) error {
	switch value {
	case "qemu":
		*hType = QemuHypervisor
		return nil
	case "mock":
		*hType = MockHypervisor
		return nil
	default:
		return fmt.Errorf("Unknown hypervisor type %s", value)
	}
}

// String converts an hypervisor type to a string.
func (hType *HypervisorType) String() string {
	switch *hType {
	case QemuHypervisor:
		return string(QemuHypervisor)
	case MockHypervisor:
		return string(MockHypervisor)
	default:
		return ""
	}
}

// newHypervisor returns an hypervisor from and hypervisor type.
func newHypervisor(hType HypervisorType) (hypervisor, error) {
	switch hType {
	case QemuHypervisor:
		return &qemu{}, nil
	case MockHypervisor:
		return &mockHypervisor{}, nil
	default:
		return nil, fmt.Errorf("Unknown hypervisor type %s", hType)
	}
}

// Param is a key/value representation for hypervisor and kernel parameters.
type Param struct {
	Key   string
	Value string
}

// HypervisorConfig is the hypervisor configuration.
type HypervisorConfig struct {
	// KernelParams are additional guest kernel parameters.
	KernelParams []Param

	// HypervisorParams are additional hypervisor parameters.
	HypervisorParams []Param

	// KernelPath is the guest kernel host path.
	KernelPath string

	// ImagePath is the guest image host path.
	ImagePath string

	// InitrdPath is the guest initrd image host path.
	// ImagePath and InitrdPath cannot be set at the same time.
	InitrdPath string

	// FirmwarePath is the bios host path
	FirmwarePath string

	// MachineAccelerators are machine specific accelerators
	MachineAccelerators string

	// HypervisorPath is the hypervisor executable host path.
	HypervisorPath string

	// BlockDeviceDriver specifies the driver to be used for block device
	// either VirtioSCSI or VirtioBlock with the default driver being defaultBlockDriver
	BlockDeviceDriver string

	// HypervisorMachineType specifies the type of machine being
	// emulated.
	HypervisorMachineType string

	// customAssets is a map of assets.
	// Each value in that map takes precedence over the configured assets.
	// For example, if there is a value for the "kernel" key in this map,
	// it will be used for the sandbox's kernel path instead of KernelPath.
	customAssets map[assetType]*asset

	// DefaultVCPUs specifies default number of vCPUs for the VM.
	DefaultVCPUs uint32

	//DefaultMaxVCPUs specifies the maximum number of vCPUs for the VM.
	DefaultMaxVCPUs uint32

	// DefaultMem specifies default memory size in MiB for the VM.
	// Sandbox configuration VMConfig.Memory overwrites this.
	DefaultMemSz uint32

	// DefaultBridges specifies default number of bridges for the VM.
	// Bridges can be used to hot plug devices
	DefaultBridges uint32

	// DisableBlockDeviceUse disallows a block device from being used.
	DisableBlockDeviceUse bool

	// EnableIOThreads enables IO to be processed in a separate thread.
	// Supported currently for virtio-scsi driver.
	EnableIOThreads bool

	// Debug changes the default hypervisor and kernel parameters to
	// enable debug output where available.
	Debug bool

	// MemPrealloc specifies if the memory should be pre-allocated
	MemPrealloc bool

	// HugePages specifies if the memory should be pre-allocated from huge pages
	HugePages bool

	// Realtime Used to enable/disable realtime
	Realtime bool

	// Mlock is used to control memory locking when Realtime is enabled
	// Realtime=true and Mlock=false, allows for swapping out of VM memory
	// enabling higher density
	Mlock bool

	// DisableNestingChecks is used to override customizations performed
	// when running on top of another VMM.
	DisableNestingChecks bool

	// Msize9p is used as the msize for 9p shares
	Msize9p uint32
}

func (conf *HypervisorConfig) valid() (bool, error) {
	if conf.KernelPath == "" {
		return false, fmt.Errorf("Missing kernel path")
	}

	if conf.ImagePath == "" && conf.InitrdPath == "" {
		return false, fmt.Errorf("Missing image and initrd path")
	}

	if conf.DefaultVCPUs == 0 {
		conf.DefaultVCPUs = defaultVCPUs
	}

	if conf.DefaultMemSz == 0 {
		conf.DefaultMemSz = defaultMemSzMiB
	}

	if conf.DefaultBridges == 0 {
		conf.DefaultBridges = defaultBridges
	}

	if conf.BlockDeviceDriver == "" {
		conf.BlockDeviceDriver = defaultBlockDriver
	}

	if conf.DefaultMaxVCPUs == 0 {
		conf.DefaultMaxVCPUs = defaultMaxQemuVCPUs
	}

	if conf.Msize9p == 0 {
		conf.Msize9p = defaultMsize9p
	}

	return true, nil
}

// AddKernelParam allows the addition of new kernel parameters to an existing
// hypervisor configuration.
func (conf *HypervisorConfig) AddKernelParam(p Param) error {
	if p.Key == "" {
		return fmt.Errorf("Empty kernel parameter")
	}

	conf.KernelParams = append(conf.KernelParams, p)

	return nil
}

func (conf *HypervisorConfig) addCustomAsset(a *asset) error {
	if a == nil || a.path == "" {
		// We did not get a custom asset, we will use the default one.
		return nil
	}

	if !a.valid() {
		return fmt.Errorf("Invalid %s at %s", a.kind, a.path)
	}

	virtLog.Debugf("Using custom %v asset %s", a.kind, a.path)

	if conf.customAssets == nil {
		conf.customAssets = make(map[assetType]*asset)
	}

	conf.customAssets[a.kind] = a

	return nil
}

func (conf *HypervisorConfig) assetPath(t assetType) (string, error) {
	// Custom assets take precedence over the configured ones
	a, ok := conf.customAssets[t]
	if ok {
		return a.path, nil
	}

	// We could not find a custom asset for the given type, let's
	// fall back to the configured ones.
	switch t {
	case kernelAsset:
		return conf.KernelPath, nil
	case imageAsset:
		return conf.ImagePath, nil
	case initrdAsset:
		return conf.InitrdPath, nil
	case hypervisorAsset:
		return conf.HypervisorPath, nil
	case firmwareAsset:
		return conf.FirmwarePath, nil
	default:
		return "", fmt.Errorf("Unknown asset type %v", t)
	}
}

func (conf *HypervisorConfig) isCustomAsset(t assetType) bool {
	_, ok := conf.customAssets[t]
	if ok {
		return true
	}

	return false
}

// KernelAssetPath returns the guest kernel path
func (conf *HypervisorConfig) KernelAssetPath() (string, error) {
	return conf.assetPath(kernelAsset)
}

// CustomKernelAsset returns true if the kernel asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomKernelAsset() bool {
	return conf.isCustomAsset(kernelAsset)
}

// ImageAssetPath returns the guest image path
func (conf *HypervisorConfig) ImageAssetPath() (string, error) {
	return conf.assetPath(imageAsset)
}

// CustomImageAsset returns true if the image asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomImageAsset() bool {
	return conf.isCustomAsset(imageAsset)
}

// InitrdAssetPath returns the guest initrd path
func (conf *HypervisorConfig) InitrdAssetPath() (string, error) {
	return conf.assetPath(initrdAsset)
}

// CustomInitrdAsset returns true if the initrd asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomInitrdAsset() bool {
	return conf.isCustomAsset(initrdAsset)
}

// HypervisorAssetPath returns the VM hypervisor path
func (conf *HypervisorConfig) HypervisorAssetPath() (string, error) {
	return conf.assetPath(hypervisorAsset)
}

// CustomHypervisorAsset returns true if the hypervisor asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomHypervisorAsset() bool {
	return conf.isCustomAsset(hypervisorAsset)
}

// FirmwareAssetPath returns the guest firmware path
func (conf *HypervisorConfig) FirmwareAssetPath() (string, error) {
	return conf.assetPath(firmwareAsset)
}

// CustomFirmwareAsset returns true if the firmware asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomFirmwareAsset() bool {
	return conf.isCustomAsset(firmwareAsset)
}

func appendParam(params []Param, parameter string, value string) []Param {
	return append(params, Param{parameter, value})
}

// SerializeParams converts []Param to []string
func SerializeParams(params []Param, delim string) []string {
	var parameters []string

	for _, p := range params {
		if p.Key == "" && p.Value == "" {
			continue
		} else if p.Key == "" {
			parameters = append(parameters, fmt.Sprintf("%s", p.Value))
		} else if p.Value == "" {
			parameters = append(parameters, fmt.Sprintf("%s", p.Key))
		} else if delim == "" {
			parameters = append(parameters, fmt.Sprintf("%s", p.Key))
			parameters = append(parameters, fmt.Sprintf("%s", p.Value))
		} else {
			parameters = append(parameters, fmt.Sprintf("%s%s%s", p.Key, delim, p.Value))
		}
	}

	return parameters
}

// DeserializeParams converts []string to []Param
func DeserializeParams(parameters []string) []Param {
	var params []Param

	for _, param := range parameters {
		if param == "" {
			continue
		}
		p := strings.SplitN(param, "=", 2)
		if len(p) == 2 {
			params = append(params, Param{Key: p[0], Value: p[1]})
		} else {
			params = append(params, Param{Key: p[0], Value: ""})
		}
	}

	return params
}

func getHostMemorySizeKb(memInfoPath string) (uint64, error) {
	f, err := os.Open(memInfoPath)
	if err != nil {
		return 0, err
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		// Expected format: ["MemTotal:", "1234", "kB"]
		parts := strings.Fields(scanner.Text())

		// Sanity checks: Skip malformed entries.
		if len(parts) < 3 || parts[0] != "MemTotal:" || parts[2] != "kB" {
			continue
		}

		sizeKb, err := strconv.ParseUint(parts[1], 0, 64)
		if err != nil {
			continue
		}

		return sizeKb, nil
	}

	// Handle errors that may have occurred during the reading of the file.
	if err := scanner.Err(); err != nil {
		return 0, err
	}

	return 0, fmt.Errorf("unable get MemTotal from %s", memInfoPath)
}

// RunningOnVMM checks if the system is running inside a VM.
func RunningOnVMM(cpuInfoPath string) (bool, error) {
	if runtime.GOARCH == "arm64" || runtime.GOARCH == "ppc64le" {
		virtLog.Info("Unable to know if the system is running inside a VM")
		return false, nil
	}

	flagsField := "flags"

	f, err := os.Open(cpuInfoPath)
	if err != nil {
		return false, err
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		// Expected format: ["flags", ":", ...] or ["flags:", ...]
		fields := strings.Fields(scanner.Text())
		if len(fields) < 2 {
			continue
		}

		if !strings.HasPrefix(fields[0], flagsField) {
			continue
		}

		for _, field := range fields[1:] {
			if field == "hypervisor" {
				return true, nil
			}
		}

		// As long as we have been able to analyze the fields from
		// "flags", there is no reason to check what comes next from
		// /proc/cpuinfo, because we already know we are not running
		// on a VMM.
		return false, nil
	}

	if err := scanner.Err(); err != nil {
		return false, err
	}

	return false, fmt.Errorf("Couldn't find %q from %q output", flagsField, cpuInfoPath)
}

// hypervisor is the virtcontainers hypervisor interface.
// The default hypervisor implementation is Qemu.
type hypervisor interface {
	init(sandbox *Sandbox) error
	createSandbox(sandboxConfig SandboxConfig) error
	startSandbox() error
	waitSandbox(timeout int) error
	stopSandbox() error
	pauseSandbox() error
	resumeSandbox() error
	addDevice(devInfo interface{}, devType deviceType) error
	hotplugAddDevice(devInfo interface{}, devType deviceType) (interface{}, error)
	hotplugRemoveDevice(devInfo interface{}, devType deviceType) (interface{}, error)
	getSandboxConsole(sandboxID string) (string, error)
	capabilities() capabilities
}
