//
// Copyright 2017 The Kubernetes Authors.
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"fmt"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"k8s.io/klog/v2"
	mountutils "k8s.io/mount-utils"
	utilexec "k8s.io/utils/exec"
)

const (
	// 'fsck' found errors and corrected them
	fsckErrorsCorrected = 1
	// 'fsck' found errors but exited without correcting them
	fsckErrorsUncorrected = 4
)

type SafeMountFormater struct {
	*mountutils.SafeFormatAndMount
}

func NewSafeMountFormater() SafeMountFormater {
	return SafeMountFormater{
		&mountutils.SafeFormatAndMount{
			Interface: mountutils.New(""),
			Exec:      utilexec.New(),
		},
	}
}

func (mounter *SafeMountFormater) IsNotSafeMountPoint(filePath string) (bool, error) {
	isMnt, err := mounter.IsMountPoint(filePath)
	if err != nil {
		return true, err
	}
	return !isMnt, nil
}

func (mounter *SafeMountFormater) DoBindmount(sourcePath, targetPath, fsType string, options []string) error {
	if err := mounter.Mount(sourcePath, targetPath, fsType, options); err != nil {
		errMsg := fmt.Sprintf("failed to mount device: %s at %s: %s", sourcePath, targetPath, err)
		return status.Error(codes.Aborted, errMsg)
	}

	return nil
}

// SafeFormatWithFstype uses unix utils to format disk
func (mounter *SafeMountFormater) SafeFormatWithFstype(source string, fstype string, options []string) error {
	readOnly := false
	for _, option := range options {
		if option == "ro" {
			readOnly = true
			break
		}
	}

	// Check if the disk is already formatted
	existingFormat, err := mounter.GetDiskFormat(source)
	if err != nil {
		return mountutils.NewMountError(mountutils.GetDiskFormatFailed, "failed to get disk format of disk %s: %v", source, err)
	}

	// Use 'ext4' as the default
	if len(fstype) == 0 {
		fstype = DefaultFsType
	}

	if existingFormat == "" {
		// Do not attempt to format the disk if mounting as readonly, return an error to reflect this.
		if readOnly {
			return mountutils.NewMountError(mountutils.UnformattedReadOnly, "cannot mount unformatted disk %s as it is in read-only mode", source)
		}

		// Disk is unformatted so format it.
		args := []string{source}
		if fstype == "ext4" || fstype == "ext3" {
			args = []string{
				"-F",  // Force flag
				"-m0", // Zero blocks reserved for super-user
				source,
			}
		}

		klog.Infof("Disk %q is unformatted, do format with type: %q and options: %v", source, fstype, args)
		mkfsCmd := fmt.Sprintf("mkfs.%s", fstype)
		if output, err := doSafeCommand(mkfsCmd, args...); err != nil {
			detailedErr := fmt.Sprintf("format disk %q failed: type:(%q) errcode:(%v) output:(%v) ", source, fstype, err, string(output))
			klog.Error(detailedErr)
			return mountutils.NewMountError(mountutils.FormatFailed, detailedErr)
		}

		klog.Infof("Disk successfully formatted (mkfs): %s - %s", fstype, source)
	} else {
		if fstype != existingFormat {
			// Do verify the disk formatted with expected fs type.
			return mountutils.NewMountError(mountutils.FilesystemMismatch, err.Error())
		}

		if !readOnly {
			// Run check tools on the disk to fix repairable issues, only do this for formatted volumes requested as rw.
			klog.V(4).Infof("Checking for issues with fsck on disk: %s", source)
			args := []string{"-a", source}
			if output, err := doSafeCommand("fsck", args...); err != nil {
				ee, isExitError := err.(utilexec.ExitError)
				switch {
				case err == utilexec.ErrExecutableNotFound:
					klog.Warningf("'fsck' not found on system; continuing mount without running 'fsck'.")
				case isExitError && ee.ExitStatus() == fsckErrorsCorrected:
					klog.Infof("Device %s has errors which were corrected by fsck.", source)
				case isExitError && ee.ExitStatus() == fsckErrorsUncorrected:
					return mountutils.NewMountError(mountutils.HasFilesystemErrors, "'fsck' found errors on device %s but could not correct them: %s", source, string(output))
				case isExitError && ee.ExitStatus() > fsckErrorsUncorrected:
					klog.Infof("`fsck` failed with error %v", string(output))
				default:
					klog.Warningf("fsck on device %s failed with error %v", source, err.Error())
				}
			}
		}
	}

	return nil
}
