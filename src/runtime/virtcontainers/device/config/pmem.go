// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package config

import (
	"fmt"
	"os"
	"syscall"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
)

const (
	// This signature is defined in the linux NVDIMM driver.
	// devices or backing files with this signature can be used
	// as pmem (persistent memory) devices in the guest.
	pfnSignature = "NVDIMM_PFN_INFO"

	// offset in the backing file where the signature should be
	pfnSignatureOffset = int64(4 * 1024)
)

var (
	pmemLog = logrus.WithField("source", "virtcontainers/device/config")
)

// SetLogger sets up a logger for this pkg
func SetLogger(logger *logrus.Entry) {
	fields := pmemLog.Data

	pmemLog = logger.WithFields(fields)
}

// PmemDeviceInfo returns a DeviceInfo if a loop device
// is mounted on source, and the backing file of the loop device
// has the PFN signature.
func PmemDeviceInfo(source, destination string) (*DeviceInfo, error) {
	stat := syscall.Stat_t{}
	err := syscall.Stat(source, &stat)
	if err != nil {
		return nil, err
	}

	// device object is still incomplete,
	// but it can be used to fetch the backing file
	device := &DeviceInfo{
		ContainerPath: destination,
		DevType:       "b",
		Major:         int64(unix.Major(stat.Dev)),
		Minor:         int64(unix.Minor(stat.Dev)),
		Pmem:          true,
		DriverOptions: make(map[string]string),
	}

	pmemLog.WithFields(
		logrus.Fields{
			"major": device.Major,
			"minor": device.Minor,
		}).Debug("looking for backing file")

	device.HostPath, err = getBackingFile(*device)
	if err != nil {
		return nil, err
	}

	pmemLog.WithField("backing-file", device.HostPath).
		Debug("backing file found: looking for PFN signature")

	if !hasPFNSignature(device.HostPath) {
		return nil, fmt.Errorf("backing file %v has not PFN signature", device.HostPath)
	}

	_, fstype, err := utils.GetDevicePathAndFsType(source)
	if err != nil {
		pmemLog.WithError(err).WithField("mount-point", source).Warn("failed to get fstype: using ext4")
		fstype = "ext4"
	}

	pmemLog.WithField("fstype", fstype).Debug("filesystem for mount point")
	device.DriverOptions["fstype"] = fstype

	return device, nil
}

// returns true if the file/device path has the PFN signature
// required to use it as PMEM device and enable DAX.
// See [1] to know more about the PFN signature.
//
// [1] - https://github.com/kata-containers/osbuilder/blob/master/image-builder/nsdax.gpl.c
func hasPFNSignature(path string) bool {
	f, err := os.Open(path)
	if err != nil {
		pmemLog.WithError(err).Error("Could not get PFN signature")
		return false
	}
	defer f.Close()

	signatureLen := len(pfnSignature)
	signature := make([]byte, signatureLen)

	l, err := f.ReadAt(signature, pfnSignatureOffset)
	if err != nil {
		pmemLog.WithError(err).Debug("Could not read pfn signature")
		return false
	}

	pmemLog.WithFields(logrus.Fields{
		"path":      path,
		"signature": string(signature),
	}).Debug("got signature")

	if l != signatureLen {
		pmemLog.WithField("read-bytes", l).Debug("Incomplete signature")
		return false
	}

	return pfnSignature == string(signature)
}
