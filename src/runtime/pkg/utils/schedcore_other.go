// Copyright (c) 2023 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

//go:build !linux

package utils

import (
	"errors"
)

// Create a new sched core domain
func Create(t PidType) error {
	return errors.New("schedcore not available on non-Linux platforms")
}

// ShareFrom shares the sched core domain from the provided pid
func ShareFrom(pid uint64, t PidType) error {
	return errors.New("schedcore not available on non-Linux platforms")
}
