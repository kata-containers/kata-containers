// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("diff", func() {
	var (
		id   string
		name string = "FirstDirectory"
	)

	BeforeEach(func() {
		id = randomDockerName()
		// Run this command with -i flag to make sure we keep the
		// container up and running.
		_, _, exitCode := dockerRun("--name", id, "-d", "-i", Image, "sh")
		Expect(exitCode).To(Equal(0))
		_, _, exitCode = dockerExec(id, "mkdir", name)
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("inspect changes in a container", func() {
		It("should retrieve the change", func() {
			stdout, _, exitCode := dockerDiff(id)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(name))
		})
	})
})
