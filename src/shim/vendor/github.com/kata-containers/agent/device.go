//
// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"fmt"
	"io/ioutil"
	"path"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"

	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const (
	driver9pType        = "9p"
	driverVirtioFSType  = "virtio-fs"
	driverBlkType       = "blk"
	driverBlkCCWType    = "blk-ccw"
	driverMmioBlkType   = "mmioblk"
	driverSCSIType      = "scsi"
	driverNvdimmType    = "nvdimm"
	driverEphemeralType = "ephemeral"
	driverLocalType     = "local"
)

const (
	pciBusMode = 0220
)

var (
	sysBusPrefix        = sysfsDir + "/bus/pci/devices"
	pciBusRescanFile    = sysfsDir + "/bus/pci/rescan"
	pciBusPathFormat    = "%s/%s/pci_bus/"
	systemDevPath       = "/dev"
	getSCSIDevPath      = getSCSIDevPathImpl
	getPmemDevPath      = getPmemDevPathImpl
	getPCIDeviceName    = getPCIDeviceNameImpl
	getDevicePCIAddress = getDevicePCIAddressImpl
	scanSCSIBus         = scanSCSIBusImpl
)

// CCW variables
var (
	blkCCWSuffix = "virtio"
)

const maxDeviceIDValue = 3

// SCSI variables
var (
	// Here in "0:0", the first number is the SCSI host number because
	// only one SCSI controller has been plugged, while the second number
	// is always 0.
	scsiHostChannel = "0:0:"
	sysClassPrefix  = sysfsDir + "/class"
	scsiBlockSuffix = "block"
	scsiHostPath    = filepath.Join(sysClassPrefix, "scsi_host")
)

type deviceHandler func(ctx context.Context, device pb.Device, spec *pb.Spec, s *sandbox) error

var deviceHandlerList = map[string]deviceHandler{
	driverMmioBlkType: virtioMmioBlkDeviceHandler,
	driverBlkType:     virtioBlkDeviceHandler,
	driverBlkCCWType:  virtioBlkCCWDeviceHandler,
	driverSCSIType:    virtioSCSIDeviceHandler,
	driverNvdimmType:  nvdimmDeviceHandler,
}

func rescanPciBus() error {
	return ioutil.WriteFile(pciBusRescanFile, []byte{'1'}, pciBusMode)
}

// getDevicePCIAddress fetches the complete PCI address in sysfs, based on the PCI
// identifier provided. This should be in the format: "bridgeAddr/deviceAddr".
// Here, bridgeAddr is the address at which the brige is attached on the root bus,
// while deviceAddr is the address at which the device is attached on the bridge.
func getDevicePCIAddressImpl(pciID string) (string, error) {
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

func getDeviceName(s *sandbox, devID string) (string, error) {
	var devName string
	var notifyChan chan string

	fieldLogger := agentLog.WithField("devID", devID)

	// Check if the dev identifier is in PCI device map.
	s.Lock()
	for key, value := range s.pciDeviceMap {
		if strings.Contains(key, devID) {
			devName = value
			fieldLogger.Infof("Device: %s found in pci device map", devID)
			break
		}
	}

	// If device is not found in the device map, hotplug event has not
	// been received yet, create and add channel to the watchers map.
	// The key of the watchers map is the device we are interested in.
	// Note this is done inside the lock, not to miss any events from the
	// global udev listener.
	if devName == "" {
		notifyChan = make(chan string, 1)
		s.deviceWatchers[devID] = notifyChan
	}
	s.Unlock()

	if devName == "" {
		fieldLogger.Infof("Waiting on channel for device: %s notification", devID)
		select {
		case devName = <-notifyChan:
		case <-time.After(hotplugTimeout):
			s.Lock()
			delete(s.deviceWatchers, devID)
			s.Unlock()

			return "", grpcStatus.Errorf(codes.DeadlineExceeded,
				"Timeout reached after %s waiting for device %s",
				hotplugTimeout, devID)
		}
	}

	return filepath.Join(systemDevPath, devName), nil
}

func getPCIDeviceNameImpl(s *sandbox, pciID string) (string, error) {
	pciAddr, err := getDevicePCIAddress(pciID)
	if err != nil {
		return "", err
	}

	fieldLogger := agentLog.WithField("pciAddr", pciAddr)

	// Rescan pci bus if we need to wait for a new pci device
	if err = rescanPciBus(); err != nil {
		fieldLogger.WithError(err).Error("Failed to scan pci bus")
		return "", err
	}

	return getDeviceName(s, pciAddr)
}

// device.Id should be the predicted device name (vda, vdb, ...)
// device.VmPath already provides a way to send it in
func virtioMmioBlkDeviceHandler(_ context.Context, device pb.Device, spec *pb.Spec, s *sandbox) error {
	if device.VmPath == "" {
		return fmt.Errorf("Invalid path for virtioMmioBlkDevice")
	}

	return updateSpecDeviceList(device, spec)
}

func virtioBlkCCWDeviceHandler(ctx context.Context, device pb.Device, spec *pb.Spec, s *sandbox) error {
	devPath, err := getBlkCCWDevPath(s, device.Id)
	if err != nil {
		return err
	}

	if devPath == "" {
		return grpcStatus.Errorf(codes.InvalidArgument,
			"Storage source is empty")
	}

	device.VmPath = devPath
	return updateSpecDeviceList(device, spec)
}

// device.Id should be the PCI address in the format  "bridgeAddr/deviceAddr".
// Here, bridgeAddr is the address at which the brige is attached on the root bus,
// while deviceAddr is the address at which the device is attached on the bridge.
func virtioBlkDeviceHandler(_ context.Context, device pb.Device, spec *pb.Spec, s *sandbox) error {
	// Get the device node path based on the PCI device address
	devPath, err := getPCIDeviceName(s, device.Id)
	if err != nil {
		return err
	}
	device.VmPath = devPath

	return updateSpecDeviceList(device, spec)
}

// device.Id should be the SCSI address of the disk in the format "scsiID:lunID"
func virtioSCSIDeviceHandler(ctx context.Context, device pb.Device, spec *pb.Spec, s *sandbox) error {
	// Retrieve the device path from SCSI address.
	devPath, err := getSCSIDevPath(s, device.Id)
	if err != nil {
		return err
	}
	device.VmPath = devPath

	return updateSpecDeviceList(device, spec)
}

func nvdimmDeviceHandler(_ context.Context, device pb.Device, spec *pb.Spec, s *sandbox) error {
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

// scanSCSIBus scans SCSI bus for the given SCSI address(SCSI-Id and LUN)
func scanSCSIBusImpl(scsiAddr string) error {
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

// getSCSIDevPathImpl scans SCSI bus looking for the provided SCSI address, then
// it waits for the SCSI disk to become available and returns the device path
// associated with the disk.
func getSCSIDevPathImpl(s *sandbox, scsiAddr string) (string, error) {
	if err := scanSCSIBus(scsiAddr); err != nil {
		return "", err
	}

	devPath := filepath.Join(scsiHostChannel+scsiAddr, scsiBlockSuffix)

	return getDeviceName(s, devPath)
}

func getPmemDevPathImpl(s *sandbox, devPmemPath string) (string, error) {
	// for example: /block/pmem1
	devPath := filepath.Join("/", scsiBlockSuffix, filepath.Base(devPmemPath))

	return getDeviceName(s, devPath)
}

// checkCCWBusFormat checks the format for the ccw bus. It needs to be in the form 0.<n>.<dddd>
// n is the subchannel set ID - integer from 0 up to 3
// dddd is the device id - integer in hex up to 0xffff
// See https://www.ibm.com/support/knowledgecenter/en/linuxonibm/com.ibm.linux.z.ldva/ldva_r_XML_Address.html
func checkCCWBusFormat(bus string) error {
	busFormat := strings.Split(bus, ".")
	if len(busFormat) != 3 {
		return fmt.Errorf("Wrong bus format. It needs to be in the form 0.<n>.<dddd>, got %s", bus)
	}

	bus0, err := strconv.ParseInt(busFormat[0], 10, 32)
	if err != nil {
		return err
	}
	if bus0 != 0 {
		return fmt.Errorf("Wrong bus format. First digit needs to be 0 instead is %d", bus0)
	}

	bus1, err := strconv.ParseInt(busFormat[1], 10, 32)
	if err != nil {
		return err
	}
	if bus1 > maxDeviceIDValue {
		return fmt.Errorf("Wrong bus format. Second digit must be lower than %d instead is %d", maxDeviceIDValue, bus1)
	}

	if len(busFormat[2]) != 4 {
		return fmt.Errorf("Wrong bus format. Third digit must be in the form <dddd>, got %s", bus)
	}
	busFormat[2] = "0x" + busFormat[2]
	_, err = strconv.ParseInt(busFormat[2], 0, 32)
	if err != nil {
		return err
	}

	return nil
}

// getBlkCCWDevPath returns the CCW block path based on the bus ID
func getBlkCCWDevPath(s *sandbox, bus string) (string, error) {
	if err := checkCCWBusFormat(bus); err != nil {
		return "", err
	}

	return getDeviceName(s, path.Join(bus, blkCCWSuffix))
}

func addDevices(ctx context.Context, devices []*pb.Device, spec *pb.Spec, s *sandbox) error {
	for _, device := range devices {
		if device == nil {
			continue
		}

		err := addDevice(ctx, device, spec, s)
		if err != nil {
			return err
		}

	}

	return nil
}

func addDevice(ctx context.Context, device *pb.Device, spec *pb.Spec, s *sandbox) error {
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

	return devHandler(ctx, *device, spec, s)
}
