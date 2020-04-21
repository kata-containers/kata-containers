// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"io/ioutil"
	"os"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("docker privileges", func() {
	var (
		args      []string
		id        string
		secondID  string
		testImage string
		exitCode  int
	)

	BeforeEach(func() {
		id = randomDockerName()
		secondID = randomDockerName()
		testImage = "testprivileges"
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
		_, _, exitCode := dockerRmi(testImage)
		Expect(exitCode).To(Equal(0))
	})

	Context("check no-new-privileges flag", func() {
		It("should display the correct uid", func() {
			args = []string{"-d", "--name", id, FedoraImage, "sh", "-c", "chmod -s /usr/bin/id"}
			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))

			file, err := ioutil.TempFile(os.TempDir(), "latest.tar")
			Expect(err).ToNot(HaveOccurred())
			_, _, exitCode := dockerExport("--output", file.Name(), id)
			Expect(exitCode).To(Equal(0))
			Expect(file.Name()).To(BeAnExistingFile())

			_, _, exitCode = dockerImport(file.Name(), testImage)
			Expect(exitCode).To(Equal(0))
			defer os.Remove(file.Name())

			args = []string{"--rm", "--name", secondID, "--user", "1000", "--security-opt=no-new-privileges", testImage, "/usr/bin/id"}
			stdout, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).NotTo(ContainSubstring("euid=0(root)"))
			Expect(stdout).To(ContainSubstring("uid=1000"))
		})
	})
})
