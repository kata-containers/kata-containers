// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"io/ioutil"
	"os"
	"path"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("docker cp", func() {
	var (
		id       string
		exitCode int
		stdout   string
	)

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode = dockerRun("-td", "--name", id, Image, "sh")
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("check files after a docker cp", func() {
		It("should have the corresponding files", func() {
			file, err := ioutil.TempFile(os.TempDir(), "file")
			Expect(err).ToNot(HaveOccurred())
			err = file.Close()
			Expect(err).ToNot(HaveOccurred())
			defer os.Remove(file.Name())
			Expect(file.Name()).To(BeAnExistingFile())

			_, _, exitCode = dockerCp(file.Name(), id+":/root/")
			Expect(exitCode).To(Equal(0))

			stdout, _, exitCode = dockerExec(id, "ls", "/root/")
			Expect(exitCode).To(Equal(0))
			testFile := path.Base(file.Name())
			Expect(stdout).To(ContainSubstring(testFile))
		})
	})
})
