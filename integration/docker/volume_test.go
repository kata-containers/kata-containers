// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"io/ioutil"
	"os"
	"path"

	"github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("[Serial Test] docker volume", func() {
	var (
		args          []string
		id            string = randomDockerName()
		id2           string = randomDockerName()
		volumeName    string = "cc3volume"
		containerPath string = "/attached_vol/"
		fileTest      string = "hello"
		exitCode      int
		stdout        string
		loopFile      string
		err           error
		diskFile      string
	)

	if os.Getuid() != 0 {
		GinkgoT().Skip("only root user can create files under /dev")
		return
	}

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

	Context("creating a text file under /dev", func() {
		It("should be passed the content to the container", func() {
			fileName := "/dev/foo"
			textContent := "hello"
			err = ioutil.WriteFile(fileName, []byte(textContent), 0644)
			Expect(err).ToNot(HaveOccurred())
			defer os.Remove(fileName)

			args = []string{"--name", id, "-v", fileName + ":" + fileName, Image, "cat", fileName}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring(textContent))

			Expect(RemoveDockerContainer(id)).To(BeTrue())
			Expect(ExistDockerContainer(id)).NotTo(BeTrue())
		})
	})

	Context("passing a block device", func() {
		It("should be mounted", func() {
			diskFile, loopFile, err = createLoopDevice()
			Expect(err).ToNot(HaveOccurred())

			loopFileP1 := fmt.Sprintf("%sp1", loopFile)
			mkfsCmd := tests.NewCommand("mkfs.ext4", loopFileP1)
			_, _, exitCode := mkfsCmd.Run()
			Expect(exitCode).To(Equal(0))
			Expect(err).ToNot(HaveOccurred())

			args = []string{"--name", id, "--cap-add=SYS_ADMIN", "--device", loopFileP1, "-v", loopFileP1 + ":" + loopFileP1, DebianImage, "bash", "-c", fmt.Sprintf("sleep 15; mount %s /mnt", loopFileP1)}
			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))

			err = deleteLoopDevice(loopFile)
			Expect(err).ToNot(HaveOccurred())

			err = os.Remove(diskFile)
			Expect(err).ToNot(HaveOccurred())

			Expect(RemoveDockerContainer(id)).To(BeTrue())
			Expect(ExistDockerContainer(id)).NotTo(BeTrue())
		})
	})

	Context("remove bind-mount source before container exits", func() {
		It("should exit cleanly without leaking process", func() {
			file, err := ioutil.TempFile(os.TempDir(), fileTest)
			Expect(err).ToNot(HaveOccurred())
			err = file.Close()
			Expect(err).ToNot(HaveOccurred())

			testFile := file.Name()
			Expect(testFile).To(BeAnExistingFile())

			args = []string{"--name", id, "-d", "-v", testFile + ":/volume_file", Image, "top"}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))

			// remove the test temp file before stop the container
			os.Remove(testFile)
			Expect(testFile).NotTo(BeAnExistingFile())

			// remove container
			Expect(RemoveDockerContainer(id)).To(BeTrue())
			Expect(ExistDockerContainer(id)).NotTo(BeTrue())
		})
	})
})
