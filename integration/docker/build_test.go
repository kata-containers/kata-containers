// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
	"os"
	"path/filepath"
)

const dockerFile = "src/github.com/kata-containers/tests/Dockerfiles/BuildTest/."

var _ = Describe("build", func() {
	var (
		args      []string
		id        string
		imageName string = "test"
		stdout    string
		exitCode  int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		_, _, exitCode = dockerRmi(imageName)
		Expect(exitCode).To(Equal(0))
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("build with docker", func() {
		Context("docker build env vars", func() {
			It("should display env vars", func() {
				gopath := os.Getenv("GOPATH")
				entirePath := filepath.Join(gopath, dockerFile)
				args = []string{"-t", imageName, entirePath}
				_, _, exitCode = dockerBuild(args...)
				Expect(exitCode).To(Equal(0))
				args = []string{"--rm", "-t", "--name", id, imageName, "sh", "-c", "'env'"}
				stdout, _, exitCode = dockerRun(args...)
				Expect(exitCode).To(Equal(0))
				Expect(stdout).To(ContainSubstring("test_env_vars"))
			})
		})
	})
})
