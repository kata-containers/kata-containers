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
	//	"fmt"
	"strings"
	"testing"
)

func testAppend(structure interface{}, expected string, t *testing.T) {
	var params []string

	switch s := structure.(type) {
	case Machine:
		config := Config{
			Machine: s,
		}

		params = appendMachine([]string{}, config)

	case Device:
		config := Config{
			Devices: []Device{s},
		}

		params = appendDevices([]string{}, config)

	case Object:
		config := Config{
			Objects: []Object{s},
		}

		params = appendObjects([]string{}, config)

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

	case NetDevice:
		config := Config{
			NetDevices: []NetDevice{s},
		}

		params = appendNetDevices([]string{}, config)
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

var deviceNVDIMMString = "-device nvdimm,id=nv0,memdev=mem0"

func TestAppendDeviceNVDIMM(t *testing.T) {
	device := Device{
		Type:   "nvdimm",
		ID:     "nv0",
		MemDev: "mem0",
	}

	testAppend(device, deviceNVDIMMString, t)
}

var deviceFSString = "-device virtio-9p-pci,fsdev=workload9p,mount_tag=rootfs"

func TestAppendDeviceFS(t *testing.T) {
	device := Device{
		Type:     "virtio-9p-pci",
		FSDev:    "workload9p",
		MountTag: "rootfs",
	}

	testAppend(device, deviceFSString, t)
}

func TestAppendEmptyDevice(t *testing.T) {
	device := Device{}

	testAppend(device, "", t)
}

var objectMemoryString = "-object memory-backend-file,id=mem0,mem-path=/root,size=65536"

func TestAppendObjectMemory(t *testing.T) {
	object := Object{
		Type:    "memory-backend-file",
		ID:      "mem0",
		MemPath: "/root",
		Size:    1 << 16,
	}

	testAppend(object, objectMemoryString, t)
}

func TestAppendEmptyObject(t *testing.T) {
	device := Device{}

	testAppend(device, "", t)
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
		Type:   "unix",
		Name:   "cc-qmp",
		Server: false,
	}

	testAppend(qmp, qmpSocketString, t)
}

var qemuString = "-name cc-qemu -cpu host -uuid 123456789"

func TestAppendStrings(t *testing.T) {
	var params []string

	config := Config{
		Path:     "qemu",
		Name:     "cc-qemu",
		UUID:     "123456789",
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

var netdevString = "-netdev tap,id=ceth0,downscript=no,script=no,fds=8:9:10,vhost=on"

func TestAppendNetDevices(t *testing.T) {
	netdev := NetDevice{
		Type:       "tap",
		ID:         "ceth0",
		Script:     "no",
		DownScript: "no",
		FDs:        []int{8, 9, 10},
		VHost:      true,
	}

	testAppend(netdev, netdevString, t)
}
