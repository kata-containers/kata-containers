/*
// Copyright (c) 2016 Intel Corporation
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
*/

package qemu

import (
	"fmt"
	"io/ioutil"
	"os"
	"strings"
	"testing"
)

const agentUUID = "4cb19522-1e18-439a-883a-f9b2a3a95f5e"
const volumeUUID = "67d86208-b46c-4465-9018-e14187d4010"

func testAppend(structure interface{}, expected string, t *testing.T) {
	var config Config
	testConfigAppend(&config, structure, expected, t)

	return
}

func testConfigAppend(config *Config, structure interface{}, expected string, t *testing.T) {
	switch s := structure.(type) {
	case Machine:
		config.Machine = s
		config.appendMachine()

	case Device:
		config.Devices = []Device{s}
		config.appendDevices()

	case Knobs:
		config.Knobs = s
		config.appendKnobs()

	case Kernel:
		config.Kernel = s
		config.appendKernel()

	case Memory:
		config.Memory = s
		config.appendMemory()

	case SMP:
		config.SMP = s
		if err := config.appendCPUs(); err != nil {
			t.Fatalf("Unexpected error: %v\n", err)
		}

	case QMPSocket:
		config.QMPSockets = []QMPSocket{s}
		config.appendQMPSockets()

	case []QMPSocket:
		config.QMPSockets = s
		config.appendQMPSockets()

	case RTC:
		config.RTC = s
		config.appendRTC()

	case IOThread:
		config.IOThreads = []IOThread{s}
		config.appendIOThreads()
	case Incoming:
		config.Incoming = s
		config.appendIncoming()
	}

	result := strings.Join(config.qemuParams, " ")
	if result != expected {
		t.Fatalf("Failed to append parameters [%s] != [%s]", result, expected)
	}
}

func TestAppendMachine(t *testing.T) {
	machineString := "-machine pc-lite,accel=kvm,kernel_irqchip,nvdimm"
	machine := Machine{
		Type:         "pc-lite",
		Acceleration: "kvm,kernel_irqchip,nvdimm",
	}
	testAppend(machine, machineString, t)

	machineString = "-machine pc-lite,accel=kvm,kernel_irqchip,nvdimm,gic-version=host,usb=off"
	machine = Machine{
		Type:         "pc-lite",
		Acceleration: "kvm,kernel_irqchip,nvdimm",
		Options:      "gic-version=host,usb=off",
	}
	testAppend(machine, machineString, t)
}

func TestAppendEmptyMachine(t *testing.T) {
	machine := Machine{}

	testAppend(machine, "", t)
}

var deviceNVDIMMString = "-device nvdimm,id=nv0,memdev=mem0 -object memory-backend-file,id=mem0,mem-path=/root,size=65536"

func TestAppendDeviceNVDIMM(t *testing.T) {
	object := Object{
		Driver:   NVDIMM,
		Type:     MemoryBackendFile,
		DeviceID: "nv0",
		ID:       "mem0",
		MemPath:  "/root",
		Size:     1 << 16,
	}

	testAppend(object, deviceNVDIMMString, t)
}

func TestAppendDeviceFS(t *testing.T) {
	fsdev := FSDevice{
		Driver:        Virtio9P,
		FSDriver:      Local,
		ID:            "workload9p",
		Path:          "/var/lib/docker/devicemapper/mnt/e31ebda2",
		MountTag:      "rootfs",
		SecurityModel: None,
		DisableModern: true,
		ROMFile:       "efi-virtio.rom",
	}

	testAppend(fsdev, deviceFSString, t)
}

func TestAppendDeviceNetwork(t *testing.T) {
	netdev := NetDevice{
		Driver:        VirtioNet,
		Type:          TAP,
		ID:            "tap0",
		IFName:        "ceth0",
		Script:        "no",
		DownScript:    "no",
		VHost:         true,
		MACAddress:    "01:02:de:ad:be:ef",
		DisableModern: true,
		ROMFile:       "efi-virtio.rom",
	}

	testAppend(netdev, deviceNetworkString, t)
}

func TestAppendDeviceNetworkMq(t *testing.T) {
	foo, _ := ioutil.TempFile(os.TempDir(), "govmm-qemu-test")
	bar, _ := ioutil.TempFile(os.TempDir(), "govmm-qemu-test")

	defer func() {
		_ = foo.Close()
		_ = bar.Close()
		_ = os.Remove(foo.Name())
		_ = os.Remove(bar.Name())
	}()

	netdev := NetDevice{
		Driver:        VirtioNet,
		Type:          TAP,
		ID:            "tap0",
		IFName:        "ceth0",
		Script:        "no",
		DownScript:    "no",
		FDs:           []*os.File{foo, bar},
		VHost:         true,
		MACAddress:    "01:02:de:ad:be:ef",
		DisableModern: true,
		ROMFile:       "efi-virtio.rom",
	}

	testAppend(netdev, deviceNetworkStringMq, t)
}

func TestAppendDeviceNetworkPCI(t *testing.T) {

	netdev := NetDevice{
		Driver:        VirtioNet,
		Type:          TAP,
		ID:            "tap0",
		IFName:        "ceth0",
		Bus:           "/pci-bus/pcie.0",
		Addr:          "255",
		Script:        "no",
		DownScript:    "no",
		VHost:         true,
		MACAddress:    "01:02:de:ad:be:ef",
		DisableModern: true,
		ROMFile:       romfile,
	}

	testAppend(netdev, deviceNetworkPCIString, t)
}

func TestAppendDeviceNetworkPCIMq(t *testing.T) {
	foo, _ := ioutil.TempFile(os.TempDir(), "govmm-qemu-test")
	bar, _ := ioutil.TempFile(os.TempDir(), "govmm-qemu-test")

	defer func() {
		_ = foo.Close()
		_ = bar.Close()
		_ = os.Remove(foo.Name())
		_ = os.Remove(bar.Name())
	}()

	netdev := NetDevice{
		Driver:        VirtioNet,
		Type:          TAP,
		ID:            "tap0",
		IFName:        "ceth0",
		Bus:           "/pci-bus/pcie.0",
		Addr:          "255",
		Script:        "no",
		DownScript:    "no",
		FDs:           []*os.File{foo, bar},
		VHost:         true,
		MACAddress:    "01:02:de:ad:be:ef",
		DisableModern: true,
		ROMFile:       romfile,
	}

	testAppend(netdev, deviceNetworkPCIStringMq, t)
}

func TestAppendDeviceSerial(t *testing.T) {
	sdev := SerialDevice{
		Driver:        VirtioSerial,
		ID:            "serial0",
		DisableModern: true,
		ROMFile:       romfile,
	}

	testAppend(sdev, deviceSerialString, t)
}

var deviceSerialPortString = "-device virtserialport,chardev=char0,id=channel0,name=channel.0 -chardev socket,id=char0,path=/tmp/char.sock,server,nowait"

func TestAppendDeviceSerialPort(t *testing.T) {
	chardev := CharDevice{
		Driver:   VirtioSerialPort,
		Backend:  Socket,
		ID:       "char0",
		DeviceID: "channel0",
		Path:     "/tmp/char.sock",
		Name:     "channel.0",
	}

	testAppend(chardev, deviceSerialPortString, t)
}

func TestAppendDeviceBlock(t *testing.T) {
	blkdev := BlockDevice{
		Driver:        VirtioBlock,
		ID:            "hd0",
		File:          "/var/lib/vm.img",
		AIO:           Threads,
		Format:        QCOW2,
		Interface:     NoInterface,
		SCSI:          false,
		WCE:           false,
		DisableModern: true,
		ROMFile:       romfile,
	}

	testAppend(blkdev, deviceBlockString, t)
}

func TestAppendDeviceVFIO(t *testing.T) {
	vfioDevice := VFIODevice{
		BDF:     "02:10.0",
		ROMFile: romfile,
	}

	testAppend(vfioDevice, deviceVFIOString, t)
}

func TestAppendVSOCK(t *testing.T) {
	vsockDevice := VSOCKDevice{
		ID:            "vhost-vsock-pci0",
		ContextID:     4,
		VHostFD:       nil,
		DisableModern: true,
		ROMFile:       romfile,
	}

	testAppend(vsockDevice, deviceVSOCKString, t)
}

func TestVSOCKValid(t *testing.T) {
	vsockDevice := VSOCKDevice{
		ID:            "vhost-vsock-pci0",
		ContextID:     MinimalGuestCID - 1,
		VHostFD:       nil,
		DisableModern: true,
	}

	if vsockDevice.Valid() {
		t.Fatalf("VSOCK Context ID is not valid")
	}

	vsockDevice.ID = ""

	if vsockDevice.Valid() {
		t.Fatalf("VSOCK ID is not valid")
	}
}

func TestAppendVirtioRng(t *testing.T) {
	var objectString = "-object rng-random,id=rng0"
	var deviceString = "-device " + string(VirtioRng) + ",rng=rng0"
	if romfile != "" {
		deviceString = deviceString + ",romfile=efi-virtio.rom"
	}
	rngDevice := RngDevice{
		ID:      "rng0",
		ROMFile: romfile,
	}

	testAppend(rngDevice, objectString+" "+deviceString, t)

	rngDevice.Filename = "/dev/urandom"
	objectString += ",filename=" + rngDevice.Filename

	testAppend(rngDevice, objectString+" "+deviceString, t)

	rngDevice.MaxBytes = 20

	deviceString += fmt.Sprintf(",max-bytes=%d", rngDevice.MaxBytes)
	testAppend(rngDevice, objectString+" "+deviceString, t)

	rngDevice.Period = 500

	deviceString += fmt.Sprintf(",period=%d", rngDevice.Period)
	testAppend(rngDevice, objectString+" "+deviceString, t)

}

func TestVirtioRngValid(t *testing.T) {
	rng := RngDevice{
		ID: "",
	}

	if rng.Valid() {
		t.Fatalf("rng should be not valid when ID is empty")
	}

	rng.ID = "rng0"
	if !rng.Valid() {
		t.Fatalf("rng should be valid")
	}

}

func TestVirtioBalloonValid(t *testing.T) {
	balloon := BalloonDevice{
		ID: "",
	}

	if balloon.Valid() {
		t.Fatalf("balloon should be not valid when ID is empty")
	}

	balloon.ID = "balloon0"
	if !balloon.Valid() {
		t.Fatalf("balloon should be valid")
	}
}

func TestAppendDeviceSCSIController(t *testing.T) {
	scsiCon := SCSIController{
		ID:      "foo",
		ROMFile: romfile,
	}
	testAppend(scsiCon, deviceSCSIControllerStr, t)

	scsiCon.Bus = "pci.0"
	scsiCon.Addr = "00:04.0"
	scsiCon.DisableModern = true
	scsiCon.IOThread = "iothread1"
	testAppend(scsiCon, deviceSCSIControllerBusAddrStr, t)
}

func TestAppendPCIBridgeDevice(t *testing.T) {

	bridge := BridgeDevice{
		Type:    PCIBridge,
		ID:      "mybridge",
		Bus:     "/pci-bus/pcie.0",
		Addr:    "255",
		Chassis: 5,
		SHPC:    true,
		ROMFile: romfile,
	}

	testAppend(bridge, devicePCIBridgeString, t)
}

func TestAppendPCIEBridgeDevice(t *testing.T) {

	bridge := BridgeDevice{
		Type:    PCIEBridge,
		ID:      "mybridge",
		Bus:     "/pci-bus/pcie.0",
		Addr:    "255",
		ROMFile: "efi-virtio.rom",
	}

	testAppend(bridge, devicePCIEBridgeString, t)
}

func TestAppendEmptyDevice(t *testing.T) {
	device := SerialDevice{}

	testAppend(device, "", t)
}

func TestAppendKnobsAllTrue(t *testing.T) {
	var knobsString = "-no-user-config -nodefaults -nographic -daemonize -realtime mlock=on -S"
	knobs := Knobs{
		NoUserConfig:        true,
		NoDefaults:          true,
		NoGraphic:           true,
		Daemonize:           true,
		MemPrealloc:         true,
		FileBackedMem:       true,
		FileBackedMemShared: true,
		Realtime:            true,
		Mlock:               true,
		Stopped:             true,
	}

	testAppend(knobs, knobsString, t)
}

func TestAppendKnobsAllFalse(t *testing.T) {
	var knobsString = "-realtime mlock=off"
	knobs := Knobs{
		NoUserConfig:        false,
		NoDefaults:          false,
		NoGraphic:           false,
		MemPrealloc:         false,
		FileBackedMem:       false,
		FileBackedMemShared: false,
		Realtime:            false,
		Mlock:               false,
		Stopped:             false,
	}

	testAppend(knobs, knobsString, t)
}

func TestAppendMemoryHugePages(t *testing.T) {
	conf := &Config{
		Memory: Memory{
			Size:   "1G",
			Slots:  8,
			MaxMem: "3G",
			Path:   "foobar",
		},
	}
	memString := "-m 1G,slots=8,maxmem=3G"
	testConfigAppend(conf, conf.Memory, memString, t)

	knobs := Knobs{
		HugePages:           true,
		MemPrealloc:         true,
		FileBackedMem:       true,
		FileBackedMemShared: true,
	}
	knobsString := "-object memory-backend-file,id=dimm1,size=1G,mem-path=/dev/hugepages,share=on,prealloc=on -numa node,memdev=dimm1"
	mlockFalseString := "-realtime mlock=off"

	testConfigAppend(conf, knobs, memString+" "+knobsString+" "+mlockFalseString, t)
}

func TestAppendMemoryMemPrealloc(t *testing.T) {
	conf := &Config{
		Memory: Memory{
			Size:   "1G",
			Slots:  8,
			MaxMem: "3G",
			Path:   "foobar",
		},
	}
	memString := "-m 1G,slots=8,maxmem=3G"
	testConfigAppend(conf, conf.Memory, memString, t)

	knobs := Knobs{
		MemPrealloc:         true,
		FileBackedMem:       true,
		FileBackedMemShared: true,
	}
	knobsString := "-object memory-backend-ram,id=dimm1,size=1G,prealloc=on -numa node,memdev=dimm1"
	mlockFalseString := "-realtime mlock=off"

	testConfigAppend(conf, knobs, memString+" "+knobsString+" "+mlockFalseString, t)
}

func TestAppendMemoryFileBackedMemShared(t *testing.T) {
	conf := &Config{
		Memory: Memory{
			Size:   "1G",
			Slots:  8,
			MaxMem: "3G",
			Path:   "foobar",
		},
	}
	memString := "-m 1G,slots=8,maxmem=3G"
	testConfigAppend(conf, conf.Memory, memString, t)

	knobs := Knobs{
		FileBackedMem:       true,
		FileBackedMemShared: true,
	}
	knobsString := "-object memory-backend-file,id=dimm1,size=1G,mem-path=foobar,share=on -numa node,memdev=dimm1"
	mlockFalseString := "-realtime mlock=off"

	testConfigAppend(conf, knobs, memString+" "+knobsString+" "+mlockFalseString, t)
}

func TestAppendMemoryFileBackedMem(t *testing.T) {
	conf := &Config{
		Memory: Memory{
			Size:   "1G",
			Slots:  8,
			MaxMem: "3G",
			Path:   "foobar",
		},
	}
	memString := "-m 1G,slots=8,maxmem=3G"
	testConfigAppend(conf, conf.Memory, memString, t)

	knobs := Knobs{
		FileBackedMem:       true,
		FileBackedMemShared: false,
	}
	knobsString := "-object memory-backend-file,id=dimm1,size=1G,mem-path=foobar -numa node,memdev=dimm1"
	mlockFalseString := "-realtime mlock=off"

	testConfigAppend(conf, knobs, memString+" "+knobsString+" "+mlockFalseString, t)
}

var kernelString = "-kernel /opt/vmlinux.container -initrd /opt/initrd.container -append root=/dev/pmem0p1 rootflags=dax,data=ordered,errors=remount-ro rw rootfstype=ext4 tsc=reliable"

func TestAppendKernel(t *testing.T) {
	kernel := Kernel{
		Path:       "/opt/vmlinux.container",
		InitrdPath: "/opt/initrd.container",
		Params:     "root=/dev/pmem0p1 rootflags=dax,data=ordered,errors=remount-ro rw rootfstype=ext4 tsc=reliable",
	}

	testAppend(kernel, kernelString, t)
}

var memoryString = "-m 2G,slots=2,maxmem=3G"

func TestAppendMemory(t *testing.T) {
	memory := Memory{
		Size:   "2G",
		Slots:  2,
		MaxMem: "3G",
		Path:   "",
	}

	testAppend(memory, memoryString, t)
}

var cpusString = "-smp 2,cores=1,threads=2,sockets=2,maxcpus=6"

func TestAppendCPUs(t *testing.T) {
	smp := SMP{
		CPUs:    2,
		Sockets: 2,
		Cores:   1,
		Threads: 2,
		MaxCPUs: 6,
	}

	testAppend(smp, cpusString, t)
}

func TestFailToAppendCPUs(t *testing.T) {
	config := Config{
		SMP: SMP{
			CPUs:    2,
			Sockets: 2,
			Cores:   1,
			Threads: 2,
			MaxCPUs: 1,
		},
	}

	if err := config.appendCPUs(); err == nil {
		t.Fatalf("Expected appendCPUs to fail")
	}
}

var qmpSingleSocketServerString = "-qmp unix:cc-qmp,server,nowait"
var qmpSingleSocketString = "-qmp unix:cc-qmp"

func TestAppendSingleQMPSocketServer(t *testing.T) {
	qmp := QMPSocket{
		Type:   "unix",
		Name:   "cc-qmp",
		Server: true,
		NoWait: true,
	}

	testAppend(qmp, qmpSingleSocketServerString, t)
}

func TestAppendSingleQMPSocket(t *testing.T) {
	qmp := QMPSocket{
		Type:   Unix,
		Name:   "cc-qmp",
		Server: false,
	}

	testAppend(qmp, qmpSingleSocketString, t)
}

var qmpSocketServerString = "-qmp unix:cc-qmp-1,server,nowait -qmp unix:cc-qmp-2,server,nowait"

func TestAppendQMPSocketServer(t *testing.T) {
	qmp := []QMPSocket{
		{
			Type:   "unix",
			Name:   "cc-qmp-1",
			Server: true,
			NoWait: true,
		},
		{
			Type:   "unix",
			Name:   "cc-qmp-2",
			Server: true,
			NoWait: true,
		},
	}

	testAppend(qmp, qmpSocketServerString, t)
}

var qemuString = "-name cc-qemu -cpu host -uuid " + agentUUID

func TestAppendStrings(t *testing.T) {
	config := Config{
		Path:     "qemu",
		Name:     "cc-qemu",
		UUID:     agentUUID,
		CPUModel: "host",
	}

	config.appendName()
	config.appendCPUModel()
	config.appendUUID()

	result := strings.Join(config.qemuParams, " ")
	if result != qemuString {
		t.Fatalf("Failed to append parameters [%s] != [%s]", result, qemuString)
	}
}

var rtcString = "-rtc base=utc,driftfix=slew,clock=host"

func TestAppendRTC(t *testing.T) {
	rtc := RTC{
		Base:     UTC,
		Clock:    Host,
		DriftFix: Slew,
	}

	testAppend(rtc, rtcString, t)
}

var ioThreadString = "-object iothread,id=iothread1"

func TestAppendIOThread(t *testing.T) {
	ioThread := IOThread{
		ID: "iothread1",
	}

	testAppend(ioThread, ioThreadString, t)
}

var incomingStringFD = "-S -incoming fd:3"

func TestAppendIncomingFD(t *testing.T) {
	source := Incoming{
		MigrationType: MigrationFD,
		FD:            os.Stdout,
	}

	testAppend(source, incomingStringFD, t)
}

var incomingStringExec = "-S -incoming exec:test migration cmd"

func TestAppendIncomingExec(t *testing.T) {
	source := Incoming{
		MigrationType: MigrationExec,
		Exec:          "test migration cmd",
	}

	testAppend(source, incomingStringExec, t)
}

func TestBadName(t *testing.T) {
	c := &Config{}
	c.appendName()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadMachine(t *testing.T) {
	c := &Config{}
	c.appendMachine()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadCPUModel(t *testing.T) {
	c := &Config{}
	c.appendCPUModel()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadQMPSockets(t *testing.T) {
	c := &Config{}
	c.appendQMPSockets()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		QMPSockets: []QMPSocket{{}},
	}

	c.appendQMPSockets()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		QMPSockets: []QMPSocket{{Name: "test"}},
	}

	c.appendQMPSockets()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		QMPSockets: []QMPSocket{
			{
				Name: "test",
				Type: QMPSocketType("ip"),
			},
		},
	}

	c.appendQMPSockets()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadDevices(t *testing.T) {
	c := &Config{}
	c.appendDevices()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		Devices: []Device{
			FSDevice{},
			FSDevice{
				ID:       "id0",
				MountTag: "tag",
			},
			CharDevice{},
			CharDevice{
				ID: "id1",
			},
			NetDevice{},
			NetDevice{
				ID:     "id1",
				IFName: "if",
				Type:   IPVTAP,
			},
			SerialDevice{},
			SerialDevice{
				ID: "id0",
			},
			BlockDevice{},
			BlockDevice{
				Driver: "drv",
				ID:     "id1",
			},
			VhostUserDevice{},
			VhostUserDevice{
				CharDevID: "devid",
			},
			VhostUserDevice{
				CharDevID:  "devid",
				SocketPath: "/var/run/sock",
			},
			VhostUserDevice{
				CharDevID:     "devid",
				SocketPath:    "/var/run/sock",
				VhostUserType: VhostUserNet,
			},
			VhostUserDevice{
				CharDevID:     "devid",
				SocketPath:    "/var/run/sock",
				VhostUserType: VhostUserSCSI,
			},
		},
	}

	c.appendDevices()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadRTC(t *testing.T) {
	c := &Config{}
	c.appendRTC()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		RTC: RTC{
			Clock: RTCClock("invalid"),
		},
	}
	c.appendRTC()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		RTC: RTC{
			Clock:    Host,
			DriftFix: RTCDriftFix("invalid"),
		},
	}
	c.appendRTC()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadGlobalParam(t *testing.T) {
	c := &Config{}
	c.appendGlobalParam()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadVGA(t *testing.T) {
	c := &Config{}
	c.appendVGA()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadKernel(t *testing.T) {
	c := &Config{}
	c.appendKernel()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadMemoryKnobs(t *testing.T) {
	c := &Config{}
	c.appendMemoryKnobs()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		Knobs: Knobs{
			HugePages: true,
		},
	}
	c.appendMemoryKnobs()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		Knobs: Knobs{
			HugePages: true,
		},
	}
	c.appendMemoryKnobs()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		Knobs: Knobs{
			MemPrealloc: true,
		},
	}
	c.appendMemoryKnobs()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		Knobs: Knobs{
			FileBackedMem: true,
		},
		Memory: Memory{
			Size: "1024",
		},
	}
	c.appendMemoryKnobs()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadKnobs(t *testing.T) {
	c := &Config{
		Knobs: Knobs{
			Mlock: true,
		},
	}
	c.appendKnobs()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadBios(t *testing.T) {
	c := &Config{}
	c.appendBios()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadIOThreads(t *testing.T) {
	c := &Config{}
	c.appendIOThreads()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		IOThreads: []IOThread{{ID: ""}},
	}
	c.appendIOThreads()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadIncoming(t *testing.T) {
	c := &Config{}
	c.appendIncoming()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestBadCPUs(t *testing.T) {
	c := &Config{}
	if err := c.appendCPUs(); err != nil {
		t.Fatalf("No error expected got %v", err)
	}
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		SMP: SMP{
			MaxCPUs: 1,
			CPUs:    2,
		},
	}
	if c.appendCPUs() == nil {
		t.Errorf("Error expected")
	}
}
