// Copyright (c) 2017-2018 Intel Corporation
// Copyright (c) 2018 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//go:build !linux

package config

import "fmt"

// BlockDeviceIsReadOnly is only meaningful on Linux, where the BLKROGET ioctl
// is available. On other platforms it is a no-op stub so the package still
// builds (e.g. for the macOS CI), and callers treat the error as "unknown".
func BlockDeviceIsReadOnly(path string) (bool, error) {
	return false, fmt.Errorf("BlockDeviceIsReadOnly is not supported on this platform")
}
