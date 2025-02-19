// Copyright contributors to the Virtual Machine Manager for Go project
//
// SPDX-License-Identifier: Apache-2.0
//

package qemu

import "testing"

// -pci devices don't play well with Z hence replace them with corresponding -ccw devices
// See https://wiki.qemu.org/Documentation/Platforms/S390X
var (
	deviceFSString                 = "-device virtio-9p-ccw,fsdev=workload9p,mount_tag=rootfs,devno=" + DevNo + " -fsdev local,id=workload9p,path=/var/lib/docker/devicemapper/mnt/e31ebda2,security_model=none,multidevs=remap"
	deviceFSIOMMUString            = "-device virtio-9p-ccw,fsdev=workload9p,mount_tag=rootfs,iommu_platform=on,devno=" + DevNo + " -fsdev local,id=workload9p,path=/var/lib/docker/devicemapper/mnt/e31ebda2,security_model=none,multidevs=remap"
	deviceNetworkString            = "-netdev tap,id=tap0,vhost=on,ifname=ceth0,downscript=no,script=no -device driver=virtio-net-ccw,netdev=tap0,mac=01:02:de:ad:be:ef,devno=" + DevNo
	deviceNetworkStringMq          = "-netdev tap,id=tap0,vhost=on,fds=3:4 -device driver=virtio-net-ccw,netdev=tap0,mac=01:02:de:ad:be:ef,mq=on,devno=" + DevNo
	deviceSerialString             = "-device virtio-serial-ccw,id=serial0,devno=" + DevNo
	deviceVSOCKString              = "-device vhost-vsock-ccw,id=vhost-vsock-pci0,guest-cid=4,devno=" + DevNo
	deviceVFIOString               = "-device vfio-ccw,host=02:10.0,devno=" + DevNo
	deviceSCSIControllerStr        = "-device virtio-scsi-ccw,id=foo,devno=" + DevNo
	deviceSCSIControllerBusAddrStr = "-device virtio-scsi-ccw,id=foo,bus=pci.0,addr=00:04.0,iothread=iothread1,devno=" + DevNo
	deviceBlockString              = "-device virtio-blk-ccw,drive=hd0,config-wce=off,devno=" + DevNo + ",share-rw=on,serial=hd0 -drive id=hd0,file=/var/lib/vm.img,aio=threads,format=qcow2,if=none,readonly=on"
	romfile                        = ""
)

func TestAppendVirtioBalloon(t *testing.T) {
	balloonDevice := BalloonDevice{
		ID: "balloon",
	}

	var deviceString = "-device " + string(VirtioBalloon) + "-" + string(TransportCCW)
	deviceString += ",id=" + balloonDevice.ID
	balloonDevice.DevNo = DevNo
	devnoOptios := ",devno=" + DevNo

	var OnDeflateOnOMM = ",deflate-on-oom=on"
	var OffDeflateOnOMM = ",deflate-on-oom=off"
	testAppend(balloonDevice, deviceString+devnoOptios+OffDeflateOnOMM, t)

	balloonDevice.DeflateOnOOM = true
	testAppend(balloonDevice, deviceString+devnoOptios+OnDeflateOnOMM, t)
}

func TestAppendDeviceFSCCW(t *testing.T) {
	defaultKnobs := Knobs{
		NoUserConfig: true,
	}

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
		Transport:     TransportCCW,
		DevNo:         DevNo,
	}

	var config Config
	config.Knobs = defaultKnobs

	testConfigAppend(&config, fsdev, deviceFSString, t)
}

func TestAppendDeviceFSCCWIOMMU(t *testing.T) {
	defaultKnobs := Knobs{
		NoUserConfig:  true,
		IOMMUPlatform: true,
	}

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
		Transport:     TransportCCW,
		DevNo:         DevNo,
	}

	var config Config
	config.Knobs = defaultKnobs

	testConfigAppend(&config, fsdev, deviceFSIOMMUString, t)
}
