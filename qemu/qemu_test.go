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
	"io/ioutil"
	"os"
	"strings"
	"testing"
)

const agentUUID = "4cb19522-1e18-439a-883a-f9b2a3a95f5e"
const volumeUUID = "67d86208-b46c-4465-9018-e14187d4010"

func testAppend(structure interface{}, expected string, t *testing.T) {
	var config Config

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
	}

	result := strings.Join(config.qemuParams, " ")
	if result != expected {
		t.Fatalf("Failed to append parameters [%s] != [%s]", result, expected)
	}
}

var machineString = "-machine pc-lite,accel=kvm,kernel_irqchip,nvdimm"

func TestAppendMachine(t *testing.T) {
	machine := Machine{
		Type:         "pc-lite",
		Acceleration: "kvm,kernel_irqchip,nvdimm",
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

var deviceFSString = "-device virtio-9p-pci,disable-modern=true,fsdev=workload9p,mount_tag=rootfs -fsdev local,id=workload9p,path=/var/lib/docker/devicemapper/mnt/e31ebda2,security_model=none"

func TestAppendDeviceFS(t *testing.T) {
	fsdev := FSDevice{
		Driver:        Virtio9P,
		FSDriver:      Local,
		ID:            "workload9p",
		Path:          "/var/lib/docker/devicemapper/mnt/e31ebda2",
		MountTag:      "rootfs",
		SecurityModel: None,
		DisableModern: true,
	}

	testAppend(fsdev, deviceFSString, t)
}

var deviceNetworkString = "-netdev tap,id=tap0,vhost=on,ifname=ceth0,downscript=no,script=no -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,disable-modern=true"

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
	}

	testAppend(netdev, deviceNetworkString, t)
}

var deviceNetworkStringMq = "-netdev tap,id=tap0,vhost=on,fds=3:4 -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,disable-modern=true,mq=on,vectors=6"

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
	}

	testAppend(netdev, deviceNetworkStringMq, t)
}

var deviceNetworkPCIString = "-netdev tap,id=tap0,vhost=on,ifname=ceth0,downscript=no,script=no -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,bus=/pci-bus/pcie.0,addr=ff,disable-modern=true"

func TestAppendDeviceNetworkPCI(t *testing.T) {

	netdev := NetDevice{
		Driver:        VirtioNetPCI,
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
	}

	testAppend(netdev, deviceNetworkPCIString, t)
}

var deviceNetworkPCIStringMq = "-netdev tap,id=tap0,vhost=on,fds=3:4 -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,bus=/pci-bus/pcie.0,addr=ff,disable-modern=true,mq=on,vectors=6"

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
		Driver:        VirtioNetPCI,
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
	}

	testAppend(netdev, deviceNetworkPCIStringMq, t)
}

var deviceSerialString = "-device virtio-serial-pci,disable-modern=true,id=serial0"

func TestAppendDeviceSerial(t *testing.T) {
	sdev := SerialDevice{
		Driver:        VirtioSerial,
		ID:            "serial0",
		DisableModern: true,
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

var deviceBlockString = "-device virtio-blk,disable-modern=true,drive=hd0,scsi=off,config-wce=off -drive id=hd0,file=/var/lib/vm.img,aio=threads,format=qcow2,if=none"

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
	}

	testAppend(blkdev, deviceBlockString, t)
}

var deviceVhostUserNetString = "-chardev socket,id=char1,path=/tmp/nonexistentsocket.socket -netdev type=vhost-user,id=net1,chardev=char1,vhostforce -device virtio-net-pci,netdev=net1,mac=00:11:22:33:44:55"
var deviceVhostUserSCSIString = "-chardev socket,id=char1,path=/tmp/nonexistentsocket.socket -device vhost-user-scsi-pci,id=scsi1,chardev=char1"
var deviceVhostUserBlkString = "-chardev socket,id=char2,path=/tmp/nonexistentsocket.socket -device vhost-user-blk-pci,logical_block_size=4096,size=512M,chardev=char2"

func TestAppendDeviceVhostUser(t *testing.T) {

	vhostuserBlkDevice := VhostUserDevice{
		SocketPath:    "/tmp/nonexistentsocket.socket",
		CharDevID:     "char2",
		TypeDevID:     "",
		Address:       "",
		VhostUserType: VhostUserBlk,
	}
	testAppend(vhostuserBlkDevice, deviceVhostUserBlkString, t)

	vhostuserSCSIDevice := VhostUserDevice{
		SocketPath:    "/tmp/nonexistentsocket.socket",
		CharDevID:     "char1",
		TypeDevID:     "scsi1",
		Address:       "",
		VhostUserType: VhostUserSCSI,
	}
	testAppend(vhostuserSCSIDevice, deviceVhostUserSCSIString, t)

	vhostuserNetDevice := VhostUserDevice{
		SocketPath:    "/tmp/nonexistentsocket.socket",
		CharDevID:     "char1",
		TypeDevID:     "net1",
		Address:       "00:11:22:33:44:55",
		VhostUserType: VhostUserNet,
	}
	testAppend(vhostuserNetDevice, deviceVhostUserNetString, t)
}

var deviceVFIOString = "-device vfio-pci,host=02:10.0"

func TestAppendDeviceVFIO(t *testing.T) {
	vfioDevice := VFIODevice{
		BDF: "02:10.0",
	}

	testAppend(vfioDevice, deviceVFIOString, t)
}

func TestAppendEmptyDevice(t *testing.T) {
	device := SerialDevice{}

	testAppend(device, "", t)
}

func TestAppendKnobsAllTrue(t *testing.T) {
	var knobsString = "-no-user-config -nodefaults -nographic -daemonize -realtime mlock=on"
	knobs := Knobs{
		NoUserConfig: true,
		NoDefaults:   true,
		NoGraphic:    true,
		Daemonize:    true,
		MemPrealloc:  true,
		Realtime:     true,
		Mlock:        true,
	}

	testAppend(knobs, knobsString, t)
}

func TestAppendKnobsAllFalse(t *testing.T) {
	var knobsString = "-realtime mlock=off"
	knobs := Knobs{
		NoUserConfig: false,
		NoDefaults:   false,
		NoGraphic:    false,
		MemPrealloc:  false,
		Realtime:     false,
		Mlock:        false,
	}

	testAppend(knobs, knobsString, t)
}

var kernelString = "-kernel /opt/vmlinux.container -append root=/dev/pmem0p1 rootflags=dax,data=ordered,errors=remount-ro rw rootfstype=ext4 tsc=reliable"

func TestAppendKernel(t *testing.T) {
	kernel := Kernel{
		Path:   "/opt/vmlinux.container",
		Params: "root=/dev/pmem0p1 rootflags=dax,data=ordered,errors=remount-ro rw rootfstype=ext4 tsc=reliable",
	}

	testAppend(kernel, kernelString, t)
}

var memoryString = "-m 2G,slots=2,maxmem=3G"

func TestAppendMemory(t *testing.T) {
	memory := Memory{
		Size:   "2G",
		Slots:  2,
		MaxMem: "3G",
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
