// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("docker commit", func() {
	var (
		id       string
		exitCode int
		stdout   string
		repoName string
	)

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode = dockerRun("-td", "--name", id, Image, "sh")
		Expect(exitCode).To(Equal(0))
		repoName = randomDockerRepoName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("commit a container with new configurations", func() {
		It("should have the new configurations", func() {
			_, _, exitCode = dockerCommit("-m", "test_commit", id, repoName)
			Expect(exitCode).To(Equal(0))

			stdout, _, exitCode = dockerImages()
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(repoName))

			_, _, exitCode = dockerRmi(repoName)
			Expect(exitCode).To(Equal(0))

			stdout, _, exitCode = dockerImages()
			Expect(exitCode).To(Equal(0))
			Expect(stdout).NotTo(ContainSubstring(repoName))
		})
	})
})
