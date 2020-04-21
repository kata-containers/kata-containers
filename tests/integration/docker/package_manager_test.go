// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"os"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

const (
	packageManagerTimeout  = 900
	packageManagerMaxTries = 5
)

func tryPackageManagerCommand(container string, command []string, expectedExitCode int) int {
	cmd := []string{container}
	exitCode := int(-1)
	for i := 0; i < packageManagerMaxTries; i++ {
		_, _, exitCode = runDockerCommandWithTimeout(packageManagerTimeout, "exec", append(cmd, command...)...)
		if exitCode == expectedExitCode {
			break
		}
	}
	return exitCode
}

var _ = Describe("[Serial Test] package manager update test", func() {
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

	Context("check apt-get update and upgrade", func() {
		It("should not fail", func() {
			args = append(args, "-td", "--name", id, DebianImage, "sh")
			_, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())

			exitCode = tryPackageManagerCommand(id, []string{"apt-get", "-y", "update"}, 0)
			Expect(exitCode).To(BeZero())

			exitCode = tryPackageManagerCommand(id, []string{"apt-get", "-y", "upgrade"}, 0)
			Expect(exitCode).To(BeZero())

			Expect(RemoveDockerContainer(id)).To(BeTrue())
		})
	})

	Context("check dnf update", func() {
		It("should not fail", func() {
			if KataConfig.Hypervisor[KataHypervisor].SharedFS == "virtio-fs" {
				Skip("Skip issue: https://github.com/kata-containers/tests/issues/2008")
			}

			// This Fedora version is used mainly because of https://github.com/kata-containers/tests/issues/2358
			args = append(args, "-td", "--name", id, Fedora30Image, "sh")
			_, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())

			if proxyValue != "" {
				_, _, exitCode = dockerExec(id, "sed", "-i", fmt.Sprintf("$ a proxy=%s", proxyValue), "/etc/dnf/dnf.conf")
				Expect(exitCode).To(BeZero())
			}

			exitCode = tryPackageManagerCommand(id, []string{"dnf", "-y", "update"}, 0)
			Expect(exitCode).To(BeZero())

			Expect(RemoveDockerContainer(id)).To(BeTrue())
		})
	})

	Context("check yum update", func() {
		It("should not fail", func() {
			if KataConfig.Hypervisor[KataHypervisor].SharedFS == "virtio-fs" {
				Skip("Skip issue: https://github.com/kata-containers/tests/issues/2008")
			}
			args = append(args, "--rm", "-td", "--name", id, CentosImage, "sh")
			_, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())

			exitCode = tryPackageManagerCommand(id, []string{"yum", "-y", "update"}, 0)
			Expect(exitCode).To(BeZero())

			Expect(RemoveDockerContainer(id)).To(BeTrue())
		})
	})
})
