// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
)

// NewHypervisor returns an hypervisor from and hypervisor type.
func NewHypervisor(hType HypervisorType) (Hypervisor, error) {

	switch hType {
	case VirtframeworkHypervisor:
		return &virtFramework{}, nil
	case MockHypervisor:
		return &mockHypervisor{}, nil
	default:
		return nil, fmt.Errorf("Unknown hypervisor type %s", hType)
	}
}

func availableGuestProtection() (guestProtection, error) {
	return noneProtection, nil
}
