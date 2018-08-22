// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("vsock test", func() {
	var (
		args     []string
		name     string
		exitCode int
	)

	BeforeEach(func() {
		name = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(name)).To(BeTrue())
		Expect(ExistDockerContainer(name)).NotTo(BeTrue())
	})

	Context("when using vsock", func() {
		It("should not create a kata-proxy process", func() {
			if !KataConfig.Hypervisor[DefaultHypervisor].Vsock {
				Skip("Use of vsock not enabled")
			}
			args = []string{"--name", name, "-d", Image, "top"}
			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))

			ctrID, _, exitCode := dockerInspect("--format", "{{.Id}}", name)
			Expect(exitCode).To(Equal(0))

			// Check no kata-proxy process is running
			Expect(ProxyRunning(ctrID)).To(BeFalse())

		})
	})
})
