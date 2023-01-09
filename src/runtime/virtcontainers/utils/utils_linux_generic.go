//go:build amd64 || arm64 || s390x || !ppc64le

// Copyright (c) 2019 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

// from <linux/vhost.h>
// VHOST_VSOCK_SET_GUEST_CID = _IOW(VHOST_VIRTIO, 0x60, __u64)
const ioctlVhostVsockSetGuestCid = 0x4008AF60
