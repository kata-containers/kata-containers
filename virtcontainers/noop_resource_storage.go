// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"github.com/kata-containers/runtime/virtcontainers/device/api"
)

type noopResourceStorage struct{}

func (n *noopResourceStorage) createAllResources(sandbox *Sandbox) error {
	return nil
}

func (n *noopResourceStorage) containerURI(sandboxID, containerID string, resource sandboxResource) (string, string, error) {
	return "", "", nil
}

func (n *noopResourceStorage) sandboxURI(sandboxID string, resource sandboxResource) (string, string, error) {
	return "", "", nil
}

func (n *noopResourceStorage) storeSandboxResource(sandboxID string, resource sandboxResource, data interface{}) error {
	return nil
}

func (n *noopResourceStorage) deleteSandboxResources(sandboxID string, resources []sandboxResource) error {
	return nil
}

func (n *noopResourceStorage) fetchSandboxConfig(sandboxID string) (SandboxConfig, error) {
	return SandboxConfig{}, nil
}

func (n *noopResourceStorage) fetchSandboxState(sandboxID string) (State, error) {
	return State{}, nil
}

func (n *noopResourceStorage) fetchSandboxNetwork(sandboxID string) (NetworkNamespace, error) {
	return NetworkNamespace{}, nil
}

func (n *noopResourceStorage) storeSandboxNetwork(sandboxID string, networkNS NetworkNamespace) error {
	return nil
}

func (n *noopResourceStorage) fetchSandboxDevices(sandboxID string) ([]api.Device, error) {
	return []api.Device{}, nil
}

func (n *noopResourceStorage) storeSandboxDevices(sandboxID string, devices []api.Device) error {
	return nil
}

func (n *noopResourceStorage) fetchHypervisorState(sandboxID string, state interface{}) error {
	return nil
}

func (n *noopResourceStorage) storeHypervisorState(sandboxID string, state interface{}) error {
	return nil
}

func (n *noopResourceStorage) fetchAgentState(sandboxID string, state interface{}) error {
	return nil
}

func (n *noopResourceStorage) storeAgentState(sandboxID string, state interface{}) error {
	return nil
}

func (n *noopResourceStorage) storeContainerResource(sandboxID, containerID string, resource sandboxResource, data interface{}) error {
	return nil
}

func (n *noopResourceStorage) deleteContainerResources(sandboxID, containerID string, resources []sandboxResource) error {
	return nil
}

func (n *noopResourceStorage) fetchContainerConfig(sandboxID, containerID string) (ContainerConfig, error) {
	return ContainerConfig{}, nil
}

func (n *noopResourceStorage) fetchContainerState(sandboxID, containerID string) (State, error) {
	return State{}, nil
}

func (n *noopResourceStorage) fetchContainerProcess(sandboxID, containerID string) (Process, error) {
	return Process{}, nil
}

func (n *noopResourceStorage) storeContainerProcess(sandboxID, containerID string, process Process) error {
	return nil
}

func (n *noopResourceStorage) fetchContainerMounts(sandboxID, containerID string) ([]Mount, error) {
	return []Mount{}, nil
}

func (n *noopResourceStorage) storeContainerMounts(sandboxID, containerID string, mounts []Mount) error {
	return nil
}

func (n *noopResourceStorage) fetchContainerDevices(sandboxID, containerID string) ([]ContainerDevice, error) {
	return []ContainerDevice{}, nil
}

func (n *noopResourceStorage) storeContainerDevices(sandboxID, containerID string, devices []ContainerDevice) error {
	return nil
}
