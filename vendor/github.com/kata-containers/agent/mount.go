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
)

const (
	type9pFs       = "9p"
	devPrefix      = "/dev/"
	timeoutHotplug = 3
	mountPerm      = os.FileMode(0755)
)

// mount mounts a source in to a destination. This will do some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
func mount(source, destination, fsType string, flags int, options string) error {
	absSource, err := filepath.EvalSymlinks(source)
	if err != nil {
		return fmt.Errorf("Could not resolve symlink for source %v", source)
	}

	if err := ensureDestinationExists(absSource, destination, fsType); err != nil {
		return fmt.Errorf("Could not create destination mount point: %v: %v",
			destination, err)
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

		if err := mount(mnt.Source, mnt.MountPoint, mnt.Fstype,
			0, strings.Join(mnt.Options, ",")); err != nil {
			return nil, err
		}

		// Expect the protocol to be updated with a Flags parameter.
		/*
			if err := mount(mnt.Source, mnt.MountPoint, mnt.Fstype,
				mnt.Flags, strings.Join(mnt.Options, ",")); err != nil {
				return nil, err
			}
		*/

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
