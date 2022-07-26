// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package utils

import (
	"fmt"
)

// Create a new sched core domain
func Create(t PidType) error {
	return fmt.Errorf("schedcore not available on Darwin")
}

// ShareFrom shares the sched core domain from the provided pid
func ShareFrom(pid uint64, t PidType) error {
	return fmt.Errorf("schedcore not available on Darwin")
}
