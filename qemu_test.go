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
	}

	result := strings.Join(params, " ")
	if result != expected {
		t.Fatalf("Failed to append Machine [%s] != [%s]", result, expected)
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
