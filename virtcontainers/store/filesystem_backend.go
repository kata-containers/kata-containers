// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"

	opentracing "github.com/opentracing/opentracing-go"
	"github.com/sirupsen/logrus"
)

const (
	// ConfigurationFile is the file name used for every JSON sandbox configuration.
	ConfigurationFile string = "config.json"

	// StateFile is the file name storing a sandbox state.
	StateFile = "state.json"

	// NetworkFile is the file name storing a sandbox network.
	NetworkFile = "network.json"

	// HypervisorFile is the file name storing a hypervisor's state.
	HypervisorFile = "hypervisor.json"

	// AgentFile is the file name storing an agent's state.
	AgentFile = "agent.json"

	// ProcessFile is the file name storing a container process.
	ProcessFile = "process.json"

	// LockFile is the file name locking the usage of a resource.
	LockFile = "lock"

	// MountsFile is the file name storing a container's mount points.
	MountsFile = "mounts.json"

	// DevicesFile is the file name storing a container's devices.
	DevicesFile = "devices.json"
)

// DirMode is the permission bits used for creating a directory
const DirMode = os.FileMode(0750) | os.ModeDir

// StoragePathSuffix is the suffix used for all storage paths
//
// Note: this very brief path represents "virtcontainers". It is as
// terse as possible to minimise path length.
const StoragePathSuffix = "vc"

// SandboxPathSuffix is the suffix used for sandbox storage
const SandboxPathSuffix = "sbs"

// VMPathSuffix is the suffix used for guest VMs.
const VMPathSuffix = "vm"

// ConfigStoragePath is the sandbox configuration directory.
// It will contain one config.json file for each created sandbox.
var ConfigStoragePath = filepath.Join("/var/lib", StoragePathSuffix, SandboxPathSuffix)

// RunStoragePath is the sandbox runtime directory.
// It will contain one state.json and one lock file for each created sandbox.
var RunStoragePath = filepath.Join("/run", StoragePathSuffix, SandboxPathSuffix)

// RunVMStoragePath is the vm directory.
// It will contain all guest vm sockets and shared mountpoints.
var RunVMStoragePath = filepath.Join("/run", StoragePathSuffix, VMPathSuffix)

func itemToFile(item Item) (string, error) {
	switch item {
	case Configuration:
		return ConfigurationFile, nil
	case State:
		return StateFile, nil
	case Network:
		return NetworkFile, nil
	case Hypervisor:
		return HypervisorFile, nil
	case Agent:
		return AgentFile, nil
	case Process:
		return ProcessFile, nil
	case Lock:
		return LockFile, nil
	case Mounts:
		return MountsFile, nil
	case Devices, DeviceIDs:
		return DevicesFile, nil
	}

	return "", fmt.Errorf("Unknown item %s", item)
}

type filesystem struct {
	ctx context.Context

	path string
}

// Logger returns a logrus logger appropriate for logging Store filesystem messages
func (f *filesystem) logger() *logrus.Entry {
	return storeLog.WithFields(logrus.Fields{
		"subsystem": "store",
		"backend":   "filesystem",
		"path":      f.path,
	})
}

func (f *filesystem) trace(name string) (opentracing.Span, context.Context) {
	if f.ctx == nil {
		f.logger().WithField("type", "bug").Error("trace called before context set")
		f.ctx = context.Background()
	}

	span, ctx := opentracing.StartSpanFromContext(f.ctx, name)

	span.SetTag("subsystem", "store")
	span.SetTag("type", "filesystem")
	span.SetTag("path", f.path)

	return span, ctx
}

func (f *filesystem) itemToPath(item Item) (string, error) {
	fileName, err := itemToFile(item)
	if err != nil {
		return "", err
	}

	return filepath.Join(f.path, fileName), nil
}

func (f *filesystem) initialize() error {
	_, err := os.Stat(f.path)
	if err == nil {
		return nil
	}

	// Our root path does not exist, we need to create the initial layout:
	// The root directory and a lock file

	// Root directory
	if err := os.MkdirAll(f.path, DirMode); err != nil {
		return err
	}

	// Lock
	lockPath := filepath.Join(f.path, LockFile)

	lockFile, err := os.Create(lockPath)
	if err != nil {
		f.delete()
		return err
	}
	lockFile.Close()

	return nil
}

func (f *filesystem) new(ctx context.Context, path string, host string) error {
	f.ctx = ctx
	f.path = path

	f.logger().Debugf("New filesystem store backend for %s", path)

	return f.initialize()
}

func (f *filesystem) delete() error {
	return os.RemoveAll(f.path)
}

func (f *filesystem) load(item Item, data interface{}) error {
	span, _ := f.trace("load")
	defer span.Finish()

	span.SetTag("item", item)

	filePath, err := f.itemToPath(item)
	if err != nil {
		return err
	}

	fileData, err := ioutil.ReadFile(filePath)
	if err != nil {
		return err
	}

	if err := json.Unmarshal(fileData, data); err != nil {
		return err
	}

	return nil
}

func (f *filesystem) store(item Item, data interface{}) error {
	span, _ := f.trace("store")
	defer span.Finish()

	span.SetTag("item", item)

	filePath, err := f.itemToPath(item)
	if err != nil {
		return err
	}

	file, err := os.Create(filePath)
	if err != nil {
		return err
	}
	defer file.Close()

	jsonOut, err := json.Marshal(data)
	if err != nil {
		return fmt.Errorf("Could not marshall data: %s", err)
	}
	file.Write(jsonOut)

	return nil
}
