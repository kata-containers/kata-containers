// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "os"

const (
	tdxKvmParameterPath = "/sys/module/kvm_intel/parameters/tdx"

	sevKvmParameterPath = "/sys/module/kvm_amd/parameters/sev"

	snpKvmParameterPath = "/sys/module/kvm_amd/parameters/sev_snp"
)

// Implementation of this function is architecture specific
func availableGuestProtection() (guestProtection, error) {
	// TDX is supported and enabled when the kvm module 'tdx' parameter is set to 'Y'
	if _, err := os.Stat(tdxKvmParameterPath); err == nil {
		if c, err := os.ReadFile(tdxKvmParameterPath); err == nil && len(c) > 0 && (c[0] == 'Y') {
			return tdxProtection, nil
		}
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
