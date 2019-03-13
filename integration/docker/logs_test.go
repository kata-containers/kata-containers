// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("logs", func() {
	var id string

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode := dockerRun("-t", "--name", id, Image, "/bin/echo", "hello")
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("logs with docker", func() {
		Context("check logs functionality", func() {
			It("should work", func() {
				stdout, _ := LogsDockerContainer(id)
				Expect(stdout).To(ContainSubstring("hello"))
			})
		})
	})
})
