// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"os"
	"strings"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

const (
	memLimitPath      = "/sys/fs/cgroup/memory/memory.limit_in_bytes"
	memSoftLimitPath  = "/sys/fs/cgroup/memory/memory.soft_limit_in_bytes"
	memSWLimitPath    = "/sys/fs/cgroup/memory/memory.memsw.limit_in_bytes"
	memSwappinessPath = "/sys/fs/cgroup/memory/memory.swappiness"
)

var _ = Describe("memory constraints", func() {
	var (
		args          []string
		id            string
		memSize       string
		limSize       string
		stderr        string
		stdout        string
		exitCode      int
		memSwappiness string
		useSwappiness bool
		useSwap       bool
		err           error
	)

	BeforeEach(func() {
		useSwappiness = true
		useSwap = true
		if _, err = os.Stat(memSWLimitPath); err != nil {
			useSwap = false
		}

		if _, err = os.Stat(memSwappinessPath); err != nil {
			useSwappiness = false
		}

		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("run container exceeding memory constraints", func() {
		It("should ran out of memory", func() {
			memSize = "256MB"
			limSize = "260M"
			args = []string{"--name", id, "--rm", "-m", memSize, StressImage, "-mem-total", limSize, "-mem-alloc-size", limSize}
			_, stderr, exitCode = dockerRun(args...)
			Expect(exitCode).NotTo(Equal(0))
			Expect(stderr).To(ContainSubstring("fatal error: runtime: out of memory"))
		})
	})

	Context("run container using memory constraints", func() {
		It("should have applied the constraints", func() {
			// 512MB
			memSize = fmt.Sprintf("%d", 512*1024*1024)
			memSwappiness = "60"
			args = []string{"--name", id, "-dti", "--rm", "-m", memSize, "--memory-reservation", memSize}

			if useSwap {
				args = append(args, "--memory-swap", memSize)
			}

			if useSwappiness {
				args = append(args, "--memory-swappiness", memSwappiness)
			}

			args = append(args, Image)

			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(BeZero())

			// check memory limit
			stdout, _, exitCode = dockerExec(id, "cat", memLimitPath)
			Expect(exitCode).To(BeZero())
			Expect(memSize).To(Equal(strings.Trim(stdout, " \n\t")))

			// check memory soft limit
			stdout, _, exitCode = dockerExec(id, "cat", memSoftLimitPath)
			Expect(exitCode).To(BeZero())
			Expect(memSize).To(Equal(strings.Trim(stdout, " \n\t")))

			// check memory swap limit
			if useSwap {
				stdout, _, exitCode = dockerExec(id, "cat", memSWLimitPath)
				Expect(exitCode).To(BeZero())
				Expect(memSize).To(Equal(strings.Trim(stdout, " \n\t")))
			}

			// check memory swappiness
			if useSwappiness {
				stdout, _, exitCode = dockerExec(id, "cat", memSwappinessPath)
				Expect(exitCode).To(BeZero())
				Expect(memSwappiness).To(Equal(strings.Trim(stdout, " \n\t")))
			}

			Expect(RemoveDockerContainer(id)).To(BeTrue())
		})
	})
})
