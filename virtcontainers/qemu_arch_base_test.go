//
// Copyright (c) 2018 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"path/filepath"
	"testing"

	govmmQemu "github.com/intel/govmm/qemu"
	"github.com/stretchr/testify/assert"
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

	smp := qemuArchBase.cpuTopology(vcpus)
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
	expectedMemory := govmmQemu.Memory{
		Size:   fmt.Sprintf("%dM", mem),
		Slots:  defaultMemSlots,
		MaxMem: fmt.Sprintf("%dM", hostMem),
	}

	m := qemuArchBase.memoryTopology(mem, hostMem)
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
	case []Volume:
		devices = qemuArchBase.append9PVolumes(devices, s)
	case Drive:
		devices = qemuArchBase.appendBlockDevice(devices, s)
	case VFIODevice:
		devices = qemuArchBase.appendVFIODevice(devices, s)
	case VhostUserNetDevice:
		devices = qemuArchBase.appendVhostUserDevice(devices, &s)
	}

	assert.Equal(devices, expected)
}

func TestQemuArchBaseAppend9PVolumes(t *testing.T) {
	volMountTag := "testVolMountTag"
	volHostPath := "testVolHostPath"

	expectedOut := []govmmQemu.Device{
		govmmQemu.FSDevice{
			Driver:        govmmQemu.Virtio9P,
			FSDriver:      govmmQemu.Local,
			ID:            fmt.Sprintf("extra-9p-%s", fmt.Sprintf("%s.1", volMountTag)),
			Path:          fmt.Sprintf("%s.1", volHostPath),
			MountTag:      fmt.Sprintf("%s.1", volMountTag),
			SecurityModel: govmmQemu.None,
		},
		govmmQemu.FSDevice{
			Driver:        govmmQemu.Virtio9P,
			FSDriver:      govmmQemu.Local,
			ID:            fmt.Sprintf("extra-9p-%s", fmt.Sprintf("%s.2", volMountTag)),
			Path:          fmt.Sprintf("%s.2", volHostPath),
			MountTag:      fmt.Sprintf("%s.2", volMountTag),
			SecurityModel: govmmQemu.None,
		},
	}

	volumes := []Volume{
		{
			MountTag: fmt.Sprintf("%s.1", volMountTag),
			HostPath: fmt.Sprintf("%s.1", volHostPath),
		},
		{
			MountTag: fmt.Sprintf("%s.2", volMountTag),
			HostPath: fmt.Sprintf("%s.2", volHostPath),
		},
	}

	testQemuArchBaseAppend(t, volumes, expectedOut)
}

func TestQemuArchBaseAppendConsoles(t *testing.T) {
	var devices []govmmQemu.Device
	assert := assert.New(t)
	qemuArchBase := newQemuArchBase()

	path := filepath.Join(runStoragePath, podID, defaultConsole)

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

	drive := Drive{
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
			VhostUserType: VhostUserNet,
		},
	}

	vhostUserDevice := VhostUserNetDevice{
		MacAddress: macAddress,
	}
	vhostUserDevice.ID = id
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

	vfDevice := VFIODevice{
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
