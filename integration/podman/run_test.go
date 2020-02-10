// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package podman

import (
	"fmt"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

func withWorkload(workload string, expectedExitCode int) TableEntry {
	return Entry(fmt.Sprintf("with '%v' as workload", workload), workload, expectedExitCode)
}

var _ = Describe("run", func() {
	var (
		args []string
		id   string
	)

	BeforeEach(func() {
		id = randomPodmanName()
		args = []string{"--rm", "--name", id, Image, "sh", "-c"}
	})

	AfterEach(func() {
		Expect(ExistPodmanContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("container with podman",
		func(workload string, expectedExitCode int) {
			args = append(args, workload)
			_, _, exitCode := podmanRun(args...)
			Expect(expectedExitCode).To(Equal(exitCode))
		},
		withWorkload("true", 0),
		withWorkload("false", 1),
		withWorkload("exit 0", 0),
		withWorkload("exit 1", 1),
		withWorkload("exit 15", 15),
		withWorkload("exit 123", 123),
	)
})

var _ = Describe("run", func() {
	var (
		args []string
		id   string
	)

	BeforeEach(func() {
		id = randomPodmanName()
		args = []string{"--name", id}
	})

	AfterEach(func() {
		Expect(RemovePodmanContainer(id)).To(BeTrue())
		Expect(ExistPodmanContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("container with podman",
		func(options, expectedStatus string) {
			args = append(args, options, Image, "sh")

			_, _, exitCode := podmanRun(args...)
			Expect(exitCode).To(BeZero())
			Expect(StatusPodmanContainer(id)).To(Equal(expectedStatus))
			Expect(ExistPodmanContainer(id)).To(BeTrue())
		},
		Entry("in background and interactive", "-di", "Up"),
		Entry("in background, interactive and with a tty", "-dit", "Up"),
	)
})

var _ = Describe("run nonexistent command", func() {
	var (
		args     []string
		id       string
		exitCode int
	)

	BeforeEach(func() {
		id = randomPodmanName()
	})

	AfterEach(func() {
		Expect(ExistPodmanContainer(id)).NotTo(BeTrue())
	})

	Context("Running nonexistent command", func() {
		It("container and its components should not exist", func() {
			args = []string{"--rm", "--name", id, Image, "does-not-exist"}
			_, _, exitCode = podmanRun(args...)
			Expect(exitCode).NotTo(Equal(0))
		})
	})
})
