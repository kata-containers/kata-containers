// +build !s390x

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

import "testing"

var (
	deviceFSString                 = "-device virtio-9p-pci,disable-modern=true,fsdev=workload9p,mount_tag=rootfs,romfile=efi-virtio.rom -fsdev local,id=workload9p,path=/var/lib/docker/devicemapper/mnt/e31ebda2,security_model=none"
	deviceNetworkString            = "-netdev tap,id=tap0,vhost=on,ifname=ceth0,downscript=no,script=no -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,disable-modern=true,romfile=efi-virtio.rom"
	deviceNetworkStringMq          = "-netdev tap,id=tap0,vhost=on,fds=3:4 -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,disable-modern=true,mq=on,vectors=6,romfile=efi-virtio.rom"
	deviceNetworkPCIString         = "-netdev tap,id=tap0,vhost=on,ifname=ceth0,downscript=no,script=no -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,bus=/pci-bus/pcie.0,addr=ff,disable-modern=true,romfile=efi-virtio.rom"
	deviceNetworkPCIStringMq       = "-netdev tap,id=tap0,vhost=on,fds=3:4 -device driver=virtio-net-pci,netdev=tap0,mac=01:02:de:ad:be:ef,bus=/pci-bus/pcie.0,addr=ff,disable-modern=true,mq=on,vectors=6,romfile=efi-virtio.rom"
	deviceSerialString             = "-device virtio-serial-pci,disable-modern=true,id=serial0,romfile=efi-virtio.rom"
	deviceVhostUserNetString       = "-chardev socket,id=char1,path=/tmp/nonexistentsocket.socket -netdev type=vhost-user,id=net1,chardev=char1,vhostforce -device virtio-net-pci,netdev=net1,mac=00:11:22:33:44:55,romfile=efi-virtio.rom"
	deviceVSOCKString              = "-device vhost-vsock-pci,disable-modern=true,id=vhost-vsock-pci0,guest-cid=4,romfile=efi-virtio.rom"
	deviceVFIOString               = "-device vfio-pci,host=02:10.0,romfile=efi-virtio.rom"
	deviceSCSIControllerStr        = "-device virtio-scsi-pci,id=foo,disable-modern=false,romfile=efi-virtio.rom"
	deviceSCSIControllerBusAddrStr = "-device virtio-scsi-pci,id=foo,bus=pci.0,addr=00:04.0,disable-modern=true,iothread=iothread1,romfile=efi-virtio.rom"
	deviceVhostUserSCSIString      = "-chardev socket,id=char1,path=/tmp/nonexistentsocket.socket -device vhost-user-scsi-pci,id=scsi1,chardev=char1,romfile=efi-virtio.rom"
	deviceVhostUserBlkString       = "-chardev socket,id=char2,path=/tmp/nonexistentsocket.socket -device vhost-user-blk-pci,logical_block_size=4096,size=512M,chardev=char2,romfile=efi-virtio.rom"
	deviceBlockString              = "-device virtio-blk,disable-modern=true,drive=hd0,scsi=off,config-wce=off,romfile=efi-virtio.rom -drive id=hd0,file=/var/lib/vm.img,aio=threads,format=qcow2,if=none"
	devicePCIBridgeString          = "-device pci-bridge,bus=/pci-bus/pcie.0,id=mybridge,chassis_nr=5,shpc=on,addr=ff,romfile=efi-virtio.rom"
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

	var deviceString = "-device " + string(VirtioBalloon)
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
