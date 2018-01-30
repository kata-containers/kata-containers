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

var _ = Describe("docker volume", func() {
	var (
		args          []string
		id            string = randomDockerName()
		id2           string = randomDockerName()
		volumeName    string = "cc3volume"
		containerPath string = "/attached_vol/"
		fileTest      string = "hello"
		exitCode      int
		stdout        string
	)

	Context("create volume", func() {
		It("should display the volume's name", func() {
			_, _, exitCode = dockerVolume("create", "--name", volumeName)
			Expect(exitCode).To(Equal(0))
			_, _, exitCode = dockerVolume("inspect", volumeName)
			Expect(exitCode).To(Equal(0))
			_, _, exitCode = dockerVolume("rm", volumeName)
			Expect(exitCode).To(Equal(0))
			stdout, _, exitCode = dockerVolume("ls")
			Expect(exitCode).To(Equal(0))
			Expect(stdout).NotTo(ContainSubstring(volumeName))
		})
	})

	Context("use volume in a container", func() {
		It("should display the volume", func() {
			args = []string{"--name", id, "-t", "-v", volumeName + ":" + containerPath, Image, "touch", containerPath + fileTest}
			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))

			args = []string{"--name", id2, "-t", "-v", volumeName + ":" + containerPath, Image, "ls", containerPath}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(fileTest))

			Expect(RemoveDockerContainer(id)).To(BeTrue())
			Expect(ExistDockerContainer(id)).NotTo(BeTrue())
			Expect(RemoveDockerContainer(id2)).To(BeTrue())
			Expect(ExistDockerContainer(id2)).NotTo(BeTrue())

			_, _, exitCode = dockerVolume("rm", volumeName)
			Expect(exitCode).To(Equal(0))

			stdout, _, exitCode = dockerVolume("ls")
			Expect(exitCode).To(Equal(0))
			Expect(stdout).NotTo(ContainSubstring(volumeName))
		})
	})

	Context("volume bind-mount a directory", func() {
		It("should display directory's name", func() {
			file, err := ioutil.TempFile(os.TempDir(), fileTest)
			Expect(err).ToNot(HaveOccurred())
			err = file.Close()
			Expect(err).ToNot(HaveOccurred())
			defer os.Remove(file.Name())
			Expect(file.Name()).To(BeAnExistingFile())

			testFile := path.Base(file.Name())
			args = []string{"--name", id, "-v", testFile + ":/root/" + fileTest, Image, "ls", "/root/"}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(fileTest))

			Expect(RemoveDockerContainer(id)).To(BeTrue())
			Expect(ExistDockerContainer(id)).NotTo(BeTrue())

			_, _, exitCode = dockerVolume("rm", testFile)
			Expect(exitCode).To(Equal(0))

			stdout, _, exitCode = dockerVolume("ls")
			Expect(exitCode).To(Equal(0))
			Expect(stdout).NotTo(ContainSubstring(testFile))
		})
	})
})
