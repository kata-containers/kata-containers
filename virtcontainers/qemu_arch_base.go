// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"encoding/hex"
	"fmt"
	"os"
	"strconv"

	govmmQemu "github.com/intel/govmm/qemu"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/runtime/virtcontainers/utils"
)

type qemuArch interface {
	// enableNestingChecks nesting checks will be honoured
	enableNestingChecks()

	// disableNestingChecks nesting checks will be ignored
	disableNestingChecks()

	// machine returns the machine type
	machine() (govmmQemu.Machine, error)

	// qemuPath returns the path to the QEMU binary
	qemuPath() (string, error)

	// kernelParameters returns the kernel parameters
	// if debug is true then kernel debug parameters are included
	kernelParameters(debug bool) []Param

	//capabilities returns the capabilities supported by QEMU
	capabilities() capabilities

	// bridges returns the number bridges for the machine type
	bridges(number uint32) []Bridge

	// cpuTopology returns the CPU topology for the given amount of vcpus
	cpuTopology(vcpus, maxvcpus uint32) govmmQemu.SMP

	// cpuModel returns the CPU model for the machine type
	cpuModel() string

	// memoryTopology returns the memory topology using the given amount of memoryMb and hostMemoryMb
	memoryTopology(memoryMb, hostMemoryMb uint64) govmmQemu.Memory

	// append9PVolumes appends volumes to devices
	append9PVolumes(devices []govmmQemu.Device, volumes []Volume) []govmmQemu.Device

	// appendConsole appends a console to devices
	appendConsole(devices []govmmQemu.Device, path string) []govmmQemu.Device

	// appendImage appends an image to devices
	appendImage(devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error)

	// appendSCSIController appens a SCSI controller to devices
	appendSCSIController(devices []govmmQemu.Device, enableIOThreads bool) ([]govmmQemu.Device, *govmmQemu.IOThread)

	// appendBridges appends bridges to devices
	appendBridges(devices []govmmQemu.Device, bridges []Bridge) []govmmQemu.Device

	// append9PVolume appends a 9P volume to devices
	append9PVolume(devices []govmmQemu.Device, volume Volume) []govmmQemu.Device

	// appendSocket appends a socket to devices
	appendSocket(devices []govmmQemu.Device, socket Socket) []govmmQemu.Device

	// appendNetwork appends a endpoint device to devices
	appendNetwork(devices []govmmQemu.Device, endpoint Endpoint) []govmmQemu.Device

	// appendBlockDevice appends a block drive to devices
	appendBlockDevice(devices []govmmQemu.Device, drive drivers.Drive) []govmmQemu.Device

	// appendVhostUserDevice appends a vhost user device to devices
	appendVhostUserDevice(devices []govmmQemu.Device, vhostUserDevice api.VhostUserDevice) []govmmQemu.Device

	// appendVFIODevice appends a VFIO device to devices
	appendVFIODevice(devices []govmmQemu.Device, vfioDevice drivers.VFIODevice) []govmmQemu.Device
}

type qemuArchBase struct {
	machineType           string
	nestedRun             bool
	networkIndex          int
	qemuPaths             map[string]string
	supportedQemuMachines []govmmQemu.Machine
	kernelParamsNonDebug  []Param
	kernelParamsDebug     []Param
	kernelParams          []Param
}

const (
	defaultCores     uint32 = 1
	defaultThreads   uint32 = 1
	defaultMemSlots  uint8  = 2
	defaultCPUModel         = "host"
	defaultBridgeBus        = "pcie.0"
	maxDevIDSize            = 31
	defaultMsize9p          = 8192
)

// This is the PCI start address assigned to the first bridge that
// is added on the qemu command line. In case of x86_64, the first two PCI
// addresses (0 and 1) are used by the platform while in case of ARM, address
// 0 is reserved.
const bridgePCIStartAddr = 2

const (
	// VirtioBlock means use virtio-blk for hotplugging drives
	VirtioBlock = "virtio-blk"

	// VirtioSCSI means use virtio-scsi for hotplugging drives
	VirtioSCSI = "virtio-scsi"
)

const (
	// QemuPCLite is the QEMU pc-lite machine type for amd64
	QemuPCLite = "pc-lite"

	// QemuPC is the QEMU pc machine type for amd64
	QemuPC = "pc"

	// QemuQ35 is the QEMU Q35 machine type for amd64
	QemuQ35 = "q35"

	// QemuVirt is the QEMU virt machine type for aarch64
	QemuVirt = "virt"
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

func (q *qemuArchBase) machine() (govmmQemu.Machine, error) {
	for _, m := range q.supportedQemuMachines {
		if m.Type == q.machineType {
			return m, nil
		}
	}

	return govmmQemu.Machine{}, fmt.Errorf("unrecognised machine type: %v", q.machineType)
}

func (q *qemuArchBase) qemuPath() (string, error) {
	p, ok := q.qemuPaths[q.machineType]
	if !ok {
		return "", fmt.Errorf("Unknown machine type: %s", q.machineType)
	}

	return p, nil
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

func (q *qemuArchBase) capabilities() capabilities {
	var caps capabilities
	caps.setBlockDeviceHotplugSupport()
	return caps
}

func (q *qemuArchBase) bridges(number uint32) []Bridge {
	var bridges []Bridge

	for i := uint32(0); i < number; i++ {
		bridges = append(bridges, Bridge{
			Type:    pciBridge,
			ID:      fmt.Sprintf("%s-bridge-%d", pciBridge, i),
			Address: make(map[uint32]string),
		})
	}

	return bridges
}

func (q *qemuArchBase) cpuTopology(vcpus, maxvcpus uint32) govmmQemu.SMP {
	smp := govmmQemu.SMP{
		CPUs:    vcpus,
		Sockets: vcpus,
		Cores:   defaultCores,
		Threads: defaultThreads,
		MaxCPUs: maxvcpus,
	}

	return smp
}

func (q *qemuArchBase) cpuModel() string {
	return defaultCPUModel
}

func (q *qemuArchBase) memoryTopology(memoryMb, hostMemoryMb uint64) govmmQemu.Memory {
	memMax := fmt.Sprintf("%dM", hostMemoryMb)
	mem := fmt.Sprintf("%dM", memoryMb)
	memory := govmmQemu.Memory{
		Size:   mem,
		Slots:  defaultMemSlots,
		MaxMem: memMax,
	}

	return memory
}

func (q *qemuArchBase) append9PVolumes(devices []govmmQemu.Device, volumes []Volume) []govmmQemu.Device {
	// Add the shared volumes
	for _, v := range volumes {
		devices = q.append9PVolume(devices, v)
	}

	return devices
}

func (q *qemuArchBase) appendConsole(devices []govmmQemu.Device, path string) []govmmQemu.Device {
	serial := govmmQemu.SerialDevice{
		Driver:        govmmQemu.VirtioSerial,
		ID:            "serial0",
		DisableModern: q.nestedRun,
	}

	devices = append(devices, serial)

	var console govmmQemu.CharDevice

	console = govmmQemu.CharDevice{
		Driver:   govmmQemu.Console,
		Backend:  govmmQemu.Socket,
		DeviceID: "console0",
		ID:       "charconsole0",
		Path:     path,
	}

	devices = append(devices, console)

	return devices
}

func (q *qemuArchBase) appendImage(devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return nil, err
	}

	randBytes, err := utils.GenerateRandomBytes(8)
	if err != nil {
		return nil, err
	}

	id := utils.MakeNameID("image", hex.EncodeToString(randBytes), maxDevIDSize)

	drive := drivers.Drive{
		File:   path,
		Format: "raw",
		ID:     id,
	}

	return q.appendBlockDevice(devices, drive), nil
}

func (q *qemuArchBase) appendSCSIController(devices []govmmQemu.Device, enableIOThreads bool) ([]govmmQemu.Device, *govmmQemu.IOThread) {
	scsiController := govmmQemu.SCSIController{
		ID:            scsiControllerID,
		DisableModern: q.nestedRun,
	}

	var t *govmmQemu.IOThread

	if enableIOThreads {
		randBytes, _ := utils.GenerateRandomBytes(8)

		t = &govmmQemu.IOThread{
			ID: fmt.Sprintf("%s-%s", "iothread", hex.EncodeToString(randBytes)),
		}

		scsiController.IOThread = t.ID
	}

	devices = append(devices, scsiController)

	return devices, t
}

// appendBridges appends to devices the given bridges
func (q *qemuArchBase) appendBridges(devices []govmmQemu.Device, bridges []Bridge) []govmmQemu.Device {
	for idx, b := range bridges {
		t := govmmQemu.PCIBridge
		if b.Type == pcieBridge {
			t = govmmQemu.PCIEBridge
		}

		bridges[idx].Addr = bridgePCIStartAddr + idx

		devices = append(devices,
			govmmQemu.BridgeDevice{
				Type: t,
				Bus:  defaultBridgeBus,
				ID:   b.ID,
				// Each bridge is required to be assigned a unique chassis id > 0
				Chassis: (idx + 1),
				SHPC:    true,
				Addr:    strconv.FormatInt(int64(bridges[idx].Addr), 10),
			},
		)
	}

	return devices
}

func (q *qemuArchBase) append9PVolume(devices []govmmQemu.Device, volume Volume) []govmmQemu.Device {
	if volume.MountTag == "" || volume.HostPath == "" {
		return devices
	}

	devID := fmt.Sprintf("extra-9p-%s", volume.MountTag)
	if len(devID) > maxDevIDSize {
		devID = devID[:maxDevIDSize]
	}

	devices = append(devices,
		govmmQemu.FSDevice{
			Driver:        govmmQemu.Virtio9P,
			FSDriver:      govmmQemu.Local,
			ID:            devID,
			Path:          volume.HostPath,
			MountTag:      volume.MountTag,
			SecurityModel: govmmQemu.None,
			DisableModern: q.nestedRun,
		},
	)

	return devices
}

func (q *qemuArchBase) appendSocket(devices []govmmQemu.Device, socket Socket) []govmmQemu.Device {
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

func networkModelToQemuType(model NetInterworkingModel) govmmQemu.NetDeviceType {
	switch model {
	case NetXConnectBridgedModel:
		return govmmQemu.MACVTAP //TODO: We should rename MACVTAP to .NET_FD
	case NetXConnectMacVtapModel:
		return govmmQemu.MACVTAP
	//case ModelEnlightened:
	// Here the Network plugin will create a VM native interface
	// which could be MacVtap, IpVtap, SRIOV, veth-tap, vhost-user
	// In these cases we will determine the interface type here
	// and pass in the native interface through
	default:
		//TAP should work for most other cases
		return govmmQemu.TAP
	}
}

func (q *qemuArchBase) appendNetwork(devices []govmmQemu.Device, endpoint Endpoint) []govmmQemu.Device {
	switch ep := endpoint.(type) {
	case *VirtualEndpoint:
		devices = append(devices,
			govmmQemu.NetDevice{
				Type:          networkModelToQemuType(ep.NetPair.NetInterworkingModel),
				Driver:        govmmQemu.VirtioNetPCI,
				ID:            fmt.Sprintf("network-%d", q.networkIndex),
				IFName:        ep.NetPair.TAPIface.Name,
				MACAddress:    ep.NetPair.TAPIface.HardAddr,
				DownScript:    "no",
				Script:        "no",
				VHost:         true,
				DisableModern: q.nestedRun,
				FDs:           ep.NetPair.VMFds,
				VhostFDs:      ep.NetPair.VhostFds,
			},
		)
		q.networkIndex++
	}

	return devices
}

func (q *qemuArchBase) appendBlockDevice(devices []govmmQemu.Device, drive drivers.Drive) []govmmQemu.Device {
	if drive.File == "" || drive.ID == "" || drive.Format == "" {
		return devices
	}

	if len(drive.ID) > maxDevIDSize {
		drive.ID = drive.ID[:maxDevIDSize]
	}

	devices = append(devices,
		govmmQemu.BlockDevice{
			Driver:        govmmQemu.VirtioBlock,
			ID:            drive.ID,
			File:          drive.File,
			AIO:           govmmQemu.Threads,
			Format:        govmmQemu.BlockDeviceFormat(drive.Format),
			Interface:     "none",
			DisableModern: q.nestedRun,
		},
	)

	return devices
}

func (q *qemuArchBase) appendVhostUserDevice(devices []govmmQemu.Device, vhostUserDevice api.VhostUserDevice) []govmmQemu.Device {
	qemuVhostUserDevice := govmmQemu.VhostUserDevice{}

	// TODO: find a way to remove dependency of drivers package
	switch vhostUserDevice := vhostUserDevice.(type) {
	case *drivers.VhostUserNetDevice:
		qemuVhostUserDevice.TypeDevID = utils.MakeNameID("net", vhostUserDevice.ID, maxDevIDSize)
		qemuVhostUserDevice.Address = vhostUserDevice.MacAddress
	case *drivers.VhostUserSCSIDevice:
		qemuVhostUserDevice.TypeDevID = utils.MakeNameID("scsi", vhostUserDevice.ID, maxDevIDSize)
	case *drivers.VhostUserBlkDevice:
	}

	qemuVhostUserDevice.VhostUserType = govmmQemu.VhostUserDeviceType(vhostUserDevice.Type())
	qemuVhostUserDevice.SocketPath = vhostUserDevice.Attrs().SocketPath
	qemuVhostUserDevice.CharDevID = utils.MakeNameID("char", vhostUserDevice.Attrs().ID, maxDevIDSize)

	devices = append(devices, qemuVhostUserDevice)

	return devices
}

func (q *qemuArchBase) appendVFIODevice(devices []govmmQemu.Device, vfioDevice drivers.VFIODevice) []govmmQemu.Device {
	if vfioDevice.BDF == "" {
		return devices
	}

	devices = append(devices,
		govmmQemu.VFIODevice{
			BDF: vfioDevice.BDF,
		},
	)

	return devices
}
