// Copyright (c) 2018 Huawei Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"flag"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"syscall"
	"testing"

	"golang.org/x/sys/unix"

	"github.com/containernetworking/plugins/pkg/ns"
	vc "github.com/kata-containers/runtime/virtcontainers"
	"github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/stretchr/testify/assert"
)

var (
	testAddInterfaceFuncReturnNil = func(ctx context.Context, sandboxID string, inf *types.Interface) (*types.Interface, error) {
		return nil, nil
	}
	testRemoveInterfaceFuncReturnNil = func(ctx context.Context, sandboxID string, inf *types.Interface) (*types.Interface, error) {
		return nil, nil
	}
	testListInterfacesFuncReturnNil = func(ctx context.Context, sandboxID string) ([]*types.Interface, error) {
		return nil, nil
	}
	testUpdateRoutsFuncReturnNil = func(ctx context.Context, sandboxID string, routes []*types.Route) ([]*types.Route, error) {
		return nil, nil
	}
	testListRoutesFuncReturnNil = func(ctx context.Context, sandboxID string) ([]*types.Route, error) {
		return nil, nil
	}
)

func TestNetworkCliFunction(t *testing.T) {
	assert := assert.New(t)

	state := vc.State{
		State: vc.StateRunning,
	}

	testingImpl.AddInterfaceFunc = testAddInterfaceFuncReturnNil
	testingImpl.RemoveInterfaceFunc = testRemoveInterfaceFuncReturnNil
	testingImpl.ListInterfacesFunc = testListInterfacesFuncReturnNil
	testingImpl.UpdateRoutesFunc = testUpdateRoutsFuncReturnNil
	testingImpl.ListRoutesFunc = testListRoutesFuncReturnNil

	path, err := createTempContainerIDMapping(testContainerID, testSandboxID)
	assert.NoError(err)
	defer os.RemoveAll(path)

	testingImpl.StatusContainerFunc = func(ctx context.Context, sandboxID, containerID string) (vc.ContainerStatus, error) {
		return newSingleContainerStatus(testContainerID, state, map[string]string{}), nil
	}

	defer func() {
		testingImpl.AddInterfaceFunc = nil
		testingImpl.RemoveInterfaceFunc = nil
		testingImpl.ListInterfacesFunc = nil
		testingImpl.UpdateRoutesFunc = nil
		testingImpl.ListRoutesFunc = nil
		testingImpl.StatusContainerFunc = nil
	}()

	set := flag.NewFlagSet("", 0)
	execCLICommandFunc(assert, addIfaceCommand, set, true)

	set.Parse([]string{testContainerID})
	execCLICommandFunc(assert, listIfacesCommand, set, false)
	execCLICommandFunc(assert, listRoutesCommand, set, false)

	f, err := ioutil.TempFile("", "interface")
	defer os.Remove(f.Name())
	assert.NoError(err)
	assert.NotNil(f)
	f.WriteString("{}")

	set.Parse([]string{testContainerID, f.Name()})
	execCLICommandFunc(assert, addIfaceCommand, set, false)
	execCLICommandFunc(assert, delIfaceCommand, set, false)

	f.Seek(0, 0)
	f.WriteString("[{}]")
	f.Close()
	execCLICommandFunc(assert, updateRoutesCommand, set, false)
}

func TestGetNetNsFromBindMount(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	mountFile := filepath.Join(tmpdir, "mountInfo")
	nsPath := filepath.Join(tmpdir, "ns123")

	// Non-existent namespace path
	_, err = getNetNsFromBindMount(nsPath, mountFile)
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
		err := ioutil.WriteFile(mountFile, []byte(d.contents), 0640)
		assert.NoError(err)

		path, err := getNetNsFromBindMount(tmpNSPath, mountFile)
		assert.NoError(err, fmt.Sprintf("got %q, test data: %+v", path, d))

		assert.Equal(d.expectedResult, path, "Test %d, expected %s, got %s", i, d.expectedResult, path)
	}
}

func TestHostNetworkingRequested(t *testing.T) {
	assert := assert.New(t)

	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedRoot)
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
	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

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
	if os.Geteuid() != 0 {
		t.Skip(testDisabledNeedNonRoot)
	}

	assert := assert.New(t)

	// Network namespace same as the host
	config := &vc.NetworkConfig{
		NetNSPath: "/proc/self/ns/net",
	}
	err := setupNetworkNamespace(config)
	assert.Error(err)

	// Non-existent netns path
	config = &vc.NetworkConfig{
		NetNSPath: "/proc/123456789/ns/net",
	}
	err = setupNetworkNamespace(config)
	assert.Error(err)

	// Existent netns path
	n, err := ns.NewNS()
	assert.NoError(err)
	config = &vc.NetworkConfig{
		NetNSPath: n.Path(),
	}
	err = setupNetworkNamespace(config)
	assert.NoError(err)
	n.Close()

	// Empty netns path
	config = &vc.NetworkConfig{}
	err = setupNetworkNamespace(config)
	assert.NoError(err)
	n, err = ns.GetNS(config.NetNSPath)
	assert.NoError(err)
	assert.NotNil(n)
	assert.True(config.NetNsCreated)
	n.Close()
	unix.Unmount(config.NetNSPath, unix.MNT_DETACH)
	os.RemoveAll(config.NetNSPath)

	// Config with DisableNewNetNs
	config = &vc.NetworkConfig{DisableNewNetNs: true}
	err = setupNetworkNamespace(config)
	assert.NoError(err)
}
