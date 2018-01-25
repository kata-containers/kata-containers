// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("docker env", func() {
	var (
		id       string
		hostname string
		stdout   string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("check that required env variables are set", func() {
		It("should have path, hostname, home", func() {
			hostname = "container"
			stdout, _, exitCode = dockerRun("--name", id, "-h", hostname, Image, "env")
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring("PATH"))
			Expect(stdout).To(ContainSubstring("HOME"))
			Expect(stdout).To(ContainSubstring("HOSTNAME=" + hostname))
		})
	})

	Context("set environment variables", func() {
		It("should have the environment variables", func() {
			envar := "ENVAR=VALUE_ENVAR"
			stdout, _, exitCode = dockerRun("-e", envar, "--name", id, Image, "env")
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(envar))
		})
	})
})
