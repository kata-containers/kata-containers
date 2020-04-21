// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("docker top", func() {
	var (
		id       string
		stdout   string
		workload string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
		workload = "sleep 10"
		_, _, exitCode = dockerRun("--name", id, "-d", Image, "sh", "-c", workload)
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("check docker top functionality", func() {
		It("should print usage statement", func() {
			Skip("Issue: https://github.com/clearcontainers/runtime/issues/876")
			stdout, _, exitCode = dockerTop(id, "-x")
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(workload))
		})
	})
})
