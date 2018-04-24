// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"os"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("package manager update test", func() {
	var (
		id         string
		args       []string
		proxyVar   string
		proxyValue string
	)

	BeforeEach(func() {
		id = randomDockerName()
		args = []string{}
		proxyVar = "http_proxy"
		proxyValue = os.Getenv(proxyVar)
		if proxyValue != "" {
			args = append(args, "-e", proxyVar+"="+proxyValue)
		}
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("check apt-get update", func() {
		It("should not fail", func() {
			args = append(args, "--rm", "--name", id, DebianImage, "apt-get", "-y", "update")
			_, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())
		})
	})

	Context("check dnf update", func() {
		It("should not fail", func() {
			Skip("Issue: https://github.com/clearcontainers/runtime/issues/868")
			args = append(args, "-td", "--name", id, FedoraImage, "sh")
			_, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())

			if proxyValue != "" {
				_, _, exitCode = dockerExec(id, "sed", "-i", fmt.Sprintf("$ a proxy=%s", proxyValue), "/etc/dnf/dnf.conf")
				Expect(exitCode).To(BeZero())
			}

			_, _, exitCode = dockerExec(id, "dnf", "-y", "update")
			Expect(exitCode).To(BeZero())

			Expect(RemoveDockerContainer(id)).To(BeTrue())
		})
	})

	Context("check yum update", func() {
		It("should not fail", func() {
			args = append(args, "--rm", "--name", id, CentosImage, "yum", "-y", "update")
			_, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())
		})
	})
})
