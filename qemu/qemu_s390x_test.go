// +build s390x

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

import "testing"

// -pci devices don't play well with Z hence replace them with corresponding -ccw devices
// See https://wiki.qemu.org/Documentation/Platforms/S390X
var (
	deviceFSString                 = "-device virtio-9p-ccw,fsdev=workload9p,mount_tag=rootfs -fsdev local,id=workload9p,path=/var/lib/docker/devicemapper/mnt/e31ebda2,security_model=none"
	deviceNetworkString            = "-netdev tap,id=tap0,vhost=on,ifname=ceth0,downscript=no,script=no -device driver=virtio-net-ccw,netdev=tap0,mac=01:02:de:ad:be:ef"
	deviceNetworkStringMq          = "-netdev tap,id=tap0,vhost=on,fds=3:4 -device driver=virtio-net-ccw,netdev=tap0,mac=01:02:de:ad:be:ef,mq=on"
	deviceNetworkPCIString         = "-netdev tap,id=tap0,vhost=on,ifname=ceth0,downscript=no,script=no -device driver=virtio-net-ccw,netdev=tap0,mac=01:02:de:ad:be:ef,bus=/pci-bus/pcie.0,addr=ff"
	deviceNetworkPCIStringMq       = "-netdev tap,id=tap0,vhost=on,fds=3:4 -device driver=virtio-net-ccw,netdev=tap0,mac=01:02:de:ad:be:ef,bus=/pci-bus/pcie.0,addr=ff,mq=on"
	deviceSerialString             = "-device virtio-serial-ccw,id=serial0"
	deviceVSOCKString              = "-device vhost-vsock-ccw,id=vhost-vsock-pci0,guest-cid=4"
	deviceVFIOString               = "-device vfio-ccw,host=02:10.0"
	deviceSCSIControllerStr        = "-device virtio-scsi-ccw,id=foo"
	deviceSCSIControllerBusAddrStr = "-device virtio-scsi-ccw,id=foo,bus=pci.0,addr=00:04.0,iothread=iothread1"
	deviceBlockString              = "-device virtio-blk,drive=hd0,scsi=off,config-wce=off -drive id=hd0,file=/var/lib/vm.img,aio=threads,format=qcow2,if=none"
	devicePCIBridgeString          = "-device pci-bridge,bus=/pci-bus/pcie.0,id=mybridge,chassis_nr=5,shpc=on,addr=ff"
	devicePCIEBridgeString         = "-device pcie-pci-bridge,bus=/pci-bus/pcie.0,id=mybridge,addr=ff"
	romfile                        = ""
)

func TestAppendVirtioBalloon(t *testing.T) {
	balloonDevice := BalloonDevice{
		ID: "balloon",
	}

	var deviceString = "-device " + string(VirtioBalloon)
	deviceString += ",id=" + balloonDevice.ID

	var OnDeflateOnOMM = ",deflate-on-oom=on"
	var OffDeflateOnOMM = ",deflate-on-oom=off"

	testAppend(balloonDevice, deviceString+OffDeflateOnOMM, t)

	balloonDevice.DeflateOnOOM = true
	testAppend(balloonDevice, deviceString+OnDeflateOnOMM, t)
}
