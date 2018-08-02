// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("port", func() {
	var (
		args []string
		id   string
	)

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode := dockerRun("-td", "-p", "50000:50000", "--name", id, Image)
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("port with docker", func() {
		Context("specify a port in a container", func() {
			It("should return assigned port", func() {
				args = []string{id, "50000/tcp"}
				stdout, _, _ := dockerPort(args...)
				Expect(stdout).To(ContainSubstring("50000"))
			})
		})
	})
})
