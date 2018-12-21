// Copyright (c) 2018 Huawei Corporation.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"flag"
	"io/ioutil"
	"os"
	"testing"

	vc "github.com/kata-containers/runtime/virtcontainers"
	vcTypes "github.com/kata-containers/runtime/virtcontainers/pkg/types"
	"github.com/kata-containers/runtime/virtcontainers/types"
	"github.com/stretchr/testify/assert"
)

var (
	testAddInterfaceFuncReturnNil = func(ctx context.Context, sandboxID string, inf *vcTypes.Interface) (*vcTypes.Interface, error) {
		return nil, nil
	}
	testRemoveInterfaceFuncReturnNil = func(ctx context.Context, sandboxID string, inf *vcTypes.Interface) (*vcTypes.Interface, error) {
		return nil, nil
	}
	testListInterfacesFuncReturnNil = func(ctx context.Context, sandboxID string) ([]*vcTypes.Interface, error) {
		return nil, nil
	}
	testUpdateRoutsFuncReturnNil = func(ctx context.Context, sandboxID string, routes []*vcTypes.Route) ([]*vcTypes.Route, error) {
		return nil, nil
	}
	testListRoutesFuncReturnNil = func(ctx context.Context, sandboxID string) ([]*vcTypes.Route, error) {
		return nil, nil
	}
)

func TestNetworkCliFunction(t *testing.T) {
	assert := assert.New(t)

	state := types.State{
		State: types.StateRunning,
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
