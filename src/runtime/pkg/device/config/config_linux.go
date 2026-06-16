// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package config

import (
	"os"

	"golang.org/x/sys/unix"
)

// BlockDeviceIsReadOnly queries the host block device at path for its
// read-only flag (BLKROGET). This reflects the device's actual writability,
// which is the ground truth for whether the guest should see it read-only:
// when the host backing is read-only, writes from the guest fail at the host
// anyway, so the device must be exposed read-only (e.g. so the guest does not
// attempt a journal replay that cannot succeed). The read-only intent for such
// devices is frequently not carried in the OCI spec (no "ro" mount option and
// no read-only cgroup device rule), so the device flag is the only reliable
// source.
func BlockDeviceIsReadOnly(path string) (bool, error) {
	f, err := os.OpenFile(path, os.O_RDONLY|unix.O_CLOEXEC|unix.O_NONBLOCK, 0)
	if err != nil {
		return false, err
	}
	defer f.Close()

	ro, err := unix.IoctlGetInt(int(f.Fd()), unix.BLKROGET)
	if err != nil {
		return false, err
	}

	return ro != 0, nil
}
