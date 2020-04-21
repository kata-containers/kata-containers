// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"syscall"

	. "github.com/onsi/ginkgo/extensions/table"
)

func withOSSignals(signalsMap map[syscall.Signal]bool) []TableEntry {
	return withGenericSignals(signalsMap)
}
