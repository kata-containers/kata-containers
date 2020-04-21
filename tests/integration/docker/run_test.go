// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"strings"

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

func distroID() string {
	pathFile := "/etc/os-release"
	if _, err := os.Stat(pathFile); os.IsNotExist(err) {
		pathFile = "/usr/lib/os-release"
	}
	cmd := exec.Command("sh", "-c", fmt.Sprintf("source %s; echo -n $ID", pathFile))
	id, err := cmd.CombinedOutput()
	if err != nil {
		LogIfFail("couldn't find distro ID %s\n", err)
		return ""
	}
	return string(id)
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

	if os.Getuid() != 0 {
		GinkgoT().Skip("only root user can create loop devices")
		return
	}

	BeforeEach(func() {
		id = randomDockerName()

		for i := 0; i < loopDevices; i++ {
			diskFile, loopFile, err = createLoopDevice()
			Expect(err).ToNot(HaveOccurred())

			diskFiles = append(diskFiles, diskFile)
			loopFiles = append(loopFiles, loopFile)
			dockerArgs = append(dockerArgs, "--device", loopFile)
		}

		dockerArgs = append(dockerArgs, "--rm", "--name", id, Image, "stat")

		dockerArgs = append(dockerArgs, loopFiles...)
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

var _ = Describe("run nonexistent command", func() {
	var (
		args     []string
		id       string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("Running nonexistent command", func() {
		It("container and its components should not exist", func() {
			args = []string{"--rm", "--name", id, Image, "does-not-exist"}
			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).NotTo(Equal(0))
		})
	})
})

var _ = Describe("Check read-only cgroup filesystem", func() {
	var (
		args     []string
		id       string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("write anything in the cgroup files", func() {
		It("should fail because of cgroup filesystem MUST BE read-only", func() {
			args = []string{"--rm", "--name", id, DebianImage, "bash", "-c",
				"for f in /sys/fs/cgroup/*/*; do echo 100 > $f && exit 1; done; exit 0"}
			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
		})
	})
})

var _ = Describe("run host networking", func() {
	var (
		args     []string
		id       string
		stderr   string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("Run with host networking", func() {
		It("should error out", func() {
			args = []string{"--name", id, "-d", "--net=host", DebianImage, "sh"}
			_, stderr, exitCode = dockerRun(args...)
			Expect(exitCode).NotTo(Equal(0))
			Expect(stderr).NotTo(BeEmpty())
		})
	})
})

var _ = Describe("check dmesg logs errors", func() {
	var (
		args     []string
		id       string
		stdout   string
		exitCode int
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("Run to check dmesg log errors", func() {
		It("should be empty", func() {
			if distroID() == "rhel" {
				Skip("Issue:https://github.com/kata-containers/tests/issues/1443")
			}
			if _, ok := KataConfig.Hypervisor[FirecrackerHypervisor]; ok {
				Skip("https://github.com/kata-containers/runtime/issues/1450")
			}
			args = []string{"--name", id, DebianImage, "dmesg", "-l", "err"}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(strings.Trim(stdout, "\n\t")).To(BeEmpty())
		})
	})
})

var _ = Describe("run and check kata-runtime list", func() {
	var (
		args            []string
		id              string
		exitCode        int
		containerStdout string
		stdout          string
	)

	if os.Getuid() != 0 {
		GinkgoT().Skip("only root user can run kata-runtime list")
		return
	}

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("Running a container and then run kata-runtime list", func() {
		It("should not fail", func() {
			args = []string{"-td", "--name", id, Image, "sh"}
			containerStdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))

			cmd := NewCommand("kata-runtime", "list", "--q")
			stdout, _, exitCode = cmd.Run()
			Expect(exitCode).To(BeZero())
			Expect(stdout).To(ContainSubstring(containerStdout))
		})
	})
})
