//
// Copyright (c) 2017 Intel Corporation
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
//

package virtcontainers

import (
	"encoding/hex"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/go-ini/ini"
	"github.com/sirupsen/logrus"
)

const (
	// DeviceVFIO is the VFIO device type
	DeviceVFIO = "vfio"

	// DeviceBlock is the block device type
	DeviceBlock = "block"

	// DeviceGeneric is a generic device type
	DeviceGeneric = "generic"
)

// Defining this as a variable instead of a const, to allow
// overriding this in the tests.
var sysIOMMUPath = "/sys/kernel/iommu_groups"

var sysDevPrefix = "/sys/dev"

const (
	vfioPath = "/dev/vfio/"
)

// Device is the virtcontainers device interface.
type Device interface {
	attach(hypervisor, *Container) error
	detach(hypervisor) error
	deviceType() string
}

// DeviceInfo is an embedded type that contains device data common to all types of devices.
type DeviceInfo struct {
	// Device path on host
	HostPath string

	// Device path inside the container
	ContainerPath string

	// Type of device: c, b, u or p
	// c , u - character(unbuffered)
	// p - FIFO
	// b - block(buffered) special file
	// More info in mknod(1).
	DevType string

	// Major, minor numbers for device.
	Major int64
	Minor int64

	// FileMode permission bits for the device.
	FileMode os.FileMode

	// id of the device owner.
	UID uint32

	// id of the device group.
	GID uint32

	// Hotplugged is used to store device state indicating if the
	// device was hotplugged.
	Hotplugged bool

	// ID for the device that is passed to the hypervisor.
	ID string
}

func deviceLogger() *logrus.Entry {
	return virtLog.WithField("subsystem", "device")
}

// VFIODevice is a vfio device meant to be passed to the hypervisor
// to be used by the Virtual Machine.
type VFIODevice struct {
	DeviceType string
	DeviceInfo DeviceInfo
	BDF        string
}

func newVFIODevice(devInfo DeviceInfo) *VFIODevice {
	return &VFIODevice{
		DeviceType: DeviceVFIO,
		DeviceInfo: devInfo,
	}
}

func (device *VFIODevice) attach(h hypervisor, c *Container) error {
	vfioGroup := filepath.Base(device.DeviceInfo.HostPath)
	iommuDevicesPath := filepath.Join(sysIOMMUPath, vfioGroup, "devices")

	deviceFiles, err := ioutil.ReadDir(iommuDevicesPath)
	if err != nil {
		return err
	}

	// Pass all devices in iommu group
	for _, deviceFile := range deviceFiles {

		//Get bdf of device eg 0000:00:1c.0
		deviceBDF, err := getBDF(deviceFile.Name())
		if err != nil {
			return err
		}

		device.BDF = deviceBDF

		randBytes, err := generateRandomBytes(8)
		if err != nil {
			return err
		}
		device.DeviceInfo.ID = hex.EncodeToString(randBytes)

		if err := h.hotplugAddDevice(*device, vfioDev); err != nil {
			deviceLogger().WithError(err).Error("Failed to add device")
			return err
		}

		deviceLogger().WithFields(logrus.Fields{
			"device-group": device.DeviceInfo.HostPath,
			"device-type":  "vfio-passthrough",
		}).Info("Device group attached")
	}

	return nil
}

func (device *VFIODevice) detach(h hypervisor) error {
	return nil
}

func (device *VFIODevice) deviceType() string {
	return device.DeviceType
}

// VhostUserDeviceType - represents a vhost-user device type
// Currently support just VhostUserNet
type VhostUserDeviceType string

const (
	//VhostUserSCSI - SCSI based vhost-user type
	VhostUserSCSI = "vhost-user-scsi-pci"
	//VhostUserNet - net based vhost-user type
	VhostUserNet = "virtio-net-pci"
	//VhostUserBlk represents a block vhostuser device type
	VhostUserBlk = "vhost-user-blk-pci"
)

// VhostUserDevice represents a vhost-user device. Shared
// attributes of a vhost-user device can be retrieved using
// the Attrs() method. Unique data can be obtained by casting
// the object to the proper type.
type VhostUserDevice interface {
	Attrs() *VhostUserDeviceAttrs
	Type() string
}

// VhostUserDeviceAttrs represents data shared by most vhost-user devices
type VhostUserDeviceAttrs struct {
	DeviceType string
	DeviceInfo DeviceInfo
	SocketPath string
	ID         string
}

// VhostUserNetDevice is a network vhost-user based device
type VhostUserNetDevice struct {
	VhostUserDeviceAttrs
	MacAddress string
}

// Attrs returns the VhostUserDeviceAttrs associated with the vhost-user device
func (vhostUserNetDevice *VhostUserNetDevice) Attrs() *VhostUserDeviceAttrs {
	return &vhostUserNetDevice.VhostUserDeviceAttrs
}

// Type returns the type associated with the vhost-user device
func (vhostUserNetDevice *VhostUserNetDevice) Type() string {
	return VhostUserNet
}

// VhostUserSCSIDevice is a SCSI vhost-user based device
type VhostUserSCSIDevice struct {
	VhostUserDeviceAttrs
}

// Attrs returns the VhostUserDeviceAttrs associated with the vhost-user device
func (vhostUserSCSIDevice *VhostUserSCSIDevice) Attrs() *VhostUserDeviceAttrs {
	return &vhostUserSCSIDevice.VhostUserDeviceAttrs
}

// Type returns the type associated with the vhost-user device
func (vhostUserSCSIDevice *VhostUserSCSIDevice) Type() string {
	return VhostUserSCSI
}

// VhostUserBlkDevice is a block vhost-user based device
type VhostUserBlkDevice struct {
	VhostUserDeviceAttrs
}

// Attrs returns the VhostUserDeviceAttrs associated with the vhost-user device
func (vhostUserBlkDevice *VhostUserBlkDevice) Attrs() *VhostUserDeviceAttrs {
	return &vhostUserBlkDevice.VhostUserDeviceAttrs
}

// Type returns the type associated with the vhost-user device
func (vhostUserBlkDevice *VhostUserBlkDevice) Type() string {
	return VhostUserBlk
}

// vhostUserAttach handles the common logic among all of the vhost-user device's
// attach functions
func vhostUserAttach(device VhostUserDevice, h hypervisor, c *Container) (err error) {
	// generate a unique ID to be used for hypervisor commandline fields
	randBytes, err := generateRandomBytes(8)
	if err != nil {
		return err
	}
	id := hex.EncodeToString(randBytes)

	device.Attrs().ID = id

	return h.addDevice(device, vhostuserDev)
}

//
// VhostUserNetDevice's implementation of the device interface:
//
func (vhostUserNetDevice *VhostUserNetDevice) attach(h hypervisor, c *Container) (err error) {
	return vhostUserAttach(vhostUserNetDevice, h, c)
}

func (vhostUserNetDevice *VhostUserNetDevice) detach(h hypervisor) error {
	return nil
}

func (vhostUserNetDevice *VhostUserNetDevice) deviceType() string {
	return vhostUserNetDevice.DeviceType
}

//
// VhostUserBlkDevice's implementation of the device interface:
//
func (vhostUserBlkDevice *VhostUserBlkDevice) attach(h hypervisor, c *Container) (err error) {
	return vhostUserAttach(vhostUserBlkDevice, h, c)
}

func (vhostUserBlkDevice *VhostUserBlkDevice) detach(h hypervisor) error {
	return nil
}

func (vhostUserBlkDevice *VhostUserBlkDevice) deviceType() string {
	return vhostUserBlkDevice.DeviceType
}

//
// VhostUserSCSIDevice's implementation of the device interface:
//
func (vhostUserSCSIDevice *VhostUserSCSIDevice) attach(h hypervisor, c *Container) (err error) {
	return vhostUserAttach(vhostUserSCSIDevice, h, c)
}

func (vhostUserSCSIDevice *VhostUserSCSIDevice) detach(h hypervisor) error {
	return nil
}

func (vhostUserSCSIDevice *VhostUserSCSIDevice) deviceType() string {
	return vhostUserSCSIDevice.DeviceType
}

// Long term, this should be made more configurable.  For now matching path
// provided by CNM VPP and OVS-DPDK plugins, available at github.com/clearcontainers/vpp and
// github.com/clearcontainers/ovsdpdk.  The plugins create the socket on the host system
// using this path.
const hostSocketSearchPath = "/tmp/vhostuser_%s/vhu.sock"

// findVhostUserNetSocketPath checks if an interface is a dummy placeholder
// for a vhost-user socket, and if it is it returns the path to the socket
func findVhostUserNetSocketPath(netInfo NetworkInfo) (string, error) {
	if netInfo.Iface.Name == "lo" {
		return "", nil
	}

	// check for socket file existence at known location.
	for _, addr := range netInfo.Addrs {
		socketPath := fmt.Sprintf(hostSocketSearchPath, addr.IPNet.IP)
		if _, err := os.Stat(socketPath); err == nil {
			return socketPath, nil
		}
	}

	return "", nil
}

// vhostUserSocketPath returns the path of the socket discovered.  This discovery
// will vary depending on the type of vhost-user socket.
//  Today only VhostUserNetDevice is supported.
func vhostUserSocketPath(info interface{}) (string, error) {

	switch v := info.(type) {
	case NetworkInfo:
		return findVhostUserNetSocketPath(v)
	default:
		return "", nil
	}

}

// BlockDevice refers to a block storage device implementation.
type BlockDevice struct {
	DeviceType string
	DeviceInfo DeviceInfo

	// SCSI Address of the block device, in case the device is attached using SCSI driver
	// SCSI address is in the format SCSI-Id:LUN
	SCSIAddr string

	// Path at which the device appears inside the VM, outside of the container mount namespace
	VirtPath string
}

func newBlockDevice(devInfo DeviceInfo) *BlockDevice {
	return &BlockDevice{
		DeviceType: DeviceBlock,
		DeviceInfo: devInfo,
	}
}

func (device *BlockDevice) attach(h hypervisor, c *Container) (err error) {
	randBytes, err := generateRandomBytes(8)
	if err != nil {
		return err
	}

	device.DeviceInfo.ID = hex.EncodeToString(randBytes)

	// Increment the block index for the pod. This is used to determine the name
	// for the block device in the case where the block device is used as container
	// rootfs and the predicted block device name needs to be provided to the agent.
	index, err := c.pod.getAndSetPodBlockIndex()

	defer func() {
		if err != nil {
			c.pod.decrementPodBlockIndex()
		}
	}()

	if err != nil {
		return err
	}

	drive := Drive{
		File:   device.DeviceInfo.HostPath,
		Format: "raw",
		ID:     makeNameID("drive", device.DeviceInfo.ID),
		Index:  index,
	}

	driveName, err := getVirtDriveName(index)
	if err != nil {
		return err
	}

	deviceLogger().WithField("device", device.DeviceInfo.HostPath).Info("Attaching block device")

	if err = h.hotplugAddDevice(drive, blockDev); err != nil {
		return err
	}

	device.DeviceInfo.Hotplugged = true

	if c.pod.config.HypervisorConfig.BlockDeviceDriver == VirtioBlock {
		device.VirtPath = filepath.Join("/dev", driveName)
	} else {
		scsiAddr, err := getSCSIAddress(index)
		if err != nil {
			return err
		}

		device.SCSIAddr = scsiAddr
	}

	return nil
}

func (device BlockDevice) detach(h hypervisor) error {
	if device.DeviceInfo.Hotplugged {
		deviceLogger().WithField("device", device.DeviceInfo.HostPath).Info("Unplugging block device")

		drive := Drive{
			ID: makeNameID("drive", device.DeviceInfo.ID),
		}

		if err := h.hotplugRemoveDevice(drive, blockDev); err != nil {
			deviceLogger().WithError(err).Error("Failed to unplug block device")
			return err
		}

	}
	return nil
}

func (device *BlockDevice) deviceType() string {
	return device.DeviceType
}

// GenericDevice refers to a device that is neither a VFIO device or block device.
type GenericDevice struct {
	DeviceType string
	DeviceInfo DeviceInfo
}

func newGenericDevice(devInfo DeviceInfo) *GenericDevice {
	return &GenericDevice{
		DeviceType: DeviceGeneric,
		DeviceInfo: devInfo,
	}
}

func (device *GenericDevice) attach(h hypervisor, c *Container) error {
	return nil
}

func (device *GenericDevice) detach(h hypervisor) error {
	return nil
}

func (device *GenericDevice) deviceType() string {
	return device.DeviceType
}

// isVFIO checks if the device provided is a vfio group.
func isVFIO(hostPath string) bool {
	// Ignore /dev/vfio/vfio character device
	if strings.HasPrefix(hostPath, filepath.Join(vfioPath, "vfio")) {
		return false
	}

	if strings.HasPrefix(hostPath, vfioPath) && len(hostPath) > len(vfioPath) {
		return true
	}

	return false
}

// isBlock checks if the device is a block device.
func isBlock(devInfo DeviceInfo) bool {
	if devInfo.DevType == "b" {
		return true
	}

	return false
}

func createDevice(devInfo DeviceInfo) Device {
	path := devInfo.HostPath

	if isVFIO(path) {
		return newVFIODevice(devInfo)
	} else if isBlock(devInfo) {
		return newBlockDevice(devInfo)
	} else {
		deviceLogger().WithField("device", path).Info("Device has not been passed to the container")
		return newGenericDevice(devInfo)
	}
}

// GetHostPath is used to fetcg the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
func GetHostPath(devInfo DeviceInfo) (string, error) {
	if devInfo.ContainerPath == "" {
		return "", fmt.Errorf("Empty path provided for device")
	}

	var pathComp string

	switch devInfo.DevType {
	case "c", "u":
		pathComp = "char"
	case "b":
		pathComp = "block"
	default:
		// Unsupported device types. Return nil error to ignore devices
		// that cannot be handled currently.
		return "", nil
	}

	format := strconv.FormatInt(devInfo.Major, 10) + ":" + strconv.FormatInt(devInfo.Minor, 10)
	sysDevPath := filepath.Join(sysDevPrefix, pathComp, format, "uevent")

	if _, err := os.Stat(sysDevPath); err != nil {
		// Some devices(eg. /dev/fuse, /dev/cuse) do not always implement sysfs interface under /sys/dev
		// These devices are passed by default by docker.
		//
		// Simply return the path passed in the device configuration, this does mean that no device renames are
		// supported for these devices.

		if os.IsNotExist(err) {
			return devInfo.ContainerPath, nil
		}

		return "", err
	}

	content, err := ini.Load(sysDevPath)
	if err != nil {
		return "", err
	}

	devName, err := content.Section("").GetKey("DEVNAME")
	if err != nil {
		return "", err
	}

	return filepath.Join("/dev", devName.String()), nil
}

// GetHostPathFunc is function pointer used to mock GetHostPath in tests.
var GetHostPathFunc = GetHostPath

func newDevices(devInfos []DeviceInfo) ([]Device, error) {
	var devices []Device

	for _, devInfo := range devInfos {
		hostPath, err := GetHostPathFunc(devInfo)
		if err != nil {
			return nil, err
		}

		devInfo.HostPath = hostPath
		device := createDevice(devInfo)
		devices = append(devices, device)
	}

	return devices, nil
}

// getBDF returns the BDF of pci device
// Expected input strng format is [<domain>]:[<bus>][<slot>].[<func>] eg. 0000:02:10.0
func getBDF(deviceSysStr string) (string, error) {
	tokens := strings.Split(deviceSysStr, ":")

	if len(tokens) != 3 {
		return "", fmt.Errorf("Incorrect number of tokens found while parsing bdf for device : %s", deviceSysStr)
	}

	tokens = strings.SplitN(deviceSysStr, ":", 2)
	return tokens[1], nil
}

// bind/unbind paths to aid in SRIOV VF bring-up/restore
var pciDriverUnbindPath = "/sys/bus/pci/devices/%s/driver/unbind"
var pciDriverBindPath = "/sys/bus/pci/drivers/%s/bind"
var vfioRemoveIDPath = "/sys/bus/pci/drivers/vfio-pci/remove_id"
var vfioNewIDPath = "/sys/bus/pci/drivers/vfio-pci/new_id"

// bindDevicetoVFIO binds the device to vfio driver after unbinding from host.
// Will be called by a network interface or a generic pcie device.
func bindDevicetoVFIO(bdf, hostDriver, vendorDeviceID string) error {

	// Unbind from the host driver
	unbindDriverPath := fmt.Sprintf(pciDriverUnbindPath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": unbindDriverPath,
	}).Info("Unbinding device from driver")

	if err := writeToFile(unbindDriverPath, []byte(bdf)); err != nil {
		return err
	}

	// Add device id to vfio driver.
	deviceLogger().WithFields(logrus.Fields{
		"vendor-device-id": vendorDeviceID,
		"vfio-new-id-path": vfioNewIDPath,
	}).Info("Writing vendor-device-id to vfio new-id path")

	if err := writeToFile(vfioNewIDPath, []byte(vendorDeviceID)); err != nil {
		return err
	}

	// Bind to vfio-pci driver.
	bindDriverPath := fmt.Sprintf(pciDriverBindPath, "vfio-pci")

	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": bindDriverPath,
	}).Info("Binding device to vfio driver")

	// Device may be already bound at this time because of earlier write to new_id, ignore error
	writeToFile(bindDriverPath, []byte(bdf))

	return nil
}

// bindDevicetoHost binds the device to the host driver driver after unbinding from vfio-pci.
func bindDevicetoHost(bdf, hostDriver, vendorDeviceID string) error {
	// Unbind from vfio-pci driver
	unbindDriverPath := fmt.Sprintf(pciDriverUnbindPath, bdf)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": unbindDriverPath,
	}).Info("Unbinding device from driver")

	if err := writeToFile(unbindDriverPath, []byte(bdf)); err != nil {
		return err
	}

	// To prevent new VFs from binding to VFIO-PCI, remove_id
	if err := writeToFile(vfioRemoveIDPath, []byte(vendorDeviceID)); err != nil {
		return err
	}

	// Bind back to host driver
	bindDriverPath := fmt.Sprintf(pciDriverBindPath, hostDriver)
	deviceLogger().WithFields(logrus.Fields{
		"device-bdf":  bdf,
		"driver-path": bindDriverPath,
	}).Info("Binding back device to host driver")

	return writeToFile(bindDriverPath, []byte(bdf))
}
