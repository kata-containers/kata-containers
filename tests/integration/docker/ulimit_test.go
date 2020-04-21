// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"runtime"
	"strings"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

func withUlimit(ulimit string, soft, hard int, option string) TableEntry {
	return Entry(fmt.Sprintf("With ulimit %s=%d:%d",
		ulimit, soft, hard), ulimit, soft, hard, option)
}

var _ = Describe("ulimits", func() {
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
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("check ulimits",
		func(ulimit string, soft, hard int, option string) {
			// ARM doesn't support data rlimit, Issue https://github.com/kata-containers/tests/issues/990
			if ulimit == "data" && runtime.GOARCH == "arm64" {
				Skip("Issue: https://github.com/kata-containers/tests/issues/990")
			}

			ulimitStr := fmt.Sprintf("%s=%d:%d", ulimit, soft, hard)

			switch ulimit {
			// these ulimits are in 1024-byte increments
			case "fsize", "data", "stack", "core", "rss", "memlock":
				ulimitStr = fmt.Sprintf("%s=%d:%d", ulimit, soft*1024, hard*1024)
			}

			args = []string{"--name", id, "--rm", "--ulimit", ulimitStr, CentosImage,
				"bash", "-c", fmt.Sprintf("echo $(ulimit %s -S):$(ulimit %s -H)", option, option)}
			stdout, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(0))
			Expect(strings.Trim(stdout, "\n\t ")).To(Equal(fmt.Sprintf("%d:%d", soft, hard)))
		},
		withUlimit("cpu", 1, 2, "-t"),
		withUlimit("fsize", 66, 82, "-f"),
		withUlimit("data", 1024000, 2048000, "-d"),
		withUlimit("stack", 45, 78, "-s"),
		withUlimit("core", 48, 95, "-c"),
		withUlimit("rss", 56, 83, "-m"),
		withUlimit("nproc", 3, 5, "-u"),
		withUlimit("nofile", 1024, 1024, "-n"),
		withUlimit("memlock", 68, 77, "-l"),
		withUlimit("locks", 1024, 2048, "-x"),
		withUlimit("sigpending", 1024, 2048, "-i"),
		withUlimit("msgqueue", 1024, 2048, "-q"),
		withUlimit("nice", 50, 70, "-e"),
		withUlimit("rtprio", 100, 120, "-r"),
	)
})
