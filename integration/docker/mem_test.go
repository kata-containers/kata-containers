// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("memory constraints", func() {
	var (
		args     []string
		id       string
		memSize  string
		limSize  string
		stderr   string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("run container exceeding memory constraints", func() {
		It("should ran out of memory", func() {
			memSize = "256MB"
			limSize = "260M"
			args = []string{"--name", id, "--rm", "-m", memSize, StressImage, "-mem-total", limSize, "-mem-alloc-size", limSize}
			_, stderr, exitCode = dockerRun(args...)
			Expect(exitCode).NotTo(Equal(0))
			Expect(stderr).To(ContainSubstring("fatal error: runtime: out of memory"))
		})
	})
})
