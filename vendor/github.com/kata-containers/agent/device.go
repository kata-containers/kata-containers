//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"github.com/kata-containers/agent/pkg/uevent"
	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const (
	driver9pType        = "9p"
	driverBlkType       = "blk"
	driverSCSIType      = "scsi"
	driverEphemeralType = "ephemeral"
)

const (
	rootBusPath = "/devices/pci0000:00"
	pciBusMode  = 0220
)

var (
	sysBusPrefix     = sysfsDir + "/bus/pci/devices"
	pciBusRescanFile = sysfsDir + "/bus/pci/rescan"
	pciBusPathFormat = "%s/%s/pci_bus/"
	systemDevPath    = "/dev"
)

// SCSI variables
var (
	// Here in "0:0", the first number is the SCSI host number because
	// only one SCSI controller has been plugged, while the second number
	// is always 0.
	scsiHostChannel = "0:0:"
	sysClassPrefix  = sysfsDir + "/class"
	scsiDiskPrefix  = filepath.Join(sysClassPrefix, "scsi_disk", scsiHostChannel)
	scsiBlockSuffix = "block"
	scsiDiskSuffix  = filepath.Join("/device", scsiBlockSuffix)
	scsiHostPath    = filepath.Join(sysClassPrefix, "scsi_host")
)

type deviceHandler func(device pb.Device, spec *pb.Spec, s *sandbox) error

var deviceHandlerList = map[string]deviceHandler{
	driverBlkType:  virtioBlkDeviceHandler,
	driverSCSIType: virtioSCSIDeviceHandler,
}

func rescanPciBus() error {
	return ioutil.WriteFile(pciBusRescanFile, []byte{'1'}, pciBusMode)
}

// getDevicePCIAddress fetches the complete PCI address in sysfs, based on the PCI
// identifier provided. This should be in the format: "bridgeAddr/deviceAddr".
// Here, bridgeAddr is the address at which the brige is attached on the root bus,
// while deviceAddr is the address at which the device is attached on the bridge.
func getDevicePCIAddress(pciID string) (string, error) {
	tokens := strings.Split(pciID, "/")

	if len(tokens) != 2 {
		return "", fmt.Errorf("PCI Identifier for device should be of format [bridgeAddr/deviceAddr], got %s", pciID)
	}

	bridgeID := tokens[0]
	deviceID := tokens[1]

	// Deduce the complete bridge address based on the bridge address identifier passed
	// and the fact that bridges are attached on the main bus with function 0.
	pciBridgeAddr := fmt.Sprintf("0000:00:%s.0", bridgeID)

	// Find out the bus exposed by bridge
	bridgeBusPath := fmt.Sprintf(pciBusPathFormat, sysBusPrefix, pciBridgeAddr)

	files, err := ioutil.ReadDir(bridgeBusPath)
	if err != nil {
		return "", fmt.Errorf("Error with getting bridge pci bus : %s", err)
	}

	busNum := len(files)
	if busNum != 1 {
		return "", fmt.Errorf("Expected an entry for bus in %s, got %d entries instead", bridgeBusPath, busNum)
	}

	bus := files[0].Name()

	// Device address is based on the bus of the bridge to which it is attached.
	// We do not pass devices as multifunction, hence the trailing 0 in the address.
	pciDeviceAddr := fmt.Sprintf("%s:%s.0", bus, deviceID)

	bridgeDevicePCIAddr := fmt.Sprintf("%s/%s", pciBridgeAddr, pciDeviceAddr)
	agentLog.WithField("completePCIAddr", bridgeDevicePCIAddr).Info("Fetched PCI address for device")

	return bridgeDevicePCIAddr, nil
}

func getPCIDeviceName(s *sandbox, pciID string) (string, error) {
	pciAddr, err := getDevicePCIAddress(pciID)
	if err != nil {
		return "", err
	}

	var devName string
	var notifyChan chan string

	fieldLogger := agentLog.WithField("pciID", pciID)

	// Check if the PCI identifier is in PCI device map.
	s.Lock()
	for key, value := range s.pciDeviceMap {
		if strings.Contains(key, pciAddr) {
			devName = value
			fieldLogger.Info("Device found in pci device map")
			break
		}
	}

	// Rescan pci bus if we need to wait for a new pci device
	if err = rescanPciBus(); err != nil {
		fieldLogger.WithError(err).Error("Failed to scan pci bus")
		s.Unlock()
		return "", err
	}

	// If device is not found in the device map, hotplug event has not
	// been received yet, create and add channel to the watchers map.
	// The key of the watchers map is the device we are interested in.
	// Note this is done inside the lock, not to miss any events from the
	// global udev listener.
	if devName == "" {
		notifyChan = make(chan string, 1)
		s.deviceWatchers[pciAddr] = notifyChan
	}
	s.Unlock()

	if devName == "" {
		fieldLogger.Info("Waiting on channel for device notification")
		select {
		case devName = <-notifyChan:
		case <-time.After(time.Duration(timeoutHotplug) * time.Second):
			s.Lock()
			delete(s.deviceWatchers, pciAddr)
			s.Unlock()

			return "", grpcStatus.Errorf(codes.DeadlineExceeded,
				"Timeout reached after %ds waiting for device %s",
				timeoutHotplug, pciAddr)
		}
	}

	return filepath.Join(systemDevPath, devName), nil
}

// device.Id should be the PCI address in the format  "bridgeAddr/deviceAddr".
// Here, bridgeAddr is the address at which the brige is attached on the root bus,
// while deviceAddr is the address at which the device is attached on the bridge.
func virtioBlkDeviceHandler(device pb.Device, spec *pb.Spec, s *sandbox) error {
	// Get the device node path based on the PCI device address
	devPath, err := getPCIDeviceName(s, device.Id)
	if err != nil {
		return err
	}
	device.VmPath = devPath

	return updateSpecDeviceList(device, spec)
}

// device.Id should be the SCSI address of the disk in the format "scsiID:lunID"
func virtioSCSIDeviceHandler(device pb.Device, spec *pb.Spec, s *sandbox) error {
	// Retrieve the device path from SCSI address.
	devPath, err := getSCSIDevPath(device.Id)
	if err != nil {
		return err
	}
	device.VmPath = devPath

	return updateSpecDeviceList(device, spec)
}

// updateSpecDeviceList takes a device description provided by the caller,
// trying to find it on the guest. Once this device has been identified, the
// "real" information that can be read from inside the VM is used to update
// the same device in the list of devices provided through the OCI spec.
// This is needed to update information about minor/major numbers that cannot
// be predicted from the caller.
func updateSpecDeviceList(device pb.Device, spec *pb.Spec) error {
	// If no ContainerPath is provided, we won't be able to match and
	// update the device in the OCI spec device list. This is an error.
	if device.ContainerPath == "" {
		return grpcStatus.Errorf(codes.Internal,
			"ContainerPath cannot be empty")
	}

	if spec.Linux == nil || len(spec.Linux.Devices) == 0 {
		return grpcStatus.Errorf(codes.Internal,
			"No devices found from the spec, cannot update")
	}

	stat := syscall.Stat_t{}
	if err := syscall.Stat(device.VmPath, &stat); err != nil {
		return err
	}

	dev := stat.Rdev

	major := int64(unix.Major(dev))
	minor := int64(unix.Minor(dev))

	agentLog.WithFields(logrus.Fields{
		"device-path":  device.VmPath,
		"device-major": major,
		"device-minor": minor,
	}).Info("handling block device")

	// Update the spec
	for idx, d := range spec.Linux.Devices {
		if d.Path == device.ContainerPath {
			hostMajor := spec.Linux.Devices[idx].Major
			hostMinor := spec.Linux.Devices[idx].Minor
			agentLog.WithFields(logrus.Fields{
				"device-path":        device.VmPath,
				"host-device-major":  hostMajor,
				"host-device-minor":  hostMinor,
				"guest-device-major": major,
				"guest-device-minor": minor,
			}).Info("updating block device major/minor into the spec")

			spec.Linux.Devices[idx].Major = major
			spec.Linux.Devices[idx].Minor = minor

			// there is no resource to update
			if spec.Linux == nil || spec.Linux.Resources == nil {
				return nil
			}

			// Resources must be updated since they are used to identify the
			// device in the devices cgroup.
			for idxRsrc, dRsrc := range spec.Linux.Resources.Devices {
				if dRsrc.Major == hostMajor && dRsrc.Minor == hostMinor {
					spec.Linux.Resources.Devices[idxRsrc].Major = major
					spec.Linux.Resources.Devices[idxRsrc].Minor = minor
				}
			}

			return nil
		}
	}

	return grpcStatus.Errorf(codes.Internal,
		"Should have found a matching device %s in the spec",
		device.VmPath)
}

type checkUeventCb func(uEv *uevent.Uevent) bool

func waitForDevice(devicePath, deviceName string, checkUevent checkUeventCb) error {
	uEvHandler, err := uevent.NewHandler()
	if err != nil {
		return err
	}
	defer uEvHandler.Close()

	fieldLogger := agentLog.WithField("device", deviceName)

	// Check if the device already exists.
	if _, err := os.Stat(devicePath); err == nil {
		fieldLogger.Info("Device already hotplugged, quit listening")
		return nil
	}

	fieldLogger.Info("Started listening for uevents for device hotplug")

	// Channel to signal when desired uevent has been received.
	done := make(chan bool)

	go func() {
		// This loop will be either ended if the hotplugged device is
		// found by listening to the netlink socket, or it will end
		// after the function returns and the uevent handler is closed.
		for {
			uEv, err := uEvHandler.Read()
			if err != nil {
				fieldLogger.Error(err)
				continue
			}

			fieldLogger = fieldLogger.WithFields(logrus.Fields{
				"uevent-action":    uEv.Action,
				"uevent-devpath":   uEv.DevPath,
				"uevent-subsystem": uEv.SubSystem,
				"uevent-seqnum":    uEv.SeqNum,
			})

			fieldLogger.Info("Got uevent")

			if checkUevent(uEv) {
				fieldLogger.Info("Hotplug event received")
				break
			}
		}

		close(done)
	}()

	select {
	case <-done:
	case <-time.After(time.Duration(timeoutHotplug) * time.Second):
		return grpcStatus.Errorf(codes.DeadlineExceeded,
			"Timeout reached after %ds waiting for device %s",
			timeoutHotplug, deviceName)
	}

	return nil
}

// scanSCSIBus scans SCSI bus for the given SCSI address(SCSI-Id and LUN)
func scanSCSIBus(scsiAddr string) error {
	files, err := ioutil.ReadDir(scsiHostPath)
	if err != nil {
		return err
	}

	tokens := strings.Split(scsiAddr, ":")
	if len(tokens) != 2 {
		return grpcStatus.Errorf(codes.Internal,
			"Unexpected format for SCSI Address : %s, expect SCSIID:LUN",
			scsiAddr)
	}

	// Scan scsi host passing in the channel, SCSI id and LUN. Channel
	// is always 0 because we have only one SCSI controller.
	scanData := []byte(fmt.Sprintf("0 %s %s", tokens[0], tokens[1]))

	for _, file := range files {
		host := file.Name()
		scanPath := filepath.Join(scsiHostPath, host, "scan")
		if err := ioutil.WriteFile(scanPath, scanData, 0200); err != nil {
			return err
		}
	}

	return nil
}

// findSCSIDisk finds the SCSI disk name associated with the given SCSI path.
// This approach eliminates the need to predict the disk name on the host side,
// but we do need to rescan SCSI bus for this.
func findSCSIDisk(scsiPath string) (string, error) {
	files, err := ioutil.ReadDir(scsiPath)
	if err != nil {
		return "", err
	}

	if len(files) != 1 {
		return "", grpcStatus.Errorf(codes.Internal,
			"Expecting a single SCSI device, found %v",
			files)
	}

	return files[0].Name(), nil
}

// getSCSIDevPath scans SCSI bus looking for the provided SCSI address, then
// it waits for the SCSI disk to become available and returns the device path
// associated with the disk.
func getSCSIDevPath(scsiAddr string) (string, error) {
	if err := scanSCSIBus(scsiAddr); err != nil {
		return "", err
	}

	devPath := filepath.Join(scsiDiskPrefix+scsiAddr, scsiDiskSuffix)

	checkUevent := func(uEv *uevent.Uevent) bool {
		devSubPath := filepath.Join(scsiHostChannel+scsiAddr, scsiBlockSuffix)
		return (uEv.Action == "add" &&
			strings.Contains(uEv.DevPath, devSubPath))
	}
	if err := waitForDevice(devPath, scsiAddr, checkUevent); err != nil {
		return "", err
	}

	scsiDiskName, err := findSCSIDisk(devPath)
	if err != nil {
		return "", err
	}

	return filepath.Join(devPrefix, scsiDiskName), nil
}

func addDevices(devices []*pb.Device, spec *pb.Spec, s *sandbox) error {
	for _, device := range devices {
		if device == nil {
			continue
		}

		err := addDevice(device, spec, s)
		if err != nil {
			return err
		}

	}

	return nil
}

func addDevice(device *pb.Device, spec *pb.Spec, s *sandbox) error {
	if device == nil {
		return grpcStatus.Error(codes.InvalidArgument, "invalid device")
	}

	if spec == nil {
		return grpcStatus.Error(codes.InvalidArgument, "invalid spec")
	}

	// log before validation to help with debugging gRPC protocol
	// version differences.
	agentLog.WithFields(logrus.Fields{
		"device-id":             device.Id,
		"device-type":           device.Type,
		"device-vm-path":        device.VmPath,
		"device-container-path": device.ContainerPath,
		"device-options":        device.Options,
	}).Debug()

	if device.Type == "" {
		return grpcStatus.Errorf(codes.InvalidArgument,
			"invalid type for device %v", device)
	}

	if device.Id == "" && device.VmPath == "" {
		return grpcStatus.Errorf(codes.InvalidArgument,
			"invalid ID and VM path for device %v", device)
	}

	if device.ContainerPath == "" {
		return grpcStatus.Errorf(codes.InvalidArgument,
			"invalid container path for device %v", device)
	}

	devHandler, ok := deviceHandlerList[device.Type]
	if !ok {
		return grpcStatus.Errorf(codes.InvalidArgument,
			"Unknown device type %q", device.Type)
	}

	return devHandler(*device, spec, s)
}
