// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"runtime"
	"strconv"
	"strings"

	hv "github.com/kata-containers/kata-containers/src/runtime/pkg/hypervisors"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	"github.com/sirupsen/logrus"
)

// HypervisorType describes an hypervisor type.
type HypervisorType string

type Operation int

const (
	AddDevice Operation = iota
	RemoveDevice
)

const (
	// FirecrackerHypervisor is the FC hypervisor.
	FirecrackerHypervisor HypervisorType = "firecracker"

	// QemuHypervisor is the QEMU hypervisor.
	QemuHypervisor HypervisorType = "qemu"

	// AcrnHypervisor is the ACRN hypervisor.
	AcrnHypervisor HypervisorType = "acrn"

	// ClhHypervisor is the ICH hypervisor.
	ClhHypervisor HypervisorType = "clh"

	// MockHypervisor is a mock hypervisor for testing purposes
	MockHypervisor HypervisorType = "mock"

	procMemInfo = "/proc/meminfo"
	procCPUInfo = "/proc/cpuinfo"

	defaultVCPUs = 1
	// 2 GiB
	defaultMemSzMiB = 2048

	defaultBridges = 1

	defaultBlockDriver = config.VirtioSCSI

	// port numbers below 1024 are called privileged ports. Only a process with
	// CAP_NET_BIND_SERVICE capability may bind to these port numbers.
	vSockPort = 1024

	// Port where the agent will send the logs. Logs are sent through the vsock in cases
	// where the hypervisor has no console.sock, i.e firecracker
	vSockLogsPort = 1025

	// MinHypervisorMemory is the minimum memory required for a VM.
	MinHypervisorMemory = 256
)

var (
	hvLogger = logrus.WithField("source", "virtcontainers/hypervisor")
)

// In some architectures the maximum number of vCPUs depends on the number of physical cores.
var defaultMaxQemuVCPUs = MaxQemuVCPUs()

// agnostic list of kernel root parameters for NVDIMM
var commonNvdimmKernelRootParams = []Param{ //nolint: unused, deadcode, varcheck
	{"root", "/dev/pmem0p1"},
	{"rootflags", "dax,data=ordered,errors=remount-ro ro"},
	{"rootfstype", "ext4"},
}

// agnostic list of kernel root parameters for NVDIMM
var commonNvdimmNoDAXKernelRootParams = []Param{ //nolint: unused, deadcode, varcheck
	{"root", "/dev/pmem0p1"},
	{"rootflags", "data=ordered,errors=remount-ro ro"},
	{"rootfstype", "ext4"},
}

// agnostic list of kernel root parameters for virtio-blk
var commonVirtioblkKernelRootParams = []Param{ //nolint: unused, deadcode, varcheck
	{"root", "/dev/vda1"},
	{"rootflags", "data=ordered,errors=remount-ro ro"},
	{"rootfstype", "ext4"},
}

// DeviceType describes a virtualized device type.
type DeviceType int

const (
	// ImgDev is the image device type.
	ImgDev DeviceType = iota

	// FsDev is the filesystem device type.
	FsDev

	// NetDev is the network device type.
	NetDev

	// BlockDev is the block device type.
	BlockDev

	// SerialPortDev is the serial port device type.
	SerialPortDev

	// VSockPCIDev is the vhost vsock PCI device type.
	VSockPCIDev

	// VFIODevice is VFIO device type
	VfioDev

	// VhostuserDev is a Vhost-user device type
	VhostuserDev

	// CPUDevice is CPU device type
	CpuDev

	// MemoryDev is memory device type
	MemoryDev

	// HybridVirtioVsockDev is a hybrid virtio-vsock device supported
	// only on certain hypervisors, like firecracker.
	HybridVirtioVsockDev
)

type MemoryDevice struct {
	Slot   int
	SizeMB int
	Addr   uint64
	Probe  bool
}

// SetHypervisorLogger sets up a logger for the hypervisor part of this pkg
func SetHypervisorLogger(logger *logrus.Entry) {
	fields := hvLogger.Data
	hvLogger = logger.WithFields(fields)
}

// Set sets an hypervisor type based on the input string.
func (hType *HypervisorType) Set(value string) error {
	switch value {
	case "qemu":
		*hType = QemuHypervisor
		return nil
	case "firecracker":
		*hType = FirecrackerHypervisor
		return nil
	case "acrn":
		*hType = AcrnHypervisor
		return nil
	case "clh":
		*hType = ClhHypervisor
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
	case FirecrackerHypervisor:
		return string(FirecrackerHypervisor)
	case AcrnHypervisor:
		return string(AcrnHypervisor)
	case ClhHypervisor:
		return string(ClhHypervisor)
	case MockHypervisor:
		return string(MockHypervisor)
	default:
		return ""
	}
}

// NewHypervisor returns an hypervisor from a hypervisor type.
func NewHypervisor(hType HypervisorType) (Hypervisor, error) {

	switch hType {
	case QemuHypervisor:
		return &qemu{}, nil
	case FirecrackerHypervisor:
		return &firecracker{}, nil
	case AcrnHypervisor:
		return &Acrn{}, nil
	case ClhHypervisor:
		return &cloudHypervisor{}, nil
	case MockHypervisor:
		return &mockHypervisor{}, nil
	default:
		return nil, fmt.Errorf("Unknown hypervisor type %s", hType)
	}
}

// GetHypervisorSocketTemplate returns the full "template" path to the
// hypervisor socket. If the specified hypervisor doesn't use a socket,
// an empty string is returned.
//
// The returned value is not the actual socket path since this function
// does not create a sandbox. Instead a path is returned with a special
// template value "{ID}" which would be replaced with the real sandbox
// name sandbox creation time.
func GetHypervisorSocketTemplate(hType HypervisorType, config *HypervisorConfig) (string, error) {
	hypervisor, err := NewHypervisor(hType)
	if err != nil {
		return "", err
	}

	if err := hypervisor.setConfig(config); err != nil {
		return "", err
	}

	// Tag that is used to represent the name of a sandbox
	const sandboxID = "{ID}"

	socket, err := hypervisor.GenerateSocket(sandboxID)
	if err != nil {
		return "", err
	}

	var socketPath string

	if hybridVsock, ok := socket.(types.HybridVSock); ok {
		socketPath = hybridVsock.UdsPath
	}

	return socketPath, nil
}

// Param is a key/value representation for hypervisor and kernel parameters.
type Param struct {
	Key   string
	Value string
}

// HypervisorConfig is the hypervisor configuration.
type HypervisorConfig struct {
	// customAssets is a map of assets.
	// Each value in that map takes precedence over the configured assets.
	// For example, if there is a value for the "kernel" key in this map,
	// it will be used for the sandbox's kernel path instead of KernelPath.
	customAssets map[types.AssetType]*types.Asset

	// Supplementary group IDs.
	Groups []uint32

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

	// CPUFeatures are cpu specific features
	CPUFeatures string

	// HypervisorPath is the hypervisor executable host path.
	HypervisorPath string

	// HypervisorCtlPath is the hypervisor ctl executable host path.
	HypervisorCtlPath string

	// JailerPath is the jailer executable host path.
	JailerPath string

	// BlockDeviceDriver specifies the driver to be used for block device
	// either VirtioSCSI or VirtioBlock with the default driver being defaultBlockDriver
	BlockDeviceDriver string

	// HypervisorMachineType specifies the type of machine being
	// emulated.
	HypervisorMachineType string

	// MemoryPath is the memory file path of VM memory. Used when either BootToBeTemplate or
	// BootFromTemplate is true.
	MemoryPath string

	// DevicesStatePath is the VM device state file path. Used when either BootToBeTemplate or
	// BootFromTemplate is true.
	DevicesStatePath string

	// EntropySource is the path to a host source of
	// entropy (/dev/random, /dev/urandom or real hardware RNG device)
	EntropySource string

	// Shared file system type:
	//   - virtio-9p
	//   - virtio-fs (default)
	SharedFS string

	// Path for filesystem sharing
	SharedPath string

	// VirtioFSDaemon is the virtio-fs vhost-user daemon path
	VirtioFSDaemon string

	// VirtioFSCache cache mode for fs version cache or "none"
	VirtioFSCache string

	// File based memory backend root directory
	FileBackedMemRootDir string

	// VhostUserStorePath is the directory path where vhost-user devices
	// related folders, sockets and device nodes should be.
	VhostUserStorePath string

	// GuestCoredumpPath is the path in host for saving guest memory dump
	GuestMemoryDumpPath string

	// GuestHookPath is the path within the VM that will be used for 'drop-in' hooks
	GuestHookPath string

	// VMid is the id of the VM that create the hypervisor if the VM is created by the factory.
	// VMid is "" if the hypervisor is not created by the factory.
	VMid string

	// VMStorePath is the location on disk where VM information will persist
	VMStorePath string

	// VMStorePath is the location on disk where runtime information will persist
	RunStorePath string

	// SELinux label for the VM
	SELinuxProcessLabel string

	// HypervisorPathList is the list of hypervisor paths names allowed in annotations
	HypervisorPathList []string

	// HypervisorCtlPathList is the list of hypervisor control paths names allowed in annotations
	HypervisorCtlPathList []string

	// JailerPathList is the list of jailer paths names allowed in annotations
	JailerPathList []string

	// EntropySourceList is the list of valid entropy sources
	EntropySourceList []string

	// VirtioFSDaemonList is the list of valid virtiofs names for annotations
	VirtioFSDaemonList []string

	// VirtioFSExtraArgs passes options to virtiofsd daemon
	VirtioFSExtraArgs []string

	// Enable annotations by name
	EnableAnnotations []string

	// FileBackedMemRootList is the list of valid root directories values for annotations
	FileBackedMemRootList []string

	// PFlash image paths
	PFlash []string

	// VhostUserStorePathList is the list of valid values for vhost-user paths
	VhostUserStorePathList []string

	// KernelParams are additional guest kernel parameters.
	KernelParams []Param

	// HypervisorParams are additional hypervisor parameters.
	HypervisorParams []Param

	// SGXEPCSize specifies the size in bytes for the EPC Section.
	// Enable SGX. Hardware-based isolation and memory encryption.
	SGXEPCSize int64

	// RxRateLimiterMaxRate is used to control network I/O inbound bandwidth on VM level.
	RxRateLimiterMaxRate uint64

	// TxRateLimiterMaxRate is used to control network I/O outbound bandwidth on VM level.
	TxRateLimiterMaxRate uint64

	// MemOffset specifies memory space for nvdimm device
	MemOffset uint64

	// PCIeRootPort is used to indicate the number of PCIe Root Port devices
	// The PCIe Root Port device is used to hot-plug the PCIe device
	PCIeRootPort uint32

	// NumVCPUs specifies default number of vCPUs for the VM.
	NumVCPUs uint32

	//DefaultMaxVCPUs specifies the maximum number of vCPUs for the VM.
	DefaultMaxVCPUs uint32

	// DefaultMem specifies default memory size in MiB for the VM.
	MemorySize uint32

	// DefaultBridges specifies default number of bridges for the VM.
	// Bridges can be used to hot plug devices
	DefaultBridges uint32

	// Msize9p is used as the msize for 9p shares
	Msize9p uint32

	// MemSlots specifies default memory slots the VM.
	MemSlots uint32

	// VirtioFSCacheSize is the DAX cache size in MiB
	VirtioFSCacheSize uint32

	// User ID.
	Uid uint32

	// Group ID.
	Gid uint32

	// BlockDeviceCacheSet specifies cache-related options will be set to block devices or not.
	BlockDeviceCacheSet bool

	// BlockDeviceCacheDirect specifies cache-related options for block devices.
	// Denotes whether use of O_DIRECT (bypass the host page cache) is enabled.
	BlockDeviceCacheDirect bool

	// BlockDeviceCacheNoflush specifies cache-related options for block devices.
	// Denotes whether flush requests for the device are ignored.
	BlockDeviceCacheNoflush bool

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

	// VirtioMem is used to enable/disable virtio-mem
	VirtioMem bool

	// IOMMU specifies if the VM should have a vIOMMU
	IOMMU bool

	// IOMMUPlatform is used to indicate if IOMMU_PLATFORM is enabled for supported devices
	IOMMUPlatform bool

	// Realtime Used to enable/disable realtime
	Realtime bool

	// Mlock is used to control memory locking when Realtime is enabled
	// Realtime=true and Mlock=false, allows for swapping out of VM memory
	// enabling higher density
	Mlock bool

	// DisableNestingChecks is used to override customizations performed
	// when running on top of another VMM.
	DisableNestingChecks bool

	// DisableImageNvdimm is used to disable guest rootfs image nvdimm devices
	DisableImageNvdimm bool

	// HotplugVFIOOnRootBus is used to indicate if devices need to be hotplugged on the
	// root bus instead of a bridge.
	HotplugVFIOOnRootBus bool

	// GuestMemoryDumpPaging is used to indicate if enable paging
	// for QEMU dump-guest-memory command
	GuestMemoryDumpPaging bool

	// Enable confidential guest support.
	// Enable or disable different hardware features, ranging
	// from memory encryption to both memory and CPU-state encryption and integrity.
	ConfidentialGuest bool

	// Enable prelaunch attestation for confidential guest.
	GuestAttestation bool

	// Location of the Guest Owner Proxy for attestation
	GuestAttestationProxy string

	// Keyset ID for post-attestation secret injection
	GuestAttestationKeyset string

	// BootToBeTemplate used to indicate if the VM is created to be a template VM
	BootToBeTemplate bool

	// BootFromTemplate used to indicate if the VM should be created from a template VM
	BootFromTemplate bool

	// DisableVhostNet is used to indicate if host supports vhost_net
	DisableVhostNet bool

	// EnableVhostUserStore is used to indicate if host supports vhost-user-blk/scsi
	EnableVhostUserStore bool

	// GuestSwap Used to enable/disable swap in the guest
	GuestSwap bool

	// Rootless is used to enable rootless VMM process
	Rootless bool

	// Disable seccomp from the hypervisor process
	DisableSeccomp bool
}

// vcpu mapping from vcpu number to thread number
type VcpuThreadIDs struct {
	vcpus map[int]int
}

func (conf *HypervisorConfig) CheckTemplateConfig() error {
	if conf.BootToBeTemplate && conf.BootFromTemplate {
		return fmt.Errorf("Cannot set both 'to be' and 'from' vm tempate")
	}

	if conf.BootToBeTemplate || conf.BootFromTemplate {
		if conf.MemoryPath == "" {
			return fmt.Errorf("Missing MemoryPath for vm template")
		}

		if conf.BootFromTemplate && conf.DevicesStatePath == "" {
			return fmt.Errorf("Missing DevicesStatePath to Load from vm template")
		}
	}

	return nil
}

func (conf *HypervisorConfig) Valid() error {

	// Kata specific checks. Should be done outside the hypervisor
	if conf.KernelPath == "" {
		return fmt.Errorf("Missing kernel path")
	}

	if conf.ImagePath == "" && conf.InitrdPath == "" {
		return fmt.Errorf("Missing image and initrd path")
	}

	if conf.ImagePath != "" && conf.InitrdPath != "" {
		return fmt.Errorf("Image and initrd path cannot be both set")
	}

	if err := conf.CheckTemplateConfig(); err != nil {
		return err
	}

	if conf.NumVCPUs == 0 {
		conf.NumVCPUs = defaultVCPUs
	}

	if conf.MemorySize == 0 {
		conf.MemorySize = defaultMemSzMiB
	}

	if conf.DefaultBridges == 0 {
		conf.DefaultBridges = defaultBridges
	}

	if conf.BlockDeviceDriver == "" {
		conf.BlockDeviceDriver = defaultBlockDriver
	} else if conf.BlockDeviceDriver == config.VirtioBlock && conf.HypervisorMachineType == "s390-ccw-virtio" {
		conf.BlockDeviceDriver = config.VirtioBlockCCW
	}

	if conf.DefaultMaxVCPUs == 0 || conf.DefaultMaxVCPUs > defaultMaxQemuVCPUs {
		conf.DefaultMaxVCPUs = defaultMaxQemuVCPUs
	}

	if conf.Msize9p == 0 && conf.SharedFS != config.VirtioFS {
		conf.Msize9p = defaultMsize9p
	}

	return nil
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

func (conf *HypervisorConfig) AddCustomAsset(a *types.Asset) error {
	if a == nil || a.Path() == "" {
		// We did not get a custom asset, we will use the default one.
		return nil
	}

	if !a.Valid() {
		return fmt.Errorf("Invalid %s at %s", a.Type(), a.Path())
	}

	hvLogger.Debugf("Using custom %v asset %s", a.Type(), a.Path())

	if conf.customAssets == nil {
		conf.customAssets = make(map[types.AssetType]*types.Asset)
	}

	conf.customAssets[a.Type()] = a

	return nil
}

func (conf *HypervisorConfig) assetPath(t types.AssetType) (string, error) {
	// Custom assets take precedence over the configured ones
	a, ok := conf.customAssets[t]
	if ok {
		return a.Path(), nil
	}

	// We could not find a custom asset for the given type, let's
	// fall back to the configured ones.
	switch t {
	case types.KernelAsset:
		return conf.KernelPath, nil
	case types.ImageAsset:
		return conf.ImagePath, nil
	case types.InitrdAsset:
		return conf.InitrdPath, nil
	case types.HypervisorAsset:
		return conf.HypervisorPath, nil
	case types.HypervisorCtlAsset:
		return conf.HypervisorCtlPath, nil
	case types.JailerAsset:
		return conf.JailerPath, nil
	case types.FirmwareAsset:
		return conf.FirmwarePath, nil
	default:
		return "", fmt.Errorf("Unknown asset type %v", t)
	}
}

func (conf *HypervisorConfig) isCustomAsset(t types.AssetType) bool {
	_, ok := conf.customAssets[t]
	return ok
}

// KernelAssetPath returns the guest kernel path
func (conf *HypervisorConfig) KernelAssetPath() (string, error) {
	return conf.assetPath(types.KernelAsset)
}

// CustomKernelAsset returns true if the kernel asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomKernelAsset() bool {
	return conf.isCustomAsset(types.KernelAsset)
}

// ImageAssetPath returns the guest image path
func (conf *HypervisorConfig) ImageAssetPath() (string, error) {
	return conf.assetPath(types.ImageAsset)
}

// CustomImageAsset returns true if the image asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomImageAsset() bool {
	return conf.isCustomAsset(types.ImageAsset)
}

// InitrdAssetPath returns the guest initrd path
func (conf *HypervisorConfig) InitrdAssetPath() (string, error) {
	return conf.assetPath(types.InitrdAsset)
}

// CustomInitrdAsset returns true if the initrd asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomInitrdAsset() bool {
	return conf.isCustomAsset(types.InitrdAsset)
}

// HypervisorAssetPath returns the VM hypervisor path
func (conf *HypervisorConfig) HypervisorAssetPath() (string, error) {
	return conf.assetPath(types.HypervisorAsset)
}

func (conf *HypervisorConfig) IfPVPanicEnabled() bool {
	return conf.GuestMemoryDumpPath != ""
}

// HypervisorCtlAssetPath returns the VM hypervisor ctl path
func (conf *HypervisorConfig) HypervisorCtlAssetPath() (string, error) {
	return conf.assetPath(types.HypervisorCtlAsset)
}

// CustomHypervisorAsset returns true if the hypervisor asset is a custom one, false otherwise.
func (conf *HypervisorConfig) CustomHypervisorAsset() bool {
	return conf.isCustomAsset(types.HypervisorAsset)
}

// FirmwareAssetPath returns the guest firmware path
func (conf *HypervisorConfig) FirmwareAssetPath() (string, error) {
	return conf.assetPath(types.FirmwareAsset)
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
			parameters = append(parameters, fmt.Sprint(p.Value))
		} else if p.Value == "" {
			parameters = append(parameters, fmt.Sprint(p.Key))
		} else if delim == "" {
			parameters = append(parameters, fmt.Sprint(p.Key))
			parameters = append(parameters, fmt.Sprint(p.Value))
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

func GetHostMemorySizeKb(memInfoPath string) (uint64, error) {
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

// CheckCmdline checks whether an option or parameter is present in the kernel command line.
// Search is case-insensitive.
// Takes path to file that contains the kernel command line, desired option, and permitted values
// (empty values to Check for options).
func CheckCmdline(kernelCmdlinePath, searchParam string, searchValues []string) (bool, error) {
	f, err := os.Open(kernelCmdlinePath)
	if err != nil {
		return false, err
	}
	defer f.Close()

	// Create Check function -- either Check for verbatim option
	// or Check for parameter and permitted values
	var check func(string, string, []string) bool
	if len(searchValues) == 0 {
		check = func(option, searchParam string, _ []string) bool {
			return strings.EqualFold(option, searchParam)
		}
	} else {
		check = func(param, searchParam string, searchValues []string) bool {
			// split parameter and value
			split := strings.SplitN(param, "=", 2)
			if len(split) < 2 || split[0] != searchParam {
				return false
			}
			for _, value := range searchValues {
				if strings.EqualFold(value, split[1]) {
					return true
				}
			}
			return false
		}
	}

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		for _, field := range strings.Fields(scanner.Text()) {
			if check(field, searchParam, searchValues) {
				return true, nil
			}
		}
	}
	return false, err
}

func CPUFlags(cpuInfoPath string) (map[string]bool, error) {
	flagsField := "flags"

	f, err := os.Open(cpuInfoPath)
	if err != nil {
		return map[string]bool{}, err
	}
	defer f.Close()

	flags := make(map[string]bool)
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
			flags[field] = true
		}

		return flags, nil
	}

	if err := scanner.Err(); err != nil {
		return map[string]bool{}, err
	}

	return map[string]bool{}, fmt.Errorf("Couldn't find %q from %q output", flagsField, cpuInfoPath)
}

// RunningOnVMM checks if the system is running inside a VM.
func RunningOnVMM(cpuInfoPath string) (bool, error) {
	if runtime.GOARCH == "amd64" {
		flags, err := CPUFlags(cpuInfoPath)
		if err != nil {
			return false, err
		}
		return flags["hypervisor"], nil
	}

	hvLogger.WithField("arch", runtime.GOARCH).Info("Unable to know if the system is running inside a VM")
	return false, nil
}

func GetHypervisorPid(h Hypervisor) int {
	pids := h.GetPids()
	if len(pids) == 0 {
		return 0
	}
	return pids[0]
}

func generateVMSocket(id string, vmStogarePath string) (interface{}, error) {
	vhostFd, contextID, err := utils.FindContextID()
	if err != nil {
		return nil, err
	}

	return types.VSock{
		VhostFd:   vhostFd,
		ContextID: contextID,
		Port:      uint32(vSockPort),
	}, nil
}

// hypervisor is the virtcontainers hypervisor interface.
// The default hypervisor implementation is Qemu.
type Hypervisor interface {
	CreateVM(ctx context.Context, id string, networkNS NetworkNamespace, hypervisorConfig *HypervisorConfig) error
	StartVM(ctx context.Context, timeout int) error

	// If wait is set, don't actively stop the sandbox:
	// just perform cleanup.
	StopVM(ctx context.Context, waitOnly bool) error
	PauseVM(ctx context.Context) error
	SaveVM() error
	ResumeVM(ctx context.Context) error
	AddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) error
	HotplugAddDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error)
	HotplugRemoveDevice(ctx context.Context, devInfo interface{}, devType DeviceType) (interface{}, error)
	ResizeMemory(ctx context.Context, memMB uint32, memoryBlockSizeMB uint32, probe bool) (uint32, MemoryDevice, error)
	ResizeVCPUs(ctx context.Context, vcpus uint32) (uint32, uint32, error)
	GetVMConsole(ctx context.Context, sandboxID string) (string, string, error)
	Disconnect(ctx context.Context)
	Capabilities(ctx context.Context) types.Capabilities
	HypervisorConfig() HypervisorConfig
	GetThreadIDs(ctx context.Context) (VcpuThreadIDs, error)
	Cleanup(ctx context.Context) error
	// getPids returns a slice of hypervisor related process ids.
	// The hypervisor pid must be put at index 0.
	setConfig(config *HypervisorConfig) error
	GetPids() []int
	GetVirtioFsPid() *int
	fromGrpc(ctx context.Context, hypervisorConfig *HypervisorConfig, j []byte) error
	toGrpc(ctx context.Context) ([]byte, error)
	Check() error

	Save() hv.HypervisorState
	Load(hv.HypervisorState)

	// generate the socket to communicate the host and guest
	GenerateSocket(id string) (interface{}, error)

	// check if hypervisor supports built-in rate limiter.
	IsRateLimiterBuiltin() bool
}
