// Copyright (c) 2021 Arm Ltd.
//
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

/*
#include <linux/kvm.h>
*/
import "C"

import (
	"github.com/sirupsen/logrus"
	"syscall"
)

// variables rather than consts to allow tests to modify them
var (
	kvmDevice      = "/dev/kvm"
	syscallSyscall = syscall.Syscall
	syscallOpen    = syscall.Open
	syscallClose   = syscall.Close
)

// This KVM_CAP_ARM_RME ABI is aligned with CCA/v8 patch sets.
// It will be changed to other number after the whole KVM CCA/RME
// support patches is merged to the upstream.
const KVM_ARM_CAP_RME_ID = 240

func availableGuestProtection() (guestProtection, error) {
	ret, err := checkKVMExtensionsRME()
	if err != nil {
		return noneProtection, err
	}
	if ret == true {
		return ccaProtection, nil
	} else {
		return noneProtection, nil
	}
}

// checkKVMExtensionsRME allows to query about the specific kvm extensions
func checkKVMExtensionsRME() (bool, error) {
	flags := syscall.O_RDWR | syscall.O_CLOEXEC
	kvm, err := syscallOpen(kvmDevice, flags, 0)
	if err != nil {
		return false, err
	}
	defer syscallClose(kvm)

	logger := hvLogger.WithFields(logrus.Fields{
		"type":        "kvm extension",
		"description": "Realm Management Extension",
		"id":          KVM_ARM_CAP_RME_ID,
	})
	ret, _, errno := syscallSyscall(syscall.SYS_IOCTL,
		uintptr(kvm),
		uintptr(C.KVM_CHECK_EXTENSION),
		uintptr(KVM_ARM_CAP_RME_ID))

	// Generally return value(ret) 0 means no and 1 means yes,
	// but some extensions may report additional information in the integer return value.
	if errno != 0 {
		logger.Error("is not supported")
		return false, errno
	}
	if int(ret) == 1 {
		return true, nil
	}
	return false, nil
}
