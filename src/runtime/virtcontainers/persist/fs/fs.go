// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package fs

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"syscall"

	"github.com/kata-containers/kata-containers/src/runtime/virtcontainers/utils"

	persistapi "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/persist/api"
	"github.com/sirupsen/logrus"
)

// persistFile is the file name for JSON sandbox/container configuration
const persistFile = "persist.json"

// dirMode is the permission bits used for creating a directory
const dirMode = os.FileMode(0700) | os.ModeDir

// fileMode is the permission bits used for creating a file
const fileMode = os.FileMode(0600)

// StoragePathSuffix is the suffix used for all storage paths
//
// Note: this very brief path represents "virtcontainers". It is as
// terse as possible to minimise path length.
const StoragePathSuffix = "vc"

// sandboxPathSuffix is the suffix used for sandbox storage
const sandboxPathSuffix = "sbs"

// vmPathSuffix is the suffix used for guest VMs.
const vmPathSuffix = "vm"

// FS storage driver implementation
type FS struct {
	sandboxState    *persistapi.SandboxState
	containerState  map[string]persistapi.ContainerState
	storageRootPath string
	driverName      string
}

var fsLog = logrus.WithField("source", "virtcontainers/persist/fs")

// Logger returns a logrus logger appropriate for logging Store messages
func (fs *FS) Logger() *logrus.Entry {
	return fsLog.WithFields(logrus.Fields{
		"subsystem": "persist",
		"driver":    fs.driverName,
	})
}

// Init FS persist driver and return abstract PersistDriver
func Init() (persistapi.PersistDriver, error) {
	return &FS{
		sandboxState:    &persistapi.SandboxState{},
		containerState:  make(map[string]persistapi.ContainerState),
		storageRootPath: filepath.Join("/run", StoragePathSuffix),
		driverName:      "fs",
	}, nil
}

func (fs *FS) sandboxDir(sandboxID string) (string, error) {
	return filepath.Join(fs.RunStoragePath(), sandboxID), nil
}

// ToDisk sandboxState and containerState to disk
func (fs *FS) ToDisk(ss persistapi.SandboxState, cs map[string]persistapi.ContainerState) (retErr error) {
	id := ss.SandboxContainer
	if id == "" {
		return fmt.Errorf("sandbox container id required")
	}

	fs.sandboxState = &ss
	fs.containerState = cs

	sandboxDir, err := fs.sandboxDir(id)
	if err != nil {
		return err
	}

	if err := utils.MkdirAllWithInheritedOwner(sandboxDir, dirMode); err != nil {
		return err
	}

	// if error happened, destroy all dirs
	defer func() {
		if retErr != nil {
			if err := fs.Destroy(id); err != nil {
				fs.Logger().WithError(err).Errorf("failed to destroy dirs")
			}
		}
	}()

	// persist sandbox configuration data
	sandboxFile := filepath.Join(sandboxDir, persistFile)
	f, err := os.OpenFile(sandboxFile, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, fileMode)
	if err != nil {
		return err
	}
	defer f.Close()

	if err := json.NewEncoder(f).Encode(fs.sandboxState); err != nil {
		return err
	}

	var dirCreationErr error
	var createdDirs []string
	defer func() {
		if dirCreationErr != nil && len(createdDirs) > 0 {
			for _, dir := range createdDirs {
				os.RemoveAll(dir)
			}
		}
	}()
	// persist container configuration data
	for cid, cstate := range fs.containerState {
		cdir := filepath.Join(sandboxDir, cid)
		if dirCreationErr = utils.MkdirAllWithInheritedOwner(cdir, dirMode); dirCreationErr != nil {
			return dirCreationErr
		}
		createdDirs = append(createdDirs, cdir)

		cfile := filepath.Join(cdir, persistFile)
		cf, err := os.OpenFile(cfile, os.O_RDWR|os.O_CREATE|os.O_TRUNC, fileMode)
		if err != nil {
			return err
		}

		defer cf.Close()
		if err := json.NewEncoder(cf).Encode(cstate); err != nil {
			return err
		}
	}

	// Walk sandbox dir and find container.
	files, err := os.ReadDir(sandboxDir)
	if err != nil {
		return err
	}

	// Remove non-existing containers
	for _, file := range files {
		if !file.IsDir() {
			continue
		}
		// Container dir exists.
		cid := file.Name()

		// Container should be removed when container id doesn't exist in cs.
		if _, ok := cs[cid]; !ok {
			if err := os.RemoveAll(filepath.Join(sandboxDir, cid)); err != nil {
				return err
			}
		}
	}
	return nil
}

// FromDisk restores state for sandbox with name sid
func (fs *FS) FromDisk(sid string) (persistapi.SandboxState, map[string]persistapi.ContainerState, error) {
	ss := persistapi.SandboxState{}
	if sid == "" {
		return ss, nil, fmt.Errorf("restore requires sandbox id")
	}

	sandboxDir, err := fs.sandboxDir(sid)
	if err != nil {
		return ss, nil, err
	}

	// get sandbox configuration from persist data
	sandboxFile := filepath.Join(sandboxDir, persistFile)
	f, err := os.OpenFile(sandboxFile, os.O_RDONLY, fileMode)
	if err != nil {
		return ss, nil, err
	}
	defer f.Close()

	if err := json.NewDecoder(f).Decode(fs.sandboxState); err != nil {
		return ss, nil, err
	}

	// walk sandbox dir and find container
	files, err := os.ReadDir(sandboxDir)
	if err != nil {
		return ss, nil, err
	}

	for _, file := range files {
		if !file.IsDir() {
			continue
		}

		cid := file.Name()
		cfile := filepath.Join(sandboxDir, cid, persistFile)
		cf, err := os.OpenFile(cfile, os.O_RDONLY, fileMode)
		if err != nil {
			// if persist.json doesn't exist, ignore and go to next
			if os.IsNotExist(err) {
				continue
			}
			return ss, nil, err
		}

		defer cf.Close()
		var cstate persistapi.ContainerState
		if err := json.NewDecoder(cf).Decode(&cstate); err != nil {
			return ss, nil, err
		}

		fs.containerState[cid] = cstate
	}

	return *fs.sandboxState, fs.containerState, nil
}

// Destroy removes everything from disk
func (fs *FS) Destroy(sandboxID string) error {
	if sandboxID == "" {
		return fmt.Errorf("sandbox container id required")
	}

	sandboxDir, err := fs.sandboxDir(sandboxID)
	if err != nil {
		return err
	}

	if err := os.RemoveAll(sandboxDir); err != nil {
		return err
	}
	return nil
}

func (fs *FS) Lock(sandboxID string, exclusive bool) (func() error, error) {
	if sandboxID == "" {
		return nil, fmt.Errorf("sandbox container id required")
	}

	sandboxDir, err := fs.sandboxDir(sandboxID)
	if err != nil {
		return nil, err
	}

	f, err := os.Open(sandboxDir)
	if err != nil {
		return nil, err
	}

	var lockType int
	if exclusive {
		lockType = syscall.LOCK_EX
	} else {
		lockType = syscall.LOCK_SH
	}

	if err := syscall.Flock(int(f.Fd()), lockType); err != nil {
		f.Close()
		return nil, err
	}

	unlockFunc := func() error {
		defer f.Close()
		if err := syscall.Flock(int(f.Fd()), syscall.LOCK_UN); err != nil {
			return err
		}

		return nil
	}
	return unlockFunc, nil
}

func (fs *FS) GlobalWrite(relativePath string, data []byte) error {
	path := filepath.Join(fs.storageRootPath, relativePath)
	path, err := filepath.Abs(filepath.Clean(path))
	if err != nil {
		return fmt.Errorf("failed to find abs path for %q: %v", relativePath, err)
	}

	dir := filepath.Dir(path)

	_, err = os.Stat(dir)
	if os.IsNotExist(err) {
		if err := utils.MkdirAllWithInheritedOwner(dir, dirMode); err != nil {
			fs.Logger().WithError(err).WithField("directory", dir).Error("failed to create dir")
			return err
		}
	} else if err != nil {
		return err
	}

	f, err := os.OpenFile(path, os.O_RDWR|os.O_CREATE, fileMode)
	if err != nil {
		fs.Logger().WithError(err).WithField("file", path).Error("failed to open file for writing")
		return err
	}
	defer f.Close()

	if _, err := f.Write(data); err != nil {
		fs.Logger().WithError(err).WithField("file", path).Error("failed to write file")
		return err
	}
	return nil
}

func (fs *FS) GlobalRead(relativePath string) ([]byte, error) {
	path := filepath.Join(fs.storageRootPath, relativePath)
	path, err := filepath.Abs(filepath.Clean(path))
	if err != nil {
		return nil, fmt.Errorf("failed to find abs path for %q: %v", relativePath, err)
	}

	f, err := os.Open(path)
	if err != nil {
		fs.Logger().WithError(err).WithField("file", path).Error("failed to open file for reading")
		return nil, err
	}
	defer f.Close()

	data, err := io.ReadAll(f)
	if err != nil {
		fs.Logger().WithError(err).WithField("file", path).Error("failed to read file")
		return nil, err
	}
	return data, nil
}

func (fs *FS) RunStoragePath() string {
	return filepath.Join(fs.storageRootPath, sandboxPathSuffix)
}

func (fs *FS) RunVMStoragePath() string {
	return filepath.Join(fs.storageRootPath, vmPathSuffix)
}
