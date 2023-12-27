//
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"

	diskfs "github.com/diskfs/go-diskfs"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"k8s.io/klog/v2"
	utilexec "k8s.io/utils/exec"
)

const (
	KataContainersDirectVolumeType = "katacontainers.direct.volume/volumetype"
	KataContainersDirectFsType     = "katacontainers.direct.volume/fstype"
	DirectVolumeTypeName           = "directvol"
	IsDirectVolume                 = "is_directvolume"
)

const (
	CapabilityInBytes = "capacity_in_bytes"
	DirectVolumeName  = "direct_volume_name"
)

const PERM os.FileMode = 0750
const DefaultFsType = "ext4"

const (
	KiB    int64 = 1024
	MiB    int64 = KiB * 1024
	GiB    int64 = MiB * 1024
	GiB100 int64 = GiB * 100
	TiB    int64 = GiB * 1024
	TiB100 int64 = TiB * 100
)

func AddDirectVolume(targetPath string, mountInfo MountInfo) error {
	mntArg, err := json.Marshal(&mountInfo)
	if err != nil {
		klog.Errorf("marshal mount info into bytes failed with error: %+v", err)
		return err
	}

	return Add(targetPath, string(mntArg))
}

func RemoveDirectVolume(targetPath string) error {
	return Remove(targetPath)
}

// storagePath/VolID/directvol.rawdisk
func SetupStoragePath(storagePath, volID string) (*string, error) {
	upperDir := filepath.Join(storagePath, volID)

	return MkPathIfNotExit(upperDir)
}

func MkPathIfNotExit(path string) (*string, error) {
	if exist, err := CheckPathExist(path); err != nil {
		return nil, errors.New("stat path failed")
	} else if !exist {
		if err := os.MkdirAll(path, PERM); err != nil {
			return nil, errors.New("mkdir all failed.")
		}
		klog.Infof("mkdir full path successfully")
	}

	return &path, nil
}

func MakeFullPath(path string) error {
	stat, err := os.Stat(path)
	if err != nil {
		if !errors.Is(err, os.ErrNotExist) {
			return errors.New("stat path failed with not exist")
		}
		if err := os.MkdirAll(path, PERM); err != nil {
			return errors.New("mkdir all failed.")
		}
	}

	if stat != nil && !stat.IsDir() {
		return errors.New("path should be a directory")
	}

	return nil
}

// IsPathEmpty is a simple check to determine if the specified directvolume directory
// is empty or not.
func IsPathEmpty(path string) (bool, error) {
	f, err := os.Open(path)
	if err != nil {
		return true, err
	}
	defer f.Close()

	_, err = f.Readdir(1)
	if err == io.EOF {
		return true, nil
	}
	return false, err
}

func CheckPathExist(path string) (bool, error) {
	if _, err := os.Stat(path); err != nil {
		if os.IsNotExist(err) {
			return false, nil
		} else {
			return false, err
		}
	}

	return true, nil
}

func CanDoBindmount(mounter *SafeMountFormater, targetPath string) (bool, error) {
	notMnt, err := mounter.IsNotSafeMountPoint(targetPath)
	if err != nil {
		if _, err = MkPathIfNotExit(targetPath); err != nil {
			return false, err
		} else {
			notMnt = true
		}
	}

	return notMnt, nil
}

func doSafeCommand(rawCmd string, args ...string) ([]byte, error) {
	executor := utilexec.New()

	path, err := executor.LookPath(rawCmd)
	if err == exec.ErrNotFound {
		return []byte{}, status.Error(codes.Internal, fmt.Sprintf("%s executable File not found in $PATH", rawCmd))
	}

	absCmdPath, err := filepath.Abs(path)
	if err != nil {
		return []byte{}, err
	}

	out, err := executor.Command(absCmdPath, args...).CombinedOutput()
	if err != nil {
		detailedErr := fmt.Sprintf("exec command %v failed with errcode:(%v)", rawCmd, err)
		klog.Errorf("do command: %v failed with %v", absCmdPath, detailedErr)
		return out, status.Error(codes.Internal, detailedErr)
	}

	return out, nil
}

// storagePath/VolID/directvol.rawdisk
func GetStoragePath(storagePath, volID string) (string, error) {
	upperPath := filepath.Join(storagePath, volID)

	return upperPath, nil
}

// createVolume create the directory for the direct volume.
// It returns the volume path or err if one occurs.
func CreateDirectBlockDevice(volID, capacityInBytesStr, storagePath string) (*string, error) {
	capacityInBytes, err := strconv.ParseInt(capacityInBytesStr, 10, 64)
	if err != nil {
		errMsg := status.Error(codes.Internal, err.Error())
		klog.Errorf("capacity in bytes convert to int failed with error: %v", errMsg)
		return nil, errMsg
	}

	diskSize := fmt.Sprintf("%dM", capacityInBytes/MiB)
	upperDir, err := SetupStoragePath(storagePath, volID)
	if err != nil {
		klog.Errorf("setup storage path failed with error: %v", err)
		return nil, err
	} else {
		// check the upper path for device exists.
		if _, err = os.Stat(*upperDir); err != nil && os.IsNotExist(err) {
			return nil, err
		}
	}

	// storagePath/62a268d9-893a-11ee-97cb-d89d6725e7b0/directvol-rawdisk.2048M
	devicePath := filepath.Join(*upperDir, fmt.Sprintf("directvol-rawdisk.%s", diskSize))
	if _, err = os.Stat(devicePath); !os.IsNotExist(err) {
		klog.Warning("direct block device exists, just skip creating it.")
		return &devicePath, nil
	}

	// create raw disk
	if _, err = diskfs.Create(devicePath, capacityInBytes, diskfs.Raw, diskfs.SectorSizeDefault); err != nil {
		errMsg := fmt.Errorf("diskfs create disk failed: %v", err)
		klog.Errorf(errMsg.Error())

		return nil, errMsg
	}

	// Create a block file.
	// storagePath/62a268d9-893a-11ee-97cb-d89d6725e7b0/directvol-rawdisk.2048M
	if _, err = os.Stat(devicePath); err != nil {
		return nil, err
	}

	// fallocate -z -l diskSize filePath
	fallocateCmd := "fallocate"
	// TODO: "-z" to be added
	args := []string{"-l", diskSize, devicePath}
	if _, err := doSafeCommand(fallocateCmd, args...); err != nil {
		klog.Infof("do fallocate %v failed with error(%v)", args, err)
		return nil, err
	}

	klog.Infof("create backend rawdisk successfully!")

	return &devicePath, nil
}
