// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

/*
#include <linux/kvm.h>

const int ioctl_KVM_CREATE_VM = KVM_CREATE_VM;
*/
import "C"

import (
	"syscall"

	"github.com/sirupsen/logrus"
)

// variables rather than consts to allow tests to modify them
var (
	kvmDevice = "/dev/kvm"
)

// archRequiredCPUFlags maps a CPU flag value to search for and a
// human-readable description of that value.
var archRequiredCPUFlags = map[string]string{
	"vmx":    "Virtualization support",
	"lm":     "64Bit CPU",
	"sse4_1": "SSE4.1",
}

// archRequiredCPUAttribs maps a CPU (non-CPU flag) attribute value to search for
// and a human-readable description of that value.
var archRequiredCPUAttribs = map[string]string{
	"GenuineIntel": "Intel Architecture CPU",
}

// archRequiredKernelModules maps a required module name to a human-readable
// description of the modules functionality and an optional list of
// required module parameters.
var archRequiredKernelModules = map[string]kernelModule{
	"kvm": {
		desc: "Kernel-based Virtual Machine",
	},
	"kvm_intel": {
		desc: "Intel KVM",
		parameters: map[string]string{
			"nested": "Y",
			// "VMX Unrestricted mode support". This is used
			// as a heuristic to determine if the system is
			// "new enough" to run a Kata Container
			// (atleast a Westmere).
			"unrestricted_guest": "Y",
		},
	},
	"vhost": {
		desc: "Host kernel accelerator for virtio",
	},
	"vhost_net": {
		desc: "Host kernel accelerator for virtio network",
	},
}

// kvmIsUsable determines if it will be possible to create a full virtual machine
// by creating a minimal VM and then deleting it.
func kvmIsUsable() error {
	flags := syscall.O_RDWR | syscall.O_CLOEXEC

	f, err := syscall.Open(kvmDevice, flags, 0)
	if err != nil {
		return err
	}
	defer syscall.Close(f)

	fieldLogger := kataLog.WithField("check-type", "full")

	fieldLogger.WithField("device", kvmDevice).Info("device available")

	vm, _, errno := syscall.Syscall(syscall.SYS_IOCTL,
		uintptr(f),
		uintptr(C.ioctl_KVM_CREATE_VM),
		0)
	if errno != 0 {
		if errno == syscall.EBUSY {
			fieldLogger.WithField("reason", "another hypervisor running").Error("cannot create VM")
		}

		return errno
	}
	defer syscall.Close(int(vm))

	fieldLogger.WithField("feature", "create-vm").Info("feature available")

	return nil
}

func archHostCanCreateVMContainer() error {
	return kvmIsUsable()
}

func archKernelParamHandler(onVMM bool, fields logrus.Fields, msg string) bool {
	param, ok := fields["parameter"].(string)
	if !ok {
		return false
	}

	// This option is not required when
	// already running under a hypervisor.
	if param == "unrestricted_guest" && onVMM {
		kataLog.WithFields(fields).Warn(kernelPropertyCorrect)
		return true
	}

	if param == "nested" {
		kataLog.WithFields(fields).Warn(msg)
		return true
	}

	// don't ignore the error
	return false
}
