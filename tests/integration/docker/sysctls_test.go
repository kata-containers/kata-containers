// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"strings"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("sysctls", func() {
	var (
		args     []string
		id       string
		stdout   string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("sysctls for fs", func() {
		It("should be applied", func() {
			fsValue := "512"
			args = []string{"--name", id, "--rm", "--sysctl", "fs.mqueue.queues_max=" + fsValue, Image, "cat", "/proc/sys/fs/mqueue/queues_max"}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(fsValue))
		})
	})

	Context("sysctls for kernel", func() {
		It("should be applied", func() {
			kernelValue := "1024"
			args = []string{"--name", id, "--rm", "--sysctl", "kernel.shmmax=" + kernelValue, Image, "cat", "/proc/sys/kernel/shmmax"}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(kernelValue))
		})
	})

	Context("sysctls for net", func() {
		It("should be applied", func() {
			pmtuValue := "1024"
			args = []string{"--name", id, "--rm", "--sysctl", "net.ipv4.route.min_pmtu=" + pmtuValue, Image, "cat", "/proc/sys/net/ipv4/route/min_pmtu"}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(pmtuValue))
		})
	})

	Context("sysctl for IP forwarding", func() {
		It("should be applied", func() {
			ipforwardValue := "1"
			args = []string{"--name", id, "--rm", "--sysctl", "net.ipv4.ip_forward=" + ipforwardValue, Image, "cat", "/proc/sys/net/ipv4/ip_forward"}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(strings.Trim(stdout, " \n\t")).To(Equal(ipforwardValue))
		})
	})
})
