// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "os"
import "io/ioutil"

// Implementation of this function is architecture specific
func availableGuestProtection() (guestProtection, error) {
	flags, err := CPUFlags(procCPUInfo)
	if err != nil {
		return noneProtection, err
	}

	// TDX is supported and properly loaded when the firmware directory exists or `tdx` is part of the CPU flags
	if d, err := os.Stat(tdxSysFirmwareDir); (err == nil && d.IsDir()) || flags[tdxCPUFlag] {
		return tdxProtection, nil
	}
	// SEV is supported and enabled when the kvm module `sev` parameter is set to `1`
	if _, err := os.Stat(sevKvmParameterPath); err == nil {
		if c, err := ioutil.ReadFile(sevKvmParameterPath); err == nil && len(c) > 0 && c[0] == '1' {
			return sevProtection, nil
		}
	}

	return noneProtection, nil
}
