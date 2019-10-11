// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package store

import (
	"context"
	"encoding/json"
	"fmt"
	"path/filepath"

	"github.com/kata-containers/runtime/virtcontainers/device/api"
	"github.com/kata-containers/runtime/virtcontainers/device/config"
	"github.com/kata-containers/runtime/virtcontainers/device/drivers"
	"github.com/kata-containers/runtime/virtcontainers/types"
)

// VCStorePrefix is only used for tests to config a temp store dir
var VCStorePrefix = ""

// VCStore is a virtcontainers specific Store.
// Virtcontainers typically needs a configuration Store for
// storing permanent items across reboots.
// It also needs a state Store for storing states and other run-time
// related items. Those should not survive a reboot.
//
// VCStore simply dispatches items into the right Store.
type VCStore struct {
	config, state, uuid *Store
}

func (s *VCStore) itemToStore(item Item) *Store {
	switch item {
	case Configuration:
		return s.config
	case State, Network, Hypervisor, Agent, Process, Lock, Mounts, Devices, DeviceIDs:
		return s.state
	case UUID:
		return s.uuid
	}

	return s.state
}

// NewVCStore creates a virtcontainers specific Store.
func NewVCStore(ctx context.Context, configRoot, stateRoot string) (*VCStore, error) {
	config, err := New(ctx, configRoot)
	if err != nil {
		fmt.Printf("config root %s\n", configRoot)
		return nil, err
	}

	state, err := New(ctx, stateRoot)
	if err != nil {
		return nil, err
	}

	uuid, err := New(ctx, VCStoreUUIDPath())
	if err != nil {
		return nil, err
	}

	return &VCStore{
		config: config,
		state:  state,
		uuid:   uuid,
	}, nil
}

// NewVCSandboxStore creates a virtcontainers sandbox Store, with filesystem backend.
func NewVCSandboxStore(ctx context.Context, sandboxID string) (*VCStore, error) {
	if sandboxID == "" {
		return nil, fmt.Errorf("sandbox ID can not be empty")
	}

	return NewVCStore(ctx,
		SandboxConfigurationRoot(sandboxID),
		SandboxRuntimeRoot(sandboxID),
	)
}

// NewVCContainerStore creates a virtcontainers container Store, with filesystem backend.
func NewVCContainerStore(ctx context.Context, sandboxID, containerID string) (*VCStore, error) {
	if sandboxID == "" {
		return nil, fmt.Errorf("sandbox ID can not be empty")
	}

	if containerID == "" {
		return nil, fmt.Errorf("container ID can not be empty")
	}

	return NewVCStore(ctx,
		ContainerConfigurationRoot(sandboxID, containerID),
		ContainerRuntimeRoot(sandboxID, containerID),
	)
}

// Store stores a virtcontainers item into the right Store.
func (s *VCStore) Store(item Item, data interface{}) error {
	return s.itemToStore(item).Store(item, data)
}

// Load loads a virtcontainers item from the right Store.
func (s *VCStore) Load(item Item, data interface{}) error {
	return s.itemToStore(item).Load(item, data)
}

// Delete deletes all artifacts created by a VCStore.
// Both config and state Stores are also removed from the manager.
func (s *VCStore) Delete() error {
	if err := s.config.Delete(); err != nil {
		return err
	}

	if err := s.state.Delete(); err != nil {
		return err
	}

	return nil
}

// LoadState loads an returns a virtcontainer state
func (s *VCStore) LoadState() (types.SandboxState, error) {
	var state types.SandboxState

	if err := s.state.Load(State, &state); err != nil {
		return types.SandboxState{}, err
	}

	return state, nil
}

// LoadContainerState loads an returns a virtcontainer state
func (s *VCStore) LoadContainerState() (types.ContainerState, error) {
	var state types.ContainerState

	if err := s.state.Load(State, &state); err != nil {
		return types.ContainerState{}, err
	}

	return state, nil
}

// TypedDevice is used as an intermediate representation for marshalling
// and unmarshalling Device implementations.
type TypedDevice struct {
	Type string

	// Data is assigned the Device object.
	// This being declared as RawMessage prevents it from being  marshalled/unmarshalled.
	// We do that explicitly depending on Type.
	Data json.RawMessage
}

// StoreDevices stores a virtcontainers devices slice.
// The Device slice is first marshalled into a TypedDevice
// one to include the type of the Device objects.
func (s *VCStore) StoreDevices(devices []api.Device) error {
	var typedDevices []TypedDevice

	for _, d := range devices {
		tempJSON, _ := json.Marshal(d)
		typedDevice := TypedDevice{
			Type: string(d.DeviceType()),
			Data: tempJSON,
		}
		typedDevices = append(typedDevices, typedDevice)
	}

	return s.state.Store(Devices, typedDevices)
}

// LoadDevices loads an returns a virtcontainer devices slice.
// We need a custom unmarshalling routine for translating TypedDevices
// into api.Devices based on their type.
func (s *VCStore) LoadDevices() ([]api.Device, error) {
	var typedDevices []TypedDevice
	var devices []api.Device

	if err := s.state.Load(Devices, &typedDevices); err != nil {
		return []api.Device{}, err
	}

	for _, d := range typedDevices {
		switch d.Type {
		case string(config.DeviceVFIO):
			// TODO: remove dependency of drivers package
			var device drivers.VFIODevice
			if err := json.Unmarshal(d.Data, &device); err != nil {
				return []api.Device{}, err
			}
			devices = append(devices, &device)
		case string(config.DeviceBlock):
			// TODO: remove dependency of drivers package
			var device drivers.BlockDevice
			if err := json.Unmarshal(d.Data, &device); err != nil {
				return []api.Device{}, err
			}
			devices = append(devices, &device)
		case string(config.DeviceGeneric):
			// TODO: remove dependency of drivers package
			var device drivers.GenericDevice
			if err := json.Unmarshal(d.Data, &device); err != nil {
				return []api.Device{}, err
			}
			devices = append(devices, &device)
		default:
			return []api.Device{}, fmt.Errorf("Unknown device type, could not unmarshal")
		}
	}

	return devices, nil
}

// Raw creates a raw item in the virtcontainer state Store. A raw
// item is a custom one, not defined through the Item enum, and that
// the caller needs to handle directly.
// Typically this is used to create a custom virtcontainers file.
// For example the Firecracker code uses this API to create temp
// files under the sandbox state root path, and uses them as block
// driver backend placeholder.
func (s *VCStore) Raw(id string) (string, error) {
	return s.state.Raw(id)
}

// Lock takes an exclusive lock on the virtcontainers state Lock item.
func (s *VCStore) Lock() (string, error) {
	return s.state.ItemLock(Lock, true)
}

// RLock takes a shared lock on the virtcontainers state Lock item.
func (s *VCStore) RLock() (string, error) {
	return s.state.ItemLock(Lock, false)
}

// Unlock unlocks the virtcontainers state Lock item.
func (s *VCStore) Unlock(token string) error {
	return s.state.ItemUnlock(Lock, token)
}

// Utilities for virtcontainers

// SandboxConfigurationRoot returns a virtcontainers sandbox configuration root URL.
// This will hold across host reboot persistent data about a sandbox configuration.
// It should look like file:///var/lib/vc/sbs/<sandboxID>/
// Or for rootless: file://<rootlessDir>/var/lib/vc/sbs/<sandboxID>/
func SandboxConfigurationRoot(id string) string {
	return filesystemScheme + "://" + SandboxConfigurationRootPath(id)
}

// SandboxConfigurationRootPath returns a virtcontainers sandbox configuration root path.
func SandboxConfigurationRootPath(id string) string {
	return filepath.Join(VCStorePrefix, ConfigStoragePath(), id)
}

// SandboxConfigurationItemPath returns a virtcontainers sandbox configuration item path.
func SandboxConfigurationItemPath(id string, item Item) (string, error) {
	if id == "" {
		return "", fmt.Errorf("Empty sandbox ID")
	}

	itemFile, err := itemToFile(item)
	if err != nil {
		return "", err
	}

	return filepath.Join(VCStorePrefix, ConfigStoragePath(), id, itemFile), nil
}

// VCStoreUUIDPath returns a virtcontainers runtime uuid URL.
func VCStoreUUIDPath() string {
	return filesystemScheme + "://" + filepath.Join(VCStorePrefix, VMUUIDStoragePath())
}

// SandboxRuntimeRoot returns a virtcontainers sandbox runtime root URL.
// This will hold data related to a sandbox run-time state that will not
// be persistent across host reboots.
// It should look like file:///run/vc/sbs/<sandboxID>/
// or if rootless: file://<rootlessDir>/run/vc/sbs/<sandboxID>/
func SandboxRuntimeRoot(id string) string {
	return filesystemScheme + "://" + SandboxRuntimeRootPath(id)
}

// SandboxRuntimeRootPath returns a virtcontainers sandbox runtime root path.
func SandboxRuntimeRootPath(id string) string {
	return filepath.Join(VCStorePrefix, RunStoragePath(), id)
}

// SandboxRuntimeItemPath returns a virtcontainers sandbox runtime item path.
func SandboxRuntimeItemPath(id string, item Item) (string, error) {
	if id == "" {
		return "", fmt.Errorf("Empty sandbox ID")
	}

	itemFile, err := itemToFile(item)
	if err != nil {
		return "", err
	}

	return filepath.Join(RunStoragePath(), id, itemFile), nil
}

// ContainerConfigurationRoot returns a virtcontainers container configuration root URL.
// This will hold across host reboot persistent data about a container configuration.
// It should look like file:///var/lib/vc/sbs/<sandboxID>/<containerID>
// Or if rootless file://<rootlessDir>/var/lib/vc/sbs/<sandboxID>/<containerID>
func ContainerConfigurationRoot(sandboxID, containerID string) string {
	return filesystemScheme + "://" + ContainerConfigurationRootPath(sandboxID, containerID)
}

// ContainerConfigurationRootPath returns a virtcontainers container configuration root path.
func ContainerConfigurationRootPath(sandboxID, containerID string) string {
	return filepath.Join(VCStorePrefix, ConfigStoragePath(), sandboxID, containerID)
}

// ContainerRuntimeRoot returns a virtcontainers container runtime root URL.
// This will hold data related to a container run-time state that will not
// be persistent across host reboots.
// It should look like file:///run/vc/sbs/<sandboxID>/<containerID>/
// Or for rootless file://<rootlessDir>/run/vc/sbs/<sandboxID>/<containerID>/
func ContainerRuntimeRoot(sandboxID, containerID string) string {
	return filesystemScheme + "://" + ContainerRuntimeRootPath(sandboxID, containerID)
}

// ContainerRuntimeRootPath returns a virtcontainers container runtime root path.
func ContainerRuntimeRootPath(sandboxID, containerID string) string {
	return filepath.Join(VCStorePrefix, RunStoragePath(), sandboxID, containerID)
}

// VCSandboxStoreExists returns true if a sandbox store already exists.
func VCSandboxStoreExists(ctx context.Context, sandboxID string) bool {
	s := stores.findStore(SandboxConfigurationRoot(sandboxID))
	return s != nil
}

// VCContainerStoreExists returns true if a container store already exists.
func VCContainerStoreExists(ctx context.Context, sandboxID string, containerID string) bool {
	s := stores.findStore(ContainerConfigurationRoot(sandboxID, containerID))
	return s != nil
}
