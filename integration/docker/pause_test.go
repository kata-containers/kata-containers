// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("pause", func() {
	var id string

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("pause with docker", func() {
		Context("check pause functionality", func() {
			It("should not be running", func() {
				id = randomDockerName()
				_, _, exitCode := dockerRun("-td", "--name", id, Image, "sh")
				Expect(exitCode).To(Equal(0))
				_, _, exitCode = dockerPause(id)
				Expect(exitCode).To(Equal(0))
				stdout, _, exitCode := dockerPs("-a", "--filter", "status=paused", "--filter", "name="+id)
				Expect(exitCode).To(Equal(0))
				Expect(stdout).To(ContainSubstring("Paused"))
				_, _, exitCode = dockerUnpause(id)
				Expect(exitCode).To(Equal(0))
				stdout, _, exitCode = dockerPs("-a", "--filter", "status=running", "--filter", "name="+id)
				Expect(exitCode).To(Equal(0))
				Expect(stdout).To(ContainSubstring("Up"))
			})
		})
	})
})
