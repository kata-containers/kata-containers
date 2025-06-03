// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import "os"

const (
	tdxKvmParameterPath = "/sys/module/kvm_intel/parameters/tdx"

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

	return noneProtection, nil
}
