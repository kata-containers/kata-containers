// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package tests

import (
	"fmt"

	"github.com/onsi/ginkgo"
)

// LogIfFail will output the message online if the test fails. This can be used
// for information that would be useful to debug a failure.
func LogIfFail(format string, args ...interface{}) {
	str := fmt.Sprintf(format, args...)
	_, _ = ginkgo.GinkgoWriter.Write([]byte(str))
}
