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
	"strings"

	"context"
)

// Machine describes the machine type qemu will emulate.
type Machine struct {
	// Type is the machine type to be used by qemu.
	Type string

	// Acceleration are the machine acceleration options to be used by qemu.
	Acceleration string
}

// Device describes a device to be created by qemu.
type Device struct {
	// Type is the qemu device type
	Type string

	// ID is the user defined device ID.
	ID string

	// MemDev is the device memory identifier.
	MemDev string

	// FSDev is the device filesystem identifier.
	FSDev string

	// MountTag is the device filesystem mount point tag.
	// It is only relevant when combined with FSDev.
	MountTag string

	// CharDev is the device character device identifier.
	CharDev string
}

// Object is a qemu object representation.
type Object struct {
	// Type is the qemu object type
	Type string

	// ID is the user defined object ID.
	ID string

	// MemPath is the object's memory path.
	// This is only relevant for memory objects
	MemPath string

	// Size is the object size in bytes
	Size uint64
}

// FSDevice represents a qemu filesystem configuration.
type FSDevice struct {
	// Type is the filesystem device type (e.g. "local")
	Type string

	// ID is the filesystem identifier.
	// It should match an existing Device ID.
	ID string

	// Path is the host root path for this filesystem.
	Path string

	// SecurityModel is the security model for this filesystem device.
	SecurityModel string
}

// RTC represents a qemu Real Time Clock configuration.
type RTC struct {
	// Base is the RTC start time.
	Base string

	// Clock is the is the RTC clock driver.
	Clock string

	// DriftFix is the drift fixing mechanism.
	DriftFix string
}

// QMPSocket represents a qemu QMP socket configuration.
type QMPSocket struct {
	// Type is the socket type (e.g. "unix").
	Type string

	// Name is the socket name.
	Name string

	// Server tells if this is a server socket.
	Server bool

	// NoWait tells if qemu should block waiting for a client to connect.
	NoWait bool
}

// SMP is the multi processors configuration structure.
type SMP struct {
	// CPUs is the number of VCPUs made available to qemu.
	CPUs uint32

	// Cores is the number of cores made available to qemu.
	Cores uint32

	// Threads is the number of threads made available to qemu.
	Threads uint32

	// Sockets is the number of sockets made available to qemu.
	Sockets uint32
}

// Memory is the guest memory configuration structure.
type Memory struct {
	// Size is the amount of memory made available to the guest.
	// It should be suffixed with M or G for sizes in megabytes or
	// gigabytes respectively.
	Size string

	// Slots is the amount of memory slots made available to the guest.
	Slots uint8

	// MaxMem is the maximum amount of memory that can be made available
	// to the guest through e.g. hot pluggable memory.
	MaxMem string
}

// Kernel is the guest kernel configuration structure.
type Kernel struct {
	// Path is the guest kernel path on the host filesystem.
	Path string

	// Params is the kernel parameters string.
	Params string
}

// Config is the qemu configuration structure.
// It allows for passing custom settings and parameters to the qemu API.
type Config struct {
	// Path is the qemu binary path.
	Path string

	// Ctx is not used at the moment.
	Ctx context.Context

	// Name is the qemu guest name
	Name string

	// UUID is the qemu process UUID.
	UUID string

	// CPUModel is the CPU model to be used by qemu.
	CPUModel string

	// Machine
	Machine Machine

	// QMPSocket is the QMP socket description.
	QMPSocket QMPSocket

	// Devices is a list of devices for qemu to create.
	Devices []Device

	// CharDevices is a list of character devices for qemu to export.
	CharDevices []string

	// Objects is a list of objects for qemu to create.
	Objects []Object

	// FilesystemDevices is a list of filesystem devices.
	FilesystemDevices []FSDevice

	// RTC is the qemu Real Time Clock configuration
	RTC RTC

	// VGA is the qemu VGA mode.
	VGA string

	// Kernel is the guest kernel configuration.
	Kernel Kernel

	// Memory is the guest memory configuration.
	Memory Memory

	// SMP is the quest multi processors configuration.
	SMP SMP

	// GlobalParam is the -global parameter
	GlobalParam string

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

func appendMachine(params []string, config Config) []string {
	if config.Machine.Type != "" {
		var machineParams []string

		machineParams = append(machineParams, config.Machine.Type)

		if config.Machine.Acceleration != "" {
			machineParams = append(machineParams, fmt.Sprintf(",accel=%s", config.Machine.Acceleration))
		}

		params = append(params, "-machine")
		params = append(params, strings.Join(machineParams, ""))
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

func appendQMPSocket(params []string, config Config) []string {
	if config.QMPSocket.Type != "" && config.QMPSocket.Name != "" {
		var qmpParams []string

		qmpParams = append(qmpParams, fmt.Sprintf("%s:", config.QMPSocket.Type))
		qmpParams = append(qmpParams, fmt.Sprintf("%s", config.QMPSocket.Name))
		if config.QMPSocket.Server == true {
			qmpParams = append(qmpParams, ",server")
			if config.QMPSocket.NoWait == true {
				qmpParams = append(qmpParams, ",nowait")
			}
		}

		params = append(params, "-qmp")
		params = append(params, strings.Join(qmpParams, ""))
	}

	return params
}

func appendDevices(params []string, config Config) []string {
	for _, d := range config.Devices {
		if d.Type != "" {
			var deviceParams []string

			deviceParams = append(deviceParams, fmt.Sprintf("%s", d.Type))

			if d.ID != "" {
				deviceParams = append(deviceParams, fmt.Sprintf(",id=%s", d.ID))
			}

			if d.MemDev != "" {
				deviceParams = append(deviceParams, fmt.Sprintf(",memdev=%s", d.MemDev))
			}

			if d.CharDev != "" {
				deviceParams = append(deviceParams, fmt.Sprintf(",chardev=%s", d.CharDev))
			}

			if d.FSDev != "" {
				deviceParams = append(deviceParams, fmt.Sprintf(",fsdev=%s", d.FSDev))

				if d.MountTag != "" {
					deviceParams = append(deviceParams, fmt.Sprintf(",mount_tag=%s", d.MountTag))
				}
			}

			params = append(params, "-device")
			params = append(params, strings.Join(deviceParams, ""))
		}
	}

	return params
}

func appendCharDevices(params []string, config Config) []string {
	for _, c := range config.CharDevices {
		params = append(params, "-chardev")
		params = append(params, c)
	}

	return params
}

func appendObjects(params []string, config Config) []string {
	for _, o := range config.Objects {
		if o.Type != "" {
			var objectParams []string

			objectParams = append(objectParams, o.Type)

			if o.ID != "" {
				objectParams = append(objectParams, fmt.Sprintf(",id=%s", o.ID))
			}

			if o.MemPath != "" {
				objectParams = append(objectParams, fmt.Sprintf(",mem-path=%s", o.MemPath))
			}

			if o.Size > 0 {
				objectParams = append(objectParams, fmt.Sprintf(",size=%d", o.Size))
			}

			params = append(params, "-object")
			params = append(params, strings.Join(objectParams, ""))
		}
	}

	return params
}

func appendFilesystemDevices(params []string, config Config) []string {
	for _, f := range config.FilesystemDevices {
		if f.Type != "" {
			var fsParams []string

			fsParams = append(fsParams, fmt.Sprintf("%s", f.Type))

			if f.ID != "" {
				fsParams = append(fsParams, fmt.Sprintf(",id=%s", f.ID))
			}

			if f.Path != "" {
				fsParams = append(fsParams, fmt.Sprintf(",path=%s", f.Path))
			}

			if f.SecurityModel != "" {
				fsParams = append(fsParams, fmt.Sprintf(",security-model=%s", f.SecurityModel))
			}

			params = append(params, "-fsdev")
			params = append(params, strings.Join(fsParams, ""))
		}
	}

	return params
}

func appendUUID(params []string, config Config) []string {
	if config.UUID != "" {
		params = append(params, "-uuid")
		params = append(params, config.UUID)
	}

	return params
}

func appendMemory(params []string, config Config) []string {
	if config.Memory.Size != "" {
		var memoryParams []string

		memoryParams = append(memoryParams, config.Memory.Size)

		if config.Memory.Slots > 0 {
			memoryParams = append(memoryParams, fmt.Sprintf(",slots=%d", config.Memory.Slots))
		}

		if config.Memory.MaxMem != "" {
			memoryParams = append(memoryParams, fmt.Sprintf(",maxmem=%s", config.Memory.MaxMem))
		}

		params = append(params, "-m")
		params = append(params, strings.Join(memoryParams, ""))
	}

	return params
}

func appendCPUs(params []string, config Config) []string {
	if config.SMP.CPUs > 0 {
		var SMPParams []string

		SMPParams = append(SMPParams, fmt.Sprintf("%d", config.SMP.CPUs))

		if config.SMP.Cores > 0 {
			SMPParams = append(SMPParams, fmt.Sprintf(",cores=%d", config.SMP.Cores))
		}

		if config.SMP.Threads > 0 {
			SMPParams = append(SMPParams, fmt.Sprintf(",threads=%d", config.SMP.Threads))
		}

		if config.SMP.Sockets > 0 {
			SMPParams = append(SMPParams, fmt.Sprintf(",sockets=%d", config.SMP.Sockets))
		}

		params = append(params, "-smp")
		params = append(params, strings.Join(SMPParams, ""))
	}

	return params
}

func appendRTC(params []string, config Config) []string {
	if config.RTC.Base != "" {
		var RTCParams []string

		RTCParams = append(RTCParams, fmt.Sprintf("base=%s", config.RTC.Base))

		if config.RTC.DriftFix != "" {
			RTCParams = append(RTCParams, fmt.Sprintf(",driftfix=%s", config.RTC.DriftFix))
		}

		if config.RTC.Clock != "" {
			RTCParams = append(RTCParams, fmt.Sprintf(",clock=%s", config.RTC.Clock))
		}

		params = append(params, "-rtc")
		params = append(params, strings.Join(RTCParams, ""))
	}

	return params
}

func appendGlobalParam(params []string, config Config) []string {
	if config.GlobalParam != "" {
		params = append(params, "-global")
		params = append(params, config.GlobalParam)
	}

	return params
}

func appendVGA(params []string, config Config) []string {
	if config.VGA != "" {
		params = append(params, "-vga")
		params = append(params, config.VGA)
	}

	return params
}

func appendKernel(params []string, config Config) []string {
	if config.Kernel.Path != "" {
		params = append(params, "-kernel")
		params = append(params, config.Kernel.Path)

		if config.Kernel.Params != "" {
			params = append(params, "-append")
			params = append(params, config.Kernel.Params)
		}
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
	params = appendUUID(params, config)
	params = appendMachine(params, config)
	params = appendCPUModel(params, config)
	params = appendQMPSocket(params, config)
	params = appendMemory(params, config)
	params = appendCPUs(params, config)
	params = appendDevices(params, config)
	params = appendCharDevices(params, config)
	params = appendFilesystemDevices(params, config)
	params = appendObjects(params, config)
	params = appendRTC(params, config)
	params = appendKernel(params, config)
	params = appendGlobalParam(params, config)
	params = appendVGA(params, config)

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
