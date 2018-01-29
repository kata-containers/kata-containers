// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("docker search", func() {
	var (
		args []string
	)

	Context("search an image", func() {
		It("should filter the requests", func() {
			args = []string{"--filter", "is-official=true", "--filter=stars=3", Image}
			stdout, _, exitCode := dockerSearch(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(Image))
		})
	})
})
