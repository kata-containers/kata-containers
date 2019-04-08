// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

type cgroupType string

const (
	cgroupCPU    cgroupType = "cpu"
	cgroupCpuset cgroupType = "cpuset"
)

const (
	sysCgroupPath     = "/sys/fs/cgroup/"
	dockerCgroupName  = "docker"
	sysCPUSharesFile  = "cpu.shares"
	sysCPUQuotaFile   = "cpu.cfs_quota_us"
	sysCPUPeriodFile  = "cpu.cfs_period_us"
	sysCpusetCpusFile = "cpuset.cpus"
)

type expectedCPUValues struct {
	shares string
	quota  string
	period string
	cpuset string
}

func containerID(name string) (string, error) {
	stdout, stderr, exitCode := dockerInspect("--format", "{{.Id}}", name)
	if exitCode != 0 {
		return "", fmt.Errorf("Could not get container ID: %v", stderr)
	}
	return strings.Trim(stdout, "\n\t "), nil
}

func containerCgroupParent(name string) (string, error) {
	stdout, stderr, exitCode := dockerInspect("--format", "{{.HostConfig.CgroupParent}}", name)
	if exitCode != 0 {
		return "", fmt.Errorf("Could not get container cgroup parent: %v", stderr)
	}
	return strings.Trim(stdout, "\n\t "), nil
}

func containerCgroupPath(name string, t cgroupType) (string, error) {
	parentCgroup := dockerCgroupName
	if path, err := containerCgroupParent(name); err != nil && path != "" {
		parentCgroup = path
	}

	if id, err := containerID(name); err == nil && id != "" {
		return filepath.Join(sysCgroupPath, string(t), parentCgroup, id), nil
	}

	return "", fmt.Errorf("Could not get container cgroup path")
}

func addProcessToCgroup(pid int, cgroupPath string) error {
	return ioutil.WriteFile(filepath.Join(cgroupPath, "cgroup.procs"),
		[]byte(fmt.Sprintf("%v", pid)), os.FileMode(0775))
}

func checkCPUCgroups(name string, expected expectedCPUValues) error {
	cpuCgroupPath, err := containerCgroupPath(name, cgroupCPU)
	if err != nil {
		return err
	}

	cpusetCgroupPath, err := containerCgroupPath(name, cgroupCpuset)
	if err != nil {
		return err
	}

	for r, v := range map[string]string{
		filepath.Join(cpuCgroupPath, sysCPUQuotaFile):      expected.quota,
		filepath.Join(cpuCgroupPath, sysCPUPeriodFile):     expected.period,
		filepath.Join(cpuCgroupPath, sysCPUSharesFile):     expected.shares,
		filepath.Join(cpusetCgroupPath, sysCpusetCpusFile): expected.cpuset,
	} {
		c, err := ioutil.ReadFile(r)
		if err != nil {
			return err
		}

		cv := strings.Trim(string(c), "\n\t ")
		if cv != v {
			return fmt.Errorf("Cgroup %v, expected: %v, got: %v", r, cv, v)
		}
	}

	return nil
}

var _ = Describe("Checking CPU cgroups in the host", func() {
	var (
		args             []string
		id               string
		cpuCgroupPath    string
		cpusetCgroupPath string
		err              error
		exitCode         int
		expected         expectedCPUValues
	)

	BeforeEach(func() {
		id = randomDockerName()
		args = []string{"--cpus=1", "--cpu-shares=800", "--cpuset-cpus=0", "-dt", "--name", id, Image, "sh"}
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("checking whether cgroups can be deleted", func() {
		Context("with a running process", func() {
			It("should be deleted", func() {
				if os.Getuid() != 0 {
					Skip("only root user can modify cgroups")
				}

				_, _, exitCode = dockerRun(args...)
				Expect(exitCode).To(BeZero())

				// check that cpu cgroups exist
				cpuCgroupPath, err = containerCgroupPath(id, cgroupCPU)
				Expect(err).ToNot(HaveOccurred())
				Expect(cpuCgroupPath).Should(BeADirectory())

				cpusetCgroupPath, err = containerCgroupPath(id, cgroupCpuset)
				Expect(err).ToNot(HaveOccurred())
				Expect(cpusetCgroupPath).Should(BeADirectory())

				// Add current process to cgroups
				err = addProcessToCgroup(os.Getpid(), cpuCgroupPath)
				Expect(err).ToNot(HaveOccurred())

				err = addProcessToCgroup(os.Getpid(), cpusetCgroupPath)
				Expect(err).ToNot(HaveOccurred())

				// remove container
				Expect(RemoveDockerContainer(id)).To(BeTrue())

				// cgroups shouldn't exist
				Expect(cpuCgroupPath).ShouldNot(BeADirectory())
				Expect(cpusetCgroupPath).ShouldNot(BeADirectory())
			})
		})
	})

	Describe("checking whether cgroups are updated", func() {
		Context("updating container cpu and cpuset cgroup", func() {
			It("should be updated", func() {
				_, _, exitCode = dockerRun(args...)
				Expect(exitCode).To(BeZero())

				expected.shares = "738"
				expected.quota = "250000"
				expected.period = "100000"
				expected.cpuset = "1"

				if runtime.GOARCH == "ppc64le" {
					expected.cpuset = "8"
				}
				_, _, exitCode = dockerUpdate("--cpus=2.5", "--cpu-shares", expected.shares, "--cpuset-cpus", expected.cpuset, id)
				Expect(exitCode).To(BeZero())

				err = checkCPUCgroups(id, expected)
				Expect(err).ToNot(HaveOccurred())

				Expect(RemoveDockerContainer(id)).To(BeTrue())
			})
		})
	})

	Describe("checking hosts's cpu cgroups", func() {
		Context("container with cpu and cpuset constraints", func() {
			It("shold have its cgroup set correctly", func() {
				_, _, exitCode = dockerRun(args...)
				Expect(exitCode).To(BeZero())

				expected.shares = "800"
				expected.quota = "100000"
				expected.period = "100000"
				expected.cpuset = "0"

				err = checkCPUCgroups(id, expected)
				Expect(err).ToNot(HaveOccurred())

				Expect(RemoveDockerContainer(id)).To(BeTrue())
			})
		})
	})
})

var _ = Describe("Check cgroup paths", func() {
	var (
		args []string
		id   string
	)

	BeforeEach(func() {
		id = randomDockerName()
		args = []string{"-d", "--name", id}
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("with a parent cgroup",
		func(parentCgroup string) {
			args = append(args, "--cgroup-parent", parentCgroup, Image)
			_, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())
		},
		withParentCgroup("../"),
		withParentCgroup("../../"),
		withParentCgroup("../../../"),
		withParentCgroup("../../../../"),
		withParentCgroup("~"),
		withParentCgroup("/../../../../hi"),
	)
})
