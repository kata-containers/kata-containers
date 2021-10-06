// Copyright (c) 2021 Arm Ltd.
//
// SPDX-License-Identifier: Apache-2.0

package virtcontainers

//Returns pefProtection if the firmware directory exists
func availableGuestProtection() (guestProtection, error) {
	return noneProtection, nil
}
