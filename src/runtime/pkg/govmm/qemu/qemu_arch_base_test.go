//go:build !s390x

// Copyright contributors to the Virtual Machine Manager for Go project
//
// SPDX-License-Identifier: Apache-2.0
//

package qemu

import "testing"

var (
	deviceFSString                 = "-device virtio-9p-pci,disable-modern=true,fsdev=workload9p,mount_tag=rootfs,romfile=efi-virtio.rom -fsdev local,id=workload9p,path=/var/lib/docker/devicemapper/mnt/e31ebda2,security_model=none,multidevs=remap"
	deviceNetworkString            = "-netdev tap,id=tap0,vhost=on,ifname=ceth0,downscript=no,script=no -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,bus=/pci-bus/pcie.0,addr=ff,disable-modern=true,romfile=efi-virtio.rom"
	deviceNetworkStringMq          = "-netdev tap,id=tap0,vhost=on,fds=3:4 -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,bus=/pci-bus/pcie.0,addr=ff,disable-modern=true,mq=on,vectors=6,romfile=efi-virtio.rom"
	deviceSerialString             = "-device virtio-serial-pci,disable-modern=true,id=serial0,romfile=efi-virtio.rom,max_ports=2"
	deviceVhostUserNetString       = "-chardev socket,id=char1,path=/tmp/nonexistentsocket.socket -netdev type=vhost-user,id=net1,chardev=char1,vhostforce -device virtio-net-pci,netdev=net1,mac=00:11:22:33:44:55,romfile=efi-virtio.rom"
	deviceVSOCKString              = "-device vhost-vsock-pci,disable-modern=true,id=vhost-vsock-pci0,guest-cid=4,romfile=efi-virtio.rom"
	deviceVFIOString               = "-device vfio-pci,host=02:10.0,x-pci-vendor-id=0x1234,x-pci-device-id=0x5678,romfile=efi-virtio.rom"
	devicePCIeRootPortSimpleString = "-device pcie-root-port,id=rp1,bus=pcie.0,chassis=0x00,slot=0x00,multifunction=off"
	devicePCIeRootPortFullString   = "-device pcie-root-port,id=rp2,bus=pcie.0,chassis=0x0,slot=0x1,addr=0x2,multifunction=on,bus-reserve=0x3,pref64-reserve=16G,mem-reserve=1G,io-reserve=512M,romfile=efi-virtio.rom"
	deviceVFIOPCIeSimpleString     = "-device vfio-pci,host=02:00.0,bus=rp0"
	deviceVFIOPCIeFullString       = "-device vfio-pci,host=02:00.0,x-pci-vendor-id=0x10de,x-pci-device-id=0x15f8,romfile=efi-virtio.rom,bus=rp1"
	deviceSCSIControllerStr        = "-device virtio-scsi-pci,id=foo,disable-modern=false,romfile=efi-virtio.rom"
	deviceSCSIControllerBusAddrStr = "-device virtio-scsi-pci,id=foo,bus=pci.0,addr=00:04.0,disable-modern=true,iothread=iothread1,romfile=efi-virtio.rom"
	deviceVhostUserSCSIString      = "-chardev socket,id=char1,path=/tmp/nonexistentsocket.socket -device vhost-user-scsi-pci,id=scsi1,chardev=char1,romfile=efi-virtio.rom"
	deviceVhostUserBlkString       = "-chardev socket,id=char2,path=/tmp/nonexistentsocket.socket -device vhost-user-blk-pci,logical_block_size=4096,size=512M,chardev=char2,romfile=efi-virtio.rom"
	deviceBlockString              = "-device virtio-blk-pci,disable-modern=true,drive=hd0,config-wce=off,romfile=efi-virtio.rom,share-rw=on,serial=hd0 -drive id=hd0,file=/var/lib/vm.img,aio=threads,format=qcow2,if=none,readonly=on"
	devicePCIBridgeString          = "-device pci-bridge,bus=/pci-bus/pcie.0,id=mybridge,chassis_nr=5,shpc=on,addr=ff,romfile=efi-virtio.rom"
	devicePCIBridgeStringReserved  = "-device pci-bridge,bus=/pci-bus/pcie.0,id=mybridge,chassis_nr=5,shpc=off,addr=ff,romfile=efi-virtio.rom,io-reserve=4k,mem-reserve=1m,pref64-reserve=1m"
	devicePCIEBridgeString         = "-device pcie-pci-bridge,bus=/pci-bus/pcie.0,id=mybridge,addr=ff,romfile=efi-virtio.rom"
	romfile                        = "efi-virtio.rom"
)

func TestAppendDeviceVhostUser(t *testing.T) {

	vhostuserBlkDevice := VhostUserDevice{
		SocketPath:    "/tmp/nonexistentsocket.socket",
		CharDevID:     "char2",
		TypeDevID:     "",
		Address:       "",
		VhostUserType: VhostUserBlk,
		ROMFile:       romfile,
	}
	testAppend(vhostuserBlkDevice, deviceVhostUserBlkString, t)

	vhostuserSCSIDevice := VhostUserDevice{
		SocketPath:    "/tmp/nonexistentsocket.socket",
		CharDevID:     "char1",
		TypeDevID:     "scsi1",
		Address:       "",
		VhostUserType: VhostUserSCSI,
		ROMFile:       romfile,
	}
	testAppend(vhostuserSCSIDevice, deviceVhostUserSCSIString, t)

	vhostuserNetDevice := VhostUserDevice{
		SocketPath:    "/tmp/nonexistentsocket.socket",
		CharDevID:     "char1",
		TypeDevID:     "net1",
		Address:       "00:11:22:33:44:55",
		VhostUserType: VhostUserNet,
		ROMFile:       romfile,
	}
	testAppend(vhostuserNetDevice, deviceVhostUserNetString, t)
}

func TestAppendVirtioBalloon(t *testing.T) {
	balloonDevice := BalloonDevice{
		ID:      "balloon",
		ROMFile: romfile,
	}

	var deviceString = "-device " + string(VirtioBalloon) + "-" + string(TransportPCI)
	deviceString += ",id=" + balloonDevice.ID + ",romfile=" + balloonDevice.ROMFile

	var OnDeflateOnOMM = ",deflate-on-oom=on"
	var OffDeflateOnOMM = ",deflate-on-oom=off"

	var OnDisableModern = ",disable-modern=true"
	var OffDisableModern = ",disable-modern=false"

	testAppend(balloonDevice, deviceString+OffDeflateOnOMM+OffDisableModern, t)

	balloonDevice.DeflateOnOOM = true
	testAppend(balloonDevice, deviceString+OnDeflateOnOMM+OffDisableModern, t)

	balloonDevice.DisableModern = true
	testAppend(balloonDevice, deviceString+OnDeflateOnOMM+OnDisableModern, t)

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

func TestAppendPCIBridgeDeviceWithReservations(t *testing.T) {

	bridge := BridgeDevice{
		Type:          PCIBridge,
		ID:            "mybridge",
		Bus:           "/pci-bus/pcie.0",
		Addr:          "255",
		Chassis:       5,
		SHPC:          false,
		ROMFile:       romfile,
		IOReserve:     "4k",
		MemReserve:    "1m",
		Pref64Reserve: "1m",
	}

	testAppend(bridge, devicePCIBridgeStringReserved, t)
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

func TestAppendDevicePCIeRootPort(t *testing.T) {
	var pcieRootPortID string

	// test empty ID
	pcieRootPortDevice := PCIeRootPortDevice{}
	if pcieRootPortDevice.Valid() {
		t.Fatalf("failed to validdate empty ID")
	}

	// test pref64_reserve and pre64_reserve
	pcieRootPortID = "rp0"
	pcieRootPortDevice = PCIeRootPortDevice{
		ID:            pcieRootPortID,
		Pref64Reserve: "16G",
		Pref32Reserve: "256M",
	}
	if pcieRootPortDevice.Valid() {
		t.Fatalf("failed to validate pref32-reserve and pref64-reserve for %v", pcieRootPortID)
	}

	// default test
	pcieRootPortID = "rp1"
	pcieRootPortDevice = PCIeRootPortDevice{
		ID: pcieRootPortID,
	}
	if !pcieRootPortDevice.Valid() {
		t.Fatalf("failed to validate for %v", pcieRootPortID)
	}
	testAppend(pcieRootPortDevice, devicePCIeRootPortSimpleString, t)

	// full test
	pcieRootPortID = "rp2"
	pcieRootPortDevice = PCIeRootPortDevice{
		ID:            pcieRootPortID,
		Multifunction: true,
		Bus:           "pcie.0",
		Chassis:       "0x0",
		Slot:          "0x1",
		Addr:          "0x2",
		Pref64Reserve: "16G",
		IOReserve:     "512M",
		MemReserve:    "1G",
		BusReserve:    "0x3",
		ROMFile:       romfile,
	}
	if !pcieRootPortDevice.Valid() {
		t.Fatalf("failed to validate for %v", pcieRootPortID)
	}
	testAppend(pcieRootPortDevice, devicePCIeRootPortFullString, t)
}

func TestAppendDeviceVFIOPCIe(t *testing.T) {
	// default test
	pcieRootPortID := "rp0"
	vfioDevice := VFIODevice{
		BDF: "02:00.0",
		Bus: pcieRootPortID,
	}
	testAppend(vfioDevice, deviceVFIOPCIeSimpleString, t)

	// full test
	pcieRootPortID = "rp1"
	vfioDevice = VFIODevice{
		BDF:      "02:00.0",
		Bus:      pcieRootPortID,
		ROMFile:  romfile,
		VendorID: "0x10de",
		DeviceID: "0x15f8",
	}
	testAppend(vfioDevice, deviceVFIOPCIeFullString, t)
}
