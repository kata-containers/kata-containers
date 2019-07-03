// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"time"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

type qemuS390x struct {
	// inherit from qemuArchBase, overwrite methods if needed
	qemuArchBase
}

const defaultQemuPath = "/usr/bin/qemu-system-s390x"

const defaultQemuMachineType = QemuCCWVirtio

const defaultQemuMachineOptions = "accel=kvm"

const virtioSerialCCW = "virtio-serial-ccw"

const qmpMigrationWaitTimeout = 5 * time.Second

var qemuPaths = map[string]string{
	QemuCCWVirtio: defaultQemuPath,
}

// Verify needed parameters
var kernelParams = []Param{
	{"console", "ttysclp0"},
}

var kernelRootParams = commonVirtioblkKernelRootParams

var ccwbridge = types.NewBridge(types.CCW, "", make(map[uint32]string, types.CCWBridgeMaxCapacity), 0)

var supportedQemuMachines = []govmmQemu.Machine{
	{
		Type:    QemuCCWVirtio,
		Options: defaultQemuMachineOptions,
	},
}

// MaxQemuVCPUs returns the maximum number of vCPUs supported
func MaxQemuVCPUs() uint32 {
	// Max number of virtual Cpu defined in qemu. See
	// https://github.com/qemu/qemu/blob/80422b00196a7af4c6efb628fae0ad8b644e98af/target/s390x/cpu.h#L55
	// #define S390_MAX_CPUS 248
	return uint32(248)
}

func newQemuArch(config HypervisorConfig) qemuArch {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	q := &qemuS390x{
		qemuArchBase{
			machineType:           machineType,
			memoryOffset:          config.MemOffset,
			qemuPaths:             qemuPaths,
			supportedQemuMachines: supportedQemuMachines,
			kernelParamsNonDebug:  kernelParamsNonDebug,
			kernelParamsDebug:     kernelParamsDebug,
			kernelParams:          kernelParams,
		},
	}
	// Set first bridge type to CCW
	q.Bridges = append(q.Bridges, ccwbridge)

	if config.ImagePath != "" {
		q.kernelParams = append(q.kernelParams, kernelRootParams...)
		q.kernelParamsNonDebug = append(q.kernelParamsNonDebug, kernelParamsSystemdNonDebug...)
		q.kernelParamsDebug = append(q.kernelParamsDebug, kernelParamsSystemdDebug...)
	}

	return q
}

func (q *qemuS390x) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.machineType)
}

// appendConsole appends a console to devices.
// The function has been overwriten to correctly set the driver to the CCW device
func (q *qemuS390x) appendConsole(devices []govmmQemu.Device, path string) []govmmQemu.Device {
	id := "serial0"
	addr, b, err := q.addDeviceToBridge(id, types.CCW)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append console")
		return devices
	}

	var devno string
	devno, err = b.AddressFormatCCW(addr)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append console")
		return devices
	}

	serial := govmmQemu.SerialDevice{
		Driver:        virtioSerialCCW,
		ID:            id,
		DisableModern: q.nestedRun,
		DevNo:         devno,
	}

	devices = append(devices, serial)

	console := govmmQemu.CharDevice{
		Driver:   govmmQemu.Console,
		Backend:  govmmQemu.Socket,
		DeviceID: "console0",
		ID:       "charconsole0",
		Path:     path,
	}

	devices = append(devices, console)

	return devices
}

func (q *qemuS390x) appendImage(devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	drive, err := genericImage(path)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append image")
		return nil, err
	}

	return q.appendBlockDevice(devices, drive), nil
}

func (q *qemuS390x) appendBlockDevice(devices []govmmQemu.Device, drive config.BlockDrive) []govmmQemu.Device {
	d, err := genericBlockDevice(drive, false)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append blk-dev")
		return devices
	}
	addr, b, err := q.addDeviceToBridge(drive.ID, types.CCW)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append blk-dev")
		return devices
	}
	d.DevNo, err = b.AddressFormatCCW(addr)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append blk-dev")
		return devices
	}
	devices = append(devices, d)
	return devices
}

// appendVhostUserDevice throws an error if vhost devices are tried to be used.
// See issue https://github.com/kata-containers/runtime/issues/659
func (q *qemuS390x) appendVhostUserDevice(devices []govmmQemu.Device, attr config.VhostUserDeviceAttrs) ([]govmmQemu.Device, error) {
	return nil, fmt.Errorf("No vhost-user devices supported on s390x")
}

// supportGuestMemoryHotplug return false for s390x architecture. The pc-dimm backend device for s390x
// is not support. PC-DIMM is not listed in the devices supported by qemu-system-s390x -device help
func (q *qemuS390x) supportGuestMemoryHotplug() bool {
	return false
}

func (q *qemuS390x) appendNetwork(devices []govmmQemu.Device, endpoint Endpoint) []govmmQemu.Device {
	d, err := genericNetwork(endpoint, false, false, q.networkIndex)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append network")
		return devices
	}
	q.networkIndex++
	addr, b, err := q.addDeviceToBridge(d.ID, types.CCW)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append network")
		return devices
	}
	d.DevNo, err = b.AddressFormatCCW(addr)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append network")
		return devices
	}

	devices = append(devices, d)
	return devices
}

func (q *qemuS390x) appendRNGDevice(devices []govmmQemu.Device, rngDev config.RNGDev) []govmmQemu.Device {
	addr, b, err := q.addDeviceToBridge(rngDev.ID, types.CCW)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append RNG-Device")
		return devices
	}
	var devno string
	devno, err = b.AddressFormatCCW(addr)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append RNG-Device")
		return devices
	}

	devices = append(devices,
		govmmQemu.RngDevice{
			ID:       rngDev.ID,
			Filename: rngDev.Filename,
			DevNo:    devno,
		},
	)

	return devices
}

func (q *qemuS390x) append9PVolume(devices []govmmQemu.Device, volume types.Volume) []govmmQemu.Device {
	if volume.MountTag == "" || volume.HostPath == "" {
		return devices
	}
	d := generic9PVolume(volume, false)
	addr, b, err := q.addDeviceToBridge(d.ID, types.CCW)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append 9p-Volume")
		return devices
	}
	d.DevNo, err = b.AddressFormatCCW(addr)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append 9p-Volume")
		return devices
	}
	devices = append(devices, d)
	return devices
}

// appendBridges appends to devices the given bridges
func (q *qemuS390x) appendBridges(devices []govmmQemu.Device) []govmmQemu.Device {
	return genericAppendBridges(devices, q.Bridges, q.machineType)
}

func (q *qemuS390x) appendSCSIController(devices []govmmQemu.Device, enableIOThreads bool) ([]govmmQemu.Device, *govmmQemu.IOThread) {
	d, t := genericSCSIController(enableIOThreads, q.nestedRun)
	addr, b, err := q.addDeviceToBridge(d.ID, types.CCW)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append scsi-controller")
		return devices, nil
	}
	d.DevNo, err = b.AddressFormatCCW(addr)
	if err != nil {
		virtLog.WithField("subsystem", "qemus390x").WithError(err).Error("Failed to append scsi-controller")
		return devices, nil
	}

	devices = append(devices, d)
	return devices, t
}
