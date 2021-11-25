// Copyright (c) 2021 Arm Ltd.
//
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

// Guest protection is not supported on ARM64.
func availableGuestProtection() (guestProtection, error) {
	return noneProtection, nil
}
