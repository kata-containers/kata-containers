// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/stretchr/testify/assert"
)

func TestNoopCreateAllResources(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.createAllResources(nil)
	assert.Nil(t, err)
}

func TestNoopContainerURI(t *testing.T) {
	n := &noopResourceStorage{}

	_, _, err := n.containerURI("", "", 0)
	assert.Nil(t, err)
}

func TestNoopSandboxURI(t *testing.T) {
	n := &noopResourceStorage{}

	_, _, err := n.sandboxURI("", 0)
	assert.Nil(t, err)
}

func TestNoopStoreSandboxResource(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeSandboxResource("", 0, nil)
	assert.Nil(t, err)
}

func TestNoopDeleteSandboxResources(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.deleteSandboxResources("", []sandboxResource{0})
	assert.Nil(t, err)
}

func TestNoopFetchSandboxConfig(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchSandboxConfig("")
	assert.Nil(t, err)
}

func TestNoopFetchSandboxState(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchSandboxState("")
	assert.Nil(t, err)
}

func TestNoopFetchSandboxNetwork(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchSandboxNetwork("")
	assert.Nil(t, err)
}

func TestNoopStoreSandboxNetwork(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeSandboxNetwork("", NetworkNamespace{})
	assert.Nil(t, err)
}

func TestNoopFetchSandboxDevices(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchSandboxDevices("")
	assert.Nil(t, err)
}

func TestNoopStoreSandboxDevices(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeSandboxDevices("", []api.Device{})
	assert.Nil(t, err)
}

func TestNoopFetchHypervisorState(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.fetchHypervisorState("", nil)
	assert.Nil(t, err)
}

func TestNoopStoreHypervisorState(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeHypervisorState("", nil)
	assert.Nil(t, err)
}

func TestNoopFetchAgentState(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.fetchAgentState("", nil)
	assert.Nil(t, err)
}

func TestNoopStoreAgentState(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeAgentState("", nil)
	assert.Nil(t, err)
}

func TestNoopStoreContainerResource(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeContainerResource("", "", 0, nil)
	assert.Nil(t, err)
}

func TestNoopDeleteContainerResources(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.deleteContainerResources("", "", []sandboxResource{0})
	assert.Nil(t, err)
}

func TestNoopFetchContainerConfig(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchContainerConfig("", "")
	assert.Nil(t, err)
}

func TestNoopFetchContainerState(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchContainerState("", "")
	assert.Nil(t, err)
}

func TestNoopFetchContainerProcess(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchContainerProcess("", "")
	assert.Nil(t, err)
}

func TestNoopStoreContainerProcess(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeContainerProcess("", "", Process{})
	assert.Nil(t, err)
}

func TestNoopFetchContainerMounts(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchContainerMounts("", "")
	assert.Nil(t, err)
}

func TestNoopStoreContainerMounts(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeContainerMounts("", "", []Mount{})
	assert.Nil(t, err)
}

func TestNoopFetchContainerDevices(t *testing.T) {
	n := &noopResourceStorage{}

	_, err := n.fetchContainerDevices("", "")
	assert.Nil(t, err)
}

func TestNoopStoreContainerDevices(t *testing.T) {
	n := &noopResourceStorage{}

	err := n.storeContainerDevices("", "", []ContainerDevice{})
	assert.Nil(t, err)
}
