// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"path/filepath"
	"testing"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/device/config"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/fs"
	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

const (
	acrnArchBaseAcrnPath    = "/usr/bin/acrn"
	acrnArchBaseAcrnCtlPath = "/usr/bin/acrnctl"
)

var acrnArchBaseKernelParamsNonDebug = []Param{
	{"quiet", ""},
}

var acrnArchBaseKernelParamsDebug = []Param{
	{"debug", ""},
}

var acrnArchBaseKernelParams = []Param{
	{"root", "/dev/vda"},
}

func newAcrnArchBase() *acrnArchBase {
	return &acrnArchBase{
		path:                 acrnArchBaseAcrnPath,
		ctlpath:              acrnArchBaseAcrnCtlPath,
		kernelParamsNonDebug: acrnArchBaseKernelParamsNonDebug,
		kernelParamsDebug:    acrnArchBaseKernelParamsDebug,
		kernelParams:         acrnArchBaseKernelParams,
	}
}

func TestAcrnArchBaseAcrnPaths(t *testing.T) {
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	p, err := acrnArchBase.acrnPath()
	assert.NoError(err)
	assert.Equal(p, acrnArchBaseAcrnPath)

	ctlp, err := acrnArchBase.acrnctlPath()
	assert.NoError(err)
	assert.Equal(ctlp, acrnArchBaseAcrnCtlPath)
}

func TestAcrnArchBaseKernelParameters(t *testing.T) {
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	// with debug params
	expectedParams := acrnArchBaseKernelParams
	debugParams := acrnArchBaseKernelParamsDebug
	expectedParams = append(expectedParams, debugParams...)
	p := acrnArchBase.kernelParameters(true)
	assert.Equal(expectedParams, p)

	// with non-debug params
	expectedParams = acrnArchBaseKernelParams
	nonDebugParams := acrnArchBaseKernelParamsNonDebug
	expectedParams = append(expectedParams, nonDebugParams...)
	p = acrnArchBase.kernelParameters(false)
	assert.Equal(expectedParams, p)
}

func TestAcrnArchBaseCapabilities(t *testing.T) {
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	c := acrnArchBase.capabilities()
	assert.True(c.IsBlockDeviceSupported())
	assert.True(c.IsBlockDeviceHotplugSupported())
	assert.False(c.IsFsSharingSupported())
}

func TestAcrnArchBaseMemoryTopology(t *testing.T) {
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	mem := uint64(8192)

	expectedMemory := Memory{
		Size: fmt.Sprintf("%dM", mem),
	}

	m := acrnArchBase.memoryTopology(mem)
	assert.Equal(expectedMemory, m)
}

func TestAcrnArchBaseAppendConsoles(t *testing.T) {
	var devices []Device
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	path := filepath.Join(filepath.Join(fs.MockRunStoragePath(), "test"), consoleSocket)

	expectedOut := []Device{
		ConsoleDevice{
			Name:     "console0",
			Backend:  Socket,
			PortType: ConsoleBE,
			Path:     path,
		},
	}

	devices = acrnArchBase.appendConsole(devices, path)
	assert.Equal(expectedOut, devices)
}

func TestAcrnArchBaseAppendImage(t *testing.T) {
	var devices []Device
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	image, err := ioutil.TempFile("", "img")
	assert.NoError(err)
	defer os.Remove(image.Name())
	err = image.Close()
	assert.NoError(err)

	devices, err = acrnArchBase.appendImage(devices, image.Name())
	assert.NoError(err)
	assert.Len(devices, 1)

	expectedOut := []Device{
		BlockDevice{
			FilePath: image.Name(),
			Index:    0,
		},
	}

	assert.Equal(expectedOut, devices)
}

func TestAcrnArchBaseAppendBridges(t *testing.T) {
	function := 0
	emul := acrnHostBridge
	config := ""

	var devices []Device
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	devices = acrnArchBase.appendBridges(devices)
	assert.Len(devices, 1)

	expectedOut := []Device{
		BridgeDevice{
			Function: function,
			Emul:     emul,
			Config:   config,
		},
	}

	assert.Equal(expectedOut, devices)
}

func TestAcrnArchBaseAppendLpcDevice(t *testing.T) {
	function := 0
	emul := acrnLPCDev

	var devices []Device
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	devices = acrnArchBase.appendLPC(devices)
	assert.Len(devices, 1)

	expectedOut := []Device{
		LPCDevice{
			Function: function,
			Emul:     emul,
		},
	}

	assert.Equal(expectedOut, devices)
}

func testAcrnArchBaseAppend(t *testing.T, structure interface{}, expected []Device) {
	var devices []Device
	var err error
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	switch s := structure.(type) {
	case types.Socket:
		devices = acrnArchBase.appendSocket(devices, s)
	case config.BlockDrive:
		devices = acrnArchBase.appendBlockDevice(devices, s)
	}

	assert.NoError(err)
	assert.Equal(devices, expected)
}

func TestAcrnArchBaseAppendSocket(t *testing.T) {
	name := "archserial.test"
	hostPath := "/tmp/archserial.sock"

	expectedOut := []Device{
		ConsoleDevice{
			Name:     name,
			Backend:  Socket,
			PortType: SerialBE,
			Path:     hostPath,
		},
	}

	socket := types.Socket{
		HostPath: hostPath,
		Name:     name,
	}

	testAcrnArchBaseAppend(t, socket, expectedOut)
}

func TestAcrnArchBaseAppendBlockDevice(t *testing.T) {
	path := "/tmp/archtest.img"
	index := 5

	expectedOut := []Device{
		BlockDevice{
			FilePath: path,
			Index:    index,
		},
	}

	drive := config.BlockDrive{
		File:  path,
		Index: index,
	}

	testAcrnArchBaseAppend(t, drive, expectedOut)
}

func TestAcrnArchBaseAppendNetwork(t *testing.T) {
	var devices []Device
	assert := assert.New(t)
	acrnArchBase := newAcrnArchBase()

	macAddr := net.HardwareAddr{0x02, 0x00, 0xCA, 0xFE, 0x00, 0x04}

	vethEp := &VethEndpoint{
		NetPair: NetworkInterfacePair{
			TapInterface: TapInterface{
				ID:   "uniqueTestID0",
				Name: "br0_kata",
				TAPIface: NetworkInterface{
					Name: "tap0_kata",
				},
			},
			VirtIface: NetworkInterface{
				Name:     "eth0",
				HardAddr: macAddr.String(),
			},
			NetInterworkingModel: DefaultNetInterworkingModel,
		},
		EndpointType: VethEndpointType,
	}

	macvtapEp := &MacvtapEndpoint{
		EndpointType: MacvtapEndpointType,
		EndpointProperties: NetworkInfo{
			Iface: NetlinkIface{
				Type: "macvtap",
			},
		},
	}

	expectedOut := []Device{
		NetDevice{
			Type:       TAP,
			IFName:     vethEp.NetPair.TAPIface.Name,
			MACAddress: vethEp.NetPair.TAPIface.HardAddr,
		},
		NetDevice{
			Type:       MACVTAP,
			IFName:     macvtapEp.Name(),
			MACAddress: macvtapEp.HardwareAddr(),
		},
	}

	devices = acrnArchBase.appendNetwork(devices, vethEp)
	devices = acrnArchBase.appendNetwork(devices, macvtapEp)
	assert.Equal(expectedOut, devices)
}
