// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("restart", func() {
	var (
		id       string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode = dockerRun("-td", "--name", id, Image, "sh")
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("restart with docker", func() {
		Context("restart a container", func() {
			It("should be running", func() {
				_, _, exitCode = dockerStop(id)
				Expect(exitCode).To(Equal(0))
				_, _, exitCode = dockerRestart(id)
				Expect(exitCode).To(Equal(0))
			})
		})
	})
})
