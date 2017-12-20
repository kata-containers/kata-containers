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
	"time"

	"github.com/kata-containers/agent/pkg/uevent"
	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/sirupsen/logrus"
	"golang.org/x/sys/unix"
)

const (
	type9pFs       = "9p"
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

// mount mounts a source in to a destination. This will do some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
func mount(source, destination, fsType string, flags int, options string) error {
	var absSource string

	if fsType != type9pFs {
		absSource, err := filepath.EvalSymlinks(source)
		if err != nil {
			return fmt.Errorf("Could not resolve symlink for source %v", source)
		}

		if err := ensureDestinationExists(absSource, destination, fsType); err != nil {
			return fmt.Errorf("Could not create destination mount point: %v: %v",
				destination, err)
		}
	} else {
		absSource = source
	}

	if err := syscall.Mount(absSource, destination,
		fsType, uintptr(flags), options); err != nil {
		return fmt.Errorf("Could not bind mount %v to %v: %v",
			absSource, destination, err)
	}

	return nil
}

// ensureDestinationExists will recursively create a given mountpoint. If directories
// are created, their permissions are initialized to mountPerm
func ensureDestinationExists(source, destination string, fsType string) error {
	fileInfo, err := os.Stat(source)
	if err != nil {
		return fmt.Errorf("could not stat source location: %v",
			source)
	}

	targetPathParent, _ := filepath.Split(destination)
	if err := os.MkdirAll(targetPathParent, mountPerm); err != nil {
		return fmt.Errorf("could not create parent directory: %v",
			targetPathParent)
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

func waitForDevice(devicePath string) error {
	deviceName := strings.TrimPrefix(devicePath, devPrefix)

	if _, err := os.Stat(devicePath); err == nil {
		return nil
	}

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

			if uEv.Action == "add" &&
				filepath.Base(uEv.DevPath) == deviceName {
				fieldLogger.Info("Hotplug event received")
				break
			}
		}

		close(done)
	}()

	select {
	case <-done:
	case <-time.After(time.Duration(timeoutHotplug) * time.Second):
		return fmt.Errorf("Timeout reached after %ds waiting for device %s",
			timeoutHotplug, deviceName)
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

func addMounts(mounts []*pb.Storage) ([]string, error) {
	var mountList []string

	for _, mnt := range mounts {
		if mnt == nil {
			continue
		}

		// Consider all other fs types as being hotpluggable, meaning
		// we should wait for them to show up before trying to mount
		// them.
		if mnt.Fstype != "" &&
			mnt.Fstype != "bind" &&
			mnt.Fstype != type9pFs {
			if err := waitForDevice(mnt.Source); err != nil {
				return nil, err
			}
		}

		flags, options, err := parseMountFlagsAndOptions(mnt.Options)
		if err != nil {
			return nil, err
		}

		if err := mount(mnt.Source, mnt.MountPoint, mnt.Fstype,
			flags, options); err != nil {
			return nil, err
		}

		// Prepend mount point to mount list.
		mountList = append([]string{mnt.MountPoint}, mountList...)
	}

	return mountList, nil
}

func removeMounts(mounts []string) error {
	for _, mount := range mounts {
		if err := syscall.Unmount(mount, 0); err != nil {
			return err
		}
	}

	return nil
}
