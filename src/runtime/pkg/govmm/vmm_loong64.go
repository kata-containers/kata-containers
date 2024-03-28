//
// Copyright (c) 2023 Loongson Technology Corporation Limited
//
// SPDX-License-Identifier: Apache-2.0
//

package govmm

// MaxVCPUs returns the maximum number of vCPUs supported
// https://github.com/qemu/qemu/blob/v8.1.0-rc2/include/hw/loongarch/virt.h #L17
// #define LOONGARCH_MAX_CPUS      256
func MaxVCPUs() uint32 {
	return uint32(256)
}
