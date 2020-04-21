// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"regexp"
	"strings"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("info", func() {
	var (
		stdout   string
		exitCode int
	)

	Context("docker info", func() {
		It("should has a runtime information", func() {
			stdout, _, exitCode = dockerInfo()
			Expect(exitCode).To(Equal(0))
			matchStdout := regexp.MustCompile("Runtimes: .*").FindString(stdout)
			checkRuntime := strings.Contains(matchStdout, Runtime)
			Expect(checkRuntime).To(BeTrue())
		})
	})
})
