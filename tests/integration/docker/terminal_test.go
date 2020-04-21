// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("terminal", func() {
	var (
		id string
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("terminal with docker", func() {
		Context("TERM env variable is set when allocating a tty", func() {
			It("should display the terminal's name", func() {
				stdout, _, exitCode := dockerRun("--name", id, "-t", Image, "env")
				Expect(exitCode).To(Equal(0))
				Expect(stdout).To(MatchRegexp("TERM=" + `[[:alnum:]]`))
			})
		})

		Context("TERM env variable is not set when not allocating a tty", func() {
			It("should not display the terminal's name", func() {
				stdout, _, exitCode := dockerRun("--name", id, Image, "env")
				Expect(exitCode).To(Equal(0))
				Expect(stdout).NotTo(ContainSubstring("TERM"))
			})
		})

		Context("Check that pseudo tty is setup properly when allocating a tty", func() {
			It("should display the pseudo tty's name", func() {
				stdout, _, exitCode := dockerRun("--name", id, "-t", Image, "tty")
				Expect(exitCode).To(Equal(0))
				Expect(stdout).To(MatchRegexp("/dev/pts/" + `[[:alnum:]]`))
			})
		})
	})
})
