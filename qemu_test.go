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
	"strings"
	"testing"

	"github.com/01org/ciao/testutil"
)

func testAppend(structure interface{}, expected string, t *testing.T) {
	var params []string

	switch s := structure.(type) {
	case Machine:
		config := Config{
			Machine: s,
		}

		params = appendMachine([]string{}, config)

	case Driver:
		config := Config{
			Drivers: []Driver{s},
		}

		params = appendDrivers([]string{}, config)

	case Knobs:
		config := Config{
			Knobs: s,
		}

		params = appendKnobs([]string{}, config)

	case Kernel:
		config := Config{
			Kernel: s,
		}

		params = appendKernel([]string{}, config)

	case Memory:
		config := Config{
			Memory: s,
		}

		params = appendMemory([]string{}, config)

	case SMP:
		config := Config{
			SMP: s,
		}

		params = appendCPUs([]string{}, config)

	case QMPSocket:
		config := Config{
			QMPSocket: s,
		}

		params = appendQMPSocket([]string{}, config)

	case RTC:
		config := Config{
			RTC: s,
		}

		params = appendRTC([]string{}, config)
	}

	result := strings.Join(params, " ")
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

var deviceNVDIMMString = "-object memory-backend-file,id=mem0,mem-path=/root,size=65536 -device nvdimm,id=nv0,memdev=mem0"

func TestAppendDeviceNVDIMM(t *testing.T) {
	object := Object{
		Type:    MemoryBackendFile,
		ID:      "mem0",
		MemPath: "/root",
		Size:    1 << 16,
	}

	driver := Driver{
		Driver: NVDIMM,
		ID:     "nv0",
		Device: object,
	}

	testAppend(driver, deviceNVDIMMString, t)
}

var deviceFSString = "-fsdev local,id=workload9p,path=/var/lib/docker/devicemapper/mnt/e31ebda2,security-model=none -device virtio-9p-pci,fsdev=workload9p,mount_tag=rootfs"

func TestAppendDeviceFS(t *testing.T) {
	fsdev := FSDevice{
		Driver:        Local,
		ID:            "workload9p",
		Path:          "/var/lib/docker/devicemapper/mnt/e31ebda2",
		MountTag:      "rootfs",
		SecurityModel: None,
	}

	driver := Driver{
		Driver: Virtio9P,
		Device: fsdev,
	}

	testAppend(driver, deviceFSString, t)
}

var deviceNetworkString = "-netdev tap,id=tap0,ifname=ceth0,downscript=no,script=no,fds=8:9:10,vhost=on -device virtio-net,netdev=tap0,mac=01:02:de:ad:be:ef"

func TestAppendDeviceNetwork(t *testing.T) {
	netdev := NetDevice{
		Type:       TAP,
		ID:         "tap0",
		IFName:     "ceth0",
		Script:     "no",
		DownScript: "no",
		FDs:        []int{8, 9, 10},
		VHost:      true,
		MACAddress: "01:02:de:ad:be:ef",
	}

	driver := Driver{
		Driver: VirtioNet,
		Device: netdev,
	}

	testAppend(driver, deviceNetworkString, t)
}

var deviceSerialString = "-device virtio-serial-pci,id=serial0"

func TestAppendDeviceSerial(t *testing.T) {
	driver := Driver{
		Driver: VirtioSerial,
		ID:     "serial0",
		Device: SerialDevice{},
	}

	testAppend(driver, deviceSerialString, t)
}

func TestAppendEmptyDevice(t *testing.T) {
	driver := Driver{}

	testAppend(driver, "", t)
}

var knobsString = "-no-user-config -nodefaults -nographic"

func TestAppendKnobsAllTrue(t *testing.T) {
	knobs := Knobs{
		NoUserConfig: true,
		NoDefaults:   true,
		NoGraphic:    true,
	}

	testAppend(knobs, knobsString, t)
}

func TestAppendKnobsAllFalse(t *testing.T) {
	knobs := Knobs{
		NoUserConfig: false,
		NoDefaults:   false,
		NoGraphic:    false,
	}

	testAppend(knobs, "", t)
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

var cpusString = "-smp 2,cores=1,threads=2,sockets=2"

func TestAppendCPUs(t *testing.T) {
	smp := SMP{
		CPUs:    2,
		Sockets: 2,
		Cores:   1,
		Threads: 2,
	}

	testAppend(smp, cpusString, t)
}

var qmpSocketServerString = "-qmp unix:cc-qmp,server,nowait"
var qmpSocketString = "-qmp unix:cc-qmp"

func TestAppendQMPSocketServer(t *testing.T) {
	qmp := QMPSocket{
		Type:   "unix",
		Name:   "cc-qmp",
		Server: true,
		NoWait: true,
	}

	testAppend(qmp, qmpSocketServerString, t)
}

func TestAppendQMPSocket(t *testing.T) {
	qmp := QMPSocket{
		Type:   Unix,
		Name:   "cc-qmp",
		Server: false,
	}

	testAppend(qmp, qmpSocketString, t)
}

var qemuString = "-name cc-qemu -cpu host -uuid " + testutil.AgentUUID

func TestAppendStrings(t *testing.T) {
	var params []string

	config := Config{
		Path:     "qemu",
		Name:     "cc-qemu",
		UUID:     testutil.AgentUUID,
		CPUModel: "host",
	}

	params = appendName(params, config)
	params = appendCPUModel(params, config)
	params = appendUUID(params, config)

	result := strings.Join(params, " ")
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
