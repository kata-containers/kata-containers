// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package functional

import (
	"fmt"
	"time"

	. "github.com/kata-containers/tests"
	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

var (
	stateWorkload = []string{"true"}
)

const (
	stateStopped  = "stopped"
	stateWaitTime = 5
)

var _ = Describe("state", func() {
	var (
		container *Container
		err       error
	)

	BeforeEach(func() {
		container, err = NewContainer(stateWorkload, true)
		Expect(err).NotTo(HaveOccurred())
		Expect(container).NotTo(BeNil())
	})

	AfterEach(func() {
		Expect(container.Teardown()).To(Succeed())
	})

	DescribeTable("container",
		func(status string, waitTime int) {
			_, stderr, exitCode := container.Run()
			Expect(exitCode).To(Equal(0))
			Expect(stderr).To(BeEmpty())

			time.Sleep(time.Second * time.Duration(waitTime))

			stdout, stderr, exitCode := container.State()
			Expect(exitCode).To(Equal(0))
			Expect(stderr).To(BeEmpty())
			subString := fmt.Sprintf("\"status\": \"%s\"", status)
			Expect(stdout).To(ContainSubstring(subString))
		},
		Entry(fmt.Sprintf("with workload %s, timeWait %d", stateWorkload, stateWaitTime), stateStopped, stateWaitTime),
	)
})
