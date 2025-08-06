// Copyright (c) 2024 Institute of Software, CAS.
//
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

// Guest protection is not available on RISC-V.
func availableGuestProtection() (guestProtection, error) {
	return noneProtection, nil
}
