//go:build linux

// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"testing"

	govmmQemu "github.com/kata-containers/kata-containers/src/runtime/pkg/govmm/qemu"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/kata-containers/src/runtime/pkg/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/pkg/errors"
)

const (
	qemuArchBaseQemuPath = "/usr/bin/qemu-system-x86_64"
)

var qemuArchBaseMachine = govmmQemu.Machine{
	Type: "q35",
}

var qemuArchBaseQemuPaths = map[string]string{
	qemuArchBaseMachine.Type: qemuArchBaseQemuPath,
}

var qemuArchBaseKernelParamsNonDebug = []Param{
	{"quiet", ""},
	{"systemd.show_status", "false"},
}

var qemuArchBaseKernelParamsDebug = []Param{
	{"debug", ""},
	{"systemd.show_status", "true"},
	{"systemd.log_level", "debug"},
}

var qemuArchBaseKernelParams = []Param{
	{"root", "/dev/vda"},
	{"rootfstype", "ext4"},
}

func newQemuArchBase() *qemuArchBase {
	return &qemuArchBase{
		qemuMachine:          qemuArchBaseMachine,
		qemuExePath:          qemuArchBaseQemuPaths[qemuArchBaseMachine.Type],
		nestedRun:            false,
		kernelParamsNonDebug: qemuArchBaseKernelParamsNonDebug,
		kernelParamsDebug:    qemuArchBaseKernelParamsDebug,
		kernelParams:         qemuArchBaseKernelParams,
	}
}

func TestQemuArchBaseEnableNestingChecks(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	qemuArchBase.enableNestingChecks()
	assert.True(qemuArchBase.nestedRun)
}

func TestQemuArchBaseDisableNestingChecks(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	qemuArchBase.disableNestingChecks()
	assert.False(qemuArchBase.nestedRun)
}

func TestQemuArchBaseMachine(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	m := qemuArchBase.machine()
	assert.Equal(m.Type, qemuArchBaseMachine.Type)
}

func TestQemuArchBaseQemuPath(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	p := qemuArchBase.qemuPath()
	assert.Equal(p, qemuArchBaseQemuPath)
}

func TestQemuArchBaseKernelParameters(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	// with debug params
	expectedParams := qemuArchBaseKernelParams
	debugParams := qemuArchBaseKernelParamsDebug
	expectedParams = append(expectedParams, debugParams...)
	p := qemuArchBase.kernelParameters(true)
	assert.Equal(expectedParams, p)

	// with non-debug params
	expectedParams = qemuArchBaseKernelParams
	nonDebugParams := qemuArchBaseKernelParamsNonDebug
	expectedParams = append(expectedParams, nonDebugParams...)
	p = qemuArchBase.kernelParameters(false)
	assert.Equal(expectedParams, p)
}

func TestQemuArchBaseCapabilities(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()
	hConfig := HypervisorConfig{}
	hConfig.SharedFS = config.VirtioFS

	c := qemuArchBase.capabilities(hConfig)
	assert.True(c.IsBlockDeviceHotplugSupported())
	assert.True(c.IsFsSharingSupported())
	assert.True(c.IsNetworkDeviceHotplugSupported())

	hConfig.SharedFS = config.NoSharedFS
	c = qemuArchBase.capabilities(hConfig)
	assert.False(c.IsFsSharingSupported())
}

func TestQemuArchBaseBridges(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()
	len := 5

	qemuArchBase.bridges(uint32(len))
	bridges := qemuArchBase.getBridges()
	assert.Len(bridges, len)

	for i, b := range bridges {
		id := fmt.Sprintf("%s-bridge-%d", types.PCI, i)
		assert.Equal(types.PCI, b.Type)
		assert.Equal(id, b.ID)
		assert.NotNil(b.Devices)
	}
}

func TestQemuAddDeviceToBridge(t *testing.T) {
	assert := assert.New(t)

	// addDeviceToBridge successfully
	q := newQemuArchBase()
	q.qemuMachine.Type = QemuQ35

	q.bridges(1)
	for i := uint32(1); i <= types.PCIBridgeMaxCapacity; i++ {
		_, _, err := q.addDeviceToBridge(context.Background(), fmt.Sprintf("qemu-bridge-%d", i), types.PCI)
		assert.Nil(err)
	}

	// fail to add device to bridge cause no more available bridge slot
	_, _, err := q.addDeviceToBridge(context.Background(), "qemu-bridge-31", types.PCI)
	exceptErr := errors.New("no more bridge slots available")
	assert.Equal(exceptErr.Error(), err.Error())

	// addDeviceToBridge fails cause q.Bridges == 0
	q = newQemuArchBase()
	q.qemuMachine.Type = QemuQ35
	q.bridges(0)
	_, _, err = q.addDeviceToBridge(context.Background(), "qemu-bridge", types.PCI)
	if assert.Error(err) {
		exceptErr = errors.New("failed to get available address from bridges")
		assert.Equal(exceptErr.Error(), err.Error())
	}
}

func TestQemuArchBaseCPUTopology(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()
	vcpus := uint32(2)

	expectedSMP := govmmQemu.SMP{
		CPUs:    vcpus,
		Sockets: defaultMaxVCPUs,
		Cores:   defaultCores,
		Threads: defaultThreads,
		MaxCPUs: defaultMaxVCPUs,
	}

	smp := qemuArchBase.cpuTopology(vcpus, defaultMaxVCPUs)
	assert.Equal(expectedSMP, smp)
}

func TestQemuArchBaseCPUModel(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	assert.Equal(defaultCPUModel, qemuArchBase.cpuModel())
}

func TestQemuArchBaseMemoryTopology(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	hostMem := uint64(100)
	mem := uint64(120)
	slots := uint8(12)
	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  slots,
		MaxMem: fmt.Sprintf("%dM", hostMem),
	}

	m := qemuArchBase.memoryTopology(mem, hostMem, slots)
	assert.Equal(expectedMemory, m)
}

func testQemuArchBaseAppend(t *testing.T, structure interface{}, expected []govmmQemu.Device) {
	var devices []govmmQemu.Device
	var err error
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	switch s := structure.(type) {
	case types.Volume:
		devices, err = qemuArchBase.append9PVolume(context.Background(), devices, s)
	case types.Socket:
		devices = qemuArchBase.appendSocket(devices, s)
	case config.BlockDrive:
		devices, err = qemuArchBase.appendBlockDevice(context.Background(), devices, s)
	case config.VFIODev:
		devices = qemuArchBase.appendVFIODevice(devices, s)
	case config.VhostUserDeviceAttrs:
		devices, err = qemuArchBase.appendVhostUserDevice(context.Background(), devices, s)
	}

	assert.NoError(err)
	assert.Equal(devices, expected)
}

func TestQemuArchBaseAppendConsoles(t *testing.T) {
	var devices []govmmQemu.Device
	var err error
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	path := filepath.Join(filepath.Join(fs.MockRunStoragePath(), "test"), consoleSocket)

	expectedOut := []govmmQemu.Device{
		govmmQemu.SerialDevice{
			Driver:   govmmQemu.VirtioSerial,
			ID:       "serial0",
			MaxPorts: uint(2),
		},
		govmmQemu.CharDevice{
			Driver:   govmmQemu.Console,
			Backend:  govmmQemu.Socket,
			DeviceID: "console0",
			ID:       "charconsole0",
			Path:     path,
		},
	}

	devices, err = qemuArchBase.appendConsole(context.Background(), devices, path)
	assert.NoError(err)
	assert.Equal(expectedOut, devices)
	assert.Contains(qemuArchBase.kernelParams, Param{"console", "hvc0"})
	assert.Contains(qemuArchBase.kernelParams, Param{"console", "hvc1"})
}

func TestQemuArchBaseAppendConsolesLegacy(t *testing.T) {
	var devices []govmmQemu.Device
	var err error
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()
	qemuArchBase.legacySerial = true

	path := filepath.Join(filepath.Join(fs.MockRunStoragePath(), "test"), consoleSocket)

	expectedOut := []govmmQemu.Device{
		govmmQemu.LegacySerialDevice{
			Chardev: "charconsole0",
		},
		govmmQemu.CharDevice{
			Driver:   govmmQemu.LegacySerial,
			Backend:  govmmQemu.Socket,
			DeviceID: "console0",
			ID:       "charconsole0",
			Path:     path,
		},
	}

	devices, err = qemuArchBase.appendConsole(context.Background(), devices, path)
	assert.NoError(err)
	assert.Equal(expectedOut, devices)
	assert.Contains(qemuArchBase.kernelParams, Param{"console", "ttyS0"})
}

func TestQemuArchBaseAppendImage(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	image, err := os.CreateTemp("", "img")
	assert.NoError(err)
	defer os.Remove(image.Name())
	err = image.Close()
	assert.NoError(err)

	devices, err = qemuArchBase.appendImage(context.Background(), devices, image.Name())
	assert.NoError(err)
	assert.Len(devices, 1)

	drive, ok := devices[0].(govmmQemu.BlockDevice)
	assert.True(ok)

	expectedOut := []govmmQemu.Device{
		govmmQemu.BlockDevice{
			Driver:    govmmQemu.VirtioBlock,
			ID:        drive.ID,
			File:      image.Name(),
			AIO:       govmmQemu.Threads,
			Format:    "raw",
			Interface: "none",
			ShareRW:   true,
			ReadOnly:  true,
		},
	}

	assert.Equal(expectedOut, devices)
}

func TestQemuArchBaseAppendBridges(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	qemuArchBase.bridges(1)
	bridges := qemuArchBase.getBridges()
	assert.Len(bridges, 1)

	devices = qemuArchBase.appendBridges(devices)
	assert.Len(devices, 1)

	expectedOut := []govmmQemu.Device{
		govmmQemu.BridgeDevice{
			Type:          govmmQemu.PCIBridge,
			Bus:           defaultBridgeBus,
			ID:            bridges[0].ID,
			Chassis:       1,
			SHPC:          false,
			Addr:          "2",
			IOReserve:     "4k",
			MemReserve:    "1m",
			Pref64Reserve: "1m",
		},
	}

	assert.Equal(expectedOut, devices)
}

func TestQemuArchBaseAppend9PVolume(t *testing.T) {
	mountTag := "testMountTag"
	hostPath := "testHostPath"

	expectedOut := []govmmQemu.Device{
		govmmQemu.FSDevice{
			Driver:        govmmQemu.Virtio9P,
			FSDriver:      govmmQemu.Local,
			ID:            fmt.Sprintf("extra-9p-%s", mountTag),
			Path:          hostPath,
			MountTag:      mountTag,
			SecurityModel: govmmQemu.None,
			Multidev:      govmmQemu.Remap,
		},
	}

	volume := types.Volume{
		MountTag: mountTag,
		HostPath: hostPath,
	}

	testQemuArchBaseAppend(t, volume, expectedOut)
}

func TestQemuArchBaseAppendSocket(t *testing.T) {
	deviceID := "channelTest"
	id := "charchTest"
	hostPath := "/tmp/hyper_test.sock"
	name := "sh.hyper.channel.test"

	expectedOut := []govmmQemu.Device{
		govmmQemu.CharDevice{
			Driver:   govmmQemu.VirtioSerialPort,
			Backend:  govmmQemu.Socket,
			DeviceID: deviceID,
			ID:       id,
			Path:     hostPath,
			Name:     name,
		},
	}

	socket := types.Socket{
		DeviceID: deviceID,
		ID:       id,
		HostPath: hostPath,
		Name:     name,
	}

	testQemuArchBaseAppend(t, socket, expectedOut)
}

func TestQemuArchBaseAppendBlockDevice(t *testing.T) {
	id := "blockDevTest"
	file := "/root"
	format := "raw"

	expectedOut := []govmmQemu.Device{
		govmmQemu.BlockDevice{
			Driver:    govmmQemu.VirtioBlock,
			ID:        id,
			File:      "/root",
			AIO:       govmmQemu.Threads,
			Format:    govmmQemu.BlockDeviceFormat(format),
			Interface: "none",
		},
	}

	drive := config.BlockDrive{
		File:   file,
		Format: format,
		ID:     id,
	}

	testQemuArchBaseAppend(t, drive, expectedOut)
}

func TestQemuArchBaseAppendVhostUserDevice(t *testing.T) {
	socketPath := "nonexistentpath.sock"
	macAddress := "00:11:22:33:44:55:66"
	id := "deadbeef"

	expectedOut := []govmmQemu.Device{
		govmmQemu.VhostUserDevice{
			SocketPath:    socketPath,
			CharDevID:     fmt.Sprintf("char-%s", id),
			TypeDevID:     fmt.Sprintf("net-%s", id),
			Address:       macAddress,
			VhostUserType: govmmQemu.VhostUserNet,
		},
	}

	vhostUserDevice := config.VhostUserDeviceAttrs{
		Type:       config.VhostUserNet,
		MacAddress: macAddress,
	}
	vhostUserDevice.DevID = id
	vhostUserDevice.SocketPath = socketPath

	testQemuArchBaseAppend(t, vhostUserDevice, expectedOut)
}

func TestQemuArchBaseAppendVFIODevice(t *testing.T) {
	bdf := "02:10.1"

	expectedOut := []govmmQemu.Device{
		govmmQemu.VFIODevice{
			BDF: bdf,
		},
	}

	vfDevice := config.VFIODev{
		BDF: bdf,
	}

	testQemuArchBaseAppend(t, vfDevice, expectedOut)
}

func TestQemuArchBaseAppendVFIODeviceWithVendorDeviceID(t *testing.T) {
	bdf := "02:10.1"
	vendorID := "0x1234"
	deviceID := "0x5678"

	expectedOut := []govmmQemu.Device{
		govmmQemu.VFIODevice{
			BDF:      bdf,
			VendorID: vendorID,
			DeviceID: deviceID,
		},
	}

	vfDevice := config.VFIODev{
		BDF:      bdf,
		VendorID: vendorID,
		DeviceID: deviceID,
	}

	testQemuArchBaseAppend(t, vfDevice, expectedOut)
}

func TestQemuArchBaseAppendSCSIController(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	expectedOut := []govmmQemu.Device{
		govmmQemu.SCSIController{
			ID: scsiControllerID,
		},
	}

	devices, ioThread, err := qemuArchBase.appendSCSIController(context.Background(), devices, false)
	assert.Equal(expectedOut, devices)
	assert.Nil(ioThread)
	assert.NoError(err)

	_, ioThread, err = qemuArchBase.appendSCSIController(context.Background(), devices, true)
	assert.NotNil(ioThread)
	assert.NoError(err)
}

func TestQemuArchBaseAppendNetwork(t *testing.T) {
	var devices []govmmQemu.Device
	var err error
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	macvlanEp := &MacvlanEndpoint{
		NetPair: NetworkInterfacePair{
			TapInterface: TapInterface{
				ID:   "uniqueTestID-4",
				Name: "br4_kata",
				TAPIface: NetworkInterface{
					Name: "tap4_kata",
				},
			},
			VirtIface: NetworkInterface{
				Name:     "eth4",
				HardAddr: macAddr.String(),
			},
			NetInterworkingModel: DefaultNetInterworkingModel,
		},
		EndpointType: MacvlanEndpointType,
	}

	macvtapEp := &MacvtapEndpoint{
		EndpointType: MacvtapEndpointType,
		EndpointProperties: NetworkInfo{
			Iface: NetlinkIface{
				Type: "macvtap",
			},
		},
	}

	expectedOut := []govmmQemu.Device{
		govmmQemu.NetDevice{
			Type:       networkModelToQemuType(macvlanEp.NetPair.NetInterworkingModel),
			Driver:     govmmQemu.VirtioNet,
			ID:         fmt.Sprintf("network-%d", 0),
			IFName:     macvlanEp.NetPair.TAPIface.Name,
			MACAddress: macvlanEp.NetPair.TAPIface.HardAddr,
			DownScript: "no",
			Script:     "no",
			FDs:        macvlanEp.NetPair.VMFds,
			VhostFDs:   macvlanEp.NetPair.VhostFds,
		},
		govmmQemu.NetDevice{
			Type:       govmmQemu.MACVTAP,
			Driver:     govmmQemu.VirtioNet,
			ID:         fmt.Sprintf("network-%d", 1),
			IFName:     macvtapEp.Name(),
			MACAddress: macvtapEp.HardwareAddr(),
			DownScript: "no",
			Script:     "no",
			FDs:        macvtapEp.VMFds,
			VhostFDs:   macvtapEp.VhostFds,
		},
	}

	devices, err = qemuArchBase.appendNetwork(context.Background(), devices, macvlanEp)
	assert.NoError(err)
	devices, err = qemuArchBase.appendNetwork(context.Background(), devices, macvtapEp)
	assert.NoError(err)
	assert.Equal(expectedOut, devices)
}

func TestQemuArchBaseAppendIOMMU(t *testing.T) {
	var devices []govmmQemu.Device
	var err error
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	expectedOut := []govmmQemu.Device{
		govmmQemu.IommuDev{
			Intremap:    true,
			DeviceIotlb: true,
			CachingMode: true,
		},
	}

	qemuArchBase.qemuMachine.Type = QemuQ35
	devices, err = qemuArchBase.appendIOMMU(devices)
	assert.NoError(err)
	assert.Equal(expectedOut, devices)
}
