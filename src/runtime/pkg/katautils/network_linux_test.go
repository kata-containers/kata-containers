// Copyright (c) 2018 Huawei Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"testing"

	"github.com/containernetworking/plugins/pkg/ns"
	"github.com/containernetworking/plugins/pkg/testutils"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	vc "github.com/kata-containers/kata-containers/src/runtime/virtcontainers"
	"github.com/stretchr/testify/assert"
	"golang.org/x/sys/unix"
)

func TestGetNetNsFromBindMount(t *testing.T) {
	assert := assert.New(t)

	tmpdir := t.TempDir()

	mountFile := filepath.Join(tmpdir, "mountInfo")
	nsPath := filepath.Join(tmpdir, "ns123")

	// Non-existent namespace path
	_, err := getNetNsFromBindMount(nsPath, mountFile)
	assert.NotNil(err)

	tmpNSPath := filepath.Join(tmpdir, "testNetNs")
	f, err := os.Create(tmpNSPath)
	assert.NoError(err)
	defer f.Close()

	type testData struct {
		contents       string
		expectedResult string
	}

	data := []testData{
		{fmt.Sprintf("711 26 0:3 net:[4026532008] %s rw shared:535 - nsfs nsfs rw", tmpNSPath), "net:[4026532008]"},
		{"711 26 0:3 net:[4026532008] /run/netns/ns123 rw shared:535 - tmpfs tmpfs rw", ""},
		{"a a a a a a a - b c d", ""},
		{"", ""},
	}

	for i, d := range data {
		err := os.WriteFile(mountFile, []byte(d.contents), 0640)
		assert.NoError(err)

		path, err := getNetNsFromBindMount(tmpNSPath, mountFile)
		assert.NoError(err, fmt.Sprintf("got %q, test data: %+v", path, d))

		assert.Equal(d.expectedResult, path, "Test %d, expected %s, got %s", i, d.expectedResult, path)
	}
}

func TestHostNetworkingRequested(t *testing.T) {
	assert := assert.New(t)

	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	if tc.NotValid(ktu.NeedKernelVersionGE("3.19.0")) {
		t.Skip("skipping this test as it requires a greater kernel version")
	}

	// Network namespace same as the host
	selfNsPath := "/proc/self/ns/net"
	isHostNs, err := hostNetworkingRequested(selfNsPath)
	assert.NoError(err)
	assert.True(isHostNs)

	// Non-existent netns path
	nsPath := "/proc/123456789/ns/net"
	_, err = hostNetworkingRequested(nsPath)
	assert.Error(err)

	// Bind-mounted Netns
	tmpdir := t.TempDir()

	// Create a bind mount to the current network namespace.
	tmpFile := filepath.Join(tmpdir, "testNetNs")
	f, err := os.Create(tmpFile)
	assert.NoError(err)
	defer f.Close()

	err = syscall.Mount(selfNsPath, tmpFile, "bind", syscall.MS_BIND, "")
	assert.Nil(err)

	isHostNs, err = hostNetworkingRequested(tmpFile)
	assert.NoError(err)
	assert.True(isHostNs)

	syscall.Unmount(tmpFile, 0)
}

func TestSetupNetworkNamespace(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)

	// Network namespace same as the host
	config := &vc.NetworkConfig{
		NetworkID: "/proc/self/ns/net",
	}
	err := SetupNetworkNamespace(config)
	assert.Error(err)

	// Non-existent netns path
	config = &vc.NetworkConfig{
		NetworkID: "/proc/123456789/ns/net",
	}
	err = SetupNetworkNamespace(config)
	assert.Error(err)

	// Existent netns path
	n, err := testutils.NewNS()
	assert.NoError(err)
	config = &vc.NetworkConfig{
		NetworkID: n.Path(),
	}
	err = SetupNetworkNamespace(config)
	assert.NoError(err)
	n.Close()

	// Empty netns path
	config = &vc.NetworkConfig{}
	err = SetupNetworkNamespace(config)
	assert.NoError(err)
	n, err = ns.GetNS(config.NetworkID)
	assert.NoError(err)
	assert.NotNil(n)
	assert.True(config.NetworkCreated)
	n.Close()
	unix.Unmount(config.NetworkID, unix.MNT_DETACH)
	os.RemoveAll(config.NetworkID)

	// Config with DisableNewNetNs
	config = &vc.NetworkConfig{DisableNewNetwork: true}
	err = SetupNetworkNamespace(config)
	assert.NoError(err)
}
