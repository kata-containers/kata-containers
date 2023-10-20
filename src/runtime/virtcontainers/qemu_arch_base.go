//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"regexp"
	"runtime"
	"strings"

	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	"gitlab.com/nvidia/cloud-native/go-nvlib/pkg/nvpci"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
)

// A deeper PCIe topology than 5 is already not advisable just for the sake
// of having enough buffer we limit ourselves to 10 and exit if we reach
// the root bus
const maxPCIeTopoDepth = 10

type qemuArch interface {
	// enableNestingChecks nesting checks will be honoured
	enableNestingChecks()

	// disableNestingChecks nesting checks will be ignored
	disableNestingChecks()

	// runNested indicates if the hypervisor runs in a nested environment
	runNested() bool

	// enableVhostNet vhost will be enabled
	enableVhostNet()

	// disableVhostNet vhost will be disabled
	disableVhostNet()

	// machine returns the machine type
	machine() govmmQemu.Machine

	// qemuPath returns the path to the QEMU binary
	qemuPath() string

	// kernelParameters returns the kernel parameters
	// if debug is true then kernel debug parameters are included
	kernelParameters(debug bool) []Param

	//capabilities returns the capabilities supported by QEMU
	capabilities(config HypervisorConfig) types.Capabilities

	// bridges sets the number bridges for the machine type
	bridges(number uint32)

	// cpuTopology returns the CPU topology for the given amount of vcpus
	cpuTopology(vcpus, maxvcpus uint32) govmmQemu.SMP

	// cpuModel returns the CPU model for the machine type
	cpuModel() string

	// memoryTopology returns the memory topology using the given amount of memoryMb and hostMemoryMb
	memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory

	// protection returns platform protection
	getProtection() guestProtection

	// appendConsole appends a console to devices
	appendConsole(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error)

	// appendImage appends an image to devices
	appendImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error)

	// appendBlockImage appends an image as block device
	appendBlockImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error)

	// appendNvdimmImage appends an image as nvdimm device
	appendNvdimmImage(devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error)

	// appendSCSIController appens a SCSI controller to devices
	appendSCSIController(context context.Context, devices []govmmQemu.Device, enableIOThreads bool) ([]govmmQemu.Device, *govmmQemu.IOThread, error)

	// appendBridges appends bridges to devices
	appendBridges(devices []govmmQemu.Device) []govmmQemu.Device

	// append9PVolume appends a 9P volume to devices
	append9PVolume(ctx context.Context, devices []govmmQemu.Device, volume types.Volume) ([]govmmQemu.Device, error)

	// appendSocket appends a socket to devices
	appendSocket(devices []govmmQemu.Device, socket types.Socket) []govmmQemu.Device

	// appendVSock appends a vsock PCI to devices
	appendVSock(ctx context.Context, devices []govmmQemu.Device, vsock types.VSock) ([]govmmQemu.Device, error)

	// appendNetwork appends a endpoint device to devices
	appendNetwork(ctx context.Context, devices []govmmQemu.Device, endpoint Endpoint) ([]govmmQemu.Device, error)

	// appendBlockDevice appends a block drive to devices
	appendBlockDevice(ctx context.Context, devices []govmmQemu.Device, drive config.BlockDrive) ([]govmmQemu.Device, error)

	// appendVhostUserDevice appends a vhost user device to devices
	appendVhostUserDevice(ctx context.Context, devices []govmmQemu.Device, drive config.VhostUserDeviceAttrs) ([]govmmQemu.Device, error)

	// appendVFIODevice appends a VFIO device to devices
	appendVFIODevice(devices []govmmQemu.Device, vfioDevice config.VFIODev) []govmmQemu.Device

	// appendRNGDevice appends a RNG device to devices
	appendRNGDevice(ctx context.Context, devices []govmmQemu.Device, rngDevice config.RNGDev) ([]govmmQemu.Device, error)

	// addDeviceToBridge adds devices to the bus
	addDeviceToBridge(ctx context.Context, ID string, t types.Type) (string, types.Bridge, error)

	// removeDeviceFromBridge removes devices to the bus
	removeDeviceFromBridge(ID string) error

	// getBridges grants access to Bridges
	getBridges() []types.Bridge

	// setBridges grants access to Bridges
	setBridges(bridges []types.Bridge)

	// addBridge adds a new Bridge to the list of Bridges
	addBridge(types.Bridge)

	// getPFlash() get pflash from configuration
	getPFlash() ([]string, error)

	// setPFlash() grants access to pflash
	setPFlash([]string)

	// handleImagePath handles the Hypervisor Config image path
	handleImagePath(config HypervisorConfig) error

	// supportGuestMemoryHotplug returns if the guest supports memory hotplug
	supportGuestMemoryHotplug() bool

	// setIgnoreSharedMemoryMigrationCaps set bypass-shared-memory capability for migration
	setIgnoreSharedMemoryMigrationCaps(context.Context, *govmmQemu.QMP) error

	// appendPCIeRootPortDevice appends a pcie-root-port device to pcie.0 bus
	appendPCIeRootPortDevice(devices []govmmQemu.Device, number uint32, memSize32bit uint64, memSize64bit uint64) []govmmQemu.Device

	// appendPCIeSwitch appends a ioh3420 device to a pcie-root-port
	appendPCIeSwitchPortDevice(devices []govmmQemu.Device, number uint32, memSize32bit uint64, memSize64bit uint64) []govmmQemu.Device

	// append vIOMMU device
	appendIOMMU(devices []govmmQemu.Device) ([]govmmQemu.Device, error)

	// append pvpanic device
	appendPVPanicDevice(devices []govmmQemu.Device) ([]govmmQemu.Device, error)

	// append protection device.
	// This implementation is architecture specific, some archs may need
	// a firmware, returns a string containing the path to the firmware that should
	// be used with the -bios option, ommit -bios option if the path is empty.
	appendProtectionDevice(devices []govmmQemu.Device, firmware, firmwareVolume string) ([]govmmQemu.Device, string, error)

	// scans the PCIe space and returns the biggest BAR sizes for 32-bit
	// and 64-bit addressable memory
	getBARsMaxAddressableMemory() (uint64, uint64)

	// Query QMP to find a device's PCI path given its QOM path or ID
	qomGetPciPath(qemuID string, qmpCh *qmpChannel) (types.PciPath, error)

	// Query QMP to find the PCI slot of a device, given its QOM path or ID
	qomGetSlot(qomPath string, qmpCh *qmpChannel) (types.PciSlot, error)
}

type qemuArchBase struct {
	qemuExePath          string
	qemuMachine          govmmQemu.Machine
	PFlash               []string
	kernelParamsNonDebug []Param
	kernelParamsDebug    []Param
	kernelParams         []Param
	Bridges              []types.Bridge
	memoryOffset         uint64
	networkIndex         int
	// Exclude from lint checking for it is ultimately only used in architecture-specific code
	protection    guestProtection //nolint:structcheck
	nestedRun     bool
	vhost         bool
	disableNvdimm bool
	dax           bool
	legacySerial  bool
}

const (
	defaultCores       uint32 = 1
	defaultThreads     uint32 = 1
	defaultCPUModel           = "host"
	defaultBridgeBus          = "pcie.0"
	defaultPCBridgeBus        = "pci.0"
	maxDevIDSize              = 31
	maxPCIeRootPort           = 16 // Limitation from QEMU
	maxPCIeSwitchPort         = 16 // Limitation from QEMU
)

// This is the PCI start address assigned to the first bridge that
// is added on the qemu command line. In case of x86_64, the first two PCI
// addresses (0 and 1) are used by the platform while in case of ARM, address
// 0 is reserved.
const bridgePCIStartAddr = 2

const (
	// QemuQ35 is the QEMU Q35 machine type for amd64
	QemuQ35 = "q35"

	// QemuMicrovm is the QEMU microvm machine type for amd64
	QemuMicrovm = "microvm"

	// QemuVirt is the QEMU virt machine type for aarch64 or amd64
	QemuVirt = "virt"

	// QemuPseries is a QEMU virt machine type for ppc64le
	QemuPseries = "pseries"

	// QemuCCWVirtio is a QEMU virt machine type for for s390x
	QemuCCWVirtio = "s390-ccw-virtio"

	qmpCapMigrationIgnoreShared = "x-ignore-shared"

	qemuNvdimmOption = "nvdimm=on"
)

// kernelParamsNonDebug is a list of the default kernel
// parameters that will be used in standard (non-debug) mode.
var kernelParamsNonDebug = []Param{
	{"quiet", ""},
}

// kernelParamsSystemdNonDebug is a list of the default systemd related
// kernel parameters that will be used in standard (non-debug) mode.
var kernelParamsSystemdNonDebug = []Param{
	{"systemd.show_status", "false"},
}

// kernelParamsDebug is a list of the default kernel
// parameters that will be used in debug mode (as much boot output as
// possible).
var kernelParamsDebug = []Param{
	{"debug", ""},
}

// kernelParamsSystemdDebug is a list of the default systemd related kernel
// parameters that will be used in debug mode (as much boot output as
// possible).
var kernelParamsSystemdDebug = []Param{
	{"systemd.show_status", "true"},
	{"systemd.log_level", "debug"},
}

func (q *qemuArchBase) enableNestingChecks() {
	q.nestedRun = true
}

func (q *qemuArchBase) disableNestingChecks() {
	q.nestedRun = false
}

func (q *qemuArchBase) runNested() bool {
	return q.nestedRun
}

func (q *qemuArchBase) enableVhostNet() {
	q.vhost = true
}

func (q *qemuArchBase) disableVhostNet() {
	q.vhost = false
}

func (q *qemuArchBase) machine() govmmQemu.Machine {
	return q.qemuMachine
}

func (q *qemuArchBase) getProtection() guestProtection {
	return q.protection
}

func (q *qemuArchBase) qemuPath() string {
	return q.qemuExePath
}

func (q *qemuArchBase) kernelParameters(debug bool) []Param {
	params := q.kernelParams

	if debug {
		params = append(params, q.kernelParamsDebug...)
	} else {
		params = append(params, q.kernelParamsNonDebug...)
	}

	return params
}

func (q *qemuArchBase) capabilities(hConfig HypervisorConfig) types.Capabilities {
	var caps types.Capabilities
	caps.SetBlockDeviceHotplugSupport()
	caps.SetMultiQueueSupport()
	caps.SetNetworkDeviceHotplugSupported()
	if hConfig.SharedFS != config.NoSharedFS {
		caps.SetFsSharingSupport()
	}
	return caps
}

func (q *qemuArchBase) bridges(number uint32) {
	for i := uint32(0); i < number; i++ {
		q.Bridges = append(q.Bridges, types.NewBridge(types.PCI, fmt.Sprintf("%s-bridge-%d", types.PCI, i), make(map[uint32]string), 0))
	}
}

func (q *qemuArchBase) cpuTopology(vcpus, maxvcpus uint32) govmmQemu.SMP {
	smp := govmmQemu.SMP{
		CPUs:    vcpus,
		Sockets: maxvcpus,
		Cores:   defaultCores,
		Threads: defaultThreads,
		MaxCPUs: maxvcpus,
	}

	return smp
}

func (q *qemuArchBase) cpuModel() string {
	return defaultCPUModel
}

func (q *qemuArchBase) memoryTopology(memoryMb, hostMemoryMb uint64, slots uint8) govmmQemu.Memory {
	memMax := fmt.Sprintf("%dM", hostMemoryMb)
	mem := fmt.Sprintf("%dM", memoryMb)
	memory := govmmQemu.Memory{
		Size:   mem,
		Slots:  slots,
		MaxMem: memMax,
	}

	return memory
}

func (q *qemuArchBase) appendConsole(_ context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	var serial, console govmmQemu.Device
	var consoleKernelParams []Param

	if q.legacySerial {
		serial = govmmQemu.LegacySerialDevice{
			Chardev: "charconsole0",
		}

		console = govmmQemu.CharDevice{
			Driver:   govmmQemu.LegacySerial,
			Backend:  govmmQemu.Socket,
			DeviceID: "console0",
			ID:       "charconsole0",
			Path:     path,
		}

		consoleKernelParams = []Param{
			{"console", "ttyS0"},
		}
	} else {
		serial = govmmQemu.SerialDevice{
			Driver:        govmmQemu.VirtioSerial,
			ID:            "serial0",
			DisableModern: q.nestedRun,
			MaxPorts:      uint(2),
		}

		console = govmmQemu.CharDevice{
			Driver:   govmmQemu.Console,
			Backend:  govmmQemu.Socket,
			DeviceID: "console0",
			ID:       "charconsole0",
			Path:     path,
		}

		consoleKernelParams = []Param{
			{"console", "hvc0"},
			{"console", "hvc1"},
		}
	}

	devices = append(devices, serial)
	devices = append(devices, console)
	q.kernelParams = append(q.kernelParams, consoleKernelParams...)

	return devices, nil
}

func genericImage(path string) (config.BlockDrive, error) {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return config.BlockDrive{}, err
	}

	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return config.BlockDrive{}, err
	}

	id := utils.MakeNameID("image", hex.EncodeToString(randBytes), maxDevIDSize)

	drive := config.BlockDrive{
		File:     path,
		Format:   "raw",
		ID:       id,
		ShareRW:  true,
		ReadOnly: true,
	}

	return drive, nil
}

func (q *qemuArchBase) appendNvdimmImage(devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	imageFile, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer imageFile.Close()

	imageStat, err := imageFile.Stat()
	if err != nil {
		return nil, err
	}

	object := govmmQemu.Object{
		Driver:   govmmQemu.NVDIMM,
		Type:     govmmQemu.MemoryBackendFile,
		DeviceID: "nv0",
		ID:       "mem0",
		MemPath:  path,
		Size:     (uint64)(imageStat.Size()),
		ReadOnly: true,
	}

	devices = append(devices, object)

	return devices, nil
}

func (q *qemuArchBase) appendImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	return q.appendBlockImage(ctx, devices, path)
}

func (q *qemuArchBase) appendBlockImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	drive, err := genericImage(path)
	if err != nil {
		return nil, err
	}
	devices, err = q.appendBlockDevice(ctx, devices, drive)
	if err != nil {
		return nil, err
	}
	return devices, nil
}

func genericSCSIController(enableIOThreads, nestedRun bool) (govmmQemu.SCSIController, *govmmQemu.IOThread) {
	scsiController := govmmQemu.SCSIController{
		ID:            scsiControllerID,
		DisableModern: nestedRun,
	}

	var t *govmmQemu.IOThread

	if enableIOThreads {
		randBytes, _ := utils.GenerateRandomBytes(8)

		t = &govmmQemu.IOThread{
			ID: fmt.Sprintf("%s-%s", "iothread", hex.EncodeToString(randBytes)),
		}

		scsiController.IOThread = t.ID
	}

	return scsiController, t
}

func (q *qemuArchBase) appendSCSIController(_ context.Context, devices []govmmQemu.Device, enableIOThreads bool) ([]govmmQemu.Device, *govmmQemu.IOThread, error) {
	d, t := genericSCSIController(enableIOThreads, q.nestedRun)
	devices = append(devices, d)
	return devices, t, nil
}

// appendBridges appends to devices the given bridges
func (q *qemuArchBase) appendBridges(devices []govmmQemu.Device) []govmmQemu.Device {
	return genericAppendBridges(devices, q.Bridges, q.qemuMachine.Type)
}

func generic9PVolume(volume types.Volume, nestedRun bool) govmmQemu.FSDevice {
	devID := fmt.Sprintf("extra-9p-%s", volume.MountTag)
	if len(devID) > maxDevIDSize {
		devID = devID[:maxDevIDSize]
	}

	return govmmQemu.FSDevice{
		Driver:        govmmQemu.Virtio9P,
		FSDriver:      govmmQemu.Local,
		ID:            devID,
		Path:          volume.HostPath,
		MountTag:      volume.MountTag,
		SecurityModel: govmmQemu.None,
		DisableModern: nestedRun,
		Multidev:      govmmQemu.Remap,
	}
}

func genericAppend9PVolume(devices []govmmQemu.Device, volume types.Volume, nestedRun bool) (govmmQemu.FSDevice, error) {
	d := generic9PVolume(volume, nestedRun)
	return d, nil
}

func (q *qemuArchBase) append9PVolume(_ context.Context, devices []govmmQemu.Device, volume types.Volume) ([]govmmQemu.Device, error) {
	if volume.MountTag == "" || volume.HostPath == "" {
		return devices, nil
	}

	d, err := genericAppend9PVolume(devices, volume, q.nestedRun)
	if err != nil {
		return nil, err
	}

	devices = append(devices, d)
	return devices, nil
}

func (q *qemuArchBase) appendSocket(devices []govmmQemu.Device, socket types.Socket) []govmmQemu.Device {
	devID := socket.ID
	if len(devID) > maxDevIDSize {
		devID = devID[:maxDevIDSize]
	}

	devices = append(devices,
		govmmQemu.CharDevice{
			Driver:   govmmQemu.VirtioSerialPort,
			Backend:  govmmQemu.Socket,
			DeviceID: socket.DeviceID,
			ID:       devID,
			Path:     socket.HostPath,
			Name:     socket.Name,
		},
	)

	return devices
}

func (q *qemuArchBase) appendVSock(_ context.Context, devices []govmmQemu.Device, vsock types.VSock) ([]govmmQemu.Device, error) {
	devices = append(devices,
		govmmQemu.VSOCKDevice{
			ID:            fmt.Sprintf("vsock-%d", vsock.ContextID),
			ContextID:     vsock.ContextID,
			VHostFD:       vsock.VhostFd,
			DisableModern: q.nestedRun,
		},
	)

	return devices, nil

}

func networkModelToQemuType(model NetInterworkingModel) govmmQemu.NetDeviceType {
	switch model {
	case NetXConnectMacVtapModel:
		return govmmQemu.MACVTAP
	default:
		//TAP should work for most other cases
		return govmmQemu.TAP
	}
}

func genericNetwork(endpoint Endpoint, vhost, nestedRun bool, index int) (govmmQemu.NetDevice, error) {
	var d govmmQemu.NetDevice
	switch ep := endpoint.(type) {
	case *VethEndpoint, *MacvlanEndpoint, *IPVlanEndpoint:
		netPair := ep.NetworkPair()
		d = govmmQemu.NetDevice{
			Type:          networkModelToQemuType(netPair.NetInterworkingModel),
			Driver:        govmmQemu.VirtioNet,
			ID:            fmt.Sprintf("network-%d", index),
			IFName:        netPair.TAPIface.Name,
			MACAddress:    netPair.TAPIface.HardAddr,
			DownScript:    "no",
			Script:        "no",
			VHost:         vhost,
			DisableModern: nestedRun,
			FDs:           netPair.VMFds,
			VhostFDs:      netPair.VhostFds,
		}
	case *MacvtapEndpoint:
		d = govmmQemu.NetDevice{
			Type:          govmmQemu.MACVTAP,
			Driver:        govmmQemu.VirtioNet,
			ID:            fmt.Sprintf("network-%d", index),
			IFName:        ep.Name(),
			MACAddress:    ep.HardwareAddr(),
			DownScript:    "no",
			Script:        "no",
			VHost:         vhost,
			DisableModern: nestedRun,
			FDs:           ep.VMFds,
			VhostFDs:      ep.VhostFds,
		}
	case *TuntapEndpoint:
		netPair := ep.NetworkPair()
		d = govmmQemu.NetDevice{
			Type:          govmmQemu.NetDeviceType("tap"),
			Driver:        govmmQemu.VirtioNet,
			ID:            fmt.Sprintf("network-%d", index),
			IFName:        netPair.TAPIface.Name,
			MACAddress:    netPair.TAPIface.HardAddr,
			DownScript:    "no",
			Script:        "no",
			VHost:         vhost,
			DisableModern: nestedRun,
			FDs:           netPair.VMFds,
			VhostFDs:      netPair.VhostFds,
		}
	default:
		return govmmQemu.NetDevice{}, fmt.Errorf("Unknown type for endpoint")
	}

	return d, nil
}

func (q *qemuArchBase) appendNetwork(_ context.Context, devices []govmmQemu.Device, endpoint Endpoint) ([]govmmQemu.Device, error) {
	d, err := genericNetwork(endpoint, q.vhost, q.nestedRun, q.networkIndex)
	if err != nil {
		return devices, fmt.Errorf("Failed to append network %v", err)
	}
	q.networkIndex++
	devices = append(devices, d)
	return devices, nil
}

func genericBlockDevice(drive config.BlockDrive, nestedRun bool) (govmmQemu.BlockDevice, error) {
	if drive.File == "" || drive.ID == "" || drive.Format == "" {
		return govmmQemu.BlockDevice{}, fmt.Errorf("Empty File, ID or Format for drive %v", drive)
	}

	if len(drive.ID) > maxDevIDSize {
		drive.ID = drive.ID[:maxDevIDSize]
	}

	return govmmQemu.BlockDevice{
		Driver:        govmmQemu.VirtioBlock,
		ID:            drive.ID,
		File:          drive.File,
		AIO:           govmmQemu.Threads,
		Format:        govmmQemu.BlockDeviceFormat(drive.Format),
		Interface:     "none",
		DisableModern: nestedRun,
		ShareRW:       drive.ShareRW,
		ReadOnly:      drive.ReadOnly,
	}, nil
}

func (q *qemuArchBase) appendBlockDevice(_ context.Context, devices []govmmQemu.Device, drive config.BlockDrive) ([]govmmQemu.Device, error) {
	d, err := genericBlockDevice(drive, q.nestedRun)
	if err != nil {
		return devices, fmt.Errorf("Failed to append block device %v", err)
	}
	devices = append(devices, d)
	return devices, nil
}

func (q *qemuArchBase) appendVhostUserDevice(ctx context.Context, devices []govmmQemu.Device, attr config.VhostUserDeviceAttrs) ([]govmmQemu.Device, error) {
	qemuVhostUserDevice := govmmQemu.VhostUserDevice{}

	switch attr.Type {
	case config.VhostUserNet:
		qemuVhostUserDevice.TypeDevID = utils.MakeNameID("net", attr.DevID, maxDevIDSize)
		qemuVhostUserDevice.Address = attr.MacAddress
		qemuVhostUserDevice.VhostUserType = govmmQemu.VhostUserNet
	case config.VhostUserSCSI:
		qemuVhostUserDevice.TypeDevID = utils.MakeNameID("scsi", attr.DevID, maxDevIDSize)
		qemuVhostUserDevice.VhostUserType = govmmQemu.VhostUserSCSI
	case config.VhostUserBlk:
		qemuVhostUserDevice.VhostUserType = govmmQemu.VhostUserBlk
	case config.VhostUserFS:
		qemuVhostUserDevice.TypeDevID = utils.MakeNameID("fs", attr.DevID, maxDevIDSize)
		qemuVhostUserDevice.Tag = attr.Tag
		qemuVhostUserDevice.CacheSize = attr.CacheSize
		qemuVhostUserDevice.QueueSize = attr.QueueSize
		qemuVhostUserDevice.VhostUserType = govmmQemu.VhostUserFS
	}

	qemuVhostUserDevice.SocketPath = attr.SocketPath
	qemuVhostUserDevice.CharDevID = utils.MakeNameID("char", attr.DevID, maxDevIDSize)

	devices = append(devices, qemuVhostUserDevice)

	return devices, nil
}

func (q *qemuArchBase) appendVFIODevice(devices []govmmQemu.Device, vfioDev config.VFIODev) []govmmQemu.Device {

	if vfioDev.BDF == "" {
		return devices
	}

	devices = append(devices,
		govmmQemu.VFIODevice{
			BDF:      vfioDev.BDF,
			VendorID: vfioDev.VendorID,
			DeviceID: vfioDev.DeviceID,
			Bus:      vfioDev.Bus,
		},
	)

	return devices
}

func (q *qemuArchBase) appendRNGDevice(_ context.Context, devices []govmmQemu.Device, rngDev config.RNGDev) ([]govmmQemu.Device, error) {
	devices = append(devices,
		govmmQemu.RngDevice{
			ID:       rngDev.ID,
			Filename: rngDev.Filename,
		},
	)

	return devices, nil
}

func (q *qemuArchBase) handleImagePath(config HypervisorConfig) error {
	if config.ImagePath != "" {
		kernelRootParams, err := GetKernelRootParams(config.RootfsType, q.disableNvdimm, false)
		if err != nil {
			return err
		}
		if !q.disableNvdimm {
			q.qemuMachine.Options = strings.Join([]string{
				q.qemuMachine.Options, qemuNvdimmOption,
			}, ",")
			kernelRootParams, err = GetKernelRootParams(config.RootfsType, q.disableNvdimm, q.dax)
			if err != nil {
				return err
			}
		}
		q.kernelParams = append(q.kernelParams, kernelRootParams...)
		q.kernelParamsNonDebug = append(q.kernelParamsNonDebug, kernelParamsSystemdNonDebug...)
		q.kernelParamsDebug = append(q.kernelParamsDebug, kernelParamsSystemdDebug...)
	}

	return nil
}

func (q *qemuArchBase) supportGuestMemoryHotplug() bool {
	return q.protection == noneProtection
}

func (q *qemuArchBase) setIgnoreSharedMemoryMigrationCaps(ctx context.Context, qmp *govmmQemu.QMP) error {
	err := qmp.ExecSetMigrationCaps(ctx, []map[string]interface{}{
		{
			"capability": qmpCapMigrationIgnoreShared,
			"state":      true,
		},
	})
	return err
}

func (q *qemuArchBase) addDeviceToBridge(ctx context.Context, ID string, t types.Type) (string, types.Bridge, error) {
	addr, b, err := genericAddDeviceToBridge(ctx, q.Bridges, ID, t)
	if err != nil {
		return "", b, err
	}

	return fmt.Sprintf("%02x", addr), b, nil
}

func genericAddDeviceToBridge(ctx context.Context, bridges []types.Bridge, ID string, t types.Type) (uint32, types.Bridge, error) {
	var err error
	var addr uint32

	if len(bridges) == 0 {
		return 0, types.Bridge{}, errors.New("failed to get available address from bridges")
	}

	// looking for an empty address in the bridges
	for _, b := range bridges {
		if t != b.Type {
			continue
		}
		addr, err = b.AddDevice(ctx, ID)
		if err == nil {
			return addr, b, nil
		}
	}

	return 0, types.Bridge{}, fmt.Errorf("no more bridge slots available")
}

func (q *qemuArchBase) removeDeviceFromBridge(ID string) error {
	var err error
	for _, b := range q.Bridges {
		err = b.RemoveDevice(ID)
		if err == nil {
			// device was removed correctly
			return nil
		}
	}

	return err
}

func (q *qemuArchBase) getBridges() []types.Bridge {
	return q.Bridges
}

func (q *qemuArchBase) setBridges(bridges []types.Bridge) {
	q.Bridges = bridges
}

func (q *qemuArchBase) addBridge(b types.Bridge) {
	q.Bridges = append(q.Bridges, b)
}

// appendPCIeRootPortDevice appends to devices the given pcie-root-port
func (q *qemuArchBase) appendPCIeRootPortDevice(devices []govmmQemu.Device, number uint32, memSize32bit uint64, memSize64bit uint64) []govmmQemu.Device {
	return genericAppendPCIeRootPort(devices, number, q.qemuMachine.Type, memSize32bit, memSize64bit)
}

// appendPCIeSwitchPortDevice appends a PCIe Switch with <number> ports
func (q *qemuArchBase) appendPCIeSwitchPortDevice(devices []govmmQemu.Device, number uint32, memSize32bit uint64, memSize64bit uint64) []govmmQemu.Device {
	return genericAppendPCIeSwitchPort(devices, number, q.qemuMachine.Type, memSize32bit, memSize64bit)
}

// getBARsMaxAddressableMemory we need to know the BAR sizes to configure the
// PCIe Root Port or PCIe Downstream Port attaching a device with huge BARs.
func (q *qemuArchBase) getBARsMaxAddressableMemory() (uint64, uint64) {

	pci := nvpci.New()
	devs, _ := pci.GetAllDevices()

	// Since we do not know which devices are going to be hotplugged,
	// we're going to use the GPU with the biggest BARs to initialize the
	// root port, this should work for all other devices as well.
	// defaults are 2MB for both, if no suitable devices found
	max32bit := uint64(2 * 1024 * 1024)
	max64bit := uint64(2 * 1024 * 1024)

	for _, dev := range devs {
		if !dev.IsGPU() {
			continue
		}
		memSize32bit, memSize64bit := dev.Resources.GetTotalAddressableMemory(true)
		if max32bit < memSize32bit {
			max32bit = memSize32bit
		}
		if max64bit < memSize64bit {
			max64bit = memSize64bit
		}
	}
	// The actual 32bit is most of the time a power of 2 but we need some
	// buffer so double that to leave space for other IO functions.
	// The 64bit size is not a power of 2 and hence is already rounded up
	// to the higher value.
	return max32bit * 2, max64bit
}

// appendIOMMU appends a virtual IOMMU device
func (q *qemuArchBase) appendIOMMU(devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	switch q.qemuMachine.Type {
	case QemuQ35:
		iommu := govmmQemu.IommuDev{
			Intremap:    true,
			DeviceIotlb: true,
			CachingMode: true,
		}

		devices = append(devices, iommu)
		return devices, nil
	default:
		return devices, fmt.Errorf("Machine Type %s does not support vIOMMU", q.qemuMachine.Type)
	}
}

// appendPVPanicDevice appends a pvpanic device
func (q *qemuArchBase) appendPVPanicDevice(devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	devices = append(devices, govmmQemu.PVPanicDevice{NoShutdown: true})
	return devices, nil
}

func (q *qemuArchBase) getPFlash() ([]string, error) {
	return q.PFlash, nil
}

func (q *qemuArchBase) setPFlash(p []string) {
	q.PFlash = p
}

// append protection device
func (q *qemuArchBase) appendProtectionDevice(devices []govmmQemu.Device, firmware, firmwareVolume string) ([]govmmQemu.Device, string, error) {
	hvLogger.WithField("arch", runtime.GOARCH).Warnf("Confidential Computing has not been implemented for this architecture")
	return devices, firmware, nil
}

// Query QMP to find the PCI slot of a device, given its QOM path or ID
func (q *qemuArchBase) qomGetSlot(qomPath string, qmpCh *qmpChannel) (types.PciSlot, error) {
	addr, err := qmpCh.qmp.ExecQomGet(qmpCh.ctx, qomPath, "addr")
	if err != nil {
		return types.PciSlot{}, err
	}
	addrf, ok := addr.(float64)
	// XXX going via float makes no real sense, but that's how
	// JSON works, and we'll get away with it for the small values
	// we have here
	if !ok {
		return types.PciSlot{}, fmt.Errorf("addr QOM property of %q is %T not a number", qomPath, addr)
	}
	addri := int(addrf)

	slotNum, funcNum := addri>>3, addri&0x7
	if funcNum != 0 {
		return types.PciSlot{}, fmt.Errorf("Unexpected non-zero PCI function (%02x.%1x) on %q",
			slotNum, funcNum, qomPath)
	}

	return types.PciSlotFromInt(slotNum)
}

// Query QMP to find a device's PCI path given its QOM path or ID
func (q *qemuArchBase) qomGetPciPath(qemuID string, qmpCh *qmpChannel) (types.PciPath, error) {

	var slots []types.PciSlot

	devSlot, err := q.qomGetSlot(qemuID, qmpCh)
	if err != nil {
		return types.PciPath{}, err
	}
	slots = append(slots, devSlot)

	// This only works for Q35 and Virt
	r, _ := regexp.Compile(`^/machine/.*/pcie.0`)

	var parentPath = qemuID
	// We do not want to use a forever loop here, a deeper PCIe topology
	// than 5 is already not advisable just for the sake of having enough
	// buffer we limit ourselves to 10 and leave the loop early if we hit
	// the root bus.
	for i := 1; i <= maxPCIeTopoDepth; i++ {
		parenBusQOM, err := qmpCh.qmp.ExecQomGet(qmpCh.ctx, parentPath, "parent_bus")
		if err != nil {
			return types.PciPath{}, err
		}

		busQOM, ok := parenBusQOM.(string)
		if !ok {
			return types.PciPath{}, fmt.Errorf("parent_bus QOM property of %s is %t not a string", qemuID, parenBusQOM)
		}

		// If we hit /machine/q35/pcie.0 we're done this is the root bus
		// we climbed the complete hierarchy
		if r.Match([]byte(busQOM)) {
			break
		}

		// `bus` is the QOM path of the QOM bus object, but we need
		// the PCI parent_bus which manages that bus.  There doesn't seem
		// to be a way to get that other than to simply drop the last
		// path component.
		idx := strings.LastIndex(busQOM, "/")
		if idx == -1 {
			return types.PciPath{}, fmt.Errorf("Bus has unexpected QOM path %s", busQOM)
		}
		parentBus := busQOM[:idx]

		parentSlot, err := q.qomGetSlot(parentBus, qmpCh)
		if err != nil {
			return types.PciPath{}, err
		}

		// Prepend the slots, since we're climbing the hierarchy
		slots = append([]types.PciSlot{parentSlot}, slots...)
		parentPath = parentBus
	}
	return types.PciPathFromSlots(slots...)
}
