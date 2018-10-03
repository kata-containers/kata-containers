// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"net"
	"path/filepath"
	"testing"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/stretchr/testify/assert"

	"github.com/kata-containers/runtime/virtcontainers/device/config"
)

const (
	qemuArchBaseMachineType = "pc"
	qemuArchBaseQemuPath    = "/usr/bin/qemu-system-x86_64"
)

var qemuArchBaseQemuPaths = map[string]string{
	qemuArchBaseMachineType: qemuArchBaseQemuPath,
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

var qemuArchBaseSupportedQemuMachines = []govmmQemu.Machine{
	{
		Type: qemuArchBaseMachineType,
	},
}

func newQemuArchBase() *qemuArchBase {
	return &qemuArchBase{
		machineType:           qemuArchBaseMachineType,
		nestedRun:             false,
		qemuPaths:             qemuArchBaseQemuPaths,
		supportedQemuMachines: qemuArchBaseSupportedQemuMachines,
		kernelParamsNonDebug:  qemuArchBaseKernelParamsNonDebug,
		kernelParamsDebug:     qemuArchBaseKernelParamsDebug,
		kernelParams:          qemuArchBaseKernelParams,
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

	m, err := qemuArchBase.machine()
	assert.NoError(err)
	assert.Equal(m.Type, qemuArchBaseMachineType)

	machines := []govmmQemu.Machine{
		{
			Type: "bad",
		},
	}
	qemuArchBase.supportedQemuMachines = machines
	m, err = qemuArchBase.machine()
	assert.Error(err)
	assert.Equal("", m.Type)
}

func TestQemuArchBaseQemuPath(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	p, err := qemuArchBase.qemuPath()
	assert.NoError(err)
	assert.Equal(p, qemuArchBaseQemuPath)

	paths := map[string]string{
		"bad": qemuArchBaseQemuPath,
	}
	qemuArchBase.qemuPaths = paths
	p, err = qemuArchBase.qemuPath()
	assert.Error(err)
	assert.Equal("", p)
}

func TestQemuArchBaseKernelParameters(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	// with debug params
	expectedParams := []Param(qemuArchBaseKernelParams)
	debugParams := []Param(qemuArchBaseKernelParamsDebug)
	expectedParams = append(expectedParams, debugParams...)
	p := qemuArchBase.kernelParameters(true)
	assert.Equal(expectedParams, p)

	// with non-debug params
	expectedParams = []Param(qemuArchBaseKernelParams)
	nonDebugParams := []Param(qemuArchBaseKernelParamsNonDebug)
	expectedParams = append(expectedParams, nonDebugParams...)
	p = qemuArchBase.kernelParameters(false)
	assert.Equal(expectedParams, p)
}

func TestQemuArchBaseCapabilities(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	c := qemuArchBase.capabilities()
	assert.True(c.isBlockDeviceHotplugSupported())
}

func TestQemuArchBaseBridges(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()
	len := 5

	bridges := qemuArchBase.bridges(uint32(len))
	assert.Len(bridges, len)

	for i, b := range bridges {
		id := fmt.Sprintf("%s-bridge-%d", pciBridge, i)
		assert.Equal(pciBridge, b.Type)
		assert.Equal(id, b.ID)
		assert.NotNil(b.Address)
	}
}

func TestQemuArchBaseCPUTopology(t *testing.T) {
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()
	vcpus := uint32(2)

	expectedSMP := govmmQemu.SMP{
		CPUs:    vcpus,
		Sockets: vcpus,
		Cores:   defaultCores,
		Threads: defaultThreads,
		MaxCPUs: defaultMaxQemuVCPUs,
	}

	smp := qemuArchBase.cpuTopology(vcpus, defaultMaxQemuVCPUs)
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
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	switch s := structure.(type) {
	case Volume:
		devices = qemuArchBase.append9PVolume(devices, s)
	case Socket:
		devices = qemuArchBase.appendSocket(devices, s)
	case config.BlockDrive:
		devices = qemuArchBase.appendBlockDevice(devices, s)
	case config.VFIODev:
		devices = qemuArchBase.appendVFIODevice(devices, s)
	case config.VhostUserDeviceAttrs:
		devices = qemuArchBase.appendVhostUserDevice(devices, s)
	}

	assert.Equal(devices, expected)
}

func TestQemuArchBaseAppendConsoles(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	path := filepath.Join(runStoragePath, sandboxID, consoleSocket)

	expectedOut := []govmmQemu.Device{
		govmmQemu.SerialDevice{
			Driver: govmmQemu.VirtioSerial,
			ID:     "serial0",
		},
		govmmQemu.CharDevice{
			Driver:   govmmQemu.Console,
			Backend:  govmmQemu.Socket,
			DeviceID: "console0",
			ID:       "charconsole0",
			Path:     path,
		},
	}

	devices = qemuArchBase.appendConsole(devices, path)
	assert.Equal(expectedOut, devices)
}

func TestQemuArchBaseAppendImage(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	image, err := ioutil.TempFile("", "img")
	assert.NoError(err)
	err = image.Close()
	assert.NoError(err)

	devices, err = qemuArchBase.appendImage(devices, image.Name())
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
		},
	}

	assert.Equal(expectedOut, devices)
}

func TestQemuArchBaseAppendBridges(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	bridges := qemuArchBase.bridges(1)
	assert.Len(bridges, 1)

	devices = qemuArchBase.appendBridges(devices, bridges)
	assert.Len(devices, 1)

	expectedOut := []govmmQemu.Device{
		govmmQemu.BridgeDevice{
			Type:    govmmQemu.PCIBridge,
			Bus:     defaultBridgeBus,
			ID:      bridges[0].ID,
			Chassis: 1,
			SHPC:    true,
			Addr:    "2",
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
		},
	}

	volume := Volume{
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

	socket := Socket{
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
			VhostUserType: config.VhostUserNet,
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

func TestQemuArchBaseAppendSCSIController(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	expectedOut := []govmmQemu.Device{
		govmmQemu.SCSIController{
			ID: scsiControllerID,
		},
	}

	devices, ioThread := qemuArchBase.appendSCSIController(devices, false)
	assert.Equal(expectedOut, devices)
	assert.Nil(ioThread)

	_, ioThread = qemuArchBase.appendSCSIController(devices, true)
	assert.NotNil(ioThread)
}

func TestQemuArchBaseAppendNetwork(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	macvlanEp := &BridgedMacvlanEndpoint{
		NetPair: NetworkInterfacePair{
			ID:   "uniqueTestID-4",
			Name: "br4_kata",
			VirtIface: NetworkInterface{
				Name:     "eth4",
				HardAddr: macAddr.String(),
			},
			TAPIface: NetworkInterface{
				Name: "tap4_kata",
			},
			NetInterworkingModel: DefaultNetInterworkingModel,
		},
		EndpointType: BridgedMacvlanEndpointType,
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
			Driver:     govmmQemu.VirtioNetPCI,
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
			Driver:     govmmQemu.VirtioNetPCI,
			ID:         fmt.Sprintf("network-%d", 1),
			IFName:     macvtapEp.Name(),
			MACAddress: macvtapEp.HardwareAddr(),
			DownScript: "no",
			Script:     "no",
			FDs:        macvtapEp.VMFds,
			VhostFDs:   macvtapEp.VhostFds,
		},
	}

	devices = qemuArchBase.appendNetwork(devices, macvlanEp)
	devices = qemuArchBase.appendNetwork(devices, macvtapEp)
	assert.Equal(expectedOut, devices)
}
