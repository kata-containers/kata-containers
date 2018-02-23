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

func withoutOption(option string, fail bool) TableEntry {
	return Entry(fmt.Sprintf("without '%s' option", option), option, fail)
}

var _ = Describe("run", func() {
	var (
		container *Container
		err       error
	)

	BeforeEach(func() {
		container, err = NewContainer([]string{"true"}, false)
		Expect(err).NotTo(HaveOccurred())
		Expect(container).NotTo(BeNil())
	})

	AfterEach(func() {
		Expect(container.Teardown()).To(Succeed())
	})

	DescribeTable("container",
		func(option string, fail bool) {
			Expect(container.RemoveOption(option)).To(Succeed())
			_, stderr, exitCode := container.Run()

			if fail {
				Expect(exitCode).ToNot(Equal(0))
				Expect(stderr).NotTo(BeEmpty())
			} else {
				Expect(exitCode).To(Equal(0))
				Expect(stderr).To(BeEmpty())
			}
		},
		withoutOption("--bundle", shouldFail),
		withoutOption("-b", shouldFail),
	)
})
