// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"math"
	"strconv"
	"strings"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

func getDefaultVCPUs() int {
	args := []string{"--rm", Image, "sh", "-c", "sleep 5; nproc"}
	stdout, _, exitCode := dockerRun(args...)
	if stdout == "" || exitCode != 0 {
		LogIfFail("Failed to get default number of vCPUs")
		return -1
	}

	stdout = strings.Trim(stdout, "\n\t ")
	vcpus, err := strconv.Atoi(stdout)
	if err != nil {
		LogIfFail("Failed to convert '%s' to int", stdout)
		return -1
	}

	return vcpus
}

func withCPUPeriodAndQuota(quota, period, defaultVCPUs int, fail bool) TableEntry {
	var msg string

	if fail {
		msg = "should fail"
	} else {
		msg = fmt.Sprintf("should have %d CPUs", ((quota+period-1)/period)+defaultVCPUs)
	}

	return Entry(msg, quota, period, fail)
}

func withCPUConstraint(cpus float64, defaultVCPUs int, fail bool) TableEntry {
	var msg string
	c := int(math.Ceil(cpus))

	if fail {
		msg = "should fail"
	} else {
		msg = fmt.Sprintf("should have %d CPUs", c+defaultVCPUs)
	}

	return Entry(msg, c, fail)
}

var _ = Describe("Hot plug CPUs", func() {
	var (
		args            []string
		id              string
		vCPUs           int
		defaultVCPUs    = getDefaultVCPUs()
		waitTime        int
		maxTries        int
		checkCpusCmdFmt string
	)

	BeforeEach(func() {
		id = RandID(30)
		checkCpusCmdFmt = `for c in $(seq 1 %d); do [ -d /sys/devices/system/cpu/cpu%d ] && nproc && exit 0; sleep %d; done; exit 1`
		waitTime = 5
		maxTries = 5
		args = []string{"--rm", "--name", id}
		Expect(defaultVCPUs).To(BeNumerically(">", 0))
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("container with CPU period and quota",
		func(quota, period int, fail bool) {
			vCPUs = ((quota + period - 1) / period) + defaultVCPUs
			args = append(args, "--cpu-quota", fmt.Sprintf("%d", quota),
				"--cpu-period", fmt.Sprintf("%d", period), DebianImage, "bash", "-c",
				fmt.Sprintf(checkCpusCmdFmt, maxTries, vCPUs-1, waitTime))
			stdout, _, exitCode := dockerRun(args...)
			if fail {
				Expect(exitCode).ToNot(BeZero())
				return
			}
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", vCPUs)).To(Equal(strings.Trim(stdout, "\n\t ")))
		},
		withCPUPeriodAndQuota(30000, 20000, defaultVCPUs, false),
		withCPUPeriodAndQuota(30000, 10000, defaultVCPUs, false),
		withCPUPeriodAndQuota(10000, 10000, defaultVCPUs, false),
		withCPUPeriodAndQuota(10000, 100, defaultVCPUs, true),
	)

	DescribeTable("container with CPU constraint",
		func(cpus int, fail bool) {
			vCPUs = cpus + defaultVCPUs
			args = append(args, "--cpus", fmt.Sprintf("%d", cpus), DebianImage, "bash", "-c",
				fmt.Sprintf(checkCpusCmdFmt, maxTries, vCPUs-1, waitTime))
			stdout, _, exitCode := dockerRun(args...)
			if fail {
				Expect(exitCode).ToNot(BeZero())
				return
			}
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", vCPUs)).To(Equal(strings.Trim(stdout, "\n\t ")))
		},
		withCPUConstraint(1, defaultVCPUs, false),
		withCPUConstraint(1.5, defaultVCPUs, false),
		withCPUConstraint(2, defaultVCPUs, false),
		withCPUConstraint(2.5, defaultVCPUs, false),
		withCPUConstraint(-5, defaultVCPUs, true),
	)
})

var _ = Describe("CPU constraints", func() {
	var (
		args              []string
		id                string
		shares            int    = 300
		quota             int    = 2000
		period            int    = 1500
		cpusetCpus        int    = 0
		cpusetMems        int    = 0
		sharesSysPath     string = "/sys/fs/cgroup/cpu,cpuacct/cpu.shares"
		quotaSysPath      string = "/sys/fs/cgroup/cpu,cpuacct/cpu.cfs_quota_us"
		periodSysPath     string = "/sys/fs/cgroup/cpu,cpuacct/cpu.cfs_period_us"
		cpusetCpusSysPath string = "/sys/fs/cgroup/cpuset/cpuset.cpus"
		cpusetMemsSysPath string = "/sys/fs/cgroup/cpuset/cpuset.mems"
	)

	BeforeEach(func() {
		id = RandID(30)
		args = []string{"--rm", "--name", id}
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("checking container with CPU constraints", func() {
		Context(fmt.Sprintf("with shares equal to %d", shares), func() {
			It(fmt.Sprintf("%s should have %d", sharesSysPath, shares), func() {
				args = append(args, "--cpu-shares", fmt.Sprintf("%d", shares), Image, "cat", sharesSysPath)
				stdout, _, exitCode := dockerRun(args...)
				Expect(exitCode).To(BeZero())
				Expect(fmt.Sprintf("%d", shares)).To(Equal(strings.Trim(stdout, "\n\t ")))
			})
		})

		Context(fmt.Sprintf("with period equal to %d", period), func() {
			It(fmt.Sprintf("%s should have %d", periodSysPath, period), func() {
				args = append(args, "--cpu-period", fmt.Sprintf("%d", period), Image, "cat", periodSysPath)
				stdout, _, exitCode := dockerRun(args...)
				Expect(exitCode).To(BeZero())
				Expect(fmt.Sprintf("%d", period)).To(Equal(strings.Trim(stdout, "\n\t ")))
			})
		})

		Context(fmt.Sprintf("with quota equal to %d", quota), func() {
			It(fmt.Sprintf("%s should have %d", quotaSysPath, quota), func() {
				args = append(args, "--cpu-quota", fmt.Sprintf("%d", quota), Image, "cat", quotaSysPath)
				stdout, _, exitCode := dockerRun(args...)
				Expect(exitCode).To(BeZero())
				Expect(fmt.Sprintf("%d", quota)).To(Equal(strings.Trim(stdout, "\n\t ")))
			})
		})

		Context(fmt.Sprintf("with cpuset-cpus to %d", cpusetCpus), func() {
			It(fmt.Sprintf("%s should have %d", cpusetCpusSysPath, cpusetCpus), func() {
				args = append(args, "--cpuset-cpus", fmt.Sprintf("%d", cpusetCpus), Image, "cat", cpusetCpusSysPath)
				stdout, _, exitCode := dockerRun(args...)
				Expect(exitCode).To(BeZero())
				Expect(fmt.Sprintf("%d", cpusetCpus)).To(Equal(strings.Trim(stdout, "\n\t ")))
			})
		})

		Context(fmt.Sprintf("with cpuset-mems to %d", cpusetMems), func() {
			It(fmt.Sprintf("%s should have %d", cpusetMemsSysPath, cpusetMems), func() {
				args = append(args, "--cpuset-mems", fmt.Sprintf("%d", cpusetMems), Image, "cat", cpusetMemsSysPath)
				stdout, _, exitCode := dockerRun(args...)
				Expect(exitCode).To(BeZero())
				Expect(fmt.Sprintf("%d", cpusetMems)).To(Equal(strings.Trim(stdout, "\n\t ")))
			})
		})
	})
})

func withParentCgroup(parentCgroup string) TableEntry {
	return Entry(fmt.Sprintf("should not fail with parent cgroup: %s", parentCgroup), parentCgroup)
}

var _ = Describe("Hot plug CPUs", func() {
	var (
		args          []string
		id            string
		cpus          uint
		quotaSysPath  string
		periodSysPath string
	)

	BeforeEach(func() {
		id = RandID(30)
		args = []string{"--rm", "--name", id}
		cpus = 2
		quotaSysPath = "/sys/fs/cgroup/cpu,cpuacct/cpu.cfs_quota_us"
		periodSysPath = "/sys/fs/cgroup/cpu,cpuacct/cpu.cfs_period_us"
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("with a parent cgroup",
		func(parentCgroup string) {
			args = append(args, "--cgroup-parent", parentCgroup, "--cpus", fmt.Sprintf("%d", cpus), DebianImage, "bash", "-c",
				fmt.Sprintf("echo $(($(cat %s)/$(cat %s)))", quotaSysPath, periodSysPath))
			stdout, _, exitCode := dockerRun(args...)
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", cpus)).To(Equal(strings.Trim(stdout, "\n\t ")))
		},
		withParentCgroup("0"),
		withParentCgroup("systemd"),
		withParentCgroup("/systemd/"),
		withParentCgroup("///systemd////"),
		withParentCgroup("systemd////"),
		withParentCgroup("////systemd"),
		withParentCgroup("docker"),
		withParentCgroup("abc/xyz/rgb"),
		withParentCgroup("/abc/xyz/rgb/"),
		withParentCgroup("///abc///xyz////rgb///"),
	)
})
