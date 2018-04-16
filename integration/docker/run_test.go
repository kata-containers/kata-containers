// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"os"
	"regexp"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

// number of loop devices to hotplug
var loopDevices = 10

func withWorkload(workload string, expectedExitCode int) TableEntry {
	return Entry(fmt.Sprintf("with '%v' as workload", workload), workload, expectedExitCode)
}

var _ = Describe("run", func() {
	var (
		args []string
		id   string
	)

	BeforeEach(func() {
		id = randomDockerName()
		args = []string{"--rm", "--name", id, Image, "sh", "-c"}
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("container with docker",
		func(workload string, expectedExitCode int) {
			args = append(args, workload)
			_, _, exitCode := dockerRun(args...)
			Expect(expectedExitCode).To(Equal(exitCode))
		},
		withWorkload("true", 0),
		withWorkload("false", 1),
		withWorkload("exit 0", 0),
		withWorkload("exit 1", 1),
		withWorkload("exit 15", 15),
		withWorkload("exit 123", 123),
	)
})

var _ = Describe("run", func() {
	var (
		args []string
		id   string
	)

	BeforeEach(func() {
		id = randomDockerName()
		args = []string{"--name", id}
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("container with docker",
		func(options, expectedStatus string) {
			args = append(args, options, Image, "sh")

			_, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())
			Expect(StatusDockerContainer(id)).To(Equal(expectedStatus))
			Expect(ExistDockerContainer(id)).To(BeTrue())
		},
		Entry("in background and interactive", "-di", "Up"),
		Entry("in background, interactive and with a tty", "-dit", "Up"),
	)
})

// creates a new disk file using 'dd' command, returns the path to disk file and
// its loop device representation
func createLoopDevice() (string, string, error) {
	f, err := ioutil.TempFile("", "dd")
	if err != nil {
		return "", "", err
	}
	defer f.Close()

	// create disk file
	ddArgs := []string{"if=/dev/zero", fmt.Sprintf("of=%s", f.Name()), "count=1", "bs=5M"}
	ddCmd := NewCommand("dd", ddArgs...)
	if _, stderr, exitCode := ddCmd.Run(); exitCode != 0 {
		return "", "", fmt.Errorf("%s", stderr)
	}

	// partitioning disk file
	fdiskArgs := []string{"-c", fmt.Sprintf(`printf "g\nn\n\n\n\nw\n" | fdisk %s`, f.Name())}
	fdiskCmd := NewCommand("bash", fdiskArgs...)
	if _, stderr, exitCode := fdiskCmd.Run(); exitCode != 0 {
		return "", "", fmt.Errorf("%s", stderr)
	}

	// create loop device
	losetupCmd := NewCommand("losetup", "-fP", f.Name())
	if _, stderr, exitCode := losetupCmd.Run(); exitCode != 0 {
		return "", "", fmt.Errorf("%s", stderr)
	}

	// get loop device path
	getLoopPath := NewCommand("losetup", "-j", f.Name())
	stdout, stderr, exitCode := getLoopPath.Run()
	if exitCode != 0 {
		return "", "", fmt.Errorf("exitCode: %d, stdout: %s, stderr: %s ", exitCode, stdout, stderr)
	}
	re := regexp.MustCompile("/dev/loop[0-9]+")
	loopPath := re.FindStringSubmatch(stdout)
	if len(loopPath) == 0 {
		return "", "", fmt.Errorf("Unable to get loop device path, stdout: %s, stderr: %s", stdout, stderr)
	}
	return f.Name(), loopPath[0], nil
}

func deleteLoopDevice(loopFile string) error {
	partxCmd := NewCommand("losetup", "-d", loopFile)
	_, stderr, exitCode := partxCmd.Run()
	if exitCode != 0 {
		return fmt.Errorf("%s", stderr)
	}

	return nil
}

var _ = Describe("run", func() {
	var (
		err        error
		diskFiles  []string
		diskFile   string
		loopFiles  []string
		loopFile   string
		dockerArgs []string
		id         string
	)

	BeforeEach(func() {
		if os.Getuid() != 0 {
			Skip("only root user can create loop devices")
		}
		id = RandID(30)

		for i := 0; i < loopDevices; i++ {
			diskFile, loopFile, err = createLoopDevice()
			Expect(err).ToNot(HaveOccurred())

			diskFiles = append(diskFiles, diskFile)
			loopFiles = append(loopFiles, loopFile)
			dockerArgs = append(dockerArgs, "--device", loopFile)
		}

		dockerArgs = append(dockerArgs, "--rm", "--name", id, Image, "stat")

		for _, lf := range loopFiles {
			dockerArgs = append(dockerArgs, lf)
		}
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
		for _, lf := range loopFiles {
			err = deleteLoopDevice(lf)
			Expect(err).ToNot(HaveOccurred())
		}
		for _, df := range diskFiles {
			err = os.Remove(df)
			Expect(err).ToNot(HaveOccurred())
		}
	})

	Context("hot plug block devices", func() {
		It("should be attached", func() {
			_, _, exitCode := dockerRun(dockerArgs...)
			Expect(exitCode).To(BeZero())
		})
	})
})

var _ = Describe("run", func() {
	var (
		args     []string
		id       string
		stderr   string
		stdout   string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("stdout using run", func() {
		It("should not display the output", func() {
			args = []string{"--rm", "--name", id, Image, "sh", "-c", "ls /etc/resolv.conf"}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring("/etc/resolv.conf"))
		})
	})

	Context("stderr using run", func() {
		It("should not display the output", func() {
			args = []string{"--rm", "--name", id, Image, "sh", "-c", "ls /etc/foo"}
			stdout, stderr, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(1))
			Expect(stdout).To(BeEmpty())
			Expect(stderr).To(ContainSubstring("ls: /etc/foo: No such file or directory"))
		})
	})

	Context("stdin using run", func() {
		It("should not display the stderr", func() {
			stdin := bytes.NewBufferString("hello")
			args = []string{"-i", "--rm", "--name", id, Image}
			_, stderr, exitCode = dockerRunWithPipe(stdin, args...)
			Expect(exitCode).NotTo(Equal(0))
			Expect(stderr).To(ContainSubstring("sh: hello: not found"))
		})
	})
})
