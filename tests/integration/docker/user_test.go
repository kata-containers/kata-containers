// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

package docker

import (
	"fmt"
	"strings"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/ginkgo/extensions/table"
	. "github.com/onsi/gomega"
)

const (
	withAdditionalGroups    = true
	withoutAdditionalGroups = false
)

func asUser(user string, groups bool, fail bool) TableEntry {
	// Some groups that do exist in the base image already
	additionalGroups := []string{"cdrom", "floppy", "video", "audio"}
	groupsMsg := fmt.Sprintf(" with additional groups %v", additionalGroups)
	if !groups {
		groupsMsg = fmt.Sprintf(" without additional groups")
		additionalGroups = []string{}
	}

	return Entry(fmt.Sprintf("as '%s' user%s", user, groupsMsg),
		user, additionalGroups, fail)
}

var _ = Describe("users and groups", func() {
	var (
		id string
	)

	BeforeEach(func() {
		id = randomDockerName()
	})

	AfterEach(func() {
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	DescribeTable("running container",
		func(user string, additionalGroups []string, fail bool) {
			cmd := []string{"--name", id, "--rm"}
			for _, ag := range additionalGroups {
				cmd = append(cmd, "--group-add", ag)
			}
			if user != "" {
				cmd = append(cmd, "-u", user)
			}
			cmd = append(cmd, Image, "id")

			stdout, stderr, exitCode := dockerRun(cmd...)
			if fail {
				Expect(exitCode).ToNot(Equal(0))
				Expect(stderr).NotTo(BeEmpty())
				// do not check stdout because container failed
				return
			}

			// check exit code and stderr
			Expect(exitCode).To(Equal(0))
			Expect(stderr).To(BeEmpty())

			var u, g string
			if user != "" {
				ug := strings.Split(user, ":")
				if len(ug) > 1 {
					u, g = ug[0], ug[1]
				} else {
					u, g = ug[0], ug[0]
				}
			}

			// default user and group is root
			if u == "" {
				u = "root"
			}
			if g == "" {
				g = "root"
			}

			fields := strings.Fields(stdout)

			// busybox id/image is a bit odd in that it does not have any
			// users in extra groups by default. If you have a --group-add or
			// you are the root user you will get the '3 field' output. If you
			// are non-root, you will not (and only get two fields).
			if len(additionalGroups) != 0 || user == "root" || user == "" {
				Expect(fields).To(HaveLen(3))
			} else {
				Expect(fields).To(HaveLen(2))
			}

			// check user (uid)
			Expect(fields[0]).To(ContainSubstring(fmt.Sprintf("(%s)", u)))

			// check group (gid)
			Expect(fields[1]).To(ContainSubstring(fmt.Sprintf("(%s)", g)))

			// check additional groups
			for _, ag := range additionalGroups {
				Expect(fields[2]).To(ContainSubstring(fmt.Sprintf("(%s)", ag)))
			}
		},
		asUser("", withAdditionalGroups, shouldNotFail),
		asUser("", withoutAdditionalGroups, shouldNotFail),
		asUser("root", withAdditionalGroups, shouldNotFail),
		asUser("root", withoutAdditionalGroups, shouldNotFail),
		asUser("mail", withAdditionalGroups, shouldNotFail),
		asUser("mail", withoutAdditionalGroups, shouldNotFail),
		asUser(":mail", withAdditionalGroups, shouldNotFail),
		asUser(":mail", withoutAdditionalGroups, shouldNotFail),
		asUser("mail:mail", withAdditionalGroups, shouldNotFail),
		asUser("mail:mail", withoutAdditionalGroups, shouldNotFail),
		asUser("root:mail", withAdditionalGroups, shouldNotFail),
		asUser("root:mail", withoutAdditionalGroups, shouldNotFail),
		asUser("nonexistentuser", withAdditionalGroups, shouldFail),
		asUser("nonexistentuser", withoutAdditionalGroups, shouldFail),
		asUser("nonexistentuser:mail", withAdditionalGroups, shouldFail),
		asUser("nonexistentuser:mail", withoutAdditionalGroups, shouldFail),
		asUser(":nonexistentuser", withAdditionalGroups, shouldFail),
		asUser(":nonexistentuser", withoutAdditionalGroups, shouldFail),
	)
})
