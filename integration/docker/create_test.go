// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("docker create", func() {
	var (
		id       string
		exitCode int
		stdout   string
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("check create functionality", func() {
		It("create a container", func() {
			_, _, exitCode = dockerCreate("-t", "--name", id, Image)
			Expect(exitCode).To(Equal(0))

			stdout, _, exitCode = dockerPs("--filter", "status=created")
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(id))
		})
	})
})
