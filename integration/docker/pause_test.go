// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"strings"
	"time"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("pause", func() {
	var id string

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("pause with docker", func() {
		Context("check pause functionality", func() {
			It("should not be running", func() {
				id = randomDockerName()
				_, _, exitCode := dockerRun("-td", "--name", id, Image, "sh")
				Expect(exitCode).To(Equal(0))
				_, _, exitCode = dockerPause(id)
				Expect(exitCode).To(Equal(0))
				stdout, _, exitCode := dockerPs("-a", "--filter", "status=paused", "--filter", "name="+id)
				Expect(exitCode).To(Equal(0))
				Expect(stdout).To(ContainSubstring("Paused"))
				_, _, exitCode = dockerUnpause(id)
				Expect(exitCode).To(Equal(0))
				stdout, _, exitCode = dockerPs("-a", "--filter", "status=running", "--filter", "name="+id)
				Expect(exitCode).To(Equal(0))
				Expect(stdout).To(ContainSubstring("Up"))
			})
		})
	})
})

// To get more info about this test, see https://github.com/kata-containers/agent/issues/231
var _ = Describe("check yamux IO timeout", func() {
	var (
		id       string
		msg      string
		stdout   string
		exitCode int
		waitTime time.Duration
	)

	BeforeEach(func() {
		id = randomDockerName()
		msg = "Hi!"
		// By default in yamux keepalive time is 30s and connection timeout is 10s.
		// Wait 45s before unpausing and checking the container.
		waitTime = 45 * time.Second
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("pause, wait and unpause a container", func() {
		Context("check yamux IO connection", func() {
			It("should keep alive", func() {
				_, _, exitCode = dockerRun("-td", "--name", id, Image, "sh")
				Expect(0).To(Equal(exitCode))
				_, _, exitCode = dockerPause(id)
				Expect(0).To(Equal(exitCode))
				time.Sleep(waitTime)
				_, _, exitCode = dockerUnpause(id)
				Expect(0).To(Equal(exitCode))
				stdout, _, exitCode = dockerExec(id, "echo", msg)
				Expect(msg).To(Equal(strings.Trim(stdout, "\n\t ")))
			})
		})
	})
})

var _ = Describe("remove paused container", func() {
	var (
		id       string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("start, pause, remove container", func() {
		Context("check if a paused container can be removed", func() {
			It("should be removed", func() {
				_, _, exitCode = dockerRun("-td", "--name", id, Image, "sh")
				Expect(0).To(Equal(exitCode))
				_, _, exitCode = dockerPause(id)
				Expect(0).To(Equal(exitCode))
				_, _, exitCode = dockerRm("-f", id)
				Expect(0).To(Equal(exitCode))
			})
		})
	})
})
