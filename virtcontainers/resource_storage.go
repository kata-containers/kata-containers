// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"encoding/json"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

// TypedDevice is used as an intermediate representation for marshalling
// and unmarshalling Device implementations.
type TypedDevice struct {
	Type string

	// Data is assigned the Device object.
	// This being declared as RawMessage prevents it from being  marshalled/unmarshalled.
	// We do that explicitly depending on Type.
	Data json.RawMessage
}

// resourceStorage is the virtcontainers resources (configuration, state, etc...)
// storage interface.
// The default resource storage implementation is filesystem.
type resourceStorage interface {
	// Create all resources for a sandbox
	createAllResources(ctx context.Context, sandbox *Sandbox) error

	// Resources URIs functions return both the URI
	// for the actual resource and the URI base.
	containerURI(sandboxID, containerID string, resource sandboxResource) (string, string, error)
	sandboxURI(sandboxID string, resource sandboxResource) (string, string, error)

	// Sandbox resources
	storeSandboxResource(sandboxID string, resource sandboxResource, data interface{}) error
	deleteSandboxResources(sandboxID string, resources []sandboxResource) error
	fetchSandboxConfig(sandboxID string) (SandboxConfig, error)
	fetchSandboxState(sandboxID string) (types.State, error)
	fetchSandboxNetwork(sandboxID string) (NetworkNamespace, error)
	storeSandboxNetwork(sandboxID string, networkNS NetworkNamespace) error
	fetchSandboxDevices(sandboxID string) ([]api.Device, error)
	storeSandboxDevices(sandboxID string, devices []api.Device) error

	// Hypervisor resources
	fetchHypervisorState(sandboxID string, state interface{}) error
	storeHypervisorState(sandboxID string, state interface{}) error

	// Agent resources
	fetchAgentState(sandboxID string, state interface{}) error
	storeAgentState(sandboxID string, state interface{}) error

	// Container resources
	storeContainerResource(sandboxID, containerID string, resource sandboxResource, data interface{}) error
	deleteContainerResources(sandboxID, containerID string, resources []sandboxResource) error
	fetchContainerConfig(sandboxID, containerID string) (ContainerConfig, error)
	fetchContainerState(sandboxID, containerID string) (types.State, error)
	fetchContainerProcess(sandboxID, containerID string) (Process, error)
	storeContainerProcess(sandboxID, containerID string, process Process) error
	fetchContainerMounts(sandboxID, containerID string) ([]Mount, error)
	storeContainerMounts(sandboxID, containerID string, mounts []Mount) error
	fetchContainerDevices(sandboxID, containerID string) ([]ContainerDevice, error)
	storeContainerDevices(sandboxID, containerID string, devices []ContainerDevice) error
	createSandboxTempFile(sandboxID string) (string, error)
}
