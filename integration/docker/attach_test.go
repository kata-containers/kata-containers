// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("docker attach", func() {
	var (
		id                string
		exitCode          int
		containerExitCode int
		destroyTimeout    int
	)

	BeforeEach(func() {
		containerExitCode = 13
		destroyTimeout = 10
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("check attach functionality", func() {
		It("should attach exit code", func() {
			_, _, exitCode = dockerRun("--name", id, "-d", Image, "sh", "-c",
				fmt.Sprintf("sleep %d && exit %d", destroyTimeout, containerExitCode))
			Expect(exitCode).To(Equal(0))
			_, _, exitCode = dockerAttach(id)
			Expect(exitCode).To(Equal(containerExitCode))
		})
	})
})
