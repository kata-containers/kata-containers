// +build !s390x,!s390x_test

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

const (
	// Virtio9P is the 9pfs device driver.
	Virtio9P DeviceDriver = "virtio-9p-pci"

	// VirtioSerial is the serial device driver.
	VirtioSerial DeviceDriver = "virtio-serial-pci"

	// VirtioNet is the virt-io pci networking device driver.
	VirtioNet DeviceDriver = VirtioNetPCI

	// Vfio is the vfio driver
	Vfio DeviceDriver = "vfio-pci"

	// VirtioScsi is the virtio-scsi device
	VirtioScsi DeviceDriver = "virtio-scsi-pci"

	// VHostVSock is a generic Vsock vhost device
	VHostVSock DeviceDriver = "vhost-vsock-pci"
)

// isVirtioPCI is a map indicating if a DeviceDriver is considered as a
// virtio PCI device, which is helpful to determine if the option "romfile"
// applies or not to this specific device.
var isVirtioPCI = map[DeviceDriver]bool{
	NVDIMM:              false,
	Virtio9P:            true,
	VirtioNetPCI:        true,
	VirtioSerial:        true,
	VirtioBlock:         true,
	Console:             false,
	VirtioSerialPort:    false,
	VHostVSock:          true,
	VirtioRng:           true,
	VirtioBalloon:       true,
	VhostUserSCSI:       true,
	VhostUserBlk:        true,
	Vfio:                true,
	VirtioScsi:          true,
	PCIBridgeDriver:     true,
	PCIePCIBridgeDriver: true,
}

// QemuNetdevParam converts to the QEMU -netdev parameter notation
func (n NetDeviceType) QemuNetdevParam() string {
	switch n {
	case TAP:
		return "tap"
	case MACVTAP:
		return "tap"
	case IPVTAP:
		return "tap"
	case VETHTAP:
		return "tap" // -netdev type=tap -device virtio-net-pci
	case VFIO:
		return "" // -device vfio-pci (no netdev)
	case VHOSTUSER:
		return "vhost-user" // -netdev type=vhost-user (no device)
	default:
		return ""

	}
}

// QemuDeviceParam converts to the QEMU -device parameter notation
func (n NetDeviceType) QemuDeviceParam() DeviceDriver {
	switch n {
	case TAP:
		return "virtio-net-pci"
	case MACVTAP:
		return "virtio-net-pci"
	case IPVTAP:
		return "virtio-net-pci"
	case VETHTAP:
		return "virtio-net-pci" // -netdev type=tap -device virtio-net-pci
	case VFIO:
		return "vfio-pci" // -device vfio-pci (no netdev)
	case VHOSTUSER:
		return "" // -netdev type=vhost-user (no device)
	default:
		return ""

	}
}
