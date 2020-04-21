// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

func withExitCode(exitCode, expectedExitCode int, interactive bool) TableEntry {
	return Entry(fmt.Sprintf("with exit code '%d' when interactive mode is: '%t', it should exit '%d'",
		exitCode, interactive, expectedExitCode), exitCode, expectedExitCode, interactive)
}

var _ = Describe("docker exit code", func() {
	var (
		args []string
		id   string
	)

	BeforeEach(func() {
		id = randomDockerName()
		args = []string{"--name", id, "--rm"}
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("check exit codes",
		func(exitCode, expectedExitCode int, interactive bool) {
			if interactive {
				args = append(args, "-i")
			}
			args = append(args, DebianImage, "/usr/bin/perl", "-e", fmt.Sprintf("exit %d", exitCode))
			_, _, exitCode = dockerRun(args...)
			Expect(exitCode).To(Equal(expectedExitCode))
		},
		withExitCode(0, 0, true),
		withExitCode(0, 0, false),
		withExitCode(1, 1, true),
		withExitCode(1, 1, false),
		withExitCode(55, 55, true),
		withExitCode(55, 55, false),
		withExitCode(-1, 255, true),
		withExitCode(-1, 255, false),
		withExitCode(255, 255, true),
		withExitCode(255, 255, false),
		withExitCode(256, 0, true),
		withExitCode(256, 0, false),
	)
})
