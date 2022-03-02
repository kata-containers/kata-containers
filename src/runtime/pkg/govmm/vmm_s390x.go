//
// Copyright (c) 2018 IBM
//
// SPDX-License-Identifier: Apache-2.0
//

package govmm

// MaxVCPUs returns the maximum number of vCPUs supported
func MaxVCPUs() uint32 {
	// Max number of virtual Cpu defined in qemu. See
	// https://github.com/qemu/qemu/blob/80422b00196a7af4c6efb628fae0ad8b644e98af/target/s390x/cpu.h#L55
	// #define S390_MAX_CPUS 248
	return uint32(248)
}
