//
// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const (
	type9pFs       = "9p"
	typeTmpFs      = "tmpfs"
	devPrefix      = "/dev/"
	timeoutHotplug = 3
	mountPerm      = os.FileMode(0755)
)

var flagList = map[string]int{
	"acl":         unix.MS_POSIXACL,
	"bind":        unix.MS_BIND,
	"defaults":    0,
	"dirsync":     unix.MS_DIRSYNC,
	"iversion":    unix.MS_I_VERSION,
	"lazytime":    unix.MS_LAZYTIME,
	"mand":        unix.MS_MANDLOCK,
	"noatime":     unix.MS_NOATIME,
	"nodev":       unix.MS_NODEV,
	"nodiratime":  unix.MS_NODIRATIME,
	"noexec":      unix.MS_NOEXEC,
	"nosuid":      unix.MS_NOSUID,
	"rbind":       unix.MS_BIND | unix.MS_REC,
	"relatime":    unix.MS_RELATIME,
	"remount":     unix.MS_REMOUNT,
	"ro":          unix.MS_RDONLY,
	"silent":      unix.MS_SILENT,
	"strictatime": unix.MS_STRICTATIME,
	"sync":        unix.MS_SYNCHRONOUS,
	"private":     unix.MS_PRIVATE,
	"shared":      unix.MS_SHARED,
	"slave":       unix.MS_SLAVE,
	"unbindable":  unix.MS_UNBINDABLE,
	"rprivate":    unix.MS_PRIVATE | unix.MS_REC,
	"rshared":     unix.MS_SHARED | unix.MS_REC,
	"rslave":      unix.MS_SLAVE | unix.MS_REC,
	"runbindable": unix.MS_UNBINDABLE | unix.MS_REC,
}

func createDestinationDir(dest string) error {
	targetPath, _ := filepath.Split(dest)

	return os.MkdirAll(targetPath, mountPerm)
}

// mount mounts a source in to a destination. This will do some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
func mount(source, destination, fsType string, flags int, options string) error {
	var absSource string

	// Log before validation. This is useful to debug cases where the gRPC
	// protocol version being used by the client is out-of-sync with the
	// agents version. gRPC message members are strictly ordered, so it's
	// quite possible that if the protocol changes, the client may
	// try to pass a valid mountpoint, but the gRPC layer may change that
	// through the member ordering to be a mount *option* for example.
	agentLog.WithFields(logrus.Fields{
		"mount-source":      source,
		"mount-destination": destination,
		"mount-fstype":      fsType,
		"mount-flags":       flags,
		"mount-options":     options,
	}).Debug()

	if source == "" {
		return fmt.Errorf("need mount source")
	}

	if destination == "" {
		return fmt.Errorf("need mount destination")
	}

	if fsType == "" {
		return fmt.Errorf("need mount FS type")
	}

	var err error
	switch fsType {
	case type9pFs:
		if err = createDestinationDir(destination); err != nil {
			return err
		}
		absSource = source
	case typeTmpFs:
		absSource = source
	default:
		absSource, err = filepath.EvalSymlinks(source)
		if err != nil {
			return grpcStatus.Errorf(codes.Internal, "Could not resolve symlink for source %v", source)
		}

		if err = ensureDestinationExists(absSource, destination, fsType); err != nil {
			return grpcStatus.Errorf(codes.Internal, "Could not create destination mount point: %v: %v",
				destination, err)
		}
	}

	if err = syscall.Mount(absSource, destination,
		fsType, uintptr(flags), options); err != nil {
		return grpcStatus.Errorf(codes.Internal, "Could not mount %v to %v: %v",
			absSource, destination, err)
	}

	return nil
}

// ensureDestinationExists will recursively create a given mountpoint. If directories
// are created, their permissions are initialized to mountPerm
func ensureDestinationExists(source, destination string, fsType string) error {
	fileInfo, err := os.Stat(source)
	if err != nil {
		return grpcStatus.Errorf(codes.Internal, "could not stat source location: %v",
			source)
	}

	if err := createDestinationDir(destination); err != nil {
		return grpcStatus.Errorf(codes.Internal, "could not create parent directory: %v",
			destination)
	}

	if fsType != "bind" || fileInfo.IsDir() {
		if err := os.Mkdir(destination, mountPerm); !os.IsExist(err) {
			return err
		}
	} else {
		file, err := os.OpenFile(destination, os.O_CREATE, mountPerm)
		if err != nil {
			return err
		}

		file.Close()
	}
	return nil
}

func parseMountFlagsAndOptions(optionList []string) (int, string, error) {
	var (
		flags   int
		options []string
	)

	for _, opt := range optionList {
		flag, ok := flagList[opt]
		if ok {
			flags |= flag
			continue
		}

		options = append(options, opt)
	}

	return flags, strings.Join(options, ","), nil
}

func removeMounts(mounts []string) error {
	for _, mount := range mounts {
		if err := syscall.Unmount(mount, 0); err != nil {
			return err
		}
	}

	return nil
}

// storageHandler is the type of callback to be defined to handle every
// type of storage driver.
type storageHandler func(storage pb.Storage, s *sandbox) (string, error)

// storageHandlerList lists the supported drivers.
var storageHandlerList = map[string]storageHandler{
	driver9pType:        virtio9pStorageHandler,
	driverBlkType:       virtioBlkStorageHandler,
	driverSCSIType:      virtioSCSIStorageHandler,
	driverEphemeralType: ephemeralStorageHandler,
}

func ephemeralStorageHandler(storage pb.Storage, s *sandbox) (string, error) {
	s.Lock()
	defer s.Unlock()
	newStorage := s.setSandboxStorage(storage.MountPoint)

	if newStorage {
		var err error
		if err = os.MkdirAll(storage.MountPoint, os.ModePerm); err == nil {
			_, err = commonStorageHandler(storage)
		}
		return "", err
	}
	return "", nil
}

// virtio9pStorageHandler handles the storage for 9p driver.
func virtio9pStorageHandler(storage pb.Storage, s *sandbox) (string, error) {
	return commonStorageHandler(storage)
}

// virtioBlkStorageHandler handles the storage for blk driver.
func virtioBlkStorageHandler(storage pb.Storage, s *sandbox) (string, error) {
	// Get the device node path based on the PCI address provided
	// in Storage Source
	devPath, err := getPCIDeviceName(s, storage.Source)
	if err != nil {
		return "", err
	}
	storage.Source = devPath

	return commonStorageHandler(storage)
}

// virtioSCSIStorageHandler handles the storage for scsi driver.
func virtioSCSIStorageHandler(storage pb.Storage, s *sandbox) (string, error) {
	// Retrieve the device path from SCSI address.
	devPath, err := getSCSIDevPath(storage.Source)
	if err != nil {
		return "", err
	}
	storage.Source = devPath

	return commonStorageHandler(storage)
}

func commonStorageHandler(storage pb.Storage) (string, error) {
	// Mount the storage device.
	if err := mountStorage(storage); err != nil {
		return "", err
	}

	return storage.MountPoint, nil
}

// mountStorage performs the mount described by the storage structure.
func mountStorage(storage pb.Storage) error {
	flags, options, err := parseMountFlagsAndOptions(storage.Options)
	if err != nil {
		return err
	}

	return mount(storage.Source, storage.MountPoint, storage.Fstype, flags, options)
}

// addStorages takes a list of storages passed by the caller, and perform the
// associated operations such as waiting for the device to show up, and mount
// it to a specific location, according to the type of handler chosen, and for
// each storage.
func addStorages(storages []*pb.Storage, s *sandbox) ([]string, error) {
	var mountList []string

	for _, storage := range storages {
		if storage == nil {
			continue
		}

		devHandler, ok := storageHandlerList[storage.Driver]
		if !ok {
			return nil, grpcStatus.Errorf(codes.InvalidArgument,
				"Unknown storage driver %q", storage.Driver)
		}

		mountPoint, err := devHandler(*storage, s)
		if err != nil {
			return nil, err
		}

		if mountPoint != "" {
			// Prepend mount point to mount list.
			mountList = append([]string{mountPoint}, mountList...)
		}
	}

	return mountList, nil
}
