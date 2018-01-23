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

var _ = Describe("export", func() {
	var (
		id       string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode = dockerRun("-td", "--name", id, Image)
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("export with docker", func() {
		Context("export a container", func() {
			It("should export filesystem as a tar archive", func() {
				file, err := ioutil.TempFile(os.TempDir(), "latest.tar")
				Expect(err).ToNot(HaveOccurred())
				defer os.Remove(file.Name())
				_, _, exitCode = dockerExport("--output", file.Name(), id)
				Expect(exitCode).To(Equal(0))
				Expect(file.Name()).To(BeAnExistingFile())
				fileInfo, err := file.Stat()
				Expect(err).ToNot(HaveOccurred())
				Expect(fileInfo.Size).NotTo(Equal(0))
				err = file.Close()
				Expect(err).ToNot(HaveOccurred())
			})
		})
	})
})
