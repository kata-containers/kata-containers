// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

func inspectFormatOptions(formatOption string, expectedStdout string) TableEntry {
	return Entry(fmt.Sprintf("inspect with %s will give you as stdout %s", formatOption, expectedStdout), formatOption, expectedStdout)
}

var _ = Describe("inspect", func() {
	var (
		id string
	)

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode := dockerRun("-d", "--name", id, Image)
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("inspect with docker",
		func(formatOption string, expectedStdout string) {
			stdout, _, _ := dockerInspect("--format", formatOption, id)
			Expect(stdout).To(ContainSubstring(expectedStdout))
		},
		inspectFormatOptions("'{{.Config.Image}}'", Image),
		inspectFormatOptions("'{{.HostConfig.Runtime}}'", Runtime),
		inspectFormatOptions("'{{json .Config}}'", Image),
	)
})
