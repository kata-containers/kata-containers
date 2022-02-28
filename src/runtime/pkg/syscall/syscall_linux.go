// Copyright (c) 2022 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package syscall

import (
	"syscall"
)

func Gettid() int {
	return syscall.Gettid()
}
