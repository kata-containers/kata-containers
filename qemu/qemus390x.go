// +build s390x s390x_test

/*
// Copyright contributors to the Virtual Machine Manager for Go project
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

import "log"

// IBM Z uses CCW devices intead of PCI devices.
// See https://wiki.qemu.org/Documentation/Platforms/S390X
const (
	// Virtio9P is the 9pfs device driver.
	Virtio9P DeviceDriver = "virtio-9p-ccw"

	// VirtioSerial is the serial device driver.
	VirtioSerial DeviceDriver = "virtio-serial-ccw"

	// VirtioNet is the virt-io ccw networking device driver.
	VirtioNet DeviceDriver = VirtioNetCCW

	// Vfio is the vfio driver
	Vfio DeviceDriver = "vfio-ccw"

	// VirtioScsi is the virtio-scsi device
	VirtioScsi DeviceDriver = "virtio-scsi-ccw"

	// VHostVSock is a generic Vsock Device
	VHostVSock DeviceDriver = "vhost-vsock-ccw"
)

// isVirtioPCI is a fake map on s390x to always avoid the "romfile"
// option
var isVirtioPCI = map[DeviceDriver]bool{
	NVDIMM:              false,
	Virtio9P:            false,
	VirtioNetCCW:        false,
	VirtioSerial:        false,
	VirtioBlock:         false,
	Console:             false,
	VirtioSerialPort:    false,
	VHostVSock:          false,
	VirtioRng:           false,
	VirtioBalloon:       false,
	VhostUserSCSI:       false,
	VhostUserBlk:        false,
	Vfio:                false,
	VirtioScsi:          false,
	PCIBridgeDriver:     false,
	PCIePCIBridgeDriver: false,
}

// QemuDeviceParam converts to the QEMU -device parameter notation
// This function has been reimplemented for the s390x architecture to deal
// with the VHOSTUSER case. Vhost user devices are not implemented on s390x
// architecture. For further details see issue
// https://github.com/kata-containers/runtime/issues/659
func (n NetDeviceType) QemuDeviceParam() string {
	switch n {
	case TAP:
		return string(VirtioNet)
	case MACVTAP:
		return string(VirtioNet)
	case IPVTAP:
		return string(VirtioNet)
	case VETHTAP:
		return string(VirtioNet)
	case VFIO:
		return string(Vfio)
	case VHOSTUSER:
		log.Fatal("vhost-user devices are not supported on IBM Z")
		return ""
	default:
		return ""
	}
}

// QemuNetdevParam converts to the QEMU -netdev parameter notation
// This function has been reimplemented for the s390x architecture to deal
// with the VHOSTUSER case. Vhost user devices are not implemented on s390x
// architecture. For further details see issue
// https://github.com/kata-containers/runtime/issues/659
func (n NetDeviceType) QemuNetdevParam() string {
	switch n {
	case TAP:
		return "tap"
	case MACVTAP:
		return "tap"
	case IPVTAP:
		return "tap"
	case VETHTAP:
		return "tap"
	case VFIO:
		return ""
	case VHOSTUSER:
		log.Fatal("vhost-user devices are not supported on IBM Z")
		return ""
	default:
		return ""

	}
}
