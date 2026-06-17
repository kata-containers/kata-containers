// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

//go:build amd64 || arm64 || s390x || riscv64

package main

// KVM ioctl request numbers used by the host KVM capability checks.
//
// These were previously obtained from <linux/kvm.h> via cgo. They are
// declared here as plain Go constants so the runtime can be built without
// cgo (CGO_ENABLED=0), which is required to produce static binaries that
// run on libc-agnostic and musl-only hosts.
//
// On the architectures handled by this file the kernel uses the
// asm-generic ioctl encoding, so _IO(KVMIO, nr) with KVMIO == 0xAE is
// simply (0xAE << 8) | nr.
const (
	ioctlKVMCreateVM       = 0xAE01 // _IO(KVMIO, 0x01)
	ioctlKVMCheckExtension = 0xAE03 // _IO(KVMIO, 0x03)
)
