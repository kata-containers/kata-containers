// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"os"
	"path"
	"runtime"
	"strings"
	"syscall"
	"testing"

	specs "github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"

	"code.cloudfoundry.org/bytefmt"
)

func TestHandleHugepages(t *testing.T) {
	if os.Getuid() != 0 {
		t.Skip("Test disabled as requires root user")
	}

	assert := assert.New(t)

	dir := t.TempDir()

	k := kataAgent{}
	var formattedSizes []string
	var mounts []specs.Mount
	var hugepageLimits []specs.LinuxHugepageLimit

	// On s390x, hugepage sizes must be set at boot and cannot be created ad hoc. Use any that
	// are present (default is 1M, can only be changed on LPAR). See
	// https://www.ibm.com/docs/en/linuxonibm/pdf/lku5dd05.pdf, p. 345 for more information.
	if runtime.GOARCH == "s390x" {
		dirs, err := os.ReadDir(sysHugepagesDir)
		assert.Nil(err)
		for _, dir := range dirs {
			formattedSizes = append(formattedSizes, strings.TrimPrefix(dir.Name(), "hugepages-"))
		}
	} else {
		formattedSizes = []string{"1G", "2M"}
	}

	for _, formattedSize := range formattedSizes {
		bytes, err := bytefmt.ToBytes(formattedSize)
		assert.Nil(err)
		hugepageLimits = append(hugepageLimits, specs.LinuxHugepageLimit{
			Pagesize: formattedSize,
			Limit:    1_000_000 * bytes,
		})

		target := path.Join(dir, fmt.Sprintf("hugepages-%s", formattedSize))
		err = os.MkdirAll(target, 0777)
		assert.NoError(err, "Unable to create dir %s", target)

		err = syscall.Mount("nodev", target, "hugetlbfs", uintptr(0), fmt.Sprintf("pagesize=%s", formattedSize))
		assert.NoError(err, "Unable to mount %s", target)

		defer syscall.Unmount(target, 0)
		defer os.RemoveAll(target)
		mount := specs.Mount{
			Type:   KataLocalDevType,
			Source: target,
		}
		mounts = append(mounts, mount)
	}

	hugepages, err := k.handleHugepages(mounts, hugepageLimits)

	assert.NoError(err, "Unable to handle hugepages %v", hugepageLimits)
	assert.NotNil(hugepages)
	assert.Equal(len(hugepages), len(formattedSizes))
}
