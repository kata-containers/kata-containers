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
	"syscall"

	"github.com/kata-containers/runtime/pkg/rootless"
	"github.com/kata-containers/runtime/virtcontainers/pkg/uuid"
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
// The function is declared this way for mocking in unit tests
var ConfigStoragePath = func() string {
	path := filepath.Join("/var/lib", StoragePathSuffix, SandboxPathSuffix)
	if rootless.IsRootless() {
		return filepath.Join(rootless.GetRootlessDir(), path)
	}
	return path
}

// RunStoragePath is the sandbox runtime directory.
// It will contain one state.json and one lock file for each created sandbox.
// The function is declared this way for mocking in unit tests
var RunStoragePath = func() string {
	path := filepath.Join("/run", StoragePathSuffix, SandboxPathSuffix)
	if rootless.IsRootless() {
		return filepath.Join(rootless.GetRootlessDir(), path)
	}
	return path
}

// RunVMStoragePath is the vm directory.
// It will contain all guest vm sockets and shared mountpoints.
// The function is declared this way for mocking in unit tests
var RunVMStoragePath = func() string {
	path := filepath.Join("/run", StoragePathSuffix, VMPathSuffix)
	if rootless.IsRootless() {
		return filepath.Join(rootless.GetRootlessDir(), path)
	}
	return path
}

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

	path    string
	rawPath string

	lockTokens map[string]*os.File
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
	// The root directory, a lock file and a raw files directory.

	// Root directory
	f.logger().WithField("path", f.path).Debugf("Creating root directory")
	if err := os.MkdirAll(f.path, DirMode); err != nil {
		return err
	}

	// Raw directory
	f.logger().WithField("path", f.rawPath).Debugf("Creating raw directory")
	if err := os.MkdirAll(f.rawPath, DirMode); err != nil {
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
	f.rawPath = filepath.Join(f.path, "raw")
	f.lockTokens = make(map[string]*os.File)

	f.logger().Debugf("New filesystem store backend for %s", path)

	return f.initialize()
}

func (f *filesystem) delete() error {
	f.logger().WithField("path", f.path).Debugf("Deleting files")
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

func (f *filesystem) raw(id string) (string, error) {
	span, _ := f.trace("raw")
	defer span.Finish()

	span.SetTag("id", id)

	// We must use the item ID.
	if id != "" {
		filePath := filepath.Join(f.rawPath, id)
		file, err := os.Create(filePath)
		if err != nil {
			return "", err
		}

		return filesystemScheme + "://" + file.Name(), nil
	}

	// Generate a random temporary file.
	file, err := ioutil.TempFile(f.rawPath, "raw-")
	if err != nil {
		return "", err
	}
	defer file.Close()

	return filesystemScheme + "://" + file.Name(), nil
}

func (f *filesystem) lock(item Item, exclusive bool) (string, error) {
	itemPath, err := f.itemToPath(item)
	if err != nil {
		return "", err
	}

	itemFile, err := os.Open(itemPath)
	if err != nil {
		return "", err
	}

	var lockType int
	if exclusive {
		lockType = syscall.LOCK_EX
	} else {
		lockType = syscall.LOCK_SH
	}

	if err := syscall.Flock(int(itemFile.Fd()), lockType); err != nil {
		itemFile.Close()
		return "", err
	}

	token := uuid.Generate().String()
	f.lockTokens[token] = itemFile

	return token, nil
}

func (f *filesystem) unlock(item Item, token string) error {
	itemFile := f.lockTokens[token]
	if itemFile == nil {
		return fmt.Errorf("No lock for token %s", token)
	}

	if err := syscall.Flock(int(itemFile.Fd()), syscall.LOCK_UN); err != nil {
		return err
	}

	itemFile.Close()
	delete(f.lockTokens, token)

	return nil
}
