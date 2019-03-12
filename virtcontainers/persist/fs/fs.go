// Copyright (c) 2016 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package fs

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"syscall"

	persistapi "github.com/kata-containers/runtime/virtcontainers/persist/api"
	"github.com/sirupsen/logrus"
)

// persistFile is the file name for JSON sandbox/container configuration
const persistFile = "persist.json"

// dirMode is the permission bits used for creating a directory
const dirMode = os.FileMode(0700)

// fileMode is the permission bits used for creating a file
const fileMode = os.FileMode(0640)

// storagePathSuffix is the suffix used for all storage paths
//
// Note: this very brief path represents "virtcontainers". It is as
// terse as possible to minimise path length.
const storagePathSuffix = "vc"

// sandboxPathSuffix is the suffix used for sandbox storage
const sandboxPathSuffix = "sbs"

// runStoragePath is the sandbox runtime directory.
// It will contain one state.json and one lock file for each created sandbox.
var runStoragePath = filepath.Join("/run", storagePathSuffix, sandboxPathSuffix)

// FS storage driver implementation
type FS struct {
	sandboxState   *persistapi.SandboxState
	containerState map[string]persistapi.ContainerState
	setFuncs       map[string]persistapi.SetFunc

	lockFile *os.File
}

var fsLog = logrus.WithField("source", "virtcontainers/persist/fs")

// Logger returns a logrus logger appropriate for logging Store messages
func (fs *FS) Logger() *logrus.Entry {
	return fsLog.WithFields(logrus.Fields{
		"subsystem": "persist",
	})
}

// Name returns driver name
func Name() string {
	return "fs"
}

// Init FS persist driver and return abstract PersistDriver
func Init() (persistapi.PersistDriver, error) {
	return &FS{
		sandboxState:   &persistapi.SandboxState{},
		containerState: make(map[string]persistapi.ContainerState),
		setFuncs:       make(map[string]persistapi.SetFunc),
	}, nil
}

func (fs *FS) sandboxDir() (string, error) {
	id := fs.sandboxState.SandboxContainer
	if id == "" {
		return "", fmt.Errorf("sandbox container id required")
	}

	return filepath.Join(runStoragePath, id), nil
}

// ToDisk sandboxState and containerState to disk
func (fs *FS) ToDisk() (retErr error) {
	// call registered hooks to set sandboxState and containerState
	for _, fun := range fs.setFuncs {
		fun(fs.sandboxState, fs.containerState)
	}

	sandboxDir, err := fs.sandboxDir()
	if err != nil {
		return err
	}

	if err := os.MkdirAll(sandboxDir, dirMode); err != nil {
		return err
	}

	if err := fs.lock(); err != nil {
		if err1 := fs.Destroy(); err1 != nil {
			fs.Logger().WithError(err1).Errorf("failed to destroy dirs")
		}
		return err
	}
	defer fs.unlock()

	// if error happened, destroy all dirs
	defer func() {
		if retErr != nil {
			if err := fs.Destroy(); err != nil {
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

	// persist container configuration data
	for cid, cstate := range fs.containerState {
		cdir := filepath.Join(sandboxDir, cid)
		if err := os.MkdirAll(cdir, dirMode); err != nil {
			return err
		}

		cfile := filepath.Join(cdir, persistFile)
		cf, err := os.OpenFile(cfile, os.O_RDWR|os.O_CREATE|os.O_TRUNC, fileMode)
		if err != nil {
			return err
		}

		if err := json.NewEncoder(cf).Encode(cstate); err != nil {
			return err
		}
		cf.Close()
	}

	return nil
}

// FromDisk restores state for sandbox with name sid
func (fs *FS) FromDisk(sid string) error {
	if sid == "" {
		return fmt.Errorf("restore requires sandbox id")
	}

	fs.sandboxState.SandboxContainer = sid

	sandboxDir, err := fs.sandboxDir()
	if err != nil {
		return err
	}

	if err := fs.lock(); err != nil {
		return err
	}
	defer fs.unlock()

	// get sandbox configuration from persist data
	sandboxFile := filepath.Join(sandboxDir, persistFile)
	f, err := os.OpenFile(sandboxFile, os.O_RDONLY, fileMode)
	if err != nil {
		return err
	}
	defer f.Close()

	if err := json.NewDecoder(f).Decode(fs.sandboxState); err != nil {
		return err
	}

	// walk sandbox dir and find container
	files, err := ioutil.ReadDir(sandboxDir)
	if err != nil {
		return err
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
			return err
		}

		var cstate persistapi.ContainerState
		if err := json.NewDecoder(cf).Decode(&cstate); err != nil {
			return err
		}
		cf.Close()

		fs.containerState[cid] = cstate
	}
	return nil
}

// Destroy removes everything from disk
func (fs *FS) Destroy() error {
	sandboxDir, err := fs.sandboxDir()
	if err != nil {
		return err
	}

	if err := os.RemoveAll(sandboxDir); err != nil {
		return err
	}
	return nil
}

// GetStates returns SandboxState and ContainerState
func (fs *FS) GetStates() (*persistapi.SandboxState, map[string]persistapi.ContainerState, error) {
	return fs.sandboxState, fs.containerState, nil
}

// AddSaveCallback registers processing hooks for Dump
func (fs *FS) AddSaveCallback(name string, f persistapi.SetFunc) {
	// only accept last registered hook with same name
	fs.setFuncs[name] = f
}

func (fs *FS) lock() error {
	sandboxDir, err := fs.sandboxDir()
	if err != nil {
		return err
	}

	f, err := os.Open(sandboxDir)
	if err != nil {
		return err
	}

	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX|syscall.LOCK_NB); err != nil {
		f.Close()
		return err
	}
	fs.lockFile = f

	return nil
}

func (fs *FS) unlock() error {
	if fs.lockFile == nil {
		return nil
	}

	lockFile := fs.lockFile
	defer lockFile.Close()
	fs.lockFile = nil
	if err := syscall.Flock(int(lockFile.Fd()), syscall.LOCK_UN); err != nil {
		return err
	}

	return nil
}

// TestSetRunStoragePath set runStoragePath to path
// this function is only used for testing purpose
func TestSetRunStoragePath(path string) {
	runStoragePath = path
}
