// Copyright contributors to the Virtual Machine Manager for Go project
//
// SPDX-License-Identifier: Apache-2.0
//

package qemu

import (
	"fmt"
	"os"
	"reflect"
	"strings"
	"testing"
)

const agentUUID = "4cb19522-1e18-439a-883a-f9b2a3a95f5e"
const volumeUUID = "67d86208-b46c-4465-9018-e14187d4010"

const DevNo = "fe.1.1234"

func testAppend(structure interface{}, expected string, t *testing.T) {
	var config Config
	testConfigAppend(&config, structure, expected, t)
}

func testConfigAppend(config *Config, structure interface{}, expected string, t *testing.T) {
	switch s := structure.(type) {
	case Machine:
		config.Machine = s
		config.appendMachine()
	case FwCfg:
		config.FwCfg = []FwCfg{s}
		config.appendFwCfg(nil)

	case Device:
		config.Devices = []Device{s}
		config.appendDevices(nil)

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
			t.Fatalf("Unexpected error: %v", err)
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

	machineString = "-machine microvm,accel=kvm,pic=off,pit=off"
	machine = Machine{
		Type:         "microvm",
		Acceleration: "kvm",
		Options:      "pic=off,pit=off",
	}
	testAppend(machine, machineString, t)
}

func TestAppendEmptyMachine(t *testing.T) {
	machine := Machine{}

	testAppend(machine, "", t)
}

var deviceNVDIMMString = "-device nvdimm,id=nv0,memdev=mem0,unarmed=on -object memory-backend-file,id=mem0,mem-path=/root,size=65536,readonly=on"

func TestAppendDeviceNVDIMM(t *testing.T) {
	object := Object{
		Driver:   NVDIMM,
		Type:     MemoryBackendFile,
		DeviceID: "nv0",
		ID:       "mem0",
		MemPath:  "/root",
		Size:     1 << 16,
		ReadOnly: true,
	}

	testAppend(object, deviceNVDIMMString, t)
}

var objectEPCString = "-object memory-backend-epc,id=epc0,size=65536,prealloc=on"

func TestAppendEPCObject(t *testing.T) {
	object := Object{
		Type:     MemoryBackendEPC,
		ID:       "epc0",
		Size:     1 << 16,
		Prealloc: true,
	}

	testAppend(object, objectEPCString, t)
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
		Multidev:      Remap,
	}

	if fsdev.Transport.isVirtioCCW(nil) {
		fsdev.DevNo = DevNo
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
		ROMFile:       romfile,
	}

	if netdev.Transport.isVirtioPCI(nil) {
		netdev.Bus = "/pci-bus/pcie.0"
		netdev.Addr = "255"
	} else if netdev.Transport.isVirtioCCW(nil) {
		netdev.DevNo = DevNo
	}

	testAppend(netdev, deviceNetworkString, t)
}

func TestAppendDeviceNetworkMq(t *testing.T) {
	foo, _ := os.CreateTemp(os.TempDir(), "govmm-qemu-test")
	bar, _ := os.CreateTemp(os.TempDir(), "govmm-qemu-test")

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
		ROMFile:       romfile,
	}

	if netdev.Transport.isVirtioPCI(nil) {
		netdev.Bus = "/pci-bus/pcie.0"
		netdev.Addr = "255"
	} else if netdev.Transport.isVirtioCCW(nil) {
		netdev.DevNo = DevNo
	}

	testAppend(netdev, deviceNetworkStringMq, t)
}

var deviceLegacySerialString = "-serial chardev:tlserial0"

func TestAppendLegacySerial(t *testing.T) {
	sdev := LegacySerialDevice{
		Chardev: "tlserial0",
	}

	testAppend(sdev, deviceLegacySerialString, t)
}

var deviceLegacySerialPortString = "-chardev file,id=char0,path=/tmp/serial.log"

func TestAppendDeviceLegacySerialPort(t *testing.T) {
	chardev := CharDevice{
		Driver:  LegacySerial,
		Backend: File,
		ID:      "char0",
		Path:    "/tmp/serial.log",
	}
	testAppend(chardev, deviceLegacySerialPortString, t)
}

func TestAppendDeviceSerial(t *testing.T) {
	sdev := SerialDevice{
		Driver:        VirtioSerial,
		ID:            "serial0",
		DisableModern: true,
		ROMFile:       romfile,
		MaxPorts:      2,
	}
	if sdev.Transport.isVirtioCCW(nil) {
		sdev.DevNo = DevNo
	}

	testAppend(sdev, deviceSerialString, t)
}

var deviceSerialPortString = "-device virtserialport,chardev=char0,id=channel0,name=channel.0 -chardev socket,id=char0,path=/tmp/char.sock,server=on,wait=off"

func TestAppendDeviceSerialPort(t *testing.T) {
	chardev := CharDevice{
		Driver:   VirtioSerialPort,
		Backend:  Socket,
		ID:       "char0",
		DeviceID: "channel0",
		Path:     "/tmp/char.sock",
		Name:     "channel.0",
	}
	if chardev.Transport.isVirtioCCW(nil) {
		chardev.DevNo = DevNo
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
		WCE:           false,
		DisableModern: true,
		ROMFile:       romfile,
		ShareRW:       true,
		ReadOnly:      true,
	}
	if blkdev.Transport.isVirtioCCW(nil) {
		blkdev.DevNo = DevNo
	}
	testAppend(blkdev, deviceBlockString, t)
}

func TestAppendDeviceVFIO(t *testing.T) {
	vfioDevice := VFIODevice{
		BDF:      "02:10.0",
		ROMFile:  romfile,
		VendorID: "0x1234",
		DeviceID: "0x5678",
	}

	if vfioDevice.Transport.isVirtioCCW(nil) {
		vfioDevice.DevNo = DevNo
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

	if vsockDevice.Transport.isVirtioCCW(nil) {
		vsockDevice.DevNo = DevNo
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

	vsockDevice.ContextID = MaxGuestCID + 1

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
	var deviceString = "-device " + string(VirtioRng)

	rngDevice := RngDevice{
		ID:      "rng0",
		ROMFile: romfile,
	}

	deviceString += "-" + rngDevice.Transport.getName(nil) + ",rng=rng0"
	if romfile != "" {
		deviceString = deviceString + ",romfile=efi-virtio.rom"
	}

	if rngDevice.Transport.isVirtioCCW(nil) {
		rngDevice.DevNo = DevNo
		deviceString += ",devno=" + rngDevice.DevNo
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

	if scsiCon.Transport.isVirtioCCW(nil) {
		scsiCon.DevNo = DevNo
	}

	testAppend(scsiCon, deviceSCSIControllerStr, t)

	scsiCon.Bus = "pci.0"
	scsiCon.Addr = "00:04.0"
	scsiCon.DisableModern = true
	scsiCon.IOThread = "iothread1"
	testAppend(scsiCon, deviceSCSIControllerBusAddrStr, t)
}

func TestAppendEmptyDevice(t *testing.T) {
	device := SerialDevice{}

	testAppend(device, "", t)
}

func TestAppendKnobsAllTrue(t *testing.T) {
	var knobsString = "-no-user-config -nodefaults -nographic --no-reboot -overcommit mem-lock=on -S"
	knobs := Knobs{
		NoUserConfig:  true,
		NoDefaults:    true,
		NoGraphic:     true,
		NoReboot:      true,
		MemPrealloc:   true,
		FileBackedMem: true,
		MemShared:     true,
		Mlock:         true,
		Stopped:       true,
	}

	testAppend(knobs, knobsString, t)
}

func TestAppendKnobsAllFalse(t *testing.T) {
	var knobsString = ""
	knobs := Knobs{
		NoUserConfig:  false,
		NoDefaults:    false,
		NoGraphic:     false,
		NoReboot:      false,
		MemPrealloc:   false,
		FileBackedMem: false,
		MemShared:     false,
		Mlock:         false,
		Stopped:       false,
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
		HugePages:     true,
		MemPrealloc:   true,
		FileBackedMem: true,
		MemShared:     true,
	}
	objMemString := "-object memory-backend-file,id=dimm1,size=1G,mem-path=/dev/hugepages,share=on,prealloc=on"
	numaMemString := "-numa node,memdev=dimm1"
	memBackendString := "-machine memory-backend=dimm1"

	knobsString := objMemString + " "
	if isDimmSupported(nil) {
		knobsString += numaMemString
	} else {
		knobsString += memBackendString
	}

	testConfigAppend(conf, knobs, memString+" "+knobsString, t)
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
		MemPrealloc: true,
		MemShared:   true,
	}
	objMemString := "-object memory-backend-ram,id=dimm1,size=1G,share=on,prealloc=on"
	numaMemString := "-numa node,memdev=dimm1"
	memBackendString := "-machine memory-backend=dimm1"

	knobsString := objMemString + " "
	if isDimmSupported(nil) {
		knobsString += numaMemString
	} else {
		knobsString += memBackendString
	}

	testConfigAppend(conf, knobs, memString+" "+knobsString, t)
}

func TestAppendMemoryMemShared(t *testing.T) {
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
		FileBackedMem: true,
		MemShared:     true,
	}
	objMemString := "-object memory-backend-file,id=dimm1,size=1G,mem-path=foobar,share=on"
	numaMemString := "-numa node,memdev=dimm1"
	memBackendString := "-machine memory-backend=dimm1"

	knobsString := objMemString + " "
	if isDimmSupported(nil) {
		knobsString += numaMemString
	} else {
		knobsString += memBackendString
	}

	testConfigAppend(conf, knobs, memString+" "+knobsString, t)
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
		FileBackedMem: true,
		MemShared:     false,
	}
	objMemString := "-object memory-backend-file,id=dimm1,size=1G,mem-path=foobar"
	numaMemString := "-numa node,memdev=dimm1"
	memBackendString := "-machine memory-backend=dimm1"

	knobsString := objMemString + " "
	if isDimmSupported(nil) {
		knobsString += numaMemString
	} else {
		knobsString += memBackendString
	}

	testConfigAppend(conf, knobs, memString+" "+knobsString, t)
}

func TestAppendMemoryFileBackedMemPrealloc(t *testing.T) {
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
		FileBackedMem: true,
		MemShared:     true,
		MemPrealloc:   true,
	}
	objMemString := "-object memory-backend-file,id=dimm1,size=1G,mem-path=foobar,share=on,prealloc=on"
	numaMemString := "-numa node,memdev=dimm1"
	memBackendString := "-machine memory-backend=dimm1"

	knobsString := objMemString + " "
	if isDimmSupported(nil) {
		knobsString += numaMemString
	} else {
		knobsString += memBackendString
	}

	testConfigAppend(conf, knobs, memString+" "+knobsString, t)
}

func TestNoRebootKnob(t *testing.T) {
	conf := &Config{}

	knobs := Knobs{
		NoReboot: true,
	}
	knobsString := "--no-reboot"

	testConfigAppend(conf, knobs, knobsString, t)
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

var qmpSingleSocketServerString = "-qmp unix:path=cc-qmp,server=on,wait=off"
var qmpSingleSocketString = "-qmp unix:path=cc-qmp"

func TestAppendSingleQMPSocketServer(t *testing.T) {
	qmp := QMPSocket{
		Type:     "unix",
		Name:     "cc-qmp",
		Server:   true,
		NoWait:   true,
		Protocol: Qmp,
	}

	testAppend(qmp, qmpSingleSocketServerString, t)
}

func TestAppendSingleQMPSocket(t *testing.T) {
	qmp := QMPSocket{
		Type:     Unix,
		Name:     "cc-qmp",
		Server:   false,
		Protocol: Qmp,
	}

	testAppend(qmp, qmpSingleSocketString, t)
}

var qmpSocketServerFdString = "-qmp unix:fd=3,server=on,wait=off"

func TestAppendQMPSocketServerFd(t *testing.T) {
	foo, _ := os.CreateTemp(os.TempDir(), "govmm-qemu-test")

	defer func() {
		_ = foo.Close()
		_ = os.Remove(foo.Name())
	}()

	qmp := QMPSocket{
		Type:     "unix",
		FD:       foo,
		Server:   true,
		NoWait:   true,
		Protocol: Qmp,
	}

	testAppend(qmp, qmpSocketServerFdString, t)
}

var qmpSocketServerString = "-qmp unix:path=cc-qmp-1,server=on,wait=off -qmp unix:path=cc-qmp-2,server=on,wait=off"

func TestAppendQMPSocketServer(t *testing.T) {
	qmp := []QMPSocket{
		{
			Type:     "unix",
			Name:     "cc-qmp-1",
			Server:   true,
			NoWait:   true,
			Protocol: Qmp,
		},
		{
			Type:     "unix",
			Name:     "cc-qmp-2",
			Server:   true,
			NoWait:   true,
			Protocol: Qmp,
		},
	}

	testAppend(qmp, qmpSocketServerString, t)
}

var pidfile = "/run/vc/vm/iamsandboxid/pidfile"
var qemuString = "-name cc-qemu -cpu host -uuid " + agentUUID + " -pidfile " + pidfile

func TestAppendStrings(t *testing.T) {
	config := Config{
		Path:     "qemu",
		Name:     "cc-qemu",
		UUID:     agentUUID,
		CPUModel: "host",
		PidFile:  pidfile,
	}

	config.appendName()
	config.appendCPUModel()
	config.appendUUID()
	config.appendPidFile()

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

var incomingStringDefer = "-S -incoming defer"

func TestAppendIncomingDefer(t *testing.T) {
	source := Incoming{
		MigrationType: MigrationDefer,
	}

	testAppend(source, incomingStringDefer, t)
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
	c.appendDevices(nil)
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

	c.appendDevices(nil)
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

func TestBadPFlash(t *testing.T) {
	c := &Config{}
	c.appendPFlashParam()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestValidPFlash(t *testing.T) {
	c := &Config{}
	c.PFlash = []string{"flash0", "flash1"}
	c.appendPFlashParam()
	expected := []string{"-pflash", "flash0", "-pflash", "flash1"}
	ok := reflect.DeepEqual(expected, c.qemuParams)
	if !ok {
		t.Errorf("Expected %v, found %v", expected, c.qemuParams)
	}
}

func TestBadSeccompSandbox(t *testing.T) {
	c := &Config{}
	c.appendSeccompSandbox()
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

func TestValidSeccompSandbox(t *testing.T) {
	c := &Config{}
	c.SeccompSandbox = string("on,obsolete=deny")
	c.appendSeccompSandbox()
	expected := []string{"-sandbox", "on,obsolete=deny"}
	ok := reflect.DeepEqual(expected, c.qemuParams)
	if !ok {
		t.Errorf("Expected %v, found %v", expected, c.qemuParams)
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
			MemShared: true,
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

func TestBadFwcfg(t *testing.T) {
	c := &Config{}
	c.appendFwCfg(nil)
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}

	c = &Config{
		FwCfg: []FwCfg{
			{
				Name: "name=opt/com.mycompany/blob",
				File: "./my_blob.bin",
				Str:  "foo",
			},
		},
	}
	c.appendFwCfg(nil)
	if len(c.qemuParams) != 0 {
		t.Errorf("Expected empty qemuParams, found %s", c.qemuParams)
	}
}

var (
	vIommuString        = "-device intel-iommu,intremap=on,device-iotlb=on,caching-mode=on"
	vIommuNoCacheString = "-device intel-iommu,intremap=on,device-iotlb=on,caching-mode=off"
)

func TestIommu(t *testing.T) {
	iommu := IommuDev{
		Intremap:    true,
		DeviceIotlb: true,
		CachingMode: true,
	}

	if !iommu.Valid() {
		t.Fatalf("iommu should be valid")
	}

	testAppend(iommu, vIommuString, t)

	iommu.CachingMode = false

	testAppend(iommu, vIommuNoCacheString, t)

}

func TestAppendFwcfg(t *testing.T) {
	fwcfgString := "-fw_cfg name=opt/com.mycompany/blob,file=./my_blob.bin"
	fwcfg := FwCfg{
		Name: "opt/com.mycompany/blob",
		File: "./my_blob.bin",
	}
	testAppend(fwcfg, fwcfgString, t)

	fwcfgString = "-fw_cfg name=opt/com.mycompany/blob,string=foo"
	fwcfg = FwCfg{
		Name: "opt/com.mycompany/blob",
		Str:  "foo",
	}
	testAppend(fwcfg, fwcfgString, t)
}

func TestAppendPVPanicDevice(t *testing.T) {
	testCases := []struct {
		dev Device
		out string
	}{
		{nil, ""},
		{PVPanicDevice{}, "-device pvpanic"},
		{PVPanicDevice{NoShutdown: true}, "-device pvpanic -no-shutdown"},
	}

	for _, tc := range testCases {
		testAppend(tc.dev, tc.out, t)
	}
}

func TestLoaderDevice(t *testing.T) {
	testCases := []struct {
		dev Device
		out string
	}{
		{nil, ""},
		{LoaderDevice{}, ""},
		{LoaderDevice{File: "f"}, ""},
		{LoaderDevice{ID: "id"}, ""},
		{LoaderDevice{File: "f", ID: "id"}, "-device loader,file=f,id=id"},
	}

	for _, tc := range testCases {
		testAppend(tc.dev, tc.out, t)
	}
}
