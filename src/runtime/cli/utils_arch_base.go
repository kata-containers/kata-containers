// +build !s390x
//
// SPDX-License-Identifier: Apache-2.0
//

package main

func archConvertStatFs(cgroupFsType int) int64 {
	return int64(cgroupFsType)
}
