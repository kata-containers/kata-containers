// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"time"

	govmmQemu "github.com/kata-containers/govmm/qemu"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
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

// Verify needed parameters
var kernelParams = []Param{
	{"console", "ttysclp0"},
}

var ccwbridge = types.NewBridge(types.CCW, "", make(map[uint32]string, types.CCWBridgeMaxCapacity), 0)

var supportedQemuMachine = govmmQemu.Machine{
	Type:    QemuCCWVirtio,
	Options: defaultQemuMachineOptions,
}

// MaxQemuVCPUs returns the maximum number of vCPUs supported
func MaxQemuVCPUs() uint32 {
	// Max number of virtual Cpu defined in qemu. See
	// https://github.com/qemu/qemu/blob/80422b00196a7af4c6efb628fae0ad8b644e98af/target/s390x/cpu.h#L55
	// #define S390_MAX_CPUS 248
	return uint32(248)
}

func newQemuArch(config HypervisorConfig) (qemuArch, error) {
	machineType := config.HypervisorMachineType
	if machineType == "" {
		machineType = defaultQemuMachineType
	}

	if machineType != defaultQemuMachineType {
		return nil, fmt.Errorf("unrecognised machinetype: %v", machineType)
	}

	q := &qemuS390x{
		qemuArchBase{
			qemuMachine:          supportedQemuMachine,
			qemuExePath:          defaultQemuPath,
			memoryOffset:         config.MemOffset,
			kernelParamsNonDebug: kernelParamsNonDebug,
			kernelParamsDebug:    kernelParamsDebug,
			kernelParams:         kernelParams,
		},
	}
	// Set first bridge type to CCW
	q.Bridges = append(q.Bridges, ccwbridge)

	if config.ImagePath != "" {
		q.kernelParams = append(q.kernelParams, commonVirtioblkKernelRootParams...)
		q.kernelParamsNonDebug = append(q.kernelParamsNonDebug, kernelParamsSystemdNonDebug...)
		q.kernelParamsDebug = append(q.kernelParamsDebug, kernelParamsSystemdDebug...)
	}

	return q, nil
}

func (q *qemuS390x) bridges(number uint32) {
	q.Bridges = genericBridges(number, q.qemuMachine.Type)
}

// appendConsole appends a console to devices.
// The function has been overwriten to correctly set the driver to the CCW device
func (q *qemuS390x) appendConsole(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	id := "serial0"
	addr, b, err := q.addDeviceToBridge(ctx, id, types.CCW)
	if err != nil {
		return devices, fmt.Errorf("Failed to append console %v", err)
	}

	var devno string
	devno, err = b.AddressFormatCCW(addr)
	if err != nil {
		return devices, fmt.Errorf("Failed to append console %v", err)
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

	return devices, nil
}

func (q *qemuS390x) appendImage(ctx context.Context, devices []govmmQemu.Device, path string) ([]govmmQemu.Device, error) {
	drive, err := genericImage(path)
	if err != nil {
		return nil, err
	}
	return q.appendCCWBlockDevice(ctx, devices, drive)
}

func (q *qemuS390x) appendBlockDevice(ctx context.Context, devices []govmmQemu.Device, drive config.BlockDrive) ([]govmmQemu.Device, error) {
	return q.appendCCWBlockDevice(ctx, devices, drive)
}

func (q *qemuS390x) appendCCWBlockDevice(ctx context.Context, devices []govmmQemu.Device, drive config.BlockDrive) ([]govmmQemu.Device, error) {
	d, err := genericBlockDevice(drive, false)
	if err != nil {
		return devices, fmt.Errorf("Failed to append blk-dev %v", err)
	}
	addr, b, err := q.addDeviceToBridge(ctx, drive.ID, types.CCW)
	if err != nil {
		return devices, fmt.Errorf("Failed to append blk-dev %v", err)
	}
	d.DevNo, err = b.AddressFormatCCW(addr)
	if err != nil {
		return devices, fmt.Errorf("Failed to append blk-dev %v", err)
	}
	devices = append(devices, d)
	return devices, nil
}

func (q *qemuS390x) appendVhostUserDevice(ctx context.Context, devices []govmmQemu.Device, attr config.VhostUserDeviceAttrs) ([]govmmQemu.Device, error) {
	if attr.Type != config.VhostUserFS {
		return devices, fmt.Errorf("vhost-user device of type %s not supported on s390x, only vhost-user-fs-ccw is supported", attr.Type)
	}

	addr, b, err := q.addDeviceToBridge(ctx, attr.DevID, types.CCW)
	if err != nil {
		return devices, fmt.Errorf("Failed to append vhost user device: %s", err)
	}
	var devno string
	devno, err = b.AddressFormatCCW(addr)
	if err != nil {
		return devices, fmt.Errorf("Failed to append vhost user device: %s", err)
	}

	qemuVhostUserDevice := govmmQemu.VhostUserDevice{
		SocketPath:    attr.SocketPath,
		CharDevID:     utils.MakeNameID("char", attr.DevID, maxDevIDSize),
		TypeDevID:     utils.MakeNameID("fs", attr.DevID, maxDevIDSize),
		Tag:           attr.Tag,
		CacheSize:     attr.CacheSize,
		VhostUserType: govmmQemu.VhostUserFS,
		DevNo:         devno,
	}

	devices = append(devices, qemuVhostUserDevice)
	return devices, nil
}

// supportGuestMemoryHotplug return false for s390x architecture. The pc-dimm backend device for s390x
// is not support. PC-DIMM is not listed in the devices supported by qemu-system-s390x -device help
func (q *qemuS390x) supportGuestMemoryHotplug() bool {
	return false
}

func (q *qemuS390x) appendNetwork(ctx context.Context, devices []govmmQemu.Device, endpoint Endpoint) ([]govmmQemu.Device, error) {
	d, err := genericNetwork(endpoint, false, false, q.networkIndex)
	if err != nil {
		return devices, fmt.Errorf("Failed to append network %v", err)
	}
	q.networkIndex++
	addr, b, err := q.addDeviceToBridge(ctx, d.ID, types.CCW)
	if err != nil {
		return devices, fmt.Errorf("Failed to append network %v", err)
	}
	d.DevNo, err = b.AddressFormatCCW(addr)
	if err != nil {
		return devices, fmt.Errorf("Failed to append network %v", err)
	}

	devices = append(devices, d)
	return devices, nil
}

func (q *qemuS390x) appendRNGDevice(ctx context.Context, devices []govmmQemu.Device, rngDev config.RNGDev) ([]govmmQemu.Device, error) {
	addr, b, err := q.addDeviceToBridge(ctx, rngDev.ID, types.CCW)
	if err != nil {
		return devices, fmt.Errorf("Failed to append RNG-Device %v", err)
	}
	var devno string
	devno, err = b.AddressFormatCCW(addr)
	if err != nil {
		return devices, fmt.Errorf("Failed to append RNG-Device %v", err)
	}

	devices = append(devices,
		govmmQemu.RngDevice{
			ID:       rngDev.ID,
			Filename: rngDev.Filename,
			DevNo:    devno,
		},
	)

	return devices, nil
}

func (q *qemuS390x) append9PVolume(ctx context.Context, devices []govmmQemu.Device, volume types.Volume) ([]govmmQemu.Device, error) {
	if volume.MountTag == "" || volume.HostPath == "" {
		return devices, nil
	}
	d := generic9PVolume(volume, false)
	addr, b, err := q.addDeviceToBridge(ctx, d.ID, types.CCW)
	if err != nil {
		return devices, fmt.Errorf("Failed to append 9p-Volume %v", err)
	}
	d.DevNo, err = b.AddressFormatCCW(addr)
	if err != nil {
		return devices, fmt.Errorf("Failed to append 9p-Volume %v", err)
	}
	devices = append(devices, d)
	return devices, nil
}

// appendBridges appends to devices the given bridges
func (q *qemuS390x) appendBridges(devices []govmmQemu.Device) []govmmQemu.Device {
	return genericAppendBridges(devices, q.Bridges, q.qemuMachine.Type)
}

func (q *qemuS390x) appendSCSIController(ctx context.Context, devices []govmmQemu.Device, enableIOThreads bool) ([]govmmQemu.Device, *govmmQemu.IOThread, error) {
	d, t := genericSCSIController(enableIOThreads, q.nestedRun)
	addr, b, err := q.addDeviceToBridge(ctx, d.ID, types.CCW)
	if err != nil {
		return devices, nil, fmt.Errorf("Failed to append scsi-controller %v", err)
	}
	d.DevNo, err = b.AddressFormatCCW(addr)
	if err != nil {
		return devices, nil, fmt.Errorf("Failed to append scsi-controller %v", err)
	}

	devices = append(devices, d)
	return devices, t, nil
}

func (q *qemuS390x) appendVSock(ctx context.Context, devices []govmmQemu.Device, vsock types.VSock) ([]govmmQemu.Device, error) {
	var devno string
	id := fmt.Sprintf("vsock-%d", vsock.ContextID)
	addr, b, err := q.addDeviceToBridge(ctx, id, types.CCW)
	if err != nil {
		return devices, fmt.Errorf("Failed to append VSock: %v", err)
	}
	devno, err = b.AddressFormatCCW(addr)
	if err != nil {
		return devices, fmt.Errorf("Failed to append VSock: %v", err)
	}
	devices = append(devices,
		govmmQemu.VSOCKDevice{
			ID:            id,
			ContextID:     vsock.ContextID,
			VHostFD:       vsock.VhostFd,
			DisableModern: false,
			DevNo:         devno,
		},
	)

	return devices, nil

}

func (q *qemuS390x) appendIOMMU(devices []govmmQemu.Device) ([]govmmQemu.Device, error) {
	return devices, fmt.Errorf("S390x does not support appending a vIOMMU")
}

func (q *qemuS390x) addDeviceToBridge(ctx context.Context, ID string, t types.Type) (string, types.Bridge, error) {
	addr, b, err := genericAddDeviceToBridge(ctx, q.Bridges, ID, types.CCW)
	if err != nil {
		return "", b, err
	}

	return fmt.Sprintf("%04x", addr), b, nil
}
