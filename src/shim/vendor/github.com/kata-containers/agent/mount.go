//
// Copyright (c) 2017-2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"syscall"

	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"
)

const (
	type9pFs       = "9p"
	typeVirtioFS   = "virtiofs"
	typeRootfs     = "rootfs"
	typeTmpFs      = "tmpfs"
	procMountStats = "/proc/self/mountstats"
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
	case type9pFs, typeVirtioFS:
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

func parseMountFlagsAndOptions(optionList []string) (int, string) {
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

	return flags, strings.Join(options, ",")
}

func parseOptions(optionList []string) map[string]string {
	options := make(map[string]string)
	for _, opt := range optionList {
		idx := strings.Index(opt, "=")
		if idx < 1 {
			continue
		}
		key, val := opt[:idx], opt[idx+1:]
		options[key] = val
	}
	return options
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
type storageHandler func(ctx context.Context, storage pb.Storage, s *sandbox) (string, error)

// storageHandlerList lists the supported drivers.
var storageHandlerList = map[string]storageHandler{
	driver9pType:        virtio9pStorageHandler,
	driverVirtioFSType:  virtioFSStorageHandler,
	driverBlkType:       virtioBlkStorageHandler,
	driverBlkCCWType:    virtioBlkCCWStorageHandler,
	driverMmioBlkType:   virtioMmioBlkStorageHandler,
	driverSCSIType:      virtioSCSIStorageHandler,
	driverEphemeralType: ephemeralStorageHandler,
	driverLocalType:     localStorageHandler,
	driverNvdimmType:    nvdimmStorageHandler,
}

func ephemeralStorageHandler(_ context.Context, storage pb.Storage, s *sandbox) (string, error) {
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

func localStorageHandler(_ context.Context, storage pb.Storage, s *sandbox) (string, error) {
	s.Lock()
	defer s.Unlock()
	newStorage := s.setSandboxStorage(storage.MountPoint)
	if newStorage {

		// Extract and parse the mode out of the storage options.
		// Default to os.ModePerm.
		opts := parseOptions(storage.Options)
		mode := os.ModePerm
		if val, ok := opts["mode"]; ok {
			m, err := strconv.ParseUint(val, 8, 32)
			if err != nil {
				return "", err
			}
			mode = os.FileMode(m)
		}

		if err := os.MkdirAll(storage.MountPoint, mode); err != nil {
			return "", err
		}

		// We chmod the permissions for the mount point, as we can't rely on os.MkdirAll to set the
		// desired permissions.
		return "", os.Chmod(storage.MountPoint, mode)
	}
	return "", nil
}

// virtio9pStorageHandler handles the storage for 9p driver.
func virtio9pStorageHandler(_ context.Context, storage pb.Storage, s *sandbox) (string, error) {
	return commonStorageHandler(storage)
}

// virtioMmioBlkStorageHandler handles the storage for mmio blk driver.
func virtioMmioBlkStorageHandler(_ context.Context, storage pb.Storage, s *sandbox) (string, error) {
	//The source path is VmPath
	return commonStorageHandler(storage)
}

// virtioBlkCCWStorageHandler handles the storage for blk ccw driver.
func virtioBlkCCWStorageHandler(ctx context.Context, storage pb.Storage, s *sandbox) (string, error) {
	devPath, err := getBlkCCWDevPath(s, storage.Source)
	if err != nil {
		return "", err
	}
	if devPath == "" {
		return "", grpcStatus.Errorf(codes.InvalidArgument,
			"Storage source is empty")
	}
	storage.Source = devPath
	return commonStorageHandler(storage)
}

// virtioFSStorageHandler handles the storage for virtio-fs.
func virtioFSStorageHandler(_ context.Context, storage pb.Storage, s *sandbox) (string, error) {
	return commonStorageHandler(storage)
}

// virtioBlkStorageHandler handles the storage for blk driver.
func virtioBlkStorageHandler(_ context.Context, storage pb.Storage, s *sandbox) (string, error) {

	// If hot-plugged, get the device node path based on the PCI address else
	// use the virt path provided in Storage Source
	if strings.HasPrefix(storage.Source, "/dev") {

		FileInfo, err := os.Stat(storage.Source)
		if err != nil {
			return "", err

		}
		// Make sure the virt path is valid
		if FileInfo.Mode()&os.ModeDevice == 0 {
			return "", fmt.Errorf("invalid device %s", storage.Source)
		}

	} else {
		devPath, err := getPCIDeviceName(s, storage.Source)
		if err != nil {
			return "", err
		}

		storage.Source = devPath
	}

	return commonStorageHandler(storage)
}

func nvdimmStorageHandler(_ context.Context, storage pb.Storage, s *sandbox) (string, error) {
	// waiting for a pmem device
	if strings.HasPrefix(storage.Source, "/dev") && strings.HasPrefix(filepath.Base(storage.Source), "pmem") {
		// Retrieve the device path from ACPI pmem address.
		// for example: /devices/LNXSYSTM:00/LNXSYBUS:00/ACPI0012:00/ndbus0/region1/pfn1.1/block/pmem1
		devPath, err := getPmemDevPath(s, storage.Source)
		if err != nil {
			return "", err
		}
		storage.Source = devPath
		return commonStorageHandler(storage)
	}

	return "", fmt.Errorf("invalid nvdimm source path: %v", storage.Source)
}

// virtioSCSIStorageHandler handles the storage for scsi driver.
func virtioSCSIStorageHandler(ctx context.Context, storage pb.Storage, s *sandbox) (string, error) {
	// Retrieve the device path from SCSI address.
	devPath, err := getSCSIDevPath(s, storage.Source)
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
	flags, options := parseMountFlagsAndOptions(storage.Options)

	return mount(storage.Source, storage.MountPoint, storage.Fstype, flags, options)
}

// addStorages takes a list of storages passed by the caller, and perform the
// associated operations such as waiting for the device to show up, and mount
// it to a specific location, according to the type of handler chosen, and for
// each storage.
func addStorages(ctx context.Context, storages []*pb.Storage, s *sandbox) (mounts []string, err error) {
	span, ctx := trace(ctx, "mount", "addStorages")
	span.setTag("sandbox", s.id)
	defer span.finish()

	var mountList []string
	var storageList []string

	defer func() {
		if err != nil {
			s.Lock()
			for _, path := range storageList {
				if err := s.unsetAndRemoveSandboxStorage(path); err != nil {
					agentLog.WithFields(logrus.Fields{
						"error": err,
						"path":  path,
					}).Error("failed to roll back addStorages")
				}
			}
			s.Unlock()
		}
	}()

	for _, storage := range storages {
		if storage == nil {
			continue
		}

		devHandler, ok := storageHandlerList[storage.Driver]
		if !ok {
			return nil, grpcStatus.Errorf(codes.InvalidArgument,
				"Unknown storage driver %q", storage.Driver)
		}

		// Wrap the span around the handler call to avoid modifying
		// the handler interface but also to avoid having to add trace
		// code to each driver.
		handlerSpan, _ := trace(ctx, "mount", storage.Driver)
		mountPoint, err := devHandler(ctx, *storage, s)
		handlerSpan.finish()

		if _, ok := s.storages[storage.MountPoint]; ok {
			storageList = append([]string{storage.MountPoint}, storageList...)
		}

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

// getMountFSType returns the FS type corresponding to the passed mount point and
// any error ecountered.
func getMountFSType(mountPoint string) (string, error) {
	if mountPoint == "" {
		return "", errors.Errorf("Invalid mount point '%s'", mountPoint)
	}

	mountstats, err := os.Open(procMountStats)
	if err != nil {
		return "", errors.Wrapf(err, "Failed to open file '%s'", procMountStats)
	}
	defer mountstats.Close()

	// Refer to fs/proc_namespace.c:show_vfsstat() for
	// the file format.
	re := regexp.MustCompile(fmt.Sprintf(`device .+ mounted on %s with fstype (.+)`, mountPoint))

	scanner := bufio.NewScanner(mountstats)
	for scanner.Scan() {
		line := scanner.Text()
		matches := re.FindStringSubmatch(line)
		if len(matches) > 1 {
			return matches[1], nil
		}
	}

	if err := scanner.Err(); err != nil {
		return "", errors.Wrapf(err, "Failed to parse proc mount stats file %s", procMountStats)
	}

	return "", errors.Errorf("Failed to find FS type for mount point '%s'", mountPoint)
}
