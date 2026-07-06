// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

//go:build ppc64le

package main

// KVM ioctl request numbers used by the host KVM capability checks.
//
// These were previously obtained from <linux/kvm.h> via cgo. They are
// declared here as plain Go constants so the runtime can be built without
// cgo (CGO_ENABLED=0), which is required to produce static binaries that
// run on libc-agnostic and musl-only hosts.
//
// powerpc uses the legacy (non asm-generic) ioctl encoding where
// _IOC_NONE == 1 and the direction field is shifted left by 29, so
// _IO(KVMIO, nr) with KVMIO == 0xAE is (1 << 29) | (0xAE << 8) | nr.
const (
	ioctlKVMCreateVM       = 0x2000AE01 // _IO(KVMIO, 0x01)
	ioctlKVMCheckExtension = 0x2000AE03 // _IO(KVMIO, 0x03)
)
