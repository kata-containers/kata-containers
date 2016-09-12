/*
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
*/

// Package qemu provides methods and types for launching and managing QEMU
// instances.  Instances can be launched with the LaunchQemu function and
// managed thereafter via QMPStart and the QMP object that this function
// returns.  To manage a qemu instance after it has been launched you need
// to pass the -qmp option during launch requesting the qemu instance to create
// a QMP unix domain manageent socket, e.g.,
// -qmp unix:/tmp/qmp-socket,server,nowait.  For more information see the
// example below.
package qemu

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"

	"context"
)

// Config is the qemu configuration structure.
// It allows for passing custom settings and parameters to the qemu API.
type Config struct {
	// Path is the qemu binary path.
	Path string

	// Ctx is not used at the moment.
	Ctx context.Context

	// Name is the qemu guest name
	Name string
	
	// MachineType is the machine type to be used by qemu.
	MachineType string

	// MachineTypeAcceleration are the machine acceleration option to be used by qemu.
	MachineTypeAcceleration string

	// CPUModel is the CPU model to be used by qemu.
	CPUModel string

	// ExtraParams is a slice of options to pass to qemu.
	ExtraParams []string

	// FDs is a list of open file descriptors to be passed to the spawned qemu process
	FDs []*os.File
}

func appendName(params []string, config Config) []string {
	if config.Name != "" {
		params = append(params, "-name")
		params = append(params, config.Name)
	}

	return params
}

func appendMachineParams(params []string, config Config) []string {
	if config.MachineType != "" && config.MachineTypeAcceleration != "" {
		params = append(params, "-machine")
		params = append(params, fmt.Sprintf("%s,accel=%s", config.MachineType, config.MachineTypeAcceleration))
	}

	return params
}

func appendCPUModel(params []string, config Config) []string {
	if config.CPUModel != "" {
		params = append(params, "-cpu")
		params = append(params, config.CPUModel)
	}

	return params
}

// LaunchQemu can be used to launch a new qemu instance.
//
// The Config parameter contains a set of qemu parameters and settings.
//
// This function writes its log output via logger parameter.
//
// The function will block until the launched qemu process exits.  "", nil
// will be returned if the launch succeeds.  Otherwise a string containing
// the contents of stderr + a Go error object will be returned.
func LaunchQemu(config Config, logger QMPLog) (string, error) {
	var params []string

	params = appendName(params, config)
	params = appendMachineParams(params, config)
	params = appendCPUModel(params, config)
	params = append(params, config.ExtraParams...)

	return LaunchCustomQemu(config.Ctx, config.Path, params, config.FDs, logger)
}

// LaunchCustomQemu can be used to launch a new qemu instance.
//
// The path parameter is used to pass the qemu executable path.
//
// The ctx parameter is not currently used but has been added so that the
// signature of this function will not need to change when launch cancellation
// is implemented.
//
// params is a slice of options to pass to qemu-system-x86_64 and fds is a
// list of open file descriptors that are to be passed to the spawned qemu
// process.
//
// This function writes its log output via logger parameter.
//
// The function will block until the launched qemu process exits.  "", nil
// will be returned if the launch succeeds.  Otherwise a string containing
// the contents of stderr + a Go error object will be returned.
func LaunchCustomQemu(ctx context.Context, path string, params []string, fds []*os.File, logger QMPLog) (string, error) {
	if logger == nil {
		logger = qmpNullLogger{}
	}

	errStr := ""

	if path == "" {
		path = "qemu-system-x86_64"
	}

	cmd := exec.Command(path, params...)
	if len(fds) > 0 {
		logger.Infof("Adding extra file %v", fds)
		cmd.ExtraFiles = fds
	}

	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	logger.Infof("launching qemu with: %v", params)

	err := cmd.Run()
	if err != nil {
		logger.Errorf("Unable to launch qemu: %v", err)
		errStr = stderr.String()
		logger.Errorf("%s", errStr)
	}
	return errStr, err
}
