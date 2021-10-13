// Copyright (c) 2019 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

//nolint:deadcode,unused
package utils

// from <linux/vhost.h>
// VHOST_VSOCK_SET_GUEST_CID = _IOW(VHOST_VIRTIO, 0x60, __u64)

// _IOC_WRITE is 1 for arch generic and 4 for powerpc
// Code reference: https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/arch/powerpc/include/uapi/asm/ioctl.h
// Explanation: https://github.com/kata-containers/runtime/pull/1989#issuecomment-525993135
const ioctlVhostVsockSetGuestCid = 0x8008AF60

func getIoctlVhostVsockGuestCid() uintptr {
	return ioctlVhostVsockSetGuestCid
}
