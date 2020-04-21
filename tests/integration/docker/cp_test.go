// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"io/ioutil"
	"os"
	"path"

	"github.com/kata-containers/tests"
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

var _ = Describe("[Serial Test] docker cp with volume attached", func() {
	var (
		id          string
		exitCode    int
		hostPath    string
		cmd         *tests.Command
		dirBeforeCp string
		dirAfterCp  string
	)

	BeforeEach(func() {
		hostPath = "/dev"
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("check host path integrity", func() {
		It("should not be modified", func() {
			file, err := ioutil.TempFile(os.TempDir(), "file")
			Expect(err).ToNot(HaveOccurred())
			err = file.Close()
			Expect(err).ToNot(HaveOccurred())
			defer os.Remove(file.Name())
			Expect(file.Name()).To(BeAnExistingFile())

			// check hostPath before running docker cp
			cmd = tests.NewCommand("ls", hostPath)
			dirBeforeCp, _, exitCode = cmd.Run()
			Expect(exitCode).To(BeZero())

			_, _, exitCode = dockerRun("-td", "-v", hostPath+":"+hostPath, "--name", id, Image, "sh")
			Expect(exitCode).To(Equal(0))
			_, _, exitCode = dockerCp(file.Name(), id+":/")
			Expect(exitCode).To(BeZero())
			Expect(RemoveDockerContainer(id)).To(BeTrue())

			// check hostPath after running docker cp
			cmd = tests.NewCommand("ls", hostPath)
			dirAfterCp, _, exitCode = cmd.Run()
			Expect(exitCode).To(BeZero())

			// hostPath files and directories should be the same
			Expect(dirBeforeCp).To(Equal(dirAfterCp))
		})
	})
})

var _ = Describe("[Serial Test] docker cp with volume", func() {
	var (
		id            string
		exitCode      int
		hostPath      string
		cmd           *tests.Command
		mountBeforeCp string
		mountAfterCp  string
	)

	BeforeEach(func() {
		hostPath = "/dev"
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("check mount points", func() {
		It("should be removed", func() {
			file, err := ioutil.TempFile(os.TempDir(), "file")
			Expect(err).ToNot(HaveOccurred())
			err = file.Close()
			Expect(err).ToNot(HaveOccurred())
			defer os.Remove(file.Name())
			Expect(file.Name()).To(BeAnExistingFile())

			// check mount before cp
			cmd = tests.NewCommand("mount")
			mountBeforeCp, _, exitCode = cmd.Run()
			Expect(exitCode).To(BeZero())

			_, _, exitCode = dockerRun("-td", "-v", hostPath+":"+hostPath, "--name", id, Image, "sh")
			Expect(exitCode).To(BeZero())

			_, _, exitCode = dockerCp(file.Name(), id+":"+hostPath)
			Expect(exitCode).To(BeZero())

			// remove container
			Expect(RemoveDockerContainer(id)).To(BeTrue())

			// check mount points
			cmd = tests.NewCommand("mount")
			mountAfterCp, _, exitCode = cmd.Run()
			Expect(exitCode).To(BeZero())

			// check variables have the same content
			Expect(mountBeforeCp).To(Equal(mountAfterCp))
		})
	})
})
