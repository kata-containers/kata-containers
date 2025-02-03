// Copyright contributors to the Virtual Machine Manager for Go project
//
// SPDX-License-Identifier: Apache-2.0
//

// Package qemu provides methods and types for launching and managing QEMU
// instances.  Instances can be launched with the LaunchQemu function and
// managed thereafter via QMPStart and the QMP object that this function
// returns.  To manage a qemu instance after it has been launched you need
// to pass the -qmp option during launch requesting the qemu instance to create
// a QMP unix domain manageent socket, e.g.,
// -qmp unix:/tmp/qmp-socket,server,nowait.  For more information see the
// example below.
package qemu

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"syscall"
)

// Machine describes the machine type qemu will emulate.
type Machine struct {
	// Type is the machine type to be used by qemu.
	Type string

	// Acceleration are the machine acceleration options to be used by qemu.
	Acceleration string

	// Options are options for the machine type
	// For example gic-version=host and usb=off
	Options string
}

const (
	// MachineTypeMicrovm is the QEMU microvm machine type for amd64
	MachineTypeMicrovm string = "microvm"
)

const (
	// Well known vsock CID for host system.
	// https://man7.org/linux/man-pages/man7/vsock.7.html
	VsockHostCid uint64 = 2
)

// Device is the qemu device interface.
type Device interface {
	Valid() bool
	QemuParams(config *Config) []string
}

// DeviceDriver is the device driver string.
type DeviceDriver string

const (
	// LegacySerial is the legacy serial device driver
	LegacySerial DeviceDriver = "serial"

	// NVDIMM is the Non Volatile DIMM device driver.
	NVDIMM DeviceDriver = "nvdimm"

	// VirtioNet is the virtio networking device driver.
	VirtioNet DeviceDriver = "virtio-net"

	// VirtioNetPCI is the virt-io pci networking device driver.
	VirtioNetPCI DeviceDriver = "virtio-net-pci"

	// VirtioNetCCW is the virt-io ccw networking device driver.
	VirtioNetCCW DeviceDriver = "virtio-net-ccw"

	// VirtioBlock is the block device driver.
	VirtioBlock DeviceDriver = "virtio-blk"

	// Console is the console device driver.
	Console DeviceDriver = "virtconsole"

	// Virtio9P is the 9pfs device driver.
	Virtio9P DeviceDriver = "virtio-9p"

	// VirtioSerial is the serial device driver.
	VirtioSerial DeviceDriver = "virtio-serial"

	// VirtioSerialPort is the serial port device driver.
	VirtioSerialPort DeviceDriver = "virtserialport"

	// VirtioRng is the paravirtualized RNG device driver.
	VirtioRng DeviceDriver = "virtio-rng"

	// VirtioBalloon is the memory balloon device driver.
	VirtioBalloon DeviceDriver = "virtio-balloon"

	//VhostUserSCSI represents a SCSI vhostuser device type.
	VhostUserSCSI DeviceDriver = "vhost-user-scsi"

	//VhostUserNet represents a net vhostuser device type.
	VhostUserNet DeviceDriver = "virtio-net"

	//VhostUserBlk represents a block vhostuser device type.
	VhostUserBlk DeviceDriver = "vhost-user-blk"

	//VhostUserFS represents a virtio-fs vhostuser device type
	VhostUserFS DeviceDriver = "vhost-user-fs"

	// PCIBridgeDriver represents a PCI bridge device type.
	PCIBridgeDriver DeviceDriver = "pci-bridge"

	// PCIePCIBridgeDriver represents a PCIe to PCI bridge device type.
	PCIePCIBridgeDriver DeviceDriver = "pcie-pci-bridge"

	// VfioPCI is the vfio driver with PCI transport.
	VfioPCI DeviceDriver = "vfio-pci"

	// VfioCCW is the vfio driver with CCW transport.
	VfioCCW DeviceDriver = "vfio-ccw"

	// VfioAP is the vfio driver with AP transport.
	VfioAP DeviceDriver = "vfio-ap"

	// VHostVSockPCI is a generic Vsock vhost device with PCI transport.
	VHostVSockPCI DeviceDriver = "vhost-vsock-pci"

	// PCIeRootPort is a PCIe Root Port, the PCIe device should be hotplugged to this port.
	PCIeRootPort DeviceDriver = "pcie-root-port"

	// PCIeSwitchUpstreamPort is a PCIe switch upstream port
	// A upstream port connects to a PCIe Root Port
	PCIeSwitchUpstreamPort DeviceDriver = "x3130-upstream"

	// PCIeSwitchDownstreamPort is a PCIe switch downstream port
	// PCIe devices can be hot-plugged to the downstream port.
	PCIeSwitchDownstreamPort DeviceDriver = "xio3130-downstream"

	// Loader is the Loader device driver.
	Loader DeviceDriver = "loader"

	// SpaprTPMProxy is used for enabling guest to run in secure mode on ppc64le.
	SpaprTPMProxy DeviceDriver = "spapr-tpm-proxy"
)

func isDimmSupported(config *Config) bool {
	switch runtime.GOARCH {
	case "amd64", "386", "ppc64le", "arm64":
		if config != nil && config.Machine.Type == MachineTypeMicrovm {
			// microvm does not support NUMA
			return false
		}
		return true
	default:
		return false
	}
}

// VirtioTransport is the transport in use for a virtio device.
type VirtioTransport string

const (
	// TransportPCI is the PCI transport for virtio device.
	TransportPCI VirtioTransport = "pci"

	// TransportCCW is the CCW transport for virtio devices.
	TransportCCW VirtioTransport = "ccw"

	// TransportMMIO is the MMIO transport for virtio devices.
	TransportMMIO VirtioTransport = "mmio"

	// TransportAP is the AP transport for virtio devices.
	TransportAP VirtioTransport = "ap"
)

// defaultTransport returns the default transport for the current combination
// of host's architecture and QEMU machine type.
func (transport VirtioTransport) defaultTransport(config *Config) VirtioTransport {
	switch runtime.GOARCH {
	case "amd64", "386":
		if config != nil && config.Machine.Type == MachineTypeMicrovm {
			return TransportMMIO
		}
		return TransportPCI
	case "s390x":
		return TransportCCW
	default:
		return TransportPCI
	}
}

// isVirtioPCI returns true if the transport is PCI.
func (transport VirtioTransport) isVirtioPCI(config *Config) bool {
	if transport == "" {
		transport = transport.defaultTransport(config)
	}

	return transport == TransportPCI
}

// isVirtioCCW returns true if the transport is CCW.
func (transport VirtioTransport) isVirtioCCW(config *Config) bool {
	if transport == "" {
		transport = transport.defaultTransport(config)
	}

	return transport == TransportCCW
}

func (transport VirtioTransport) isVirtioAP(config *Config) bool {
	if transport == "" {
		transport = transport.defaultTransport(config)
	}

	return transport == TransportAP
}

// getName returns the name of the current transport.
func (transport VirtioTransport) getName(config *Config) string {
	if transport == "" {
		transport = transport.defaultTransport(config)
	}

	return string(transport)
}

// disableModern returns the parameters with the disable-modern option.
// In case the device driver is not a PCI device and it doesn't have the option
// an empty string is returned.
func (transport VirtioTransport) disableModern(config *Config, disable bool) string {
	if !transport.isVirtioPCI(config) {
		return ""
	}

	if disable {
		return "disable-modern=true"
	}

	return "disable-modern=false"
}

// ObjectType is a string representing a qemu object type.
type ObjectType string

const (
	// MemoryBackendFile represents a guest memory mapped file.
	MemoryBackendFile ObjectType = "memory-backend-file"

	// MemoryBackendEPC represents a guest memory backend EPC for SGX.
	MemoryBackendEPC ObjectType = "memory-backend-epc"

	// TDXGuest represents a TDX object
	TDXGuest ObjectType = "tdx-guest"

	// SEVGuest represents an SEV guest object
	SEVGuest ObjectType = "sev-guest"

	// SNPGuest represents an SNP guest object
	SNPGuest ObjectType = "sev-snp-guest"

	// SecExecGuest represents an s390x Secure Execution (Protected Virtualization in QEMU) object
	SecExecGuest ObjectType = "s390-pv-guest"

	// PEFGuest represent ppc64le PEF(Protected Execution Facility) object.
	PEFGuest ObjectType = "pef-guest"
)

// Object is a qemu object representation.
// nolint: govet
type Object struct {
	// Driver is the qemu device driver
	Driver DeviceDriver

	// Type is the qemu object type.
	Type ObjectType

	// ID is the user defined object ID.
	ID string

	// DeviceID is the user defined device ID.
	DeviceID string

	// MemPath is the object's memory path.
	// This is only relevant for memory objects
	MemPath string

	// Size is the object size in bytes
	Size uint64

	// Debug this is a debug object
	Debug bool

	// File is the device file
	File string

	// FirmwareVolume is the configuration volume for the firmware
	// it can be used to split the TDVF/OVMF UEFI firmware in UEFI variables
	// and UEFI program image.
	FirmwareVolume string

	// CBitPos is the location of the C-bit in a guest page table entry
	// This is only relevant for sev-guest objects
	CBitPos uint32

	// ReducedPhysBits is the reduction in the guest physical address space
	// This is only relevant for sev-guest objects
	ReducedPhysBits uint32

	// ReadOnly specifies whether `MemPath` is opened read-only or read/write (default)
	ReadOnly bool

	// Prealloc enables memory preallocation
	Prealloc bool

	// QgsPort defines Intel Quote Generation Service port exposed from the host
	QgsPort uint32
}

// Valid returns true if the Object structure is valid and complete.
func (object Object) Valid() bool {
	switch object.Type {
	case MemoryBackendFile:
		return object.ID != "" && object.MemPath != "" && object.Size != 0
	case MemoryBackendEPC:
		return object.ID != "" && object.Size != 0
	case TDXGuest:
		return object.ID != "" && object.File != "" && object.DeviceID != "" && object.QgsPort != 0
	case SEVGuest:
		fallthrough
	case SNPGuest:
		return object.ID != "" && object.File != "" && object.CBitPos != 0 && object.ReducedPhysBits != 0
	case SecExecGuest:
		return object.ID != ""
	case PEFGuest:
		return object.ID != "" && object.File != ""

	default:
		return false
	}
}

// QemuParams returns the qemu parameters built out of this Object device.
func (object Object) QemuParams(config *Config) []string {
	var objectParams []string
	var deviceParams []string
	var driveParams []string
	var qemuParams []string

	switch object.Type {
	case MemoryBackendFile:
		objectParams = append(objectParams, string(object.Type))
		objectParams = append(objectParams, fmt.Sprintf("id=%s", object.ID))
		objectParams = append(objectParams, fmt.Sprintf("mem-path=%s", object.MemPath))
		objectParams = append(objectParams, fmt.Sprintf("size=%d", object.Size))

		deviceParams = append(deviceParams, string(object.Driver))
		deviceParams = append(deviceParams, fmt.Sprintf("id=%s", object.DeviceID))
		deviceParams = append(deviceParams, fmt.Sprintf("memdev=%s", object.ID))

		if object.ReadOnly {
			objectParams = append(objectParams, "readonly=on")
			deviceParams = append(deviceParams, "unarmed=on")
		}
	case MemoryBackendEPC:
		objectParams = append(objectParams, string(object.Type))
		objectParams = append(objectParams, fmt.Sprintf("id=%s", object.ID))
		objectParams = append(objectParams, fmt.Sprintf("size=%d", object.Size))
		if object.Prealloc {
			objectParams = append(objectParams, "prealloc=on")
		}

	case TDXGuest:
		objectParams = append(objectParams, prepareTDXObject(object))
		config.Bios = object.File
	case SEVGuest:
		objectParams = append(objectParams, string(object.Type))
		objectParams = append(objectParams, fmt.Sprintf("id=%s", object.ID))
		objectParams = append(objectParams, fmt.Sprintf("cbitpos=%d", object.CBitPos))
		objectParams = append(objectParams, fmt.Sprintf("reduced-phys-bits=%d", object.ReducedPhysBits))
		driveParams = append(driveParams, "if=pflash,format=raw,readonly=on")
		driveParams = append(driveParams, fmt.Sprintf("file=%s", object.File))
	case SNPGuest:
		objectParams = append(objectParams, string(object.Type))
		objectParams = append(objectParams, fmt.Sprintf("id=%s", object.ID))
		objectParams = append(objectParams, fmt.Sprintf("cbitpos=%d", object.CBitPos))
		objectParams = append(objectParams, fmt.Sprintf("reduced-phys-bits=%d", object.ReducedPhysBits))
		objectParams = append(objectParams, "kernel-hashes=on")
		config.Bios = object.File
	case SecExecGuest:
		objectParams = append(objectParams, string(object.Type))
		objectParams = append(objectParams, fmt.Sprintf("id=%s", object.ID))
	case PEFGuest:
		objectParams = append(objectParams, string(object.Type))
		objectParams = append(objectParams, fmt.Sprintf("id=%s", object.ID))

		deviceParams = append(deviceParams, string(object.Driver))
		deviceParams = append(deviceParams, fmt.Sprintf("id=%s", object.DeviceID))
		deviceParams = append(deviceParams, fmt.Sprintf("host-path=%s", object.File))
	}

	if len(deviceParams) > 0 {
		qemuParams = append(qemuParams, "-device")
		qemuParams = append(qemuParams, strings.Join(deviceParams, ","))
	}

	if len(objectParams) > 0 {
		qemuParams = append(qemuParams, "-object")
		qemuParams = append(qemuParams, strings.Join(objectParams, ","))
	}

	if len(driveParams) > 0 {
		qemuParams = append(qemuParams, "-drive")
		qemuParams = append(qemuParams, strings.Join(driveParams, ","))
	}

	return qemuParams
}

type SocketAddress struct {
	Type string `json:"type"`
	Cid  string `json:"cid"`
	Port string `json:"port"`
}

type TdxQomObject struct {
	QomType               string        `json:"qom-type"`
	Id                    string        `json:"id"`
	MrConfigId            string        `json:"mrconfigid,omitempty"`
	MrOwner               string        `json:"mrowner,omitempty"`
	MrOwnerConfig         string        `json:"mrownerconfig,omitempty"`
	QuoteGenerationSocket SocketAddress `json:"quote-generation-socket,omitempty"`
	Debug                 *bool         `json:"debug,omitempty"`
}

func (this *SocketAddress) String() string {
	b, err := json.Marshal(*this)

	if err != nil {
		log.Fatalf("Unable to marshal SocketAddress object: %s", err.Error())
		return ""
	}

	return string(b)
}

func (this *TdxQomObject) String() string {
	b, err := json.Marshal(*this)

	if err != nil {
		log.Fatalf("Unable to marshal TDX QOM object: %s", err.Error())
		return ""
	}

	return string(b)
}

func prepareTDXObject(object Object) string {
	qgsSocket := SocketAddress{"vsock", fmt.Sprint(VsockHostCid), fmt.Sprint(object.QgsPort)}
	tdxObject := TdxQomObject{
		string(object.Type), // qom-type
		object.ID,           // id
		"",                  // mrconfigid
		"",                  // mrowner
		"",                  // mrownerconfig
		qgsSocket,           // quote-generation-socket
		nil}

	if object.Debug {
		*tdxObject.Debug = true
	}

	return tdxObject.String()
}

// Virtio9PMultidev filesystem behaviour to deal
// with multiple devices being shared with a 9p export.
type Virtio9PMultidev string

const (
	// Remap shares multiple devices with only one export.
	Remap Virtio9PMultidev = "remap"

	// Warn assumes that only one device is shared by the same export.
	// Only a warning message is logged (once) by qemu on host side.
	// This is the default behaviour.
	Warn Virtio9PMultidev = "warn"

	// Forbid like "warn" but also deny access to additional devices on guest.
	Forbid Virtio9PMultidev = "forbid"
)

// FSDriver represents a qemu filesystem driver.
type FSDriver string

// SecurityModelType is a qemu filesystem security model type.
type SecurityModelType string

const (
	// Local is the local qemu filesystem driver.
	Local FSDriver = "local"

	// Handle is the handle qemu filesystem driver.
	Handle FSDriver = "handle"

	// Proxy is the proxy qemu filesystem driver.
	Proxy FSDriver = "proxy"
)

const (
	// None is like passthrough without failure reports.
	None SecurityModelType = "none"

	// PassThrough uses the same credentials on both the host and guest.
	PassThrough SecurityModelType = "passthrough"

	// MappedXattr stores some files attributes as extended attributes.
	MappedXattr SecurityModelType = "mapped-xattr"

	// MappedFile stores some files attributes in the .virtfs directory.
	MappedFile SecurityModelType = "mapped-file"
)

// FSDevice represents a qemu filesystem configuration.
// nolint: govet
type FSDevice struct {
	// Driver is the qemu device driver
	Driver DeviceDriver

	// FSDriver is the filesystem driver backend.
	FSDriver FSDriver

	// ID is the filesystem identifier.
	ID string

	// Path is the host root path for this filesystem.
	Path string

	// MountTag is the device filesystem mount point tag.
	MountTag string

	// SecurityModel is the security model for this filesystem device.
	SecurityModel SecurityModelType

	// DisableModern prevents qemu from relying on fast MMIO.
	DisableModern bool

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport

	// Multidev is the filesystem behaviour to deal
	// with multiple devices being shared with a 9p export
	Multidev Virtio9PMultidev
}

// Virtio9PTransport is a map of the virtio-9p device name that corresponds
// to each transport.
var Virtio9PTransport = map[VirtioTransport]string{
	TransportPCI:  "virtio-9p-pci",
	TransportCCW:  "virtio-9p-ccw",
	TransportMMIO: "virtio-9p-device",
}

// Valid returns true if the FSDevice structure is valid and complete.
func (fsdev FSDevice) Valid() bool {
	if fsdev.ID == "" || fsdev.Path == "" || fsdev.MountTag == "" {
		return false
	}

	return true
}

// QemuParams returns the qemu parameters built out of this filesystem device.
func (fsdev FSDevice) QemuParams(config *Config) []string {
	var fsParams []string
	var deviceParams []string
	var qemuParams []string

	deviceParams = append(deviceParams, fsdev.deviceName(config))
	if s := fsdev.Transport.disableModern(config, fsdev.DisableModern); s != "" {
		deviceParams = append(deviceParams, s)
	}
	deviceParams = append(deviceParams, fmt.Sprintf("fsdev=%s", fsdev.ID))
	deviceParams = append(deviceParams, fmt.Sprintf("mount_tag=%s", fsdev.MountTag))
	if fsdev.Transport.isVirtioPCI(config) && fsdev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", fsdev.ROMFile))
	}
	if fsdev.Transport.isVirtioCCW(config) {
		if config.Knobs.IOMMUPlatform {
			deviceParams = append(deviceParams, "iommu_platform=on")
		}
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", fsdev.DevNo))
	}

	fsParams = append(fsParams, string(fsdev.FSDriver))
	fsParams = append(fsParams, fmt.Sprintf("id=%s", fsdev.ID))
	fsParams = append(fsParams, fmt.Sprintf("path=%s", fsdev.Path))
	fsParams = append(fsParams, fmt.Sprintf("security_model=%s", fsdev.SecurityModel))

	if fsdev.Multidev != "" {
		fsParams = append(fsParams, fmt.Sprintf("multidevs=%s", fsdev.Multidev))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	qemuParams = append(qemuParams, "-fsdev")
	qemuParams = append(qemuParams, strings.Join(fsParams, ","))

	return qemuParams
}

// deviceName returns the QEMU shared filesystem device name for the current
// combination of driver and transport.
func (fsdev FSDevice) deviceName(config *Config) string {
	if fsdev.Transport == "" {
		fsdev.Transport = fsdev.Transport.defaultTransport(config)
	}

	switch fsdev.Driver {
	case Virtio9P:
		return Virtio9PTransport[fsdev.Transport]
	}

	return string(fsdev.Driver)
}

// CharDeviceBackend is the character device backend for qemu
type CharDeviceBackend string

const (
	// Pipe creates a 2 way connection to the guest.
	Pipe CharDeviceBackend = "pipe"

	// Socket creates a 2 way stream socket (TCP or Unix).
	Socket CharDeviceBackend = "socket"

	// CharConsole sends traffic from the guest to QEMU's standard output.
	CharConsole CharDeviceBackend = "console"

	// Serial sends traffic from the guest to a serial device on the host.
	Serial CharDeviceBackend = "serial"

	// TTY is an alias for Serial.
	TTY CharDeviceBackend = "tty"

	// PTY creates a new pseudo-terminal on the host and connect to it.
	PTY CharDeviceBackend = "pty"

	// File sends traffic from the guest to a file on the host.
	File CharDeviceBackend = "file"
)

// CharDevice represents a qemu character device.
// nolint: govet
type CharDevice struct {
	Backend CharDeviceBackend

	// Driver is the qemu device driver
	Driver DeviceDriver

	// Bus is the serial bus associated to this device.
	Bus string

	// DeviceID is the user defined device ID.
	DeviceID string

	ID   string
	Path string
	Name string

	// DisableModern prevents qemu from relying on fast MMIO.
	DisableModern bool

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// VirtioSerialTransport is a map of the virtio-serial device name that
// corresponds to each transport.
var VirtioSerialTransport = map[VirtioTransport]string{
	TransportPCI:  "virtio-serial-pci",
	TransportCCW:  "virtio-serial-ccw",
	TransportMMIO: "virtio-serial-device",
}

// Valid returns true if the CharDevice structure is valid and complete.
func (cdev CharDevice) Valid() bool {
	if cdev.ID == "" || cdev.Path == "" {
		return false
	}

	return true
}

// QemuParams returns the qemu parameters built out of this character device.
func (cdev CharDevice) QemuParams(config *Config) []string {
	var cdevParams []string
	var deviceParams []string
	var qemuParams []string

	deviceParams = append(deviceParams, cdev.deviceName(config))
	if cdev.Driver == VirtioSerial {
		if s := cdev.Transport.disableModern(config, cdev.DisableModern); s != "" {
			deviceParams = append(deviceParams, s)
		}
	}
	if cdev.Bus != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("bus=%s", cdev.Bus))
	}
	deviceParams = append(deviceParams, fmt.Sprintf("chardev=%s", cdev.ID))
	deviceParams = append(deviceParams, fmt.Sprintf("id=%s", cdev.DeviceID))
	if cdev.Name != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("name=%s", cdev.Name))
	}
	if cdev.Driver == VirtioSerial && cdev.Transport.isVirtioPCI(config) && cdev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", cdev.ROMFile))
	}

	if cdev.Driver == VirtioSerial && cdev.Transport.isVirtioCCW(config) {
		if config.Knobs.IOMMUPlatform {
			deviceParams = append(deviceParams, "iommu_platform=on")
		}
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", cdev.DevNo))
	}

	cdevParams = append(cdevParams, string(cdev.Backend))
	cdevParams = append(cdevParams, fmt.Sprintf("id=%s", cdev.ID))
	if cdev.Backend == Socket {
		cdevParams = append(cdevParams, fmt.Sprintf("path=%s,server=on,wait=off", cdev.Path))
	} else {
		cdevParams = append(cdevParams, fmt.Sprintf("path=%s", cdev.Path))
	}

	// Legacy serial is special. It does not follow the device + driver model
	if cdev.Driver != LegacySerial {
		qemuParams = append(qemuParams, "-device")
		qemuParams = append(qemuParams, strings.Join(deviceParams, ","))
	}

	qemuParams = append(qemuParams, "-chardev")
	qemuParams = append(qemuParams, strings.Join(cdevParams, ","))

	return qemuParams
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (cdev CharDevice) deviceName(config *Config) string {
	if cdev.Transport == "" {
		cdev.Transport = cdev.Transport.defaultTransport(config)
	}

	switch cdev.Driver {
	case VirtioSerial:
		return VirtioSerialTransport[cdev.Transport]
	}

	return string(cdev.Driver)
}

// NetDeviceType is a qemu networking device type.
type NetDeviceType string

const (
	// TAP is a TAP networking device type.
	TAP NetDeviceType = "tap"

	// MACVTAP is a macvtap networking device type.
	MACVTAP NetDeviceType = "macvtap"

	// IPVTAP is a ipvtap virtual networking device type.
	IPVTAP NetDeviceType = "ipvtap"

	// VETHTAP is a veth-tap virtual networking device type.
	VETHTAP NetDeviceType = "vethtap"

	// VFIO is a direct assigned PCI device or PCI VF
	VFIO NetDeviceType = "VFIO"

	// VHOSTUSER is a vhost-user port (socket)
	VHOSTUSER NetDeviceType = "vhostuser"
)

// QemuNetdevParam converts to the QEMU -netdev parameter notation
func (n NetDeviceType) QemuNetdevParam(netdev *NetDevice, config *Config) string {
	if netdev.Transport == "" {
		netdev.Transport = netdev.Transport.defaultTransport(config)
	}

	switch n {
	case TAP:
		return "tap"
	case MACVTAP:
		return "tap"
	case IPVTAP:
		return "tap"
	case VETHTAP:
		return "tap" // -netdev type=tap -device virtio-net-pci
	case VFIO:
		if netdev.Transport == TransportMMIO {
			log.Fatal("vfio devices are not support with the MMIO transport")
		}
		return "" // -device vfio-pci (no netdev)
	case VHOSTUSER:
		if netdev.Transport == TransportCCW {
			log.Fatal("vhost-user devices are not supported on IBM Z")
		}
		return "vhost-user" // -netdev type=vhost-user (no device)
	default:
		return ""

	}
}

// QemuDeviceParam converts to the QEMU -device parameter notation
func (n NetDeviceType) QemuDeviceParam(netdev *NetDevice, config *Config) DeviceDriver {
	if netdev.Transport == "" {
		netdev.Transport = netdev.Transport.defaultTransport(config)
	}

	var device string

	switch n {
	case TAP:
		device = "virtio-net"
	case MACVTAP:
		device = "virtio-net"
	case IPVTAP:
		device = "virtio-net"
	case VETHTAP:
		device = "virtio-net" // -netdev type=tap -device virtio-net-pci
	case VFIO:
		if netdev.Transport == TransportMMIO {
			log.Fatal("vfio devices are not support with the MMIO transport")
		}
		device = "vfio" // -device vfio-pci (no netdev)
	case VHOSTUSER:
		if netdev.Transport == TransportCCW {
			log.Fatal("vhost-user devices are not supported on IBM Z")
		}
		return "" // -netdev type=vhost-user (no device)
	default:
		return ""
	}

	switch netdev.Transport {
	case TransportPCI:
		return DeviceDriver(device + "-pci")
	case TransportCCW:
		return DeviceDriver(device + "-ccw")
	case TransportMMIO:
		return DeviceDriver(device + "-device")
	default:
		return ""
	}
}

// NetDevice represents a guest networking device
// nolint: govet
type NetDevice struct {
	// Type is the netdev type (e.g. tap).
	Type NetDeviceType

	// Driver is the qemu device driver
	Driver DeviceDriver

	// ID is the netdevice identifier.
	ID string

	// IfName is the interface name,
	IFName string

	// Bus is the bus path name of a PCI device.
	Bus string

	// Addr is the address offset of a PCI device.
	Addr string

	// DownScript is the tap interface deconfiguration script.
	DownScript string

	// Script is the tap interface configuration script.
	Script string

	// FDs represents the list of already existing file descriptors to be used.
	// This is mostly useful for mq support.
	FDs      []*os.File
	VhostFDs []*os.File

	// VHost enables virtio device emulation from the host kernel instead of from qemu.
	VHost bool

	// MACAddress is the networking device interface MAC address.
	MACAddress string

	// DisableModern prevents qemu from relying on fast MMIO.
	DisableModern bool

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// VirtioNetTransport is a map of the virtio-net device name that corresponds
// to each transport.
var VirtioNetTransport = map[VirtioTransport]string{
	TransportPCI:  "virtio-net-pci",
	TransportCCW:  "virtio-net-ccw",
	TransportMMIO: "virtio-net-device",
}

// Valid returns true if the NetDevice structure is valid and complete.
func (netdev NetDevice) Valid() bool {
	if netdev.ID == "" || netdev.IFName == "" {
		return false
	}

	switch netdev.Type {
	case TAP:
		return true
	case MACVTAP:
		return true
	default:
		return false
	}
}

// mqParameter returns the parameters for multi-queue driver. If the driver is a PCI device then the
// vector flag is required. If the driver is a CCW type than the vector flag is not implemented and only
// multi-queue option mq needs to be activated. See comment in libvirt code at
// https://github.com/libvirt/libvirt/blob/6e7e965dcd3d885739129b1454ce19e819b54c25/src/qemu/qemu_command.c#L3633
func (netdev NetDevice) mqParameter(config *Config) string {
	p := []string{"mq=on"}

	if netdev.Transport.isVirtioPCI(config) {
		// https://www.linux-kvm.org/page/Multiqueue
		// -netdev tap,vhost=on,queues=N
		// enable mq and specify msix vectors in qemu cmdline
		// (2N+2 vectors, N for tx queues, N for rx queues, 1 for config, and one for possible control vq)
		// -device virtio-net-pci,mq=on,vectors=2N+2...
		// enable mq in guest by 'ethtool -L eth0 combined $queue_num'
		// Clearlinux automatically sets up the queues properly
		// The agent implementation should do this to ensure that it is
		// always set
		vectors := len(netdev.FDs)*2 + 2
		p = append(p, fmt.Sprintf("vectors=%d", vectors))
	}

	return strings.Join(p, ",")
}

// QemuDeviceParams returns the -device parameters for this network device
func (netdev NetDevice) QemuDeviceParams(config *Config) []string {
	var deviceParams []string

	driver := netdev.Type.QemuDeviceParam(&netdev, config)
	if driver == "" {
		return nil
	}

	deviceParams = append(deviceParams, fmt.Sprintf("driver=%s", driver))
	deviceParams = append(deviceParams, fmt.Sprintf("netdev=%s", netdev.ID))
	deviceParams = append(deviceParams, fmt.Sprintf("mac=%s", netdev.MACAddress))

	if netdev.Bus != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("bus=%s", netdev.Bus))
	}

	if netdev.Addr != "" {
		addr, err := strconv.Atoi(netdev.Addr)
		if err == nil && addr >= 0 {
			deviceParams = append(deviceParams, fmt.Sprintf("addr=%x", addr))
		}
	}
	if s := netdev.Transport.disableModern(config, netdev.DisableModern); s != "" {
		deviceParams = append(deviceParams, s)
	}

	if len(netdev.FDs) > 0 {
		// Note: We are appending to the device params here
		deviceParams = append(deviceParams, netdev.mqParameter(config))
	}

	if netdev.Transport.isVirtioPCI(config) && netdev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", netdev.ROMFile))
	}

	if netdev.Transport.isVirtioCCW(config) {
		if config.Knobs.IOMMUPlatform {
			deviceParams = append(deviceParams, "iommu_platform=on")
		}
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", netdev.DevNo))
	}

	return deviceParams
}

// QemuNetdevParams returns the -netdev parameters for this network device
func (netdev NetDevice) QemuNetdevParams(config *Config) []string {
	var netdevParams []string

	netdevType := netdev.Type.QemuNetdevParam(&netdev, config)
	if netdevType == "" {
		return nil
	}

	netdevParams = append(netdevParams, netdevType)
	netdevParams = append(netdevParams, fmt.Sprintf("id=%s", netdev.ID))

	if netdev.VHost {
		netdevParams = append(netdevParams, "vhost=on")
		if len(netdev.VhostFDs) > 0 {
			var fdParams []string
			qemuFDs := config.appendFDs(netdev.VhostFDs)
			for _, fd := range qemuFDs {
				fdParams = append(fdParams, fmt.Sprintf("%d", fd))
			}
			netdevParams = append(netdevParams, fmt.Sprintf("vhostfds=%s", strings.Join(fdParams, ":")))
		}
	}

	if len(netdev.FDs) > 0 {
		var fdParams []string

		qemuFDs := config.appendFDs(netdev.FDs)
		for _, fd := range qemuFDs {
			fdParams = append(fdParams, fmt.Sprintf("%d", fd))
		}

		netdevParams = append(netdevParams, fmt.Sprintf("fds=%s", strings.Join(fdParams, ":")))

	} else {
		netdevParams = append(netdevParams, fmt.Sprintf("ifname=%s", netdev.IFName))
		if netdev.DownScript != "" {
			netdevParams = append(netdevParams, fmt.Sprintf("downscript=%s", netdev.DownScript))
		}
		if netdev.Script != "" {
			netdevParams = append(netdevParams, fmt.Sprintf("script=%s", netdev.Script))
		}
	}
	return netdevParams
}

// QemuParams returns the qemu parameters built out of this network device.
func (netdev NetDevice) QemuParams(config *Config) []string {
	var netdevParams []string
	var deviceParams []string
	var qemuParams []string

	// Macvtap can only be connected via fds
	if (netdev.Type == MACVTAP) && (len(netdev.FDs) == 0) {
		return nil // implicit error
	}

	if netdev.Type.QemuNetdevParam(&netdev, config) != "" {
		netdevParams = netdev.QemuNetdevParams(config)
		if netdevParams != nil {
			qemuParams = append(qemuParams, "-netdev")
			qemuParams = append(qemuParams, strings.Join(netdevParams, ","))
		}
	}

	if netdev.Type.QemuDeviceParam(&netdev, config) != "" {
		deviceParams = netdev.QemuDeviceParams(config)
		if deviceParams != nil {
			qemuParams = append(qemuParams, "-device")
			qemuParams = append(qemuParams, strings.Join(deviceParams, ","))
		}
	}

	return qemuParams
}

// LegacySerialDevice represents a qemu legacy serial device.
type LegacySerialDevice struct {
	// ID is the serial device identifier.
	// This maps to the char dev associated with the device
	// as serial does not have a notion of id
	// e.g:
	// -chardev stdio,id=char0,mux=on,logfile=serial.log,signal=off -serial chardev:char0
	// -chardev file,id=char0,path=serial.log -serial chardev:char0
	Chardev string
}

// Valid returns true if the LegacySerialDevice structure is valid and complete.
func (dev LegacySerialDevice) Valid() bool {
	return dev.Chardev != ""
}

// QemuParams returns the qemu parameters built out of this serial device.
func (dev LegacySerialDevice) QemuParams(config *Config) []string {
	var deviceParam string
	var qemuParams []string

	deviceParam = fmt.Sprintf("chardev:%s", dev.Chardev)

	qemuParams = append(qemuParams, "-serial")
	qemuParams = append(qemuParams, deviceParam)

	return qemuParams
}

/* Not used currently
// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (dev LegacySerialDevice) deviceName(config *Config) string {
	return dev.Chardev
}
*/

// SerialDevice represents a qemu serial device.
// nolint: govet
type SerialDevice struct {
	// Driver is the qemu device driver
	Driver DeviceDriver

	// ID is the serial device identifier.
	ID string

	// DisableModern prevents qemu from relying on fast MMIO.
	DisableModern bool

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport

	// MaxPorts is the maximum number of ports for this device.
	MaxPorts uint
}

// Valid returns true if the SerialDevice structure is valid and complete.
func (dev SerialDevice) Valid() bool {
	if dev.Driver == "" || dev.ID == "" {
		return false
	}

	return true
}

// QemuParams returns the qemu parameters built out of this serial device.
func (dev SerialDevice) QemuParams(config *Config) []string {
	var deviceParams []string
	var qemuParams []string

	deviceParams = append(deviceParams, dev.deviceName(config))
	if s := dev.Transport.disableModern(config, dev.DisableModern); s != "" {
		deviceParams = append(deviceParams, s)
	}
	deviceParams = append(deviceParams, fmt.Sprintf("id=%s", dev.ID))
	if dev.Transport.isVirtioPCI(config) && dev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", dev.ROMFile))
		if dev.Driver == VirtioSerial && dev.MaxPorts != 0 {
			deviceParams = append(deviceParams, fmt.Sprintf("max_ports=%d", dev.MaxPorts))
		}
	}

	if dev.Transport.isVirtioCCW(config) {
		if config.Knobs.IOMMUPlatform {
			deviceParams = append(deviceParams, "iommu_platform=on")
		}
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", dev.DevNo))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (dev SerialDevice) deviceName(config *Config) string {
	if dev.Transport == "" {
		dev.Transport = dev.Transport.defaultTransport(config)
	}

	switch dev.Driver {
	case VirtioSerial:
		return VirtioSerialTransport[dev.Transport]
	}

	return string(dev.Driver)
}

// BlockDeviceInterface defines the type of interface the device is connected to.
type BlockDeviceInterface string

// BlockDeviceAIO defines the type of asynchronous I/O the block device should use.
type BlockDeviceAIO string

// BlockDeviceFormat defines the image format used on a block device.
type BlockDeviceFormat string

const (
	// NoInterface for block devices with no interfaces.
	NoInterface BlockDeviceInterface = "none"

	// SCSI represents a SCSI block device interface.
	SCSI BlockDeviceInterface = "scsi"
)

const (
	// Threads is the pthread asynchronous I/O implementation.
	Threads BlockDeviceAIO = "threads"

	// Native is the native Linux AIO implementation.
	Native BlockDeviceAIO = "native"

	// IOUring is the Linux io_uring I/O implementation.
	IOUring BlockDeviceAIO = "io_uring"
)

const (
	// QCOW2 is the Qemu Copy On Write v2 image format.
	QCOW2 BlockDeviceFormat = "qcow2"
)

// BlockDevice represents a qemu block device.
// nolint: govet
type BlockDevice struct {
	Driver    DeviceDriver
	ID        string
	File      string
	Interface BlockDeviceInterface
	AIO       BlockDeviceAIO
	Format    BlockDeviceFormat
	SCSI      bool
	WCE       bool

	// DisableModern prevents qemu from relying on fast MMIO.
	DisableModern bool

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// ShareRW enables multiple qemu instances to share the File
	ShareRW bool

	// ReadOnly sets the block device in readonly mode
	ReadOnly bool

	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// VirtioBlockTransport is a map of the virtio-blk device name that corresponds
// to each transport.
var VirtioBlockTransport = map[VirtioTransport]string{
	TransportPCI:  "virtio-blk-pci",
	TransportCCW:  "virtio-blk-ccw",
	TransportMMIO: "virtio-blk-device",
}

// Valid returns true if the BlockDevice structure is valid and complete.
func (blkdev BlockDevice) Valid() bool {
	if blkdev.Driver == "" || blkdev.ID == "" || blkdev.File == "" {
		return false
	}

	return true
}

// QemuParams returns the qemu parameters built out of this block device.
func (blkdev BlockDevice) QemuParams(config *Config) []string {
	var blkParams []string
	var deviceParams []string
	var qemuParams []string

	deviceParams = append(deviceParams, blkdev.deviceName(config))
	if s := blkdev.Transport.disableModern(config, blkdev.DisableModern); s != "" {
		deviceParams = append(deviceParams, s)
	}
	deviceParams = append(deviceParams, fmt.Sprintf("drive=%s", blkdev.ID))
	if !blkdev.WCE {
		deviceParams = append(deviceParams, "config-wce=off")
	}

	if blkdev.Transport.isVirtioPCI(config) && blkdev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", blkdev.ROMFile))
	}

	if blkdev.Transport.isVirtioCCW(config) {
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", blkdev.DevNo))
	}

	if blkdev.ShareRW {
		deviceParams = append(deviceParams, "share-rw=on")
	}

	deviceParams = append(deviceParams, fmt.Sprintf("serial=%s", blkdev.ID))

	blkParams = append(blkParams, fmt.Sprintf("id=%s", blkdev.ID))
	blkParams = append(blkParams, fmt.Sprintf("file=%s", blkdev.File))
	blkParams = append(blkParams, fmt.Sprintf("aio=%s", blkdev.AIO))
	blkParams = append(blkParams, fmt.Sprintf("format=%s", blkdev.Format))
	blkParams = append(blkParams, fmt.Sprintf("if=%s", blkdev.Interface))

	if blkdev.ReadOnly {
		blkParams = append(blkParams, "readonly=on")
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	qemuParams = append(qemuParams, "-drive")
	qemuParams = append(qemuParams, strings.Join(blkParams, ","))

	return qemuParams
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (blkdev BlockDevice) deviceName(config *Config) string {
	if blkdev.Transport == "" {
		blkdev.Transport = blkdev.Transport.defaultTransport(config)
	}

	switch blkdev.Driver {
	case VirtioBlock:
		return VirtioBlockTransport[blkdev.Transport]
	}

	return string(blkdev.Driver)
}

// PVPanicDevice represents a qemu pvpanic device.
type PVPanicDevice struct {
	NoShutdown bool
}

// Valid always returns true for pvpanic device
func (dev PVPanicDevice) Valid() bool {
	return true
}

// QemuParams returns the qemu parameters built out of this serial device.
func (dev PVPanicDevice) QemuParams(config *Config) []string {
	if dev.NoShutdown {
		return []string{"-device", "pvpanic", "-no-shutdown"}
	}
	return []string{"-device", "pvpanic"}
}

// LoaderDevice represents a qemu loader device.
type LoaderDevice struct {
	File string
	ID   string
}

// Valid returns true if there is a valid structure defined for LoaderDevice
func (dev LoaderDevice) Valid() bool {
	if dev.File == "" {
		return false
	}

	if dev.ID == "" {
		return false
	}

	return true
}

// QemuParams returns the qemu parameters built out of this loader device.
func (dev LoaderDevice) QemuParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string

	deviceParams = append(deviceParams, "loader")
	deviceParams = append(deviceParams, fmt.Sprintf("file=%s", dev.File))
	deviceParams = append(deviceParams, fmt.Sprintf("id=%s", dev.ID))

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// VhostUserDevice represents a qemu vhost-user device meant to be passed
// in to the guest
// nolint: govet
type VhostUserDevice struct {
	SocketPath    string //path to vhostuser socket on host
	CharDevID     string
	TypeDevID     string //variable QEMU parameter based on value of VhostUserType
	Address       string //used for MAC address in net case
	Tag           string //virtio-fs volume id for mounting inside guest
	CacheSize     uint32 //virtio-fs DAX cache size in MiB
	QueueSize     uint32 //size of virtqueues
	VhostUserType DeviceDriver

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the CCW device for s390x.
	DevNo string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// VhostUserNetTransport is a map of the virtio-net device name that
// corresponds to each transport.
var VhostUserNetTransport = map[VirtioTransport]string{
	TransportPCI:  "virtio-net-pci",
	TransportCCW:  "virtio-net-ccw",
	TransportMMIO: "virtio-net-device",
}

// VhostUserSCSITransport is a map of the vhost-user-scsi device name that
// corresponds to each transport.
var VhostUserSCSITransport = map[VirtioTransport]string{
	TransportPCI:  "vhost-user-scsi-pci",
	TransportCCW:  "vhost-user-scsi-ccw",
	TransportMMIO: "vhost-user-scsi-device",
}

// VhostUserBlkTransport is a map of the vhost-user-blk device name that
// corresponds to each transport.
var VhostUserBlkTransport = map[VirtioTransport]string{
	TransportPCI:  "vhost-user-blk-pci",
	TransportCCW:  "vhost-user-blk-ccw",
	TransportMMIO: "vhost-user-blk-device",
}

// VhostUserFSTransport is a map of the vhost-user-fs device name that
// corresponds to each transport.
var VhostUserFSTransport = map[VirtioTransport]string{
	TransportPCI:  "vhost-user-fs-pci",
	TransportCCW:  "vhost-user-fs-ccw",
	TransportMMIO: "vhost-user-fs-device",
}

// Valid returns true if there is a valid structure defined for VhostUserDevice
func (vhostuserDev VhostUserDevice) Valid() bool {

	if vhostuserDev.SocketPath == "" || vhostuserDev.CharDevID == "" {
		return false
	}

	switch vhostuserDev.VhostUserType {
	case VhostUserNet:
		if vhostuserDev.TypeDevID == "" || vhostuserDev.Address == "" {
			return false
		}
	case VhostUserSCSI:
		if vhostuserDev.TypeDevID == "" {
			return false
		}
	case VhostUserBlk:
	case VhostUserFS:
		if vhostuserDev.Tag == "" {
			return false
		}
	default:
		return false
	}

	return true
}

// QemuNetParams builds QEMU netdev and device parameters for a VhostUserNet device
func (vhostuserDev VhostUserDevice) QemuNetParams(config *Config) []string {
	var qemuParams []string
	var netParams []string
	var deviceParams []string

	driver := vhostuserDev.deviceName(config)
	if driver == "" {
		return nil
	}

	netParams = append(netParams, "type=vhost-user")
	netParams = append(netParams, fmt.Sprintf("id=%s", vhostuserDev.TypeDevID))
	netParams = append(netParams, fmt.Sprintf("chardev=%s", vhostuserDev.CharDevID))
	netParams = append(netParams, "vhostforce")

	deviceParams = append(deviceParams, driver)
	deviceParams = append(deviceParams, fmt.Sprintf("netdev=%s", vhostuserDev.TypeDevID))
	deviceParams = append(deviceParams, fmt.Sprintf("mac=%s", vhostuserDev.Address))

	if vhostuserDev.Transport.isVirtioPCI(config) && vhostuserDev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", vhostuserDev.ROMFile))
	}

	qemuParams = append(qemuParams, "-netdev")
	qemuParams = append(qemuParams, strings.Join(netParams, ","))
	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// QemuSCSIParams builds QEMU device parameters for a VhostUserSCSI device
func (vhostuserDev VhostUserDevice) QemuSCSIParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string

	driver := vhostuserDev.deviceName(config)
	if driver == "" {
		return nil
	}

	deviceParams = append(deviceParams, driver)
	deviceParams = append(deviceParams, fmt.Sprintf("id=%s", vhostuserDev.TypeDevID))
	deviceParams = append(deviceParams, fmt.Sprintf("chardev=%s", vhostuserDev.CharDevID))

	if vhostuserDev.Transport.isVirtioPCI(config) && vhostuserDev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", vhostuserDev.ROMFile))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// QemuBlkParams builds QEMU device parameters for a VhostUserBlk device
func (vhostuserDev VhostUserDevice) QemuBlkParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string

	driver := vhostuserDev.deviceName(config)
	if driver == "" {
		return nil
	}

	deviceParams = append(deviceParams, driver)
	deviceParams = append(deviceParams, "logical_block_size=4096")
	deviceParams = append(deviceParams, "size=512M")
	deviceParams = append(deviceParams, fmt.Sprintf("chardev=%s", vhostuserDev.CharDevID))

	if vhostuserDev.Transport.isVirtioPCI(config) && vhostuserDev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", vhostuserDev.ROMFile))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// QemuFSParams builds QEMU device parameters for a VhostUserFS device
func (vhostuserDev VhostUserDevice) QemuFSParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string

	driver := vhostuserDev.deviceName(config)
	if driver == "" {
		return nil
	}

	deviceParams = append(deviceParams, driver)
	deviceParams = append(deviceParams, fmt.Sprintf("chardev=%s", vhostuserDev.CharDevID))
	deviceParams = append(deviceParams, fmt.Sprintf("tag=%s", vhostuserDev.Tag))
	queueSize := uint32(1024)
	if vhostuserDev.QueueSize != 0 {
		queueSize = vhostuserDev.QueueSize
	}
	deviceParams = append(deviceParams, fmt.Sprintf("queue-size=%d", queueSize))
	if vhostuserDev.CacheSize != 0 {
		deviceParams = append(deviceParams, fmt.Sprintf("cache-size=%dM", vhostuserDev.CacheSize))
	}
	if vhostuserDev.Transport.isVirtioCCW(config) {
		if config.Knobs.IOMMUPlatform {
			deviceParams = append(deviceParams, "iommu_platform=on")
		}
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", vhostuserDev.DevNo))
	}
	if vhostuserDev.Transport.isVirtioPCI(config) && vhostuserDev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", vhostuserDev.ROMFile))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// QemuParams returns the qemu parameters built out of this vhostuser device.
func (vhostuserDev VhostUserDevice) QemuParams(config *Config) []string {
	var qemuParams []string
	var charParams []string
	var deviceParams []string

	charParams = append(charParams, "socket")
	charParams = append(charParams, fmt.Sprintf("id=%s", vhostuserDev.CharDevID))
	charParams = append(charParams, fmt.Sprintf("path=%s", vhostuserDev.SocketPath))

	qemuParams = append(qemuParams, "-chardev")
	qemuParams = append(qemuParams, strings.Join(charParams, ","))

	switch vhostuserDev.VhostUserType {
	case VhostUserNet:
		deviceParams = vhostuserDev.QemuNetParams(config)
	case VhostUserSCSI:
		deviceParams = vhostuserDev.QemuSCSIParams(config)
	case VhostUserBlk:
		deviceParams = vhostuserDev.QemuBlkParams(config)
	case VhostUserFS:
		deviceParams = vhostuserDev.QemuFSParams(config)
	default:
		return nil
	}

	if deviceParams != nil {
		return append(qemuParams, deviceParams...)
	}

	return nil
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (vhostuserDev VhostUserDevice) deviceName(config *Config) string {
	if vhostuserDev.Transport == "" {
		vhostuserDev.Transport = vhostuserDev.Transport.defaultTransport(config)
	}

	switch vhostuserDev.VhostUserType {
	case VhostUserNet:
		return VhostUserNetTransport[vhostuserDev.Transport]
	case VhostUserSCSI:
		return VhostUserSCSITransport[vhostuserDev.Transport]
	case VhostUserBlk:
		return VhostUserBlkTransport[vhostuserDev.Transport]
	case VhostUserFS:
		return VhostUserFSTransport[vhostuserDev.Transport]
	default:
		return ""
	}
}

// PCIeRootPortDevice represents a memory balloon device.
// nolint: govet
type PCIeRootPortDevice struct {
	ID string // format: rp{n}, n>=0

	Bus     string // default is pcie.0
	Chassis string // (slot, chassis) pair is mandatory and must be unique for each pcie-root-port, >=0, default is 0x00
	Slot    string // >=0, default is 0x00

	Multifunction bool   // true => "on", false => "off", default is off
	Addr          string // >=0, default is 0x00

	// The PCIE-PCI bridge can be hot-plugged only into pcie-root-port that has 'bus-reserve' property value to
	// provide secondary bus for the hot-plugged bridge.
	BusReserve    string
	Pref64Reserve string // reserve prefetched MMIO aperture, 64-bit
	Pref32Reserve string // reserve prefetched MMIO aperture, 32-bit
	MemReserve    string // reserve non-prefetched MMIO aperture, 32-bit *only*
	IOReserve     string // IO reservation

	ROMFile string // ROMFile specifies the ROM file being used for this device.

	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// QemuParams returns the qemu parameters built out of the PCIeRootPortDevice.
func (b PCIeRootPortDevice) QemuParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string
	driver := PCIeRootPort

	deviceParams = append(deviceParams, fmt.Sprintf("%s,id=%s", driver, b.ID))

	if b.Bus == "" {
		b.Bus = "pcie.0"
	}
	deviceParams = append(deviceParams, fmt.Sprintf("bus=%s", b.Bus))

	if b.Chassis == "" {
		b.Chassis = "0x00"
	}
	deviceParams = append(deviceParams, fmt.Sprintf("chassis=%s", b.Chassis))

	if b.Slot == "" {
		b.Slot = "0x00"
	}
	deviceParams = append(deviceParams, fmt.Sprintf("slot=%s", b.Slot))

	multifunction := "off"
	if b.Multifunction {
		multifunction = "on"
		if b.Addr == "" {
			b.Addr = "0x00"
		}
		deviceParams = append(deviceParams, fmt.Sprintf("addr=%s", b.Addr))
	}
	deviceParams = append(deviceParams, fmt.Sprintf("multifunction=%v", multifunction))

	if b.BusReserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("bus-reserve=%s", b.BusReserve))
	}

	if b.Pref64Reserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("pref64-reserve=%s", b.Pref64Reserve))
	}

	if b.Pref32Reserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("pref32-reserve=%s", b.Pref32Reserve))
	}

	if b.MemReserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("mem-reserve=%s", b.MemReserve))
	}

	if b.IOReserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("io-reserve=%s", b.IOReserve))
	}

	if b.Transport.isVirtioPCI(config) && b.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", b.ROMFile))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))
	return qemuParams
}

// Valid returns true if the PCIeRootPortDevice structure is valid and complete.
func (b PCIeRootPortDevice) Valid() bool {
	// the "pref32-reserve" and "pref64-reserve" hints are mutually exclusive.
	if b.Pref64Reserve != "" && b.Pref32Reserve != "" {
		return false
	}
	if b.ID == "" {
		return false
	}
	return true
}

// PCIeSwitchUpstreamPortDevice is the port connecting to the root port
type PCIeSwitchUpstreamPortDevice struct {
	ID  string // format: sup{n}, n>=0
	Bus string // default is rp0
}

// QemuParams returns the qemu parameters built out of the PCIeSwitchUpstreamPortDevice.
func (b PCIeSwitchUpstreamPortDevice) QemuParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string

	driver := PCIeSwitchUpstreamPort

	deviceParams = append(deviceParams, fmt.Sprintf("%s,id=%s", driver, b.ID))
	deviceParams = append(deviceParams, fmt.Sprintf("bus=%s", b.Bus))

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))
	return qemuParams
}

// Valid returns true if the PCIeSwitchUpstreamPortDevice structure is valid and complete.
func (b PCIeSwitchUpstreamPortDevice) Valid() bool {
	if b.ID == "" {
		return false
	}
	if b.Bus == "" {
		return false
	}
	return true
}

// PCIeSwitchDownstreamPortDevice is the port connecting to the root port
type PCIeSwitchDownstreamPortDevice struct {
	ID      string // format: sup{n}, n>=0
	Bus     string // default is rp0
	Chassis string // (slot, chassis) pair is mandatory and must be unique for each downstream port, >=0, default is 0x00
	Slot    string // >=0, default is 0x00
	// This to work needs patches to QEMU
	BusReserve string
	// Pref64 and Pref32 are not allowed to be set simultaneously
	Pref64Reserve string // reserve prefetched MMIO aperture, 64-bit
	Pref32Reserve string // reserve prefetched MMIO aperture, 32-bit
	MemReserve    string // reserve non-prefetched MMIO aperture, 32-bit *only*
	IOReserve     string // IO reservation

}

// QemuParams returns the qemu parameters built out of the PCIeSwitchUpstreamPortDevice.
func (b PCIeSwitchDownstreamPortDevice) QemuParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string
	driver := PCIeSwitchDownstreamPort

	deviceParams = append(deviceParams, fmt.Sprintf("%s,id=%s", driver, b.ID))
	deviceParams = append(deviceParams, fmt.Sprintf("bus=%s", b.Bus))
	deviceParams = append(deviceParams, fmt.Sprintf("chassis=%s", b.Chassis))
	deviceParams = append(deviceParams, fmt.Sprintf("slot=%s", b.Slot))
	if b.BusReserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("bus-reserve=%s", b.BusReserve))
	}

	if b.Pref64Reserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("pref64-reserve=%s", b.Pref64Reserve))
	}

	if b.Pref32Reserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("pref32-reserve=%s", b.Pref32Reserve))
	}

	if b.MemReserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("mem-reserve=%s", b.MemReserve))
	}

	if b.IOReserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("io-reserve=%s", b.IOReserve))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))
	return qemuParams
}

// Valid returns true if the PCIeSwitchUpstremPortDevice structure is valid and complete.
func (b PCIeSwitchDownstreamPortDevice) Valid() bool {
	if b.ID == "" {
		return false
	}
	if b.Bus == "" {
		return false
	}
	if b.Chassis == "" {
		return false
	}
	if b.Slot == "" {
		return false
	}
	return true
}

// VFIODevice represents a qemu vfio device meant for direct access by guest OS.
type VFIODevice struct {
	// Bus-Device-Function of device
	BDF string

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// VendorID specifies vendor id
	VendorID string

	// DeviceID specifies device id
	DeviceID string

	// Bus specifies device bus
	Bus string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport

	// SysfsDev specifies the sysfs matrix entry for the AP device
	SysfsDev string
}

// VFIODeviceTransport is a map of the vfio device name that corresponds to
// each transport.
var VFIODeviceTransport = map[VirtioTransport]string{
	TransportPCI:  "vfio-pci",
	TransportCCW:  "vfio-ccw",
	TransportMMIO: "vfio-device",
	TransportAP:   "vfio-ap",
}

// Valid returns true if the VFIODevice structure is valid and complete.
// s390x architecture requires SysfsDev to be set.
func (vfioDev VFIODevice) Valid() bool {
	return vfioDev.BDF != "" || vfioDev.SysfsDev != ""
}

// QemuParams returns the qemu parameters built out of this vfio device.
func (vfioDev VFIODevice) QemuParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string

	driver := vfioDev.deviceName(config)

	if vfioDev.Transport.isVirtioAP(config) {
		deviceParams = append(deviceParams, fmt.Sprintf("%s,sysfsdev=%s", driver, vfioDev.SysfsDev))

		qemuParams = append(qemuParams, "-device")
		qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

		return qemuParams
	}

	deviceParams = append(deviceParams, fmt.Sprintf("%s,host=%s", driver, vfioDev.BDF))
	if vfioDev.Transport.isVirtioPCI(config) {
		if vfioDev.VendorID != "" {
			deviceParams = append(deviceParams, fmt.Sprintf("x-pci-vendor-id=%s", vfioDev.VendorID))
		}
		if vfioDev.DeviceID != "" {
			deviceParams = append(deviceParams, fmt.Sprintf("x-pci-device-id=%s", vfioDev.DeviceID))
		}
		if vfioDev.ROMFile != "" {
			deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", vfioDev.ROMFile))
		}
	}

	if vfioDev.Bus != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("bus=%s", vfioDev.Bus))
	}

	if vfioDev.Transport.isVirtioCCW(config) {
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", vfioDev.DevNo))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (vfioDev VFIODevice) deviceName(config *Config) string {
	if vfioDev.Transport == "" {
		vfioDev.Transport = vfioDev.Transport.defaultTransport(config)
	}

	return VFIODeviceTransport[vfioDev.Transport]
}

// SCSIController represents a SCSI controller device.
// nolint: govet
type SCSIController struct {
	ID string

	// Bus on which the SCSI controller is attached, this is optional
	Bus string

	// Addr is the PCI address offset, this is optional
	Addr string

	// DisableModern prevents qemu from relying on fast MMIO.
	DisableModern bool

	// IOThread is the IO thread on which IO will be handled
	IOThread string

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// SCSIControllerTransport is a map of the virtio-scsi device name that
// corresponds to each transport.
var SCSIControllerTransport = map[VirtioTransport]string{
	TransportPCI:  "virtio-scsi-pci",
	TransportCCW:  "virtio-scsi-ccw",
	TransportMMIO: "virtio-scsi-device",
}

// Valid returns true if the SCSIController structure is valid and complete.
func (scsiCon SCSIController) Valid() bool {
	return scsiCon.ID != ""
}

// QemuParams returns the qemu parameters built out of this SCSIController device.
func (scsiCon SCSIController) QemuParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string

	driver := scsiCon.deviceName(config)
	deviceParams = append(deviceParams, fmt.Sprintf("%s,id=%s", driver, scsiCon.ID))
	if scsiCon.Bus != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("bus=%s", scsiCon.Bus))
	}
	if scsiCon.Addr != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("addr=%s", scsiCon.Addr))
	}
	if s := scsiCon.Transport.disableModern(config, scsiCon.DisableModern); s != "" {
		deviceParams = append(deviceParams, s)
	}
	if scsiCon.IOThread != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("iothread=%s", scsiCon.IOThread))
	}
	if scsiCon.Transport.isVirtioPCI(config) && scsiCon.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", scsiCon.ROMFile))
	}

	if scsiCon.Transport.isVirtioCCW(config) {
		if config.Knobs.IOMMUPlatform {
			deviceParams = append(deviceParams, "iommu_platform=on")
		}
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", scsiCon.DevNo))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (scsiCon SCSIController) deviceName(config *Config) string {
	if scsiCon.Transport == "" {
		scsiCon.Transport = scsiCon.Transport.defaultTransport(config)
	}

	return SCSIControllerTransport[scsiCon.Transport]
}

// BridgeType is the type of the bridge
type BridgeType uint

const (
	// PCIBridge is a pci bridge
	PCIBridge BridgeType = iota

	// PCIEBridge is a pcie bridge
	PCIEBridge
)

// BridgeDevice represents a qemu bridge device like pci-bridge, pxb, etc.
// nolint: govet
type BridgeDevice struct {
	// Type of the bridge
	Type BridgeType

	// Bus number where the bridge is plugged, typically pci.0 or pcie.0
	Bus string

	// ID is used to identify the bridge in qemu
	ID string

	// Chassis number
	Chassis int

	// SHPC is used to enable or disable the standard hot plug controller
	SHPC bool

	// PCI Slot
	Addr string

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// Address range reservations for devices behind the bridge
	// NB: strings seem an odd choice, but if they were integers,
	// they'd default to 0 by Go's rules in all the existing users
	// who don't set them.  0 is a valid value for certain cases,
	// but not you want by default.
	IOReserve     string
	MemReserve    string
	Pref64Reserve string
}

// Valid returns true if the BridgeDevice structure is valid and complete.
func (bridgeDev BridgeDevice) Valid() bool {
	if bridgeDev.Type != PCIBridge && bridgeDev.Type != PCIEBridge {
		return false
	}

	if bridgeDev.Bus == "" {
		return false
	}

	if bridgeDev.ID == "" {
		return false
	}

	return true
}

// QemuParams returns the qemu parameters built out of this bridge device.
func (bridgeDev BridgeDevice) QemuParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string
	var driver DeviceDriver

	switch bridgeDev.Type {
	case PCIEBridge:
		driver = PCIePCIBridgeDriver
		deviceParams = append(deviceParams, fmt.Sprintf("%s,bus=%s,id=%s", driver, bridgeDev.Bus, bridgeDev.ID))
	default:
		driver = PCIBridgeDriver
		shpc := "off"
		if bridgeDev.SHPC {
			shpc = "on"
		}
		deviceParams = append(deviceParams, fmt.Sprintf("%s,bus=%s,id=%s,chassis_nr=%d,shpc=%s", driver, bridgeDev.Bus, bridgeDev.ID, bridgeDev.Chassis, shpc))
	}

	if bridgeDev.Addr != "" {
		addr, err := strconv.Atoi(bridgeDev.Addr)
		if err == nil && addr >= 0 {
			deviceParams = append(deviceParams, fmt.Sprintf("addr=%x", addr))
		}
	}

	var transport VirtioTransport
	if transport.isVirtioPCI(config) && bridgeDev.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", bridgeDev.ROMFile))
	}

	if bridgeDev.IOReserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("io-reserve=%s", bridgeDev.IOReserve))
	}
	if bridgeDev.MemReserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("mem-reserve=%s", bridgeDev.MemReserve))
	}
	if bridgeDev.Pref64Reserve != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("pref64-reserve=%s", bridgeDev.Pref64Reserve))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// VSOCKDevice represents a AF_VSOCK socket.
// nolint: govet
type VSOCKDevice struct {
	ID string

	ContextID uint64

	// VHostFD vhost file descriptor that holds the ContextID
	VHostFD *os.File

	// DisableModern prevents qemu from relying on fast MMIO.
	DisableModern bool

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// VSOCKDeviceTransport is a map of the vhost-vsock device name that
// corresponds to each transport.
var VSOCKDeviceTransport = map[VirtioTransport]string{
	TransportPCI:  "vhost-vsock-pci",
	TransportCCW:  "vhost-vsock-ccw",
	TransportMMIO: "vhost-vsock-device",
}

const (
	// MinimalGuestCID is the smallest valid context ID for a guest.
	MinimalGuestCID uint64 = 3

	// MaxGuestCID is the largest valid context ID for a guest.
	MaxGuestCID uint64 = 1<<32 - 1
)

const (
	// VSOCKGuestCID is the VSOCK guest CID parameter.
	VSOCKGuestCID = "guest-cid"
)

// Valid returns true if the VSOCKDevice structure is valid and complete.
func (vsock VSOCKDevice) Valid() bool {
	if vsock.ID == "" || vsock.ContextID < MinimalGuestCID || vsock.ContextID > MaxGuestCID {
		return false
	}

	return true
}

// QemuParams returns the qemu parameters built out of the VSOCK device.
func (vsock VSOCKDevice) QemuParams(config *Config) []string {
	var deviceParams []string
	var qemuParams []string

	driver := vsock.deviceName(config)
	deviceParams = append(deviceParams, driver)
	if s := vsock.Transport.disableModern(config, vsock.DisableModern); s != "" {
		deviceParams = append(deviceParams, s)
	}
	if vsock.VHostFD != nil {
		qemuFDs := config.appendFDs([]*os.File{vsock.VHostFD})
		deviceParams = append(deviceParams, fmt.Sprintf("vhostfd=%d", qemuFDs[0]))
	}
	deviceParams = append(deviceParams, fmt.Sprintf("id=%s", vsock.ID))
	deviceParams = append(deviceParams, fmt.Sprintf("%s=%d", VSOCKGuestCID, vsock.ContextID))

	if vsock.Transport.isVirtioPCI(config) && vsock.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", vsock.ROMFile))
	}

	if vsock.Transport.isVirtioCCW(config) {
		if config.Knobs.IOMMUPlatform {
			deviceParams = append(deviceParams, "iommu_platform=on")
		}
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", vsock.DevNo))
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (vsock VSOCKDevice) deviceName(config *Config) string {
	if vsock.Transport == "" {
		vsock.Transport = vsock.Transport.defaultTransport(config)
	}

	return VSOCKDeviceTransport[vsock.Transport]
}

// RngDevice represents a random number generator device.
// nolint: govet
type RngDevice struct {
	// ID is the device ID
	ID string
	// Filename is entropy source on the host
	Filename string
	// MaxBytes is the bytes allowed to guest to get from the host’s entropy per period
	MaxBytes uint
	// Period is duration of a read period in seconds
	Period uint
	// ROMFile specifies the ROM file being used for this device.
	ROMFile string
	// DevNo identifies the ccw devices for s390x architecture
	DevNo string
	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// RngDeviceTransport is a map of the virtio-rng device name that corresponds
// to each transport.
var RngDeviceTransport = map[VirtioTransport]string{
	TransportPCI:  "virtio-rng-pci",
	TransportCCW:  "virtio-rng-ccw",
	TransportMMIO: "virtio-rng-device",
}

// Valid returns true if the RngDevice structure is valid and complete.
func (v RngDevice) Valid() bool {
	return v.ID != ""
}

// QemuParams returns the qemu parameters built out of the RngDevice.
func (v RngDevice) QemuParams(config *Config) []string {
	var qemuParams []string

	//-object rng-random,filename=/dev/hwrng,id=rng0
	var objectParams []string
	//-device virtio-rng-pci,rng=rng0,max-bytes=1024,period=1000
	var deviceParams []string

	objectParams = append(objectParams, "rng-random")
	objectParams = append(objectParams, "id="+v.ID)

	deviceParams = append(deviceParams, v.deviceName(config))
	deviceParams = append(deviceParams, "rng="+v.ID)

	if v.Transport.isVirtioPCI(config) && v.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", v.ROMFile))
	}

	if v.Transport.isVirtioCCW(config) {
		if config.Knobs.IOMMUPlatform {
			deviceParams = append(deviceParams, "iommu_platform=on")
		}
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", v.DevNo))
	}

	if v.Filename != "" {
		objectParams = append(objectParams, "filename="+v.Filename)
	}

	if v.MaxBytes > 0 {
		deviceParams = append(deviceParams, fmt.Sprintf("max-bytes=%d", v.MaxBytes))
	}

	if v.Period > 0 {
		deviceParams = append(deviceParams, fmt.Sprintf("period=%d", v.Period))
	}

	qemuParams = append(qemuParams, "-object")
	qemuParams = append(qemuParams, strings.Join(objectParams, ","))

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (v RngDevice) deviceName(config *Config) string {
	if v.Transport == "" {
		v.Transport = v.Transport.defaultTransport(config)
	}

	return RngDeviceTransport[v.Transport]
}

// BalloonDevice represents a memory balloon device.
// nolint: govet
type BalloonDevice struct {
	DeflateOnOOM  bool
	DisableModern bool
	ID            string

	// ROMFile specifies the ROM file being used for this device.
	ROMFile string

	// DevNo identifies the ccw devices for s390x architecture
	DevNo string

	// Transport is the virtio transport for this device.
	Transport VirtioTransport
}

// BalloonDeviceTransport is a map of the virtio-balloon device name that
// corresponds to each transport.
var BalloonDeviceTransport = map[VirtioTransport]string{
	TransportPCI:  "virtio-balloon-pci",
	TransportCCW:  "virtio-balloon-ccw",
	TransportMMIO: "virtio-balloon-device",
}

// QemuParams returns the qemu parameters built out of the BalloonDevice.
func (b BalloonDevice) QemuParams(config *Config) []string {
	var qemuParams []string
	var deviceParams []string

	deviceParams = append(deviceParams, b.deviceName(config))

	if b.ID != "" {
		deviceParams = append(deviceParams, "id="+b.ID)
	}

	if b.Transport.isVirtioPCI(config) && b.ROMFile != "" {
		deviceParams = append(deviceParams, fmt.Sprintf("romfile=%s", b.ROMFile))
	}

	if b.Transport.isVirtioCCW(config) {
		deviceParams = append(deviceParams, fmt.Sprintf("devno=%s", b.DevNo))
	}

	if b.DeflateOnOOM {
		deviceParams = append(deviceParams, "deflate-on-oom=on")
	} else {
		deviceParams = append(deviceParams, "deflate-on-oom=off")
	}
	if s := b.Transport.disableModern(config, b.DisableModern); s != "" {
		deviceParams = append(deviceParams, s)
	}
	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))

	return qemuParams
}

// Valid returns true if the balloonDevice structure is valid and complete.
func (b BalloonDevice) Valid() bool {
	return b.ID != ""
}

// deviceName returns the QEMU device name for the current combination of
// driver and transport.
func (b BalloonDevice) deviceName(config *Config) string {
	if b.Transport == "" {
		b.Transport = b.Transport.defaultTransport(config)
	}

	return BalloonDeviceTransport[b.Transport]
}

// IommuDev represents a Intel IOMMU Device
type IommuDev struct {
	Intremap    bool
	DeviceIotlb bool
	CachingMode bool
}

// Valid returns true if the IommuDev is valid
func (dev IommuDev) Valid() bool {
	return true
}

// deviceName the qemu device name
func (dev IommuDev) deviceName() string {
	return "intel-iommu"
}

// QemuParams returns the qemu parameters built out of the IommuDev.
func (dev IommuDev) QemuParams(_ *Config) []string {
	var qemuParams []string
	var deviceParams []string

	deviceParams = append(deviceParams, dev.deviceName())
	if dev.Intremap {
		deviceParams = append(deviceParams, "intremap=on")
	} else {
		deviceParams = append(deviceParams, "intremap=off")
	}

	if dev.DeviceIotlb {
		deviceParams = append(deviceParams, "device-iotlb=on")
	} else {
		deviceParams = append(deviceParams, "device-iotlb=off")
	}

	if dev.CachingMode {
		deviceParams = append(deviceParams, "caching-mode=on")
	} else {
		deviceParams = append(deviceParams, "caching-mode=off")
	}

	qemuParams = append(qemuParams, "-device")
	qemuParams = append(qemuParams, strings.Join(deviceParams, ","))
	return qemuParams
}

// RTCBaseType is the qemu RTC base time type.
type RTCBaseType string

// RTCClock is the qemu RTC clock type.
type RTCClock string

// RTCDriftFix is the qemu RTC drift fix type.
type RTCDriftFix string

const (
	// UTC is the UTC base time for qemu RTC.
	UTC RTCBaseType = "utc"

	// LocalTime is the local base time for qemu RTC.
	LocalTime RTCBaseType = "localtime"
)

const (
	// Host is for using the host clock as a reference.
	Host RTCClock = "host"

	// RT is for using the host monotonic clock as a reference.
	RT RTCClock = "rt"

	// VM is for using the guest clock as a reference
	VM RTCClock = "vm"
)

const (
	// Slew is the qemu RTC Drift fix mechanism.
	Slew RTCDriftFix = "slew"

	// NoDriftFix means we don't want/need to fix qemu's RTC drift.
	NoDriftFix RTCDriftFix = "none"
)

// RTC represents a qemu Real Time Clock configuration.
type RTC struct {
	// Base is the RTC start time.
	Base RTCBaseType

	// Clock is the is the RTC clock driver.
	Clock RTCClock

	// DriftFix is the drift fixing mechanism.
	DriftFix RTCDriftFix
}

// Valid returns true if the RTC structure is valid and complete.
func (rtc RTC) Valid() bool {
	if rtc.Clock != Host && rtc.Clock != RT && rtc.Clock != VM {
		return false
	}

	if rtc.DriftFix != Slew && rtc.DriftFix != NoDriftFix {
		return false
	}

	return true
}

// QMPSocketType is the type of socket used for QMP communication.
type QMPSocketType string

const (
	// Unix socket for QMP.
	Unix QMPSocketType = "unix"
)

// MonitorProtocol tells what protocol is used on a QMPSocket
type MonitorProtocol string

const (
	// Socket using a human-friendly text-based protocol.
	Hmp MonitorProtocol = "hmp"

	// Socket using a richer json-based protocol.
	Qmp MonitorProtocol = "qmp"

	// Same as Qmp with pretty json formatting.
	QmpPretty MonitorProtocol = "qmp-pretty"
)

// QMPSocket represents a qemu QMP or HMP socket configuration.
// nolint: govet
type QMPSocket struct {
	// Type is the socket type (e.g. "unix").
	Type QMPSocketType

	// Protocol is the protocol to be used on the socket.
	Protocol MonitorProtocol

	// QMP listener file descriptor to be passed to qemu
	FD *os.File

	// Name is the socket name.
	Name string

	// Server tells if this is a server socket.
	Server bool

	// NoWait tells if qemu should block waiting for a client to connect.
	NoWait bool
}

// Valid returns true if the QMPSocket structure is valid and complete.
func (qmp QMPSocket) Valid() bool {
	// Exactly one of Name of FD must be set.
	if qmp.Type == "" || (qmp.Name == "") == (qmp.FD == nil) {
		return false
	}

	if qmp.Type != Unix {
		return false
	}

	if qmp.Protocol != Hmp && qmp.Protocol != Qmp && qmp.Protocol != QmpPretty {
		return false
	}

	return true
}

// SMP is the multi processors configuration structure.
type SMP struct {
	// CPUs is the number of VCPUs made available to qemu.
	CPUs uint32

	// Cores is the number of cores made available to qemu.
	Cores uint32

	// Threads is the number of threads made available to qemu.
	Threads uint32

	// Sockets is the number of sockets made available to qemu.
	Sockets uint32

	// MaxCPUs is the maximum number of VCPUs that a VM can have.
	// This value, if non-zero, MUST BE equal to or greater than CPUs
	MaxCPUs uint32
}

// Memory is the guest memory configuration structure.
// nolint: govet
type Memory struct {
	// Size is the amount of memory made available to the guest.
	// It should be suffixed with M or G for sizes in megabytes or
	// gigabytes respectively.
	Size string

	// Slots is the amount of memory slots made available to the guest.
	Slots uint8

	// MaxMem is the maximum amount of memory that can be made available
	// to the guest through e.g. hot pluggable memory.
	MaxMem string

	// Path is the file path of the memory device. It points to a local
	// file path used by FileBackedMem.
	Path string
}

// Kernel is the guest kernel configuration structure.
type Kernel struct {
	// Path is the guest kernel path on the host filesystem.
	Path string

	// InitrdPath is the guest initrd path on the host filesystem.
	InitrdPath string

	// Params is the kernel parameters string.
	Params string
}

// FwCfg allows QEMU to pass entries to the guest
// File and Str are mutually exclusive
type FwCfg struct {
	Name string
	File string
	Str  string
}

// Valid returns true if the FwCfg structure is valid and complete.
func (fwcfg FwCfg) Valid() bool {
	if fwcfg.Name == "" {
		return false
	}

	if fwcfg.File != "" && fwcfg.Str != "" {
		return false
	}

	if fwcfg.File == "" && fwcfg.Str == "" {
		return false
	}

	return true
}

// QemuParams returns the qemu parameters built out of the FwCfg object
func (fwcfg FwCfg) QemuParams(config *Config) []string {
	var fwcfgParams []string
	var qemuParams []string

	for _, f := range config.FwCfg {
		if f.Name != "" {
			fwcfgParams = append(fwcfgParams, fmt.Sprintf("name=%s", f.Name))

			if f.File != "" {
				fwcfgParams = append(fwcfgParams, fmt.Sprintf("file=%s", f.File))
			}

			if f.Str != "" {
				fwcfgParams = append(fwcfgParams, fmt.Sprintf("string=%s", f.Str))
			}
		}

		qemuParams = append(qemuParams, "-fw_cfg")
		qemuParams = append(qemuParams, strings.Join(fwcfgParams, ","))
	}

	return qemuParams
}

// Knobs regroups a set of qemu boolean settings
type Knobs struct {
	// NoUserConfig prevents qemu from loading user config files.
	NoUserConfig bool

	// NoDefaults prevents qemu from creating default devices.
	NoDefaults bool

	// NoGraphic completely disables graphic output.
	NoGraphic bool

	// Both HugePages and MemPrealloc require the Memory.Size of the VM
	// to be set, as they need to reserve the memory upfront in order
	// for the VM to boot without errors.
	//
	// HugePages always results in memory pre-allocation.
	// However the setup is different from normal pre-allocation.
	// Hence HugePages has precedence over MemPrealloc
	// HugePages will pre-allocate all the RAM from huge pages
	HugePages bool

	// MemPrealloc will allocate all the RAM upfront
	MemPrealloc bool

	// FileBackedMem requires Memory.Size and Memory.Path of the VM to
	// be set.
	FileBackedMem bool

	// MemShared will set the memory device as shared.
	MemShared bool

	// Mlock will control locking of memory
	Mlock bool

	// Stopped will not start guest CPU at startup
	Stopped bool

	// Exit instead of rebooting
	// Prevents QEMU from rebooting in the event of a Triple Fault.
	NoReboot bool

	// IOMMUPlatform will enable IOMMU for supported devices
	IOMMUPlatform bool
}

// IOThread allows IO to be performed on a separate thread.
type IOThread struct {
	ID string
}

const (
	// MigrationFD is the migration incoming type based on open file descriptor.
	// Skip default 0 so that it must be set on purpose.
	MigrationFD = 1
	// MigrationExec is the migration incoming type based on commands.
	MigrationExec = 2
	// MigrationDefer is the defer incoming type
	MigrationDefer = 3
)

// Incoming controls migration source preparation
// nolint: govet
type Incoming struct {
	// Possible values are MigrationFD, MigrationExec
	MigrationType int
	// Only valid if MigrationType == MigrationFD
	FD *os.File
	// Only valid if MigrationType == MigrationExec
	Exec string
}

// Config is the qemu configuration structure.
// It allows for passing custom settings and parameters to the qemu API.
// nolint: govet
type Config struct {
	// Path is the qemu binary path.
	Path string

	// Ctx is the context used when launching qemu.
	Ctx context.Context

	// User ID.
	Uid uint32
	// Group ID.
	Gid uint32
	// Supplementary group IDs.
	Groups []uint32

	// Name is the qemu guest name
	Name string

	// UUID is the qemu process UUID.
	UUID string

	// CPUModel is the CPU model to be used by qemu.
	CPUModel string

	// SeccompSandbox is the qemu function which enables the seccomp feature
	SeccompSandbox string

	// Machine
	Machine Machine

	// QMPSockets is a slice of QMP socket description.
	QMPSockets []QMPSocket

	// Devices is a list of devices for qemu to create and drive.
	Devices []Device

	// RTC is the qemu Real Time Clock configuration
	RTC RTC

	// VGA is the qemu VGA mode.
	VGA string

	// Kernel is the guest kernel configuration.
	Kernel Kernel

	// Memory is the guest memory configuration.
	Memory Memory

	// SMP is the quest multi processors configuration.
	SMP SMP

	// GlobalParam is the -global parameter.
	GlobalParam string

	// Knobs is a set of qemu boolean settings.
	Knobs Knobs

	// Bios is the -bios parameter
	Bios string

	// PFlash specifies the parallel flash images (-pflash parameter)
	PFlash []string

	// Incoming controls migration source preparation
	Incoming Incoming

	// fds is a list of open file descriptors to be passed to the spawned qemu process
	fds []*os.File

	// FwCfg is the -fw_cfg parameter
	FwCfg []FwCfg

	IOThreads []IOThread

	// PidFile is the -pidfile parameter
	PidFile string

	qemuParams []string

	Debug bool
}

// appendFDs appends a list of arbitrary file descriptors to the qemu configuration and
// returns a slice of consecutive file descriptors that will be seen by the qemu process.
// Please see the comment below for details.
func (config *Config) appendFDs(fds []*os.File) []int {
	var fdInts []int

	oldLen := len(config.fds)

	config.fds = append(config.fds, fds...)

	// The magic 3 offset comes from https://golang.org/src/os/exec/exec.go:
	//     ExtraFiles specifies additional open files to be inherited by the
	//     new process. It does not include standard input, standard output, or
	//     standard error. If non-nil, entry i becomes file descriptor 3+i.
	// This means that arbitrary file descriptors fd0, fd1... fdN passed in
	// the array will be presented to the guest as consecutive descriptors
	// 3, 4... N+3. The golang library internally relies on dup2() to do
	// the renumbering.
	for i := range fds {
		fdInts = append(fdInts, oldLen+3+i)
	}

	return fdInts
}

func (config *Config) appendSeccompSandbox() {
	if config.SeccompSandbox != "" {
		config.qemuParams = append(config.qemuParams, "-sandbox")
		config.qemuParams = append(config.qemuParams, config.SeccompSandbox)
	}
}

func (config *Config) appendName() {
	if config.Name != "" {
		var nameParams []string
		nameParams = append(nameParams, config.Name)

		if config.Debug {
			nameParams = append(nameParams, "debug-threads=on")
		}

		config.qemuParams = append(config.qemuParams, "-name")
		config.qemuParams = append(config.qemuParams, strings.Join(nameParams, ","))
	}
}

func (config *Config) appendMachine() {
	if config.Machine.Type != "" {
		var machineParams []string

		machineParams = append(machineParams, config.Machine.Type)

		if config.Machine.Acceleration != "" {
			machineParams = append(machineParams, fmt.Sprintf("accel=%s", config.Machine.Acceleration))
		}

		if config.Machine.Options != "" {
			machineParams = append(machineParams, config.Machine.Options)
		}

		config.qemuParams = append(config.qemuParams, "-machine")
		config.qemuParams = append(config.qemuParams, strings.Join(machineParams, ","))
	}
}

func (config *Config) appendCPUModel() {
	if config.CPUModel != "" {
		config.qemuParams = append(config.qemuParams, "-cpu")
		config.qemuParams = append(config.qemuParams, config.CPUModel)
	}
}

func (config *Config) appendQMPSockets() {
	for _, q := range config.QMPSockets {
		if !q.Valid() {
			continue
		}

		var qmpParams []string
		if q.FD != nil {
			qemuFDs := config.appendFDs([]*os.File{q.FD})
			qmpParams = append([]string{}, fmt.Sprintf("%s:fd=%d", q.Type, qemuFDs[0]))
		} else {
			qmpParams = append([]string{}, fmt.Sprintf("%s:path=%s", q.Type, q.Name))
		}
		if q.Server {
			qmpParams = append(qmpParams, "server=on")
			if q.NoWait {
				qmpParams = append(qmpParams, "wait=off")
			}
		}

		switch q.Protocol {
		case Hmp:
			config.qemuParams = append(config.qemuParams, "-monitor")
		default:
			config.qemuParams = append(config.qemuParams, fmt.Sprintf("-%s", q.Protocol))
		}

		config.qemuParams = append(config.qemuParams, strings.Join(qmpParams, ","))
	}
}

func (config *Config) appendDevices(logger QMPLog) {
	if logger == nil {
		logger = qmpNullLogger{}
	}

	for _, d := range config.Devices {
		if !d.Valid() {
			logger.Errorf("vm device is not valid: %+v", d)
			continue
		}
		config.qemuParams = append(config.qemuParams, d.QemuParams(config)...)
	}
}

func (config *Config) appendUUID() {
	if config.UUID != "" {
		config.qemuParams = append(config.qemuParams, "-uuid")
		config.qemuParams = append(config.qemuParams, config.UUID)
	}
}

func (config *Config) appendMemory() {
	if config.Memory.Size != "" {
		var memoryParams []string

		memoryParams = append(memoryParams, config.Memory.Size)

		if config.Memory.Slots > 0 {
			memoryParams = append(memoryParams, fmt.Sprintf("slots=%d", config.Memory.Slots))
		}

		if config.Memory.MaxMem != "" {
			memoryParams = append(memoryParams, fmt.Sprintf("maxmem=%s", config.Memory.MaxMem))
		}

		config.qemuParams = append(config.qemuParams, "-m")
		config.qemuParams = append(config.qemuParams, strings.Join(memoryParams, ","))
	}
}

func (config *Config) appendCPUs() error {
	if config.SMP.CPUs > 0 {
		var SMPParams []string

		SMPParams = append(SMPParams, fmt.Sprintf("%d", config.SMP.CPUs))

		if config.SMP.Cores > 0 {
			SMPParams = append(SMPParams, fmt.Sprintf("cores=%d", config.SMP.Cores))
		}

		if config.SMP.Threads > 0 {
			SMPParams = append(SMPParams, fmt.Sprintf("threads=%d", config.SMP.Threads))
		}

		if config.SMP.Sockets > 0 {
			SMPParams = append(SMPParams, fmt.Sprintf("sockets=%d", config.SMP.Sockets))
		}

		if config.SMP.MaxCPUs > 0 {
			if config.SMP.MaxCPUs < config.SMP.CPUs {
				return fmt.Errorf("MaxCPUs %d must be equal to or greater than CPUs %d",
					config.SMP.MaxCPUs, config.SMP.CPUs)
			}
			SMPParams = append(SMPParams, fmt.Sprintf("maxcpus=%d", config.SMP.MaxCPUs))
		}

		config.qemuParams = append(config.qemuParams, "-smp")
		config.qemuParams = append(config.qemuParams, strings.Join(SMPParams, ","))
	}

	return nil
}

func (config *Config) appendRTC() {
	if !config.RTC.Valid() {
		return
	}

	var RTCParams []string

	RTCParams = append(RTCParams, fmt.Sprintf("base=%s", string(config.RTC.Base)))

	if config.RTC.DriftFix != "" {
		RTCParams = append(RTCParams, fmt.Sprintf("driftfix=%s", config.RTC.DriftFix))
	}

	if config.RTC.Clock != "" {
		RTCParams = append(RTCParams, fmt.Sprintf("clock=%s", config.RTC.Clock))
	}

	config.qemuParams = append(config.qemuParams, "-rtc")
	config.qemuParams = append(config.qemuParams, strings.Join(RTCParams, ","))
}

func (config *Config) appendGlobalParam() {
	if config.GlobalParam != "" {
		config.qemuParams = append(config.qemuParams, "-global")
		config.qemuParams = append(config.qemuParams, config.GlobalParam)
	}
}

func (config *Config) appendPFlashParam() {
	for _, p := range config.PFlash {
		config.qemuParams = append(config.qemuParams, "-pflash")
		config.qemuParams = append(config.qemuParams, p)
	}
}

func (config *Config) appendVGA() {
	if config.VGA != "" {
		config.qemuParams = append(config.qemuParams, "-vga")
		config.qemuParams = append(config.qemuParams, config.VGA)
	}
}

func (config *Config) appendKernel() {
	if config.Kernel.Path != "" {
		config.qemuParams = append(config.qemuParams, "-kernel")
		config.qemuParams = append(config.qemuParams, config.Kernel.Path)

		if config.Kernel.InitrdPath != "" {
			config.qemuParams = append(config.qemuParams, "-initrd")
			config.qemuParams = append(config.qemuParams, config.Kernel.InitrdPath)
		}

		if config.Kernel.Params != "" {
			config.qemuParams = append(config.qemuParams, "-append")
			config.qemuParams = append(config.qemuParams, config.Kernel.Params)
		}
	}
}

func (config *Config) appendMemoryKnobs() {
	if config.Memory.Size == "" {
		return
	}
	var objMemParam, numaMemParam string
	dimmName := "dimm1"
	if config.Knobs.HugePages {
		objMemParam = "memory-backend-file,id=" + dimmName + ",size=" + config.Memory.Size + ",mem-path=/dev/hugepages"
		numaMemParam = "node,memdev=" + dimmName
	} else if config.Knobs.FileBackedMem && config.Memory.Path != "" {
		objMemParam = "memory-backend-file,id=" + dimmName + ",size=" + config.Memory.Size + ",mem-path=" + config.Memory.Path
		numaMemParam = "node,memdev=" + dimmName
	} else {
		objMemParam = "memory-backend-ram,id=" + dimmName + ",size=" + config.Memory.Size
		numaMemParam = "node,memdev=" + dimmName
	}

	if config.Knobs.MemShared {
		objMemParam += ",share=on"
	}
	if config.Knobs.MemPrealloc {
		objMemParam += ",prealloc=on"
	}
	config.qemuParams = append(config.qemuParams, "-object")
	config.qemuParams = append(config.qemuParams, objMemParam)

	if isDimmSupported(config) {
		config.qemuParams = append(config.qemuParams, "-numa")
		config.qemuParams = append(config.qemuParams, numaMemParam)
	} else {
		config.qemuParams = append(config.qemuParams, "-machine")
		config.qemuParams = append(config.qemuParams, "memory-backend="+dimmName)
	}
}

func (config *Config) appendKnobs() {
	if config.Knobs.NoUserConfig {
		config.qemuParams = append(config.qemuParams, "-no-user-config")
	}

	if config.Knobs.NoDefaults {
		config.qemuParams = append(config.qemuParams, "-nodefaults")
	}

	if config.Knobs.NoGraphic {
		config.qemuParams = append(config.qemuParams, "-nographic")
	}

	if config.Knobs.NoReboot {
		config.qemuParams = append(config.qemuParams, "--no-reboot")
	}

	config.appendMemoryKnobs()

	if config.Knobs.Mlock {
		config.qemuParams = append(config.qemuParams, "-overcommit")
		config.qemuParams = append(config.qemuParams, "mem-lock=on")
	}

	if config.Knobs.Stopped {
		config.qemuParams = append(config.qemuParams, "-S")
	}
}

func (config *Config) appendBios() {
	if config.Bios != "" {
		config.qemuParams = append(config.qemuParams, "-bios")
		config.qemuParams = append(config.qemuParams, config.Bios)
	}
}

func (config *Config) appendIOThreads() {
	for _, t := range config.IOThreads {
		if t.ID != "" {
			config.qemuParams = append(config.qemuParams, "-object")
			config.qemuParams = append(config.qemuParams, fmt.Sprintf("iothread,id=%s", t.ID))
		}
	}
}

func (config *Config) appendIncoming() {
	var uri string
	switch config.Incoming.MigrationType {
	case MigrationExec:
		uri = fmt.Sprintf("exec:%s", config.Incoming.Exec)
	case MigrationFD:
		chFDs := config.appendFDs([]*os.File{config.Incoming.FD})
		uri = fmt.Sprintf("fd:%d", chFDs[0])
	case MigrationDefer:
		uri = "defer"
	default:
		return
	}
	config.qemuParams = append(config.qemuParams, "-S", "-incoming", uri)
}

func (config *Config) appendPidFile() {
	if config.PidFile != "" {
		config.qemuParams = append(config.qemuParams, "-pidfile")
		config.qemuParams = append(config.qemuParams, config.PidFile)
	}
}

func (config *Config) appendFwCfg(logger QMPLog) {
	if logger == nil {
		logger = qmpNullLogger{}
	}

	for _, f := range config.FwCfg {
		if !f.Valid() {
			logger.Errorf("fw_cfg is not valid: %+v", config.FwCfg)
			continue
		}

		config.qemuParams = append(config.qemuParams, f.QemuParams(config)...)
	}
}

// LaunchQemu can be used to launch a new qemu instance.
//
// The Config parameter contains a set of qemu parameters and settings.
//
// See LaunchCustomQemu for more information.
func LaunchQemu(config Config, logger QMPLog) (*exec.Cmd, io.ReadCloser, error) {
	config.appendName()
	config.appendUUID()
	config.appendMachine()
	config.appendCPUModel()
	config.appendQMPSockets()
	config.appendMemory()
	config.appendDevices(logger)
	config.appendRTC()
	config.appendGlobalParam()
	config.appendPFlashParam()
	config.appendVGA()
	config.appendKnobs()
	config.appendKernel()
	config.appendBios()
	config.appendIOThreads()
	config.appendIncoming()
	config.appendPidFile()
	config.appendFwCfg(logger)
	config.appendSeccompSandbox()

	if err := config.appendCPUs(); err != nil {
		return nil, nil, err
	}

	ctx := config.Ctx
	if ctx == nil {
		ctx = context.Background()
	}

	attr := syscall.SysProcAttr{}
	attr.Credential = &syscall.Credential{
		Uid:    config.Uid,
		Gid:    config.Gid,
		Groups: config.Groups,
	}

	return LaunchCustomQemu(ctx, config.Path, config.qemuParams,
		config.fds, &attr, logger)
}

// LaunchCustomQemu can be used to launch a new qemu instance.
//
// The path parameter is used to pass the qemu executable path.
//
// params is a slice of options to pass to qemu-system-x86_64 and fds is a
// list of open file descriptors that are to be passed to the spawned qemu
// process.  The attrs parameter can be used to control aspects of the
// newly created qemu process, such as the user and group under which it
// runs.  It may be nil.
//
// This function writes its log output via logger parameter.
//
// The function returns cmd, reader, nil where cmd is a Go exec.Cmd object
// representing the QEMU process and reader a Go io.ReadCloser object
// connected to QEMU's stderr, if launched successfully. Otherwise
// nil, nil, err where err is a Go error object is returned.
func LaunchCustomQemu(ctx context.Context, path string, params []string, fds []*os.File,
	attr *syscall.SysProcAttr, logger QMPLog) (*exec.Cmd, io.ReadCloser, error) {
	if logger == nil {
		logger = qmpNullLogger{}
	}

	if path == "" {
		path = "qemu-system-x86_64"
	}

	/* #nosec */
	cmd := exec.CommandContext(ctx, path, params...)
	if len(fds) > 0 {
		logger.Infof("Adding extra file %v", fds)
		cmd.ExtraFiles = fds
	}

	cmd.SysProcAttr = attr

	reader, err := cmd.StderrPipe()
	if err != nil {
		logger.Errorf("Unable to connect stderr to a pipe")
		return nil, nil, err
	}
	logger.Infof("launching %s with: %v", path, params)

	err = cmd.Start()
	if err != nil {
		logger.Errorf("Unable to launch %s: %v", path, err)
		return nil, nil, err
	}
	return cmd, reader, nil
}
