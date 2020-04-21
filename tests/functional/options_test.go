// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package functional

import (
	"fmt"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

func withOption(option string, fail bool) TableEntry {
	return Entry(fmt.Sprintf("with option '%s'", option), option, fail)
}

var _ = Describe("global options", func() {
	DescribeTable("option",
		func(option string, fail bool) {
			command := NewCommand(Runtime, option)
			stdout, stderr, exitCode := command.Run()

			if fail {
				Expect(exitCode).NotTo(Equal(0))
				Expect(stderr).NotTo(BeEmpty())
				Expect(stdout).NotTo(BeEmpty())
			} else {
				Expect(exitCode).To(Equal(0))
				Expect(stderr).To(BeEmpty())
				Expect(stdout).NotTo(BeEmpty())
			}
		},
		withOption("--version", shouldNotFail),
		withOption("--v", shouldNotFail),
		withOption("--help", shouldNotFail),
		withOption("--h", shouldNotFail),
		withOption("--this-option-does-not-exist", shouldFail),
	)
})
