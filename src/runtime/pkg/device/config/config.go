// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/container-orchestrated-devices/container-device-interface/pkg/cdi"
	"github.com/go-ini/ini"
	vcTypes "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/opencontainers/runtime-spec/specs-go"
	"golang.org/x/sys/unix"
)

// DeviceType indicates device type
type DeviceType string

const (
	// DeviceVFIO is the VFIO device type
	DeviceVFIO DeviceType = "vfio"

	// DeviceBlock is the block device type
	DeviceBlock DeviceType = "block"

	// DeviceGeneric is a generic device type
	DeviceGeneric DeviceType = "generic"

	//VhostUserSCSI - SCSI based vhost-user type
	VhostUserSCSI = "vhost-user-scsi-pci"

	//VhostUserNet - net based vhost-user type
	VhostUserNet = "virtio-net-pci"

	//VhostUserBlk represents a block vhostuser device type
	VhostUserBlk = "vhost-user-blk-pci"

	//VhostUserFS represents a virtio-fs vhostuser device type
	VhostUserFS = "vhost-user-fs-pci"
)

const (
	// VirtioMmio means use virtio-mmio for mmio based drives
	VirtioMmio = "virtio-mmio"

	// VirtioBlock means use virtio-blk for hotplugging drives
	VirtioBlock = "virtio-blk"

	// VirtioBlockCCW means use virtio-blk-ccw for hotplugging drives
	VirtioBlockCCW = "virtio-blk-ccw"

	// VirtioSCSI means use virtio-scsi for hotplugging drives
	VirtioSCSI = "virtio-scsi"

	// Nvdimm means use nvdimm for hotplugging drives
	Nvdimm = "nvdimm"
)

const (
	// AIOThreads is the pthread asynchronous I/O implementation.
	AIOThreads = "threads"

	// AIONative is the native Linux AIO implementation
	AIONative = "native"

	// AIOUring is the Linux io_uring I/O implementation
	AIOIOUring = "io_uring"
)

const (
	// Virtio9P means use virtio-9p for the shared file system
	Virtio9P = "virtio-9p"

	// VirtioFS means use virtio-fs for the shared file system
	VirtioFS = "virtio-fs"

	// VirtioFSNydus means use nydus for the shared file system
	VirtioFSNydus = "virtio-fs-nydus"

	// NoSharedFS means *no* shared file system solution will be used
	// and files will be copied into the guest system.
	//
	// WARNING: This should be carefully used, and only used in very few
	// specific cases, as any update to the mount will *NOT* be reflected
	// during the lifecycle of the pod, causing issues with rotation of
	// secrets, certs, or configurations via kubernetes objects like
	// configMaps or secrets, as those will be copied into the guest at
	// *pod* *creation* *time*.
	NoSharedFS = "none"
)

const (
	// Define the string key for DriverOptions in DeviceInfo struct
	FsTypeOpt      = "fstype"
	BlockDriverOpt = "block-driver"

	VhostUserReconnectTimeOutOpt = "vhost-user-reconnect-timeout"
)

const (
	// The OCI spec requires the major-minor number to be provided for a
	// device. We have chosen the below major numbers to represent
	// vhost-user devices.
	VhostUserBlkMajor  = 241
	VhostUserSCSIMajor = 242
)

const (

	// The timeout for reconnecting on non-server sockets when the remote end
	// goes away.
	// qemu will delay this many seconds and then attempt to reconnect.  Zero
	// disables reconnecting, and is the default.
	DefaultVhostUserReconnectTimeOut = 0
)

// Defining these as a variable instead of a const, to allow
// overriding this in the tests.

// SysDevPrefix is static string of /sys/dev
var SysDevPrefix = "/sys/dev"

// SysIOMMUGroupPath is static string of /sys/kernel/iommu_groups
var SysIOMMUGroupPath = "/sys/kernel/iommu_groups"

// SysBusPciDevicesPath is static string of /sys/bus/pci/devices
var SysBusPciDevicesPath = "/sys/bus/pci/devices"

var getSysDevPath = getSysDevPathImpl

// PCIePortBusPrefix gives us the correct bus nameing dependeing on the port
// used to hot(cold)-plug the device
type PCIePortBusPrefix string

const (
	PCIeRootPortPrefix              PCIePortBusPrefix = "rp"
	PCIeSwitchPortPrefix            PCIePortBusPrefix = "sw"
	PCIeSwitchUpstreamPortPrefix    PCIePortBusPrefix = "swup"
	PCIeSwitchhDownstreamPortPrefix PCIePortBusPrefix = "swdp"
	PCIBridgePortPrefix             PCIePortBusPrefix = "bp"
)

func (p PCIePortBusPrefix) String() string {
	switch p {
	case PCIeRootPortPrefix:
		fallthrough
	case PCIeSwitchPortPrefix:
		fallthrough
	case PCIeSwitchUpstreamPortPrefix:
		fallthrough
	case PCIeSwitchhDownstreamPortPrefix:
		fallthrough
	case PCIBridgePortPrefix:
		return string(p)
	}
	return fmt.Sprintf("<unknown PCIePortBusPrefix: %s>", string(p))
}

// PCIePort distinguish only between root and switch port
type PCIePort string

const (
	// RootPort attach VFIO devices to a root-port
	RootPort PCIePort = "root-port"
	// SwitchPort attach VFIO devices to a switch-port
	SwitchPort = "switch-port"
	// BridgePort is the default
	BridgePort = "bridge-port"
	// NoPort is for disabling VFIO hotplug/coldplug
	NoPort = "no-port"
	// InvalidPort is for invalid port
	InvalidPort = "invalid-port"
)

func (p PCIePort) String() string {
	switch p {
	case RootPort:
		fallthrough
	case SwitchPort:
		fallthrough
	case BridgePort:
		fallthrough
	case NoPort:
		fallthrough
	case InvalidPort:
		return string(p)
	}
	return fmt.Sprintf("<unknown PCIePort: %s>", string(p))
}

var PCIePortPrefixMapping = map[PCIePort]PCIePortBusPrefix{
	RootPort:   PCIeRootPortPrefix,
	SwitchPort: PCIeSwitchhDownstreamPortPrefix,
	BridgePort: PCIBridgePortPrefix,
}

func (p PCIePort) Invalid() bool {
	switch p {
	case RootPort:
		fallthrough
	case SwitchPort:
		fallthrough
	case BridgePort:
		fallthrough
	case NoPort:
		return false
	}
	return true
}

func (p PCIePort) Valid() bool {
	switch p {
	case RootPort:
		fallthrough
	case SwitchPort:
		fallthrough
	case BridgePort:
		fallthrough
	case NoPort:
		return true
	}
	return false
}

var (
	// Each of this structures keeps track of the devices attached to the
	// different types of PCI ports. We can deduces the Bus number from it
	// and eliminate duplicates being assigned.
	PCIeDevicesPerPort = map[PCIePort][]VFIODev{}
)

// DeviceInfo is an embedded type that contains device data common to all types of devices.
type DeviceInfo struct {
	// DriverOptions is specific options for each device driver
	// for example, for BlockDevice, we can set DriverOptions["block-driver"]="virtio-blk"
	DriverOptions map[string]string

	// Hostpath is device path on host
	HostPath string

	// ContainerPath is device path inside container
	ContainerPath string `json:"-"`

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// ID for the device that is passed to the hypervisor.
	ID string

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// id of the device owner.
	UID uint32

	// id of the device group.
	GID uint32

	// Pmem enabled persistent memory. Use HostPath as backing file
	// for a nvdimm device in the guest.
	Pmem bool

	// If applicable, should this device be considered RO
	ReadOnly bool

	// ColdPlug specifies whether the device must be cold plugged (true)
	// or hot plugged (false).
	ColdPlug bool

	// Specifies the PCIe port type to which the device is attached
	Port PCIePort
}

// BlockDrive represents a block storage drive which may be used in case the storage
// driver has an underlying block storage device.
type BlockDrive struct {
	// File is the path to the disk-image/device which will be used with this drive
	File string

	// Format of the drive
	Format string

	// ID is used to identify this drive in the hypervisor options.
	ID string

	// MmioAddr is used to identify the slot at which the drive is attached (order?).
	MmioAddr string

	// SCSI Address of the block device, in case the device is attached using SCSI driver
	// SCSI address is in the format SCSI-Id:LUN
	SCSIAddr string

	// NvdimmID is the nvdimm id inside the VM
	NvdimmID string

	// VirtPath at which the device appears inside the VM, outside of the container mount namespace
	VirtPath string

	// DevNo identifies the css bus id for virtio-blk-ccw
	DevNo string

	// PCIPath is the PCI path used to identify the slot at which the drive is attached.
	PCIPath vcTypes.PciPath

	// Index assigned to the drive. In case of virtio-scsi, this is used as SCSI LUN index
	Index int

	// ShareRW enables multiple qemu instances to share the File
	ShareRW bool

	// ReadOnly sets the device file readonly
	ReadOnly bool

	// Pmem enables persistent memory. Use File as backing file
	// for a nvdimm device in the guest
	Pmem bool

	// This block device is for swap
	Swap bool
}

// VFIOMode indicates e behaviour mode for handling devices in the VM
type VFIOModeType uint32

const (
	// VFIOModeVFIO specifies OCI compliant behaviour: VFIO
	// devices specified to Kata appear as VFIO devices within the
	// container
	VFIOModeVFIO VFIOModeType = iota

	// VFIOModeGuestKernel specifies Kata-specific behaviour
	// useful in certain cases: VFIO devices specified to Kata are
	// bound to whatever driver in the VM will take them.  This
	// requires specialized containers expecting this behaviour to
	// locate and use the devices
	VFIOModeGuestKernel
)

const (
	vfioModeVfioStr        = "vfio"
	vfioModeGuestKernelStr = "guest-kernel"
)

func (m *VFIOModeType) VFIOSetMode(modeName string) error {
	switch modeName {
	case vfioModeVfioStr:
		*m = VFIOModeVFIO
		return nil
	case vfioModeGuestKernelStr:
		*m = VFIOModeGuestKernel
		return nil
	}
	return fmt.Errorf("Unknown VFIO mode %s", modeName)
}

// VFIODeviceType indicates VFIO device type
type VFIODeviceType uint32

const (
	// VFIODeviceErrorType is the error type of VFIO device
	VFIODeviceErrorType VFIODeviceType = iota

	// VFIOPCIDeviceNormalType is a normal VFIO PCI device type
	VFIOPCIDeviceNormalType

	// VFIOPCIDeviceMediatedType is a VFIO PCI mediated device type
	VFIOPCIDeviceMediatedType

	// VFIOAPDeviceMediatedType is a VFIO AP mediated device type
	VFIOAPDeviceMediatedType
)

// VFIODev represents a VFIO PCI device used for hotplugging
type VFIODev struct {
	// ID is used to identify this drive in the hypervisor options.
	ID string

	// BDF (Bus:Device.Function) of the PCI address
	BDF string

	// sysfsdev of VFIO mediated device
	SysfsDev string

	// VendorID specifies vendor id
	VendorID string

	// DeviceID specifies device id
	DeviceID string

	// PCI Class Code
	Class string

	// Bus of VFIO PCIe device
	Bus string

	// Guest PCI path of device
	GuestPciPath vcTypes.PciPath

	// Type of VFIO device
	Type VFIODeviceType

	// IsPCIe specifies device is PCIe or PCI
	IsPCIe bool

	// APDevices are the Adjunct Processor devices assigned to the mdev
	APDevices []string

	// Port is the PCIe port type to which the device is attached
	Port PCIePort

	// HostPath is the path to the device on the host we need it as a reference
	// to match a /dev/vfio/<num> device to a device in GK mode
	HostPath string
}

// RNGDev represents a random number generator device
type RNGDev struct {
	// ID is used to identify the device in the hypervisor options.
	ID string
	// Filename is the file to use as entropy source.
	Filename string
}

// VhostUserDeviceAttrs represents data shared by most vhost-user devices
type VhostUserDeviceAttrs struct {
	DevID      string
	SocketPath string
	// MacAddress is only meaningful for vhost user net device
	MacAddress string

	// These are only meaningful for vhost user fs devices
	Tag string

	Cache string

	Type DeviceType

	// PCIPath is the PCI path used to identify the slot at which
	// the drive is attached.  It is only meaningful for vhost
	// user block devices
	PCIPath vcTypes.PciPath

	// Block index of the device if assigned
	Index int

	CacheSize uint32

	QueueSize uint32

	// Reconnect timeout for socket of vhost user block device
	ReconnectTime uint32
}

// GetHostPathFunc is function pointer used to mock GetHostPath in tests.
var GetHostPathFunc = GetHostPath

// GetVhostUserNodeStatFunc is function pointer used to mock GetVhostUserNodeStat
// in tests. Through this functon, user can get device type information.
var GetVhostUserNodeStatFunc = GetVhostUserNodeStat

// GetHostPath is used to fetch the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
func GetHostPath(devInfo DeviceInfo, vhostUserStoreEnabled bool, vhostUserStorePath string) (string, error) {
	if devInfo.ContainerPath == "" {
		return "", fmt.Errorf("Empty path provided for device")
	}

	if devInfo.Major == -1 {
		return devInfo.HostPath, nil
	}

	// Filter out vhost-user storage devices by device Major numbers.
	if vhostUserStoreEnabled && devInfo.DevType == "b" &&
		(devInfo.Major == VhostUserSCSIMajor || devInfo.Major == VhostUserBlkMajor) {
		return getVhostUserHostPath(devInfo, vhostUserStorePath)
	}

	ueventPath := filepath.Join(getSysDevPath(devInfo), "uevent")
	if _, err := os.Stat(ueventPath); err != nil {
		// Some devices(eg. /dev/fuse, /dev/cuse) do not always implement sysfs interface under /sys/dev
		// These devices are passed by default by docker.
		//
		// Simply return the path passed in the device configuration, this does mean that no device renames are
		// supported for these devices.

		if os.IsNotExist(err) {
			return devInfo.ContainerPath, nil
		}

		return "", err
	}

	content, err := ini.Load(ueventPath)
	if err != nil {
		return "", err
	}

	devName, err := content.Section("").GetKey("DEVNAME")
	if err != nil {
		return "", err
	}

	return filepath.Join("/dev", devName.String()), nil
}

// getBackingFile is used to fetch the backing file for the device.
func getBackingFile(devInfo DeviceInfo) (string, error) {
	backingFilePath := filepath.Join(getSysDevPath(devInfo), "loop", "backing_file")
	data, err := os.ReadFile(backingFilePath)
	if err != nil {
		return "", err
	}

	return strings.TrimSpace(string(data)), nil
}

func getSysDevPathImpl(devInfo DeviceInfo) string {
	var pathComp string

	switch devInfo.DevType {
	case "c", "u":
		pathComp = "char"
	case "b":
		pathComp = "block"
	default:
		// Unsupported device types. Return nil error to ignore devices
		// that cannot be handled currently.
		return ""
	}

	format := strconv.FormatInt(devInfo.Major, 10) + ":" + strconv.FormatInt(devInfo.Minor, 10)
	return filepath.Join(SysDevPrefix, pathComp, format)
}

// getVhostUserHostPath is used to fetch host path for the vhost-user device.
// For vhost-user block device like vhost-user-blk or vhost-user-scsi, its
// socket should be under directory "<vhostUserStorePath>/block/sockets/";
// its corresponding device node should be under directory
// "<vhostUserStorePath>/block/devices/"
func getVhostUserHostPath(devInfo DeviceInfo, vhostUserStorePath string) (string, error) {
	vhostUserDevNodePath := filepath.Join(vhostUserStorePath, "/block/devices/")
	vhostUserSockPath := filepath.Join(vhostUserStorePath, "/block/sockets/")

	sockFileName, err := getVhostUserDevName(vhostUserDevNodePath,
		uint32(devInfo.Major), uint32(devInfo.Minor))
	if err != nil {
		return "", err
	}

	// Locate socket path of vhost-user device
	sockFilePath := filepath.Join(vhostUserSockPath, sockFileName)
	if _, err = os.Stat(sockFilePath); os.IsNotExist(err) {
		return "", err
	}

	return sockFilePath, nil
}

func GetVhostUserNodeStat(devNodePath string, devNodeStat *unix.Stat_t) (err error) {
	return unix.Stat(devNodePath, devNodeStat)
}

// Filter out name of the device node whose device type is Major:Minor from directory
func getVhostUserDevName(dirname string, majorNum, minorNum uint32) (string, error) {
	files, err := os.ReadDir(dirname)
	if err != nil {
		return "", err
	}

	for _, file := range files {
		var devStat unix.Stat_t

		devFilePath := filepath.Join(dirname, file.Name())
		err = GetVhostUserNodeStatFunc(devFilePath, &devStat)
		if err != nil {
			return "", err
		}

		devMajor := unix.Major(uint64(devStat.Rdev))
		devMinor := unix.Minor(uint64(devStat.Rdev))
		if devMajor == majorNum && devMinor == minorNum {
			return file.Name(), nil
		}
	}

	return "", fmt.Errorf("Required device node (%d:%d) doesn't exist under directory %s",
		majorNum, minorNum, dirname)
}

// DeviceState is a structure which represents host devices
// plugged to a hypervisor, one Device can be shared among containers in POD
// Refs: pkg/device/drivers/generic.go:GenericDevice
type DeviceState struct {
	// DriverOptions is specific options for each device driver
	// for example, for BlockDevice, we can set DriverOptions["block-driver"]="virtio-blk"
	DriverOptions map[string]string

	// VhostUserDeviceAttrs is specific for vhost-user device driver
	VhostUserDev *VhostUserDeviceAttrs `json:",omitempty"`

	// BlockDrive is specific for block device driver
	BlockDrive *BlockDrive `json:",omitempty"`

	ID string

	// Type is used to specify driver type
	// Refs: pkg/device/config/config.go:DeviceType
	Type string

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// VFIODev is specific VFIO device driver
	VFIODevs []*VFIODev `json:",omitempty"`

	RefCount    uint
	AttachCount uint

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// ColdPlug specifies whether the device must be cold plugged (true)
	// or hot plugged (false).
	ColdPlug bool
}

// CDI (Container Device Interface), is a specification, for container- runtimes,
// to support third-party devices.
// It introduces an abstract notion of a device as a resource. Such devices are
// uniquely specified by a fully-qualified name that is constructed from a
// vendor ID, a device class, and a name that is unique per vendor ID-device
// class pair.
//
// vendor.com/class=unique_name
//
// The combination of vendor ID and device class (vendor.com/class in the
// above example) is referred to as the device kind.
// CDI concerns itself only with enabling containers to be device aware.
// Areas like resource management are explicitly left out of CDI (and are
// expected to be handled by the orchestrator). Because of this focus, the CDI
// specification is simple to implement and allows great flexibility for
// runtimes and orchestrators.
func WithCDI(annotations map[string]string, cdiSpecDirs []string, spec *specs.Spec) (*specs.Spec, error) {
	// Add devices from CDI annotations
	_, devsFromAnnotations, err := cdi.ParseAnnotations(annotations)
	if err != nil {
		return nil, fmt.Errorf("failed to parse CDI device annotations: %w", err)
	}

	if len(devsFromAnnotations) == 0 {
		// No devices found, skip device injection
		return spec, nil
	}

	var registry cdi.Registry
	if len(cdiSpecDirs) > 0 {
		// We can override the directories where to search for CDI specs
		// if needed, the default is /etc/cdi /var/run/cdi
		registry = cdi.GetRegistry(cdi.WithSpecDirs(cdiSpecDirs...))
	} else {
		registry = cdi.GetRegistry()
	}

	if err = registry.Refresh(); err != nil {
		// We don't consider registry refresh failure a fatal error.
		// For instance, a dynamically generated invalid CDI Spec file for
		// any particular vendor shouldn't prevent injection of devices of
		// different vendors. CDI itself knows better and it will fail the
		// injection if necessary.
		return nil, fmt.Errorf("CDI registry refresh failed: %w", err)
	}

	if _, err := registry.InjectDevices(spec, devsFromAnnotations...); err != nil {
		return nil, fmt.Errorf("CDI device injection failed: %w", err)
	}

	// One crucial thing to keep in mind is that CDI device injection
	// might add OCI Spec environment variables, hooks, and mounts as
	// well. Therefore it is important that none of the corresponding
	// OCI Spec fields are reset up in the call stack once we return.
	return spec, nil
}
