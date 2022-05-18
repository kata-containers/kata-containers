// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package katatestutils

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/blang/semver"
)

func getKernelVersion() (string, error) {
	const procVersion = "/proc/version"

	contents, err := getFileContents(procVersion)
	if err != nil {
		return "", err
	}

	fields := strings.Fields(contents)
	l := len(fields)
	if l < 3 {
		return "", fmt.Errorf("unexpected contents in %v", procVersion)
	}

	return fixKernelVersion(fields[2]), nil
}

// fixKernelVersion replaces underscores with dashes in a version string.
// This change is primarily for Fedora, RHEL and CentOS version numbers which
// can contain underscores. By replacing them with dashes, a valid semantic
// version string is created.
//
// Examples of actual kernel versions which can be made into valid semver
// format by calling this function:
//
//   centos: 3.10.0-957.12.1.el7.x86_64
//   fedora: 5.0.9-200.fc29.x86_64
//
// For some self compiled kernel, the kernel version will be with "+" as its suffix
// For example:
//   5.12.0-rc4+
// These kernel version can't be parsed by the current lib and lead to panic
// therefore the '+' should be removed.
//
func fixKernelVersion(version string) string {
	version = strings.Replace(version, "_", "-", -1)
	return strings.Replace(version, "+", "", -1)
}
