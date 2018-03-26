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

func withUser(user, regexp string) TableEntry {
	return Entry(fmt.Sprintf("with --user='%s'", user), user, regexp)
}

var _ = Describe("docker exec", func() {
	var (
		args     []string
		id       string
		exitCode int
		stdout   string
		stderr   string
	)

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode = dockerRun("-td", "--name", id, Image, "sh")
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Context("modifying a container with exec", func() {
		It("should have the changes", func() {
			args = []string{"-d", id, "sh", "-c", "echo 'hello world' > file"}
			_, _, exitCode = dockerExec(args...)
			Expect(exitCode).To(Equal(0))

			args = []string{id, "sh", "-c", "cat /file"}
			stdout, _, exitCode = dockerExec(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).NotTo(BeEmpty())
			Expect(stdout).To(ContainSubstring("hello world"))
		})
	})

	Context("check exit code using exec", func() {
		It("should have the value assigned", func() {
			_, _, exitCode = dockerExec(id, "sh", "-c", "exit 42")
			Expect(exitCode).To(Equal(42))
		})
	})

	Context("check stdout forwarded using exec", func() {
		It("should displayed it", func() {
			args = []string{id, "sh", "-c", "ls /etc/resolv.conf 2>/dev/null"}
			stdout, _, exitCode = dockerExec(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stdout).To(ContainSubstring("/etc/resolv.conf"))
		})
	})

	Context("check stderr forwarded using exec", func() {
		It("should not exist", func() {
			args = []string{id, "sh", "-c", "ls /etc/foo >/dev/null"}
			stdout, stderr, exitCode = dockerExec(args...)
			Expect(exitCode).To(Equal(1))
			Expect(stdout).To(BeEmpty())
			Expect(stderr).ToNot(BeEmpty())
		})
	})

	DescribeTable("check exec honours '--user'",
		func(user, regexp string) {
			args = []string{"-t", "--user", user, id, "id"}
			stdout, stderr, exitCode = dockerExec(args...)
			Expect(exitCode).To(Equal(0))
			Expect(stderr).To(BeEmpty())
			Expect(stdout).To(MatchRegexp(regexp))
		},

		// users and groups
		withUser("daemon", `uid=\d+\(daemon\) gid=\d+\(daemon\)`),
		withUser("daemon:", `uid=\d+\(daemon\) gid=\d+\(daemon\)`),
		withUser("daemon:bin", `uid=\d+\(daemon\) gid=\d+\(bin\)`),
		withUser(":adm", `uid=\d+\(root\) gid=\d+\(adm\)`),

		// uids and gids
		withUser("999", "uid=999 gid=0"),
		withUser("999:", "uid=999 gid=0"),
		withUser("999:888", "uid=999 gid=888"),
		withUser(":999", `uid=0\(root\) gid=999`),
	)
})
