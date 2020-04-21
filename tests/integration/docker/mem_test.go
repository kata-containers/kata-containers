// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"math"
	"os"
	"strconv"
	"strings"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

const (
	memLimitPath      = "/sys/fs/cgroup/memory/memory.limit_in_bytes"
	memSoftLimitPath  = "/sys/fs/cgroup/memory/memory.soft_limit_in_bytes"
	memSWLimitPath    = "/sys/fs/cgroup/memory/memory.memsw.limit_in_bytes"
	memSwappinessPath = "/sys/fs/cgroup/memory/memory.swappiness"
	memKmemLimitPath  = "/sys/fs/cgroup/memory/memory.kmem.limit_in_bytes"
	memBlockSizePath  = "/sys/devices/system/memory/block_size_bytes"
	sysfsMemPath      = "/sys/devices/system/memory/"
)

func withDockerMemory(dockerMem int64) TableEntry {
	msg := "hotplug memory when create containers should not fail"
	return Entry(msg, dockerMem)
}

func withUpdateMemoryConstraints(dockerMem int64, updateMem int64, fail bool) TableEntry {
	var msg string

	if fail {
		msg = "update memory constraints should fail"
	} else {
		msg = "update memory constraints should not fail"
	}

	return Entry(msg, dockerMem, updateMem, fail)
}

var _ = Describe("Hotplug memory when create containers", func() {
	var (
		args         []string
		id           string
		defaultMemSz int64
		memBlockSize int64
		exitCode     int
		stdout       string
		err          error
		data         string
		memBlockNum  int
	)

	BeforeEach(func() {
		id = randomDockerName()
		defaultMemSz = int64(KataConfig.Hypervisor[KataHypervisor].DefaultMemSz) << 20
		Expect(defaultMemSz).To(BeNumerically(">", int64(0)))
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("Hotplug memory when create containers",
		func(dockerMem int64) {
			args = []string{"--name", id, "-tid", "--rm", "-m", fmt.Sprintf("%d", dockerMem), Image}
			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(BeZero())

			stdout, _, exitCode = dockerExec(id, "cat", memBlockSizePath)
			Expect(exitCode).To(BeZero())
			data = strings.Trim(stdout, "\n\t ")
			memBlockSize, err = strconv.ParseInt(data, 16, 64)
			Expect(err).ToNot(HaveOccurred())

			stdout, _, exitCode = dockerExec(id, "sh", "-c", fmt.Sprintf("find %v -name memory* | wc -l", sysfsMemPath))
			Expect(exitCode).To(BeZero())
			memBlockNum, err = strconv.Atoi(strings.Trim(stdout, "\n\t "))
			Expect(err).ToNot(HaveOccurred())
			memBlockNum--

			mem := int64(math.Ceil(float64(dockerMem)/float64(memBlockSize))) * memBlockSize
			Expect(int64(memBlockNum) * memBlockSize).To(Equal(mem + defaultMemSz))

			Expect(RemoveDockerContainer(id)).To(BeTrue())
		},
		withDockerMemory(500*1024*1024),
		withDockerMemory(640*1024*1024),
		withDockerMemory(768*1024*1024),
		withDockerMemory(1024*1024*1024),
	)
})

var _ = Describe("memory constraints", func() {
	var (
		args          []string
		id            string
		memSize       string
		kmemSize      string
		limSize       string
		stderr        string
		stdout        string
		exitCode      int
		memSwappiness string
		useSwappiness bool
		useSwap       bool
		useKmem       bool
		err           error
		defaultMemSz  int
		hotMemSz      int
	)

	BeforeEach(func() {
		useSwappiness = true
		useSwap = true
		useKmem = true
		if _, err = os.Stat(memSWLimitPath); err != nil {
			useSwap = false
		}

		if _, err = os.Stat(memSwappinessPath); err != nil {
			useSwappiness = false
		}

		if _, err = os.Stat(memKmemLimitPath); err != nil {
			useKmem = false
		}

		id = randomDockerName()

		defaultMemSz = int(KataConfig.Hypervisor[KataHypervisor].DefaultMemSz)
		Expect(defaultMemSz).To(BeNumerically(">", 0))
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("run container exceeding memory constraints", func() {
		It("should ran out of memory", func() {
			hotMemSz = 256
			memSize = fmt.Sprintf("%dMB", hotMemSz)
			limSize = fmt.Sprintf("%dM", (hotMemSz*2)+defaultMemSz)
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
			// 10 MB
			kmemSize = fmt.Sprintf("%d", 10*1024*1024)
			memSwappiness = "60"
			args = []string{"--name", id, "-dti", "--rm", "-m", memSize, "--memory-reservation", memSize}

			if useSwap {
				args = append(args, "--memory-swap", memSize)
			}

			if useSwappiness {
				args = append(args, "--memory-swappiness", memSwappiness)
			}

			if useKmem {
				args = append(args, "--kernel-memory", kmemSize)
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

			// check kernel memory
			if useKmem {
				stdout, _, exitCode = dockerExec(id, "cat", memKmemLimitPath)
				Expect(exitCode).To(BeZero())
				Expect(kmemSize).To(Equal(strings.Trim(stdout, " \n\t")))
			}

			Expect(RemoveDockerContainer(id)).To(BeTrue())
		})
	})
})

var _ = Describe("run container and update its memory constraints", func() {
	var (
		args     []string
		id       string
		memSize  string
		stdout   string
		exitCode int
		useSwap  bool
		err      error
	)

	BeforeEach(func() {
		useSwap = true
		if _, err = os.Stat(memSWLimitPath); err != nil {
			useSwap = false
		}

		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("should have applied the memory constraints",
		func(dockerMem int64, updateMem int64, fail bool) {
			memSize = fmt.Sprintf("%d", dockerMem)
			args = []string{"--name", id, "-dti", "--rm", "-m", memSize, Image}

			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(BeZero())

			memSize = fmt.Sprintf("%d", updateMem)
			args = []string{"--memory", memSize, "--memory-reservation", memSize}
			if useSwap {
				args = append(args, "--memory-swap", memSize)
			}

			args = append(args, id)

			// update memory constraints
			_, _, exitCode = dockerUpdate(args...)
			Expect(exitCode).To(BeZero())

			// check memory limit
			stdout, _, exitCode = dockerExec(id, "cat", memLimitPath)
			Expect(exitCode).To(BeZero())
			if fail {
				Expect(memSize).ToNot(Equal(strings.Trim(stdout, " \n\t")))
			} else {
				Expect(memSize).To(Equal(strings.Trim(stdout, " \n\t")))
			}

			// check memory soft limit
			stdout, _, exitCode = dockerExec(id, "cat", memSoftLimitPath)
			Expect(exitCode).To(BeZero())
			if fail {
				Expect(memSize).ToNot(Equal(strings.Trim(stdout, " \n\t")))
			} else {
				Expect(memSize).To(Equal(strings.Trim(stdout, " \n\t")))
			}

			if useSwap {
				// check memory swap limit
				stdout, _, exitCode = dockerExec(id, "cat", memSWLimitPath)
				Expect(exitCode).To(BeZero())
				if fail {
					Expect(memSize).ToNot(Equal(strings.Trim(stdout, " \n\t")))
				} else {
					Expect(memSize).To(Equal(strings.Trim(stdout, " \n\t")))
				}
			}

			Expect(RemoveDockerContainer(id)).To(BeTrue())
		},
		withUpdateMemoryConstraints(500*1024*1024, 400*1024*1024, shouldNotFail),
		withUpdateMemoryConstraints(500*1024*1024, 500*1024*1024, shouldNotFail),
		withUpdateMemoryConstraints(500*1024*1024, 600*1024*1024, shouldNotFail),
		withUpdateMemoryConstraints(500*1024*1024, 500*1024*1024+1, shouldFail),
		withUpdateMemoryConstraints(500*1024*1024, 500*1024*1024+4096, shouldNotFail),
	)
})
