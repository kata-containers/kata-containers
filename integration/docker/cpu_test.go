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
		args         []string
		id           string
		vCPUs        int
		defaultVCPUs = getDefaultVCPUs()
	)

	BeforeEach(func() {
		id = RandID(30)
		args = []string{"--rm", "--name", id}
		Expect(defaultVCPUs).To(BeNumerically(">", 0))
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("container with CPU period and quota",
		func(quota, period int, fail bool) {
			args = append(args, "--cpu-quota", fmt.Sprintf("%d", quota),
				"--cpu-period", fmt.Sprintf("%d", period), Image, "sh", "-c", "sleep 5; nproc")
			vCPUs = (quota + period - 1) / period
			stdout, _, exitCode := dockerRun(args...)
			if fail {
				Expect(exitCode).ToNot(BeZero())
				return
			}
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", vCPUs+defaultVCPUs)).To(Equal(strings.Trim(stdout, "\n\t ")))
		},
		withCPUPeriodAndQuota(30000, 20000, defaultVCPUs, false),
		withCPUPeriodAndQuota(30000, 10000, defaultVCPUs, false),
		withCPUPeriodAndQuota(10000, 10000, defaultVCPUs, false),
		withCPUPeriodAndQuota(10000, 100, defaultVCPUs, true),
	)

	DescribeTable("container with CPU constraint",
		func(cpus int, fail bool) {
			args = append(args, "--cpus", fmt.Sprintf("%d", cpus), Image, "sh", "-c", "sleep 5; nproc")
			stdout, _, exitCode := dockerRun(args...)
			if fail {
				Expect(exitCode).ToNot(BeZero())
				return
			}
			Expect(exitCode).To(BeZero())
			Expect(fmt.Sprintf("%d", cpus+defaultVCPUs)).To(Equal(strings.Trim(stdout, "\n\t ")))
		},
		withCPUConstraint(1, defaultVCPUs, false),
		withCPUConstraint(1.5, defaultVCPUs, false),
		withCPUConstraint(2, defaultVCPUs, false),
		withCPUConstraint(2.5, defaultVCPUs, false),
		withCPUConstraint(-5, defaultVCPUs, true),
	)
})
