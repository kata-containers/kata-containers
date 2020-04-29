// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"os/exec"
	"strings"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/sirupsen/logrus"
)

type acrnArch interface {

	// acrnPath returns the path to the acrn binary
	acrnPath() (string, error)

	// acrnctlPath returns the path to the acrnctl binary
	acrnctlPath() (string, error)

	// kernelParameters returns the kernel parameters
	// if debug is true then kernel debug parameters are included
	kernelParameters(debug bool) []Param

	//capabilities returns the capabilities supported by acrn
	capabilities() types.Capabilities

	// memoryTopology returns the memory topology using the given amount of memoryMb and hostMemoryMb
	memoryTopology(memMb uint64) Memory

	// appendConsole appends a console to devices
	appendConsole(devices []Device, path string) []Device

	// appendImage appends an image to devices
	appendImage(devices []Device, path string) ([]Device, error)

	// appendBridges appends bridges to devices
	appendBridges(devices []Device) []Device

	// appendLPC appends LPC to devices
	// UART device emulated by the acrn-dm is connected to the system by the LPC bus
	appendLPC(devices []Device) []Device

	// appendSocket appends a socket to devices
	appendSocket(devices []Device, socket types.Socket) []Device

	// appendNetwork appends a endpoint device to devices
	appendNetwork(devices []Device, endpoint Endpoint) []Device

	// appendBlockDevice appends a block drive to devices
	appendBlockDevice(devices []Device, drive config.BlockDrive) []Device

	// handleImagePath handles the Hypervisor Config image path
	handleImagePath(config HypervisorConfig)
}

type acrnArchBase struct {
	path                 string
	ctlpath              string
	kernelParamsNonDebug []Param
	kernelParamsDebug    []Param
	kernelParams         []Param
}

const acrnPath = "/usr/bin/acrn-dm"
const acrnctlPath = "/usr/bin/acrnctl"

// acrn GVT-g slot is harded code to 2 as there is
// no simple way to pass arguments of PCI slots from
// device model (acrn-dm) to ACRNGT module.
const acrnGVTgReservedSlot = 2

const acrnLPCDev = "lpc"
const acrnHostBridge = "hostbridge"

var baselogger *logrus.Entry

// AcrnBlkDevPoolSz defines the number of dummy virtio-blk
// device that will be created for hot-plugging container
// rootfs. Since acrn doesn't support hot-plug, dummy virtio-blk
// devices are added and later replaced with container-rootfs.
var AcrnBlkDevPoolSz = 8

// AcrnBlkdDevSlot array provides translation between
// the vitio-blk device index and slot it is currently
// attached.
// Allocating extra 1 to accommodate for VM rootfs
// which is at driveIndex 0
var AcrnBlkdDevSlot = make([]int, AcrnBlkDevPoolSz+1)

// acrnKernelParamsNonDebug is a list of the default kernel
// parameters that will be used in standard (non-debug) mode.
var acrnKernelParamsNonDebug = []Param{
	{"quiet", ""},
}

// acrnKernelParamsSystemdNonDebug is a list of the default systemd related
// kernel parameters that will be used in standard (non-debug) mode.
var acrnKernelParamsSystemdNonDebug = []Param{
	{"systemd.show_status", "false"},
}

// acrnKernelParamsDebug is a list of the default kernel
// parameters that will be used in debug mode (as much boot output as
// possible).
var acrnKernelParamsDebug = []Param{
	{"debug", ""},
}

// acrnKernelParamsSystemdDebug is a list of the default systemd related kernel
// parameters that will be used in debug mode (as much boot output as
// possible).
var acrnKernelParamsSystemdDebug = []Param{
	{"systemd.show_status", "true"},
	{"systemd.log_level", "debug"},
	{"systemd.log_target", "kmsg"},
	{"printk.devkmsg", "on"},
}

var acrnKernelRootParams = []Param{
	{"root", "/dev/vda1 rw rootwait"},
}

var acrnKernelParams = []Param{
	{"tsc", "reliable"},
	{"no_timer_check", ""},
	{"nohpet", ""},
	{"console", "tty0"},
	{"console", "ttyS0"},
	{"console", "hvc0"},
	{"log_buf_len", "16M"},
	{"consoleblank", "0"},
	{"iommu", "off"},
	{"i915.avail_planes_per_pipe", "0x070F00"},
	{"i915.enable_hangcheck", "0"},
	{"i915.nuclear_pageflip", "1"},
	{"i915.enable_guc_loading", "0"},
	{"i915.enable_guc_submission", "0"},
	{"i915.enable_guc", "0"},
}

// Device is the acrn device interface.
type Device interface {
	Valid() bool
	AcrnParams(slot int, config *Config) []string
}

// ConsoleDeviceBackend is the character device backend for acrn
type ConsoleDeviceBackend string

const (

	// Socket creates a 2 way stream socket (TCP or Unix).
	Socket ConsoleDeviceBackend = "socket"

	// Stdio sends traffic from the guest to acrn's standard output.
	Stdio ConsoleDeviceBackend = "console"

	// File backend only supports console output to a file (no input).
	File ConsoleDeviceBackend = "file"

	// TTY is an alias for Serial.
	TTY ConsoleDeviceBackend = "tty"

	// PTY creates a new pseudo-terminal on the host and connect to it.
	PTY ConsoleDeviceBackend = "pty"
)

// BEPortType marks the port as console port or virtio-serial port
type BEPortType int

const (
	// SerialBE marks the port as serial port
	SerialBE BEPortType = iota

	//ConsoleBE marks the port as console port (append @)
	ConsoleBE
)

// ConsoleDevice represents a acrn console device.
type ConsoleDevice struct {
	// Name of the socket
	Name string

	//Backend device used for virtio-console
	Backend ConsoleDeviceBackend

	// PortType marks the port as serial or console port (@)
	PortType BEPortType

	//Path to virtio-console backend (can be omitted for pty, tty, stdio)
	Path string
}

// NetDeviceType is a acrn networking device type.
type NetDeviceType string

const (
	// TAP is a TAP networking device type.
	TAP NetDeviceType = "tap"

	// MACVTAP is a macvtap networking device type.
	MACVTAP NetDeviceType = "macvtap"
)

// NetDevice represents a guest networking device
type NetDevice struct {
	// Type is the netdev type (e.g. tap).
	Type NetDeviceType

	// IfName is the interface name
	IFName string

	//MACAddress is the networking device interface MAC address
	MACAddress string
}

// BlockDevice represents a acrn block device.
type BlockDevice struct {

	// mem path to block device
	FilePath string

	//BlkIndex - Blk index corresponding to slot
	Index int
}

// BridgeDevice represents a acrn bridge device like pci-bridge, pxb, etc.
type BridgeDevice struct {

	// Function is PCI function. Func can be from 0 to 7
	Function int

	// Emul is a string describing the type of PCI device e.g. virtio-net
	Emul string

	// Config is an optional string, depending on the device, that can be
	// used for configuration
	Config string
}

// LPCDevice represents a acrn LPC device
type LPCDevice struct {

	// Function is PCI function. Func can be from 0 to 7
	Function int

	// Emul is a string describing the type of PCI device e.g. virtio-net
	Emul string
}

// Memory is the guest memory configuration structure.
type Memory struct {
	// Size is the amount of memory made available to the guest.
	// It should be suffixed with M or G for sizes in megabytes or
	// gigabytes respectively.
	Size string
}

// Kernel is the guest kernel configuration structure.
type Kernel struct {
	// Path is the guest kernel path on the host filesystem.
	Path string

	// InitrdPath is the guest initrd path on the host filesystem.
	ImagePath string

	// Params is the kernel parameters string.
	Params string
}

// Config is the acrn configuration structure.
// It allows for passing custom settings and parameters to the acrn-dm API.
type Config struct {

	// Path is the acrn binary path.
	Path string

	// Path is the acrn binary path.
	CtlPath string

	// Name is the acrn guest name
	Name string

	// UUID is the acrn process UUID.
	UUID string

	// Devices is a list of devices for acrn to create and drive.
	Devices []Device

	// Kernel is the guest kernel configuration.
	Kernel Kernel

	// Memory is the guest memory configuration.
	Memory Memory

	acrnParams []string

	// ACPI virtualization support
	ACPIVirt bool
}

// MaxAcrnVCPUs returns the maximum number of vCPUs supported
func MaxAcrnVCPUs() uint32 {
	return uint32(8)
}

func newAcrnArch(config HypervisorConfig) acrnArch {
	a := &acrnArchBase{
		path:                 acrnPath,
		ctlpath:              acrnctlPath,
		kernelParamsNonDebug: acrnKernelParamsNonDebug,
		kernelParamsDebug:    acrnKernelParamsDebug,
		kernelParams:         acrnKernelParams,
	}

	a.handleImagePath(config)
	return a
}

func (a *acrnArchBase) acrnPath() (string, error) {
	p := a.path
	return p, nil
}

func (a *acrnArchBase) acrnctlPath() (string, error) {
	ctlpath := a.ctlpath
	return ctlpath, nil
}

func (a *acrnArchBase) kernelParameters(debug bool) []Param {
	params := a.kernelParams

	if debug {
		params = append(params, a.kernelParamsDebug...)
	} else {
		params = append(params, a.kernelParamsNonDebug...)
	}

	return params
}

func (a *acrnArchBase) memoryTopology(memoryMb uint64) Memory {
	mem := fmt.Sprintf("%dM", memoryMb)
	memory := Memory{
		Size: mem,
	}

	return memory
}

func (a *acrnArchBase) capabilities() types.Capabilities {
	var caps types.Capabilities

	caps.SetBlockDeviceSupport()
	caps.SetBlockDeviceHotplugSupport()

	return caps
}

// Valid returns true if the CharDevice structure is valid and complete.
func (cdev ConsoleDevice) Valid() bool {
	if cdev.Backend != "tty" && cdev.Backend != "pty" &&
		cdev.Backend != "console" && cdev.Backend != "socket" &&
		cdev.Backend != "file" {
		return false
	} else if cdev.PortType != ConsoleBE && cdev.PortType != SerialBE {
		return false
	} else if cdev.Path == "" {
		return false
	} else {
		return true
	}
}

// AcrnParams returns the acrn parameters built out of this console device.
func (cdev ConsoleDevice) AcrnParams(slot int, config *Config) []string {
	var acrnParams []string
	var deviceParams []string

	acrnParams = append(acrnParams, "-s")
	deviceParams = append(deviceParams, fmt.Sprintf("%d,virtio-console,", slot))

	if cdev.PortType == ConsoleBE {
		deviceParams = append(deviceParams, "@")
	}

	switch cdev.Backend {
	case "pty":
		deviceParams = append(deviceParams, "pty:pty_port")
	case "tty":
		deviceParams = append(deviceParams, fmt.Sprintf("tty:tty_port=%s", cdev.Path))
	case "socket":
		deviceParams = append(deviceParams, fmt.Sprintf("socket:%s=%s", cdev.Name, cdev.Path))
	case "file":
		deviceParams = append(deviceParams, fmt.Sprintf("file:file_port=%s", cdev.Path))
	case "stdio":
		deviceParams = append(deviceParams, "stdio:stdio_port")
	default:
		// do nothing. Error should be already caught
	}

	acrnParams = append(acrnParams, strings.Join(deviceParams, ""))
	return acrnParams
}

// AcrnNetdevParam converts to the acrn type to string
func (netdev NetDevice) AcrnNetdevParam() []string {
	var deviceParams []string

	switch netdev.Type {
	case TAP:
		deviceParams = append(deviceParams, netdev.IFName)
		deviceParams = append(deviceParams, fmt.Sprintf(",mac=%s", netdev.MACAddress))
	case MACVTAP:
		deviceParams = append(deviceParams, netdev.IFName)
		deviceParams = append(deviceParams, fmt.Sprintf(",mac=%s", netdev.MACAddress))
	default:
		deviceParams = append(deviceParams, netdev.IFName)

	}

	return deviceParams
}

// Valid returns true if the NetDevice structure is valid and complete.
func (netdev NetDevice) Valid() bool {
	if netdev.IFName == "" {
		return false
	} else if netdev.MACAddress == "" {
		return false
	} else if netdev.Type != TAP && netdev.Type != MACVTAP {
		return false
	} else {
		return true
	}
}

// AcrnParams returns the acrn parameters built out of this network device.
func (netdev NetDevice) AcrnParams(slot int, config *Config) []string {
	var acrnParams []string

	acrnParams = append(acrnParams, "-s")
	acrnParams = append(acrnParams, fmt.Sprintf("%d,virtio-net,%s", slot, strings.Join(netdev.AcrnNetdevParam(), "")))

	return acrnParams
}

// Valid returns true if the BlockDevice structure is valid and complete.
func (blkdev BlockDevice) Valid() bool {
	return blkdev.FilePath != ""
}

// AcrnParams returns the acrn parameters built out of this block device.
func (blkdev BlockDevice) AcrnParams(slot int, config *Config) []string {
	var acrnParams []string

	device := "virtio-blk"
	acrnParams = append(acrnParams, "-s")
	acrnParams = append(acrnParams, fmt.Sprintf("%d,%s,%s",
		slot, device, blkdev.FilePath))

	// Update the global array (BlkIndex<->slot)
	// Used to identify slots for the hot-plugged virtio-blk devices
	if blkdev.Index <= AcrnBlkDevPoolSz {
		AcrnBlkdDevSlot[blkdev.Index] = slot
	} else {
		baselogger.WithFields(logrus.Fields{
			"device": device,
			"index":  blkdev.Index,
		}).Info("Invalid index device")
	}

	return acrnParams
}

// Valid returns true if the BridgeDevice structure is valid and complete.
func (bridgeDev BridgeDevice) Valid() bool {
	if bridgeDev.Function != 0 || bridgeDev.Emul != acrnHostBridge {
		return false
	}
	return true
}

// AcrnParams returns the acrn parameters built out of this bridge device.
func (bridgeDev BridgeDevice) AcrnParams(slot int, config *Config) []string {
	var acrnParams []string

	acrnParams = append(acrnParams, "-s")
	acrnParams = append(acrnParams, fmt.Sprintf("%d:%d,%s", slot,
		bridgeDev.Function, bridgeDev.Emul))

	return acrnParams
}

// Valid returns true if the BridgeDevice structure is valid and complete.
func (lpcDev LPCDevice) Valid() bool {
	return lpcDev.Emul == acrnLPCDev
}

// AcrnParams returns the acrn parameters built out of this bridge device.
func (lpcDev LPCDevice) AcrnParams(slot int, config *Config) []string {
	var acrnParams []string
	var deviceParams []string

	acrnParams = append(acrnParams, "-s")
	acrnParams = append(acrnParams, fmt.Sprintf("%d:%d,%s", slot,
		lpcDev.Function, lpcDev.Emul))

	//define UART port
	deviceParams = append(deviceParams, "-l")
	deviceParams = append(deviceParams, "com1,stdio")
	acrnParams = append(acrnParams, strings.Join(deviceParams, ""))

	return acrnParams
}

func (config *Config) appendName() {
	if config.Name != "" {
		config.acrnParams = append(config.acrnParams, config.Name)
	}
}

func (config *Config) appendDevices() {
	slot := 0
	for _, d := range config.Devices {
		if !d.Valid() {
			continue
		}

		if slot == acrnGVTgReservedSlot {
			slot++ /*Slot 2 is assigned for GVT-g in acrn, so skip 2 */
			baselogger.Info("Slot 2 is assigned for GVT-g in acrn, so skipping this slot")

		}
		config.acrnParams = append(config.acrnParams, d.AcrnParams(slot, config)...)
		slot++
	}
}

func (config *Config) appendUUID() {
	if config.UUID != "" {
		config.acrnParams = append(config.acrnParams, "-U")
		config.acrnParams = append(config.acrnParams, config.UUID)
	}
}

func (config *Config) appendACPI() {
	if config.ACPIVirt {
		config.acrnParams = append(config.acrnParams, "-A")
	}
}

func (config *Config) appendMemory() {
	if config.Memory.Size != "" {
		config.acrnParams = append(config.acrnParams, "-m")
		config.acrnParams = append(config.acrnParams, config.Memory.Size)
	}
}

func (config *Config) appendKernel() {
	if config.Kernel.Path == "" {
		return
	}
	config.acrnParams = append(config.acrnParams, "-k")
	config.acrnParams = append(config.acrnParams, config.Kernel.Path)

	if config.Kernel.Params == "" {
		return
	}
	config.acrnParams = append(config.acrnParams, "-B")
	config.acrnParams = append(config.acrnParams, config.Kernel.Params)
}

// LaunchAcrn can be used to launch a new acrn instance.
//
// The Config parameter contains a set of acrn parameters and settings.
//
// This function writes its log output via logger parameter.
func LaunchAcrn(config Config, logger *logrus.Entry) (int, string, error) {
	baselogger = logger
	config.appendUUID()
	config.appendACPI()
	config.appendMemory()
	config.appendDevices()
	config.appendKernel()
	config.appendName()

	return LaunchCustomAcrn(context.Background(), config.Path, config.acrnParams, logger)
}

// LaunchCustomAcrn can be used to launch a new acrn instance.
//
// The path parameter is used to pass the acrn executable path.
//
// params is a slice of options to pass to acrn-dm
//
// This function writes its log output via logger parameter.
func LaunchCustomAcrn(ctx context.Context, path string, params []string,
	logger *logrus.Entry) (int, string, error) {

	errStr := ""

	if path == "" {
		path = "acrn-dm"
	}

	/* #nosec */
	cmd := exec.CommandContext(ctx, path, params...)

	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	logger.WithFields(logrus.Fields{
		"Path":   path,
		"Params": params,
	}).Info("launching acrn with:")

	err := cmd.Start()
	if err != nil {
		logger.Errorf("Unable to launch %s: %v", path, err)
		errStr = stderr.String()
		logger.Errorf("%s", errStr)
	}
	return cmd.Process.Pid, errStr, err
}

func (a *acrnArchBase) appendImage(devices []Device, path string) ([]Device, error) {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return nil, err
	}

	ImgBlkdevice := BlockDevice{
		FilePath: path,
		Index:    0,
	}

	devices = append(devices, ImgBlkdevice)

	return devices, nil
}

// appendBridges appends to devices the given bridges
func (a *acrnArchBase) appendBridges(devices []Device) []Device {
	devices = append(devices,
		BridgeDevice{
			Function: 0,
			Emul:     acrnHostBridge,
			Config:   "",
		},
	)

	return devices
}

// appendBridges appends to devices the given bridges
func (a *acrnArchBase) appendLPC(devices []Device) []Device {
	devices = append(devices,
		LPCDevice{
			Function: 0,
			Emul:     acrnLPCDev,
		},
	)

	return devices
}

func (a *acrnArchBase) appendConsole(devices []Device, path string) []Device {
	console := ConsoleDevice{
		Name:     "console0",
		Backend:  Socket,
		PortType: ConsoleBE,
		Path:     path,
	}

	devices = append(devices, console)
	return devices
}

func (a *acrnArchBase) appendSocket(devices []Device, socket types.Socket) []Device {
	serailsocket := ConsoleDevice{
		Name:     socket.Name,
		Backend:  Socket,
		PortType: SerialBE,
		Path:     socket.HostPath,
	}

	devices = append(devices, serailsocket)
	return devices
}

func networkModelToAcrnType(model NetInterworkingModel) NetDeviceType {
	switch model {
	case NetXConnectMacVtapModel:
		return MACVTAP
	default:
		//TAP should work for most other cases
		return TAP
	}
}

func (a *acrnArchBase) appendNetwork(devices []Device, endpoint Endpoint) []Device {
	switch ep := endpoint.(type) {
	case *VethEndpoint:
		netPair := ep.NetworkPair()
		devices = append(devices,
			NetDevice{
				Type:       networkModelToAcrnType(netPair.NetInterworkingModel),
				IFName:     netPair.TAPIface.Name,
				MACAddress: netPair.TAPIface.HardAddr,
			},
		)
	case *MacvtapEndpoint:
		devices = append(devices,
			NetDevice{
				Type:       MACVTAP,
				IFName:     ep.Name(),
				MACAddress: ep.HardwareAddr(),
			},
		)
	default:
		// Return devices as is for unsupported endpoint.
		baselogger.WithField("Endpoint", endpoint).Error("Unsupported N/W Endpoint")
	}

	return devices
}

func (a *acrnArchBase) appendBlockDevice(devices []Device, drive config.BlockDrive) []Device {
	if drive.File == "" {
		return devices
	}

	devices = append(devices,
		BlockDevice{
			FilePath: drive.File,
			Index:    drive.Index,
		},
	)

	return devices
}

func (a *acrnArchBase) handleImagePath(config HypervisorConfig) {
	if config.ImagePath != "" {
		a.kernelParams = append(a.kernelParams, acrnKernelRootParams...)
		a.kernelParamsNonDebug = append(a.kernelParamsNonDebug, acrnKernelParamsSystemdNonDebug...)
		a.kernelParamsDebug = append(a.kernelParamsDebug, acrnKernelParamsSystemdDebug...)
	}
}
