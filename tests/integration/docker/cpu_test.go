// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"math"
	"runtime"
	"strings"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

const (
	sharesSysPath     = "/sys/fs/cgroup/cpu/cpu.shares"
	quotaSysPath      = "/sys/fs/cgroup/cpu/cpu.cfs_quota_us"
	periodSysPath     = "/sys/fs/cgroup/cpu/cpu.cfs_period_us"
	cpusetCpusSysPath = "/sys/fs/cgroup/cpuset/cpuset.cpus"
	cpusetMemsSysPath = "/sys/fs/cgroup/cpuset/cpuset.mems"

	checkCpusCmdFmt = `for c in $(seq 1 %d); do if [ "$(nproc)" == "%d" ]; then nproc; exit 0; fi; sleep %d; done; exit 1`
)

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
		args         []string
		id           string
		vCPUs        int
		defaultVCPUs int
		waitTime     int
		maxTries     int
		exitCode     int
		stdout       string
	)

	BeforeEach(func() {
		id = ""
		waitTime = 5
		maxTries = 5
		args = []string{}
		defaultVCPUs = int(KataConfig.Hypervisor[KataHypervisor].DefaultVCPUs)
		Expect(defaultVCPUs).To(BeNumerically(">", 0))
	})

	DescribeTable("container with CPU period and quota",
		func(quota, period int, fail bool) {
			vCPUs = ((quota + period - 1) / period) + defaultVCPUs

			for i := 0; i < maxTries; i++ {
				id = randomDockerName()
				args = []string{
					"--rm", "--name", id,
					"--cpu-quota", fmt.Sprintf("%d", quota),
					"--cpu-period", fmt.Sprintf("%d", period),
					DebianImage, "bash", "-c",
					fmt.Sprintf(checkCpusCmdFmt, maxTries, vCPUs, waitTime),
				}

				stdout, _, exitCode = dockerRun(args...)
				Expect(ExistDockerContainer(id)).NotTo(BeTrue())

				if fail {
					Expect(exitCode).ToNot(BeZero())
					return
				}

				if exitCode == 0 {
					break
				}
			}
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", vCPUs)).To(Equal(strings.TrimSpace(stdout)))
		},
		withCPUPeriodAndQuota(30000, 20000, defaultVCPUs, false),
		withCPUPeriodAndQuota(30000, 10000, defaultVCPUs, false),
		withCPUPeriodAndQuota(10000, 10000, defaultVCPUs, false),
		withCPUPeriodAndQuota(10000, 100, defaultVCPUs, true),
	)

	DescribeTable("container with CPU constraint",
		func(cpus int, fail bool) {
			vCPUs = cpus + defaultVCPUs

			for i := 0; i < maxTries; i++ {
				id = randomDockerName()
				args = []string{
					"--rm", "--name", id,
					"--cpus", fmt.Sprintf("%d", cpus),
					DebianImage, "bash", "-c",
					fmt.Sprintf(checkCpusCmdFmt, maxTries, vCPUs, waitTime),
				}

				stdout, _, exitCode = dockerRun(args...)
				Expect(ExistDockerContainer(id)).NotTo(BeTrue())

				if fail {
					Expect(exitCode).ToNot(BeZero())
					return
				}
				if exitCode == 0 {
					break
				}
			}
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", vCPUs)).To(Equal(strings.TrimSpace(stdout)))
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
		args       []string
		id         string
		shares     int = 300
		quota      int = 20000
		period     int = 15000
		cpusetCpus int = 0
		cpusetMems int = 0
	)

	BeforeEach(func() {
		id = randomDockerName()
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

var _ = Describe("[Serial Test] Hot plug CPUs", func() {
	var (
		args []string
		id   string
		cpus uint
	)

	BeforeEach(func() {
		id = randomDockerName()
		args = []string{"--rm", "--name", id}
		cpus = 2
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

var _ = Describe("Update number of CPUs", func() {
	var (
		runArgs      []string
		updateArgs   []string
		execArgs     []string
		id           string
		vCPUs        int
		defaultVCPUs int
		waitTime     int
		maxTries     int
		stdout       string
		exitCode     int
	)

	BeforeEach(func() {
		id = ""
		waitTime = 5
		maxTries = 5

		defaultVCPUs = int(KataConfig.Hypervisor[KataHypervisor].DefaultVCPUs)
		Expect(defaultVCPUs).To(BeNumerically(">", 0))

		runArgs = []string{}
		updateArgs = []string{}
		execArgs = []string{}
	})

	DescribeTable("Update CPU period and quota",
		func(quota, period int, fail bool) {
			vCPUs = ((quota + period - 1) / period) + defaultVCPUs

			for i := 0; i < maxTries; i++ {
				id = randomDockerName()
				runArgs = []string{
					"-dt", "--name", id,
					DebianImage, "bash",
				}
				updateArgs = []string{
					"--cpu-quota", fmt.Sprintf("%d", quota),
					"--cpu-period", fmt.Sprintf("%d", period),
					id,
				}
				execArgs = []string{
					id, "bash", "-c",
					fmt.Sprintf(checkCpusCmdFmt, maxTries, vCPUs, waitTime),
				}

				_, _, exitCode = dockerRun(runArgs...)
				Expect(exitCode).To(BeZero())

				stdout, _, exitCode = dockerUpdate(updateArgs...)
				if fail {
					Expect(RemoveDockerContainer(id)).To(BeTrue())
					Expect(ExistDockerContainer(id)).NotTo(BeTrue())
					Expect(exitCode).ToNot(BeZero())
					return
				}
				Expect(exitCode).To(BeZero())

				stdout, _, exitCode = dockerExec(execArgs...)
				Expect(RemoveDockerContainer(id)).To(BeTrue())
				Expect(ExistDockerContainer(id)).NotTo(BeTrue())
				if exitCode == 0 {
					break
				}
			}
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", vCPUs)).To(Equal(strings.TrimSpace(stdout)))
		},
		withCPUPeriodAndQuota(30000, 20000, defaultVCPUs, false),
		withCPUPeriodAndQuota(30000, 10000, defaultVCPUs, false),
		withCPUPeriodAndQuota(10000, 10000, defaultVCPUs, false),
		withCPUPeriodAndQuota(10000, 100, defaultVCPUs, true),
	)

	DescribeTable("Update CPU constraint",
		func(cpus int, fail bool) {
			vCPUs = cpus + defaultVCPUs

			for i := 0; i < maxTries; i++ {
				id = randomDockerName()
				runArgs = []string{
					"-dt", "--name", id,
					DebianImage, "bash",
				}
				execArgs = []string{
					id, "bash", "-c",
					fmt.Sprintf(checkCpusCmdFmt, maxTries, vCPUs, waitTime),
				}
				updateArgs = []string{"--cpus", fmt.Sprintf("%d", cpus), id}

				_, _, exitCode = dockerRun(runArgs...)
				Expect(exitCode).To(BeZero())

				stdout, _, exitCode = dockerUpdate(updateArgs...)
				if fail {
					Expect(RemoveDockerContainer(id)).To(BeTrue())
					Expect(ExistDockerContainer(id)).NotTo(BeTrue())
					Expect(exitCode).ToNot(BeZero())
					return
				}
				Expect(exitCode).To(BeZero())

				stdout, _, exitCode = dockerExec(execArgs...)
				Expect(RemoveDockerContainer(id)).To(BeTrue())
				Expect(ExistDockerContainer(id)).NotTo(BeTrue())
				if exitCode == 0 {
					break
				}
			}
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", vCPUs)).To(Equal(strings.TrimSpace(stdout)))
		},
		withCPUConstraint(1, defaultVCPUs, false),
		withCPUConstraint(1.3, defaultVCPUs, false),
		withCPUConstraint(2, defaultVCPUs, false),
		withCPUConstraint(2.5, defaultVCPUs, false),
		withCPUConstraint(3, defaultVCPUs, false),
	)
})

func withCPUConstraintCheckPeriodAndQuota(cpus float64, fail bool) TableEntry {
	return Entry(fmt.Sprintf("quota/period should be equal to %.1f", cpus), cpus, fail)
}

func withCPUSetConstraint(cpuset string, minCpusNeeded int, fail bool) TableEntry {
	// test should fail when the actual number of cpus is less than the minimum number
	// of cpus needed to run the test, for example cpuset=0-2 requires 3 cpus(0,1,2)
	if runtime.NumCPU() < minCpusNeeded {
		fail = true
	}

	return Entry(fmt.Sprintf("cpuset should be equal to %s", cpuset), cpuset, fail)
}

var _ = Describe("Update CPU constraints", func() {
	var (
		runArgs    []string
		updateArgs []string
		execArgs   []string
		id         string
		exitCode   int
		stdout     string
	)

	BeforeEach(func() {
		id = randomDockerName()

		updateArgs = []string{}
		execArgs = []string{}
		runArgs = []string{}
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("Update number of CPUs to check period and quota",
		func(cpus float64, fail bool) {
			runArgs = []string{"--rm", "--name", id, "-dt", DebianImage, "bash"}
			_, _, exitCode = dockerRun(runArgs...)
			Expect(exitCode).To(BeZero())

			updateArgs = append(updateArgs, "--cpus", fmt.Sprintf("%f", cpus), id)
			stdout, _, exitCode = dockerUpdate(updateArgs...)
			if fail {
				Expect(exitCode).ToNot(BeZero())
				return
			}
			Expect(exitCode).To(BeZero())

			execArgs = append(execArgs, id, "bash", "-c",
				fmt.Sprintf(`perl -e "printf ('%%.1f', $(cat %s)/$(cat %s))"`, quotaSysPath, periodSysPath))
			stdout, _, exitCode = dockerExec(execArgs...)
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%.1f", cpus)).To(Equal(strings.Trim(stdout, "\n\t ")))
		},
		withCPUConstraintCheckPeriodAndQuota(0.5, shouldNotFail),
		withCPUConstraintCheckPeriodAndQuota(1, shouldNotFail),
		withCPUConstraintCheckPeriodAndQuota(1.2, shouldNotFail),
		withCPUConstraintCheckPeriodAndQuota(2, shouldNotFail),
		withCPUConstraintCheckPeriodAndQuota(2.8, shouldNotFail),
		withCPUConstraintCheckPeriodAndQuota(3, shouldNotFail),
		withCPUConstraintCheckPeriodAndQuota(-3, shouldFail),
		withCPUConstraintCheckPeriodAndQuota(-2.5, shouldFail),
	)

	DescribeTable("Update CPU set",
		func(cpuset string, fail bool) {
			// Use the actual number of CPUs
			runArgs = []string{"--rm", fmt.Sprintf("--cpus=%d", runtime.NumCPU()),
				"--name", id, "-dt", DebianImage, "bash"}
			_, _, exitCode = dockerRun(runArgs...)
			Expect(exitCode).To(BeZero())

			updateArgs = append(updateArgs, "--cpuset-cpus", cpuset, id)
			stdout, _, exitCode = dockerUpdate(updateArgs...)
			if fail {
				Expect(exitCode).ToNot(BeZero())
				return
			}
			Expect(exitCode).To(BeZero())

			execArgs = append(execArgs, id, "cat", cpusetCpusSysPath)
			stdout, _, exitCode = dockerExec(execArgs...)
			Expect(exitCode).To(BeZero())
			Expect(cpuset).To(Equal(strings.Trim(stdout, "\n\t ")))
		},
		withCPUSetConstraint("0", 1, shouldNotFail),
		withCPUSetConstraint("2", 3, shouldNotFail),
		withCPUSetConstraint("0-1", 2, shouldNotFail),
		withCPUSetConstraint("0-2", 3, shouldNotFail),
		withCPUSetConstraint("0-3", 4, shouldNotFail),
		withCPUSetConstraint("0,2", 3, shouldNotFail),
		withCPUSetConstraint("0,3", 4, shouldNotFail),
		withCPUSetConstraint("0,-2,3", 0, shouldFail),
		withCPUSetConstraint("-1-3", 0, shouldFail),
	)
})

var _ = Describe("CPUs and CPU set", func() {
	type cpuTest struct {
		cpus         string
		cpusetcpus   string
		expectedCpus string
	}

	var (
		args          []string
		id            string
		cpuTests      []cpuTest
		exitCode      int
		stdout        string
		updateCheckFn func(cpus, cpusetCpus, expectedCpus string)
	)

	BeforeEach(func() {
		id = randomDockerName()
		args = []string{"--rm", "-dt", "--name", id, Image, "sh"}
		cpuTests = []cpuTest{
			{"1", "0-1", "2"},
			{"3", "1,2", "2"},
			{"2", "1", "1"},
		}
		_, _, exitCode = dockerRun(args...)
		Expect(exitCode).To(BeZero())
		updateCheckFn = func(cpus, cpusetCpus, expectedCpus string) {
			args = []string{"--cpus", cpus, "--cpuset-cpus", cpusetCpus, id}
			_, _, exitCode = dockerUpdate(args...)
			Expect(exitCode).To(BeZero())
			stdout, _, exitCode = dockerExec(id, "nproc")
			Expect(expectedCpus).To(Equal(strings.Trim(stdout, "\n\t ")))
		}
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("updating", func() {
		Context("cpus and cpuset of a running container", func() {
			It("should have the right number of vCPUs", func() {
				for _, c := range cpuTests {
					updateCheckFn(c.cpus, c.cpusetcpus, c.expectedCpus)
				}
			})
		})
	})
})
