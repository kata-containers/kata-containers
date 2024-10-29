// Copyright (c) 2021 Arm Ltd.
//
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

/*
#include <linux/kvm.h>

const int KVM_CAP_ARM_RME_ID = KVM_CAP_ARM_RME;
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

// Guest protection is not supported on ARM64.
func availableGuestProtection() (guestProtection, error) {
	ret, err := checkKVMExtensionsRME()
	if err != nil {
		return noneProtection, err
	}
	if ret == true {
		return rmeProtection, nil
	} else {
		return noneProtection, nil
	}
}

// checkKVMExtensionsRME allows to query about the specific kvm extensions
// nolint: unused, deadcode
func checkKVMExtensionsRME() (bool, error) {
	flags := syscall.O_RDWR | syscall.O_CLOEXEC
	kvm, err := syscall.Open(kvmDevice, flags, 0)
	if err != nil {
		return false, err
	}
	defer syscall.Close(kvm)

	logger := hvLogger.WithFields(logrus.Fields{
		"type":        "kvm extension",
		"description": "Realm Management Extension",
		"id":          C.KVM_CAP_ARM_RME_ID,
	})
	ret, _, errno := syscall.Syscall(syscall.SYS_IOCTL,
		uintptr(kvm),
		uintptr(C.KVM_CHECK_EXTENSION),
		uintptr(C.KVM_CAP_ARM_RME_ID))

	// Generally return value(ret) 0 means no and 1 means yes,
	// but some extensions may report additional information in the integer return value.
	if errno != 0 {
		logger.Error("is not supported")
		return false, errno
	}
	if ret == 1 {
                return true, nil
	}
	return false, nil
}
