// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "os"

const (
	tdxSeamSysFirmwareDir = "/sys/firmware/tdx_seam/"

	tdxSysFirmwareDir = "/sys/firmware/tdx/"

	sevKvmParameterPath = "/sys/module/kvm_amd/parameters/sev"

	snpKvmParameterPath = "/sys/module/kvm_amd/parameters/sev_snp"
)

// TDX is supported and properly loaded when the firmware directory (either tdx or tdx_seam) exists or `tdx` is part of the CPU flag
func checkTdxGuestProtection(flags map[string]bool) bool {
	if d, err := os.Stat(tdxSysFirmwareDir); err == nil && d.IsDir() {
		return true
	}

	if d, err := os.Stat(tdxSeamSysFirmwareDir); err == nil && d.IsDir() {
		return true
	}

	return false
}

// Implementation of this function is architecture specific
func availableGuestProtection() (guestProtection, error) {
	flags, err := CPUFlags(procCPUInfo)
	if err != nil {
		return noneProtection, err
	}

	if checkTdxGuestProtection(flags) {
		return tdxProtection, nil
	}

	// SEV-SNP is supported and enabled when the kvm module `sev_snp` parameter is set to `Y`
	// SEV-SNP support infers SEV (-ES) support
	if _, err := os.Stat(snpKvmParameterPath); err == nil {
		if c, err := os.ReadFile(snpKvmParameterPath); err == nil && len(c) > 0 && (c[0] == 'Y') {
			return snpProtection, nil
		}
	}
	// SEV is supported and enabled when the kvm module `sev` parameter is set to `1` (or `Y` for linux >= 5.12)
	if _, err := os.Stat(sevKvmParameterPath); err == nil {
		if c, err := os.ReadFile(sevKvmParameterPath); err == nil && len(c) > 0 && (c[0] == '1' || c[0] == 'Y') {
			return sevProtection, nil
		}
	}

	return noneProtection, nil
}
