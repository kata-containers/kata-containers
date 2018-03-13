//
// Copyright (c) 2016 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package virtcontainers

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"

	"github.com/sirupsen/logrus"
)

// podResource is an int representing a pod resource type.
//
// Note that some are specific to the pod itself and others can apply to
// pods and containers.
type podResource int

const (
	// configFileType represents a configuration file type
	configFileType podResource = iota

	// stateFileType represents a state file type
	stateFileType

	// networkFileType represents a network file type (pod only)
	networkFileType

	// hypervisorFileType represents a hypervisor file type (pod only)
	hypervisorFileType

	// agentFileType represents an agent file type (pod only)
	agentFileType

	// processFileType represents a process file type
	processFileType

	// lockFileType represents a lock file type (pod only)
	lockFileType

	// mountsFileType represents a mount file type
	mountsFileType

	// devicesFileType represents a device file type
	devicesFileType
)

// configFile is the file name used for every JSON pod configuration.
const configFile = "config.json"

// stateFile is the file name storing a pod state.
const stateFile = "state.json"

// networkFile is the file name storing a pod network.
const networkFile = "network.json"

// hypervisorFile is the file name storing a hypervisor's state.
const hypervisorFile = "hypervisor.json"

// agentFile is the file name storing an agent's state.
const agentFile = "agent.json"

// processFile is the file name storing a container process.
const processFile = "process.json"

// lockFile is the file name locking the usage of a pod.
const lockFileName = "lock"

const mountsFile = "mounts.json"

// devicesFile is the file name storing a container's devices.
const devicesFile = "devices.json"

// dirMode is the permission bits used for creating a directory
const dirMode = os.FileMode(0750) | os.ModeDir

// storagePathSuffix is the suffix used for all storage paths
const storagePathSuffix = "/virtcontainers/pods"

// configStoragePath is the pod configuration directory.
// It will contain one config.json file for each created pod.
var configStoragePath = filepath.Join("/var/lib", storagePathSuffix)

// runStoragePath is the pod runtime directory.
// It will contain one state.json and one lock file for each created pod.
var runStoragePath = filepath.Join("/run", storagePathSuffix)

// resourceStorage is the virtcontainers resources (configuration, state, etc...)
// storage interface.
// The default resource storage implementation is filesystem.
type resourceStorage interface {
	// Create all resources for a pod
	createAllResources(pod Pod) error

	// Resources URIs functions return both the URI
	// for the actual resource and the URI base.
	containerURI(podID, containerID string, resource podResource) (string, string, error)
	podURI(podID string, resource podResource) (string, string, error)

	// Pod resources
	storePodResource(podID string, resource podResource, data interface{}) error
	deletePodResources(podID string, resources []podResource) error
	fetchPodConfig(podID string) (PodConfig, error)
	fetchPodState(podID string) (State, error)
	fetchPodNetwork(podID string) (NetworkNamespace, error)
	storePodNetwork(podID string, networkNS NetworkNamespace) error

	// Hypervisor resources
	fetchHypervisorState(podID string, state interface{}) error
	storeHypervisorState(podID string, state interface{}) error

	// Agent resources
	fetchAgentState(podID string, state interface{}) error
	storeAgentState(podID string, state interface{}) error

	// Container resources
	storeContainerResource(podID, containerID string, resource podResource, data interface{}) error
	deleteContainerResources(podID, containerID string, resources []podResource) error
	fetchContainerConfig(podID, containerID string) (ContainerConfig, error)
	fetchContainerState(podID, containerID string) (State, error)
	fetchContainerProcess(podID, containerID string) (Process, error)
	storeContainerProcess(podID, containerID string, process Process) error
	fetchContainerMounts(podID, containerID string) ([]Mount, error)
	storeContainerMounts(podID, containerID string, mounts []Mount) error
	fetchContainerDevices(podID, containerID string) ([]Device, error)
	storeContainerDevices(podID, containerID string, devices []Device) error
}

// filesystem is a resourceStorage interface implementation for a local filesystem.
type filesystem struct {
}

// Logger returns a logrus logger appropriate for logging filesystem messages
func (fs *filesystem) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "filesystem")
}

func (fs *filesystem) createAllResources(pod Pod) (err error) {
	for _, resource := range []podResource{stateFileType, configFileType} {
		_, path, _ := fs.podURI(pod.id, resource)
		err = os.MkdirAll(path, dirMode)
		if err != nil {
			return err
		}
	}

	for _, container := range pod.containers {
		for _, resource := range []podResource{stateFileType, configFileType} {
			_, path, _ := fs.containerURI(pod.id, container.id, resource)
			err = os.MkdirAll(path, dirMode)
			if err != nil {
				fs.deletePodResources(pod.id, nil)
				return err
			}
		}
	}

	podlockFile, _, err := fs.podURI(pod.id, lockFileType)
	if err != nil {
		fs.deletePodResources(pod.id, nil)
		return err
	}

	_, err = os.Stat(podlockFile)
	if err != nil {
		lockFile, err := os.Create(podlockFile)
		if err != nil {
			fs.deletePodResources(pod.id, nil)
			return err
		}
		lockFile.Close()
	}

	return nil
}

func (fs *filesystem) storeFile(file string, data interface{}) error {
	if file == "" {
		return errNeedFile
	}

	f, err := os.Create(file)
	if err != nil {
		return err
	}
	defer f.Close()

	jsonOut, err := json.Marshal(data)
	if err != nil {
		return fmt.Errorf("Could not marshall data: %s", err)
	}
	f.Write(jsonOut)

	return nil
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

// storeDeviceFile is used to provide custom marshalling for Device objects.
// Device is first marshalled into TypedDevice to include the type
// of the Device object.
func (fs *filesystem) storeDeviceFile(file string, data interface{}) error {
	if file == "" {
		return errNeedFile
	}

	f, err := os.Create(file)
	if err != nil {
		return err
	}
	defer f.Close()

	devices, ok := data.([]Device)
	if !ok {
		return fmt.Errorf("Incorrect data type received, Expected []Device")
	}

	var typedDevices []TypedDevice
	for _, d := range devices {
		tempJSON, _ := json.Marshal(d)
		typedDevice := TypedDevice{
			Type: d.deviceType(),
			Data: tempJSON,
		}
		typedDevices = append(typedDevices, typedDevice)
	}

	jsonOut, err := json.Marshal(typedDevices)
	if err != nil {
		return fmt.Errorf("Could not marshal devices: %s", err)
	}

	if _, err := f.Write(jsonOut); err != nil {
		return err
	}

	return nil
}

func (fs *filesystem) fetchFile(file string, resource podResource, data interface{}) error {
	if file == "" {
		return errNeedFile
	}

	fileData, err := ioutil.ReadFile(file)
	if err != nil {
		return err
	}

	switch resource {
	case devicesFileType:
		devices, ok := data.(*[]Device)
		if !ok {
			return fmt.Errorf("Could not cast %v into *[]Device type", data)
		}

		return fs.fetchDeviceFile(fileData, devices)
	}

	return json.Unmarshal(fileData, data)
}

// fetchDeviceFile is used for custom unmarshalling of device interface objects.
func (fs *filesystem) fetchDeviceFile(fileData []byte, devices *[]Device) error {
	var typedDevices []TypedDevice
	if err := json.Unmarshal(fileData, &typedDevices); err != nil {
		return err
	}

	var tempDevices []Device
	for _, d := range typedDevices {
		l := fs.Logger().WithField("device-type", d.Type)
		l.Info("Device type found")

		switch d.Type {
		case DeviceVFIO:
			var device VFIODevice
			if err := json.Unmarshal(d.Data, &device); err != nil {
				return err
			}
			tempDevices = append(tempDevices, &device)
			l.Infof("VFIO device unmarshalled [%v]", device)

		case DeviceBlock:
			var device BlockDevice
			if err := json.Unmarshal(d.Data, &device); err != nil {
				return err
			}
			tempDevices = append(tempDevices, &device)
			l.Infof("Block Device unmarshalled [%v]", device)

		case DeviceGeneric:
			var device GenericDevice
			if err := json.Unmarshal(d.Data, &device); err != nil {
				return err
			}
			tempDevices = append(tempDevices, &device)
			l.Infof("Generic device unmarshalled [%v]", device)

		default:
			return fmt.Errorf("Unknown device type, could not unmarshal")
		}
	}

	*devices = tempDevices
	return nil
}

// resourceNeedsContainerID determines if the specified
// podResource needs a containerID. Since some podResources can
// be used for both pods and containers, it is necessary to specify
// whether the resource is being used in a pod-specific context using
// the podSpecific parameter.
func resourceNeedsContainerID(podSpecific bool, resource podResource) bool {

	switch resource {
	case lockFileType, networkFileType, hypervisorFileType, agentFileType:
		// pod-specific resources
		return false
	default:
		return !podSpecific
	}
}

func resourceDir(podSpecific bool, podID, containerID string, resource podResource) (string, error) {
	if podID == "" {
		return "", errNeedPodID
	}

	if resourceNeedsContainerID(podSpecific, resource) == true && containerID == "" {
		return "", errNeedContainerID
	}

	var path string

	switch resource {
	case configFileType:
		path = configStoragePath
		break
	case stateFileType, networkFileType, processFileType, lockFileType, mountsFileType, devicesFileType, hypervisorFileType, agentFileType:
		path = runStoragePath
		break
	default:
		return "", errInvalidResource
	}

	dirPath := filepath.Join(path, podID, containerID)

	return dirPath, nil
}

// If podSpecific is true, the resource is being applied for an empty
// pod (meaning containerID may be blank).
// Note that this function defers determining if containerID can be
// blank to resourceDIR()
func (fs *filesystem) resourceURI(podSpecific bool, podID, containerID string, resource podResource) (string, string, error) {
	if podID == "" {
		return "", "", errNeedPodID
	}

	var filename string

	dirPath, err := resourceDir(podSpecific, podID, containerID, resource)
	if err != nil {
		return "", "", err
	}

	switch resource {
	case configFileType:
		filename = configFile
		break
	case stateFileType:
		filename = stateFile
	case networkFileType:
		filename = networkFile
	case hypervisorFileType:
		filename = hypervisorFile
	case agentFileType:
		filename = agentFile
	case processFileType:
		filename = processFile
	case lockFileType:
		filename = lockFileName
		break
	case mountsFileType:
		filename = mountsFile
		break
	case devicesFileType:
		filename = devicesFile
		break
	default:
		return "", "", errInvalidResource
	}

	filePath := filepath.Join(dirPath, filename)

	return filePath, dirPath, nil
}

func (fs *filesystem) containerURI(podID, containerID string, resource podResource) (string, string, error) {
	if podID == "" {
		return "", "", errNeedPodID
	}

	if containerID == "" {
		return "", "", errNeedContainerID
	}

	return fs.resourceURI(false, podID, containerID, resource)
}

func (fs *filesystem) podURI(podID string, resource podResource) (string, string, error) {
	return fs.resourceURI(true, podID, "", resource)
}

// commonResourceChecks performs basic checks common to both setting and
// getting a podResource.
func (fs *filesystem) commonResourceChecks(podSpecific bool, podID, containerID string, resource podResource) error {
	if podID == "" {
		return errNeedPodID
	}

	if resourceNeedsContainerID(podSpecific, resource) == true && containerID == "" {
		return errNeedContainerID
	}

	switch resource {
	case configFileType:
	case stateFileType:
	case networkFileType:
	case hypervisorFileType:
	case agentFileType:
	case processFileType:
	case mountsFileType:
	case devicesFileType:
	default:
		return errInvalidResource
	}

	return nil
}

func (fs *filesystem) storePodAndContainerConfigResource(podSpecific bool, podID, containerID string, resource podResource, file interface{}) error {
	if resource != configFileType {
		return errInvalidResource
	}

	configFile, _, err := fs.resourceURI(podSpecific, podID, containerID, configFileType)
	if err != nil {
		return err
	}

	return fs.storeFile(configFile, file)
}

func (fs *filesystem) storeStateResource(podSpecific bool, podID, containerID string, resource podResource, file interface{}) error {
	if resource != stateFileType {
		return errInvalidResource
	}

	stateFile, _, err := fs.resourceURI(podSpecific, podID, containerID, stateFileType)
	if err != nil {
		return err
	}

	return fs.storeFile(stateFile, file)
}

func (fs *filesystem) storeNetworkResource(podSpecific bool, podID, containerID string, resource podResource, file interface{}) error {
	if resource != networkFileType {
		return errInvalidResource
	}

	// pod only resource
	networkFile, _, err := fs.resourceURI(true, podID, containerID, networkFileType)
	if err != nil {
		return err
	}

	return fs.storeFile(networkFile, file)
}

func (fs *filesystem) storeProcessResource(podSpecific bool, podID, containerID string, resource podResource, file interface{}) error {
	if resource != processFileType {
		return errInvalidResource
	}

	processFile, _, err := fs.resourceURI(podSpecific, podID, containerID, processFileType)
	if err != nil {
		return err
	}

	return fs.storeFile(processFile, file)
}

func (fs *filesystem) storeMountResource(podSpecific bool, podID, containerID string, resource podResource, file interface{}) error {
	if resource != mountsFileType {
		return errInvalidResource
	}

	mountsFile, _, err := fs.resourceURI(podSpecific, podID, containerID, mountsFileType)
	if err != nil {
		return err
	}

	return fs.storeFile(mountsFile, file)
}

func (fs *filesystem) storeDeviceResource(podSpecific bool, podID, containerID string, resource podResource, file interface{}) error {
	if resource != devicesFileType {
		return errInvalidResource
	}

	devicesFile, _, err := fs.resourceURI(podSpecific, podID, containerID, devicesFileType)
	if err != nil {
		return err
	}

	return fs.storeDeviceFile(devicesFile, file)
}

func (fs *filesystem) storeResource(podSpecific bool, podID, containerID string, resource podResource, data interface{}) error {
	if err := fs.commonResourceChecks(podSpecific, podID, containerID, resource); err != nil {
		return err
	}

	switch file := data.(type) {
	case PodConfig, ContainerConfig:
		return fs.storePodAndContainerConfigResource(podSpecific, podID, containerID, resource, file)

	case State:
		return fs.storeStateResource(podSpecific, podID, containerID, resource, file)

	case NetworkNamespace:
		return fs.storeNetworkResource(podSpecific, podID, containerID, resource, file)

	case Process:
		return fs.storeProcessResource(podSpecific, podID, containerID, resource, file)

	case []Mount:
		return fs.storeMountResource(podSpecific, podID, containerID, resource, file)

	case []Device:
		return fs.storeDeviceResource(podSpecific, podID, containerID, resource, file)

	default:
		return fmt.Errorf("Invalid resource data type")
	}
}

func (fs *filesystem) fetchResource(podSpecific bool, podID, containerID string, resource podResource, data interface{}) error {
	if err := fs.commonResourceChecks(podSpecific, podID, containerID, resource); err != nil {
		return err
	}

	path, _, err := fs.resourceURI(podSpecific, podID, containerID, resource)
	if err != nil {
		return err
	}

	return fs.fetchFile(path, resource, data)
}

func (fs *filesystem) storePodResource(podID string, resource podResource, data interface{}) error {
	return fs.storeResource(true, podID, "", resource, data)
}

func (fs *filesystem) fetchPodConfig(podID string) (PodConfig, error) {
	var podConfig PodConfig

	if err := fs.fetchResource(true, podID, "", configFileType, &podConfig); err != nil {
		return PodConfig{}, err
	}

	return podConfig, nil
}

func (fs *filesystem) fetchPodState(podID string) (State, error) {
	var state State

	if err := fs.fetchResource(true, podID, "", stateFileType, &state); err != nil {
		return State{}, err
	}

	return state, nil
}

func (fs *filesystem) fetchPodNetwork(podID string) (NetworkNamespace, error) {
	var networkNS NetworkNamespace

	if err := fs.fetchResource(true, podID, "", networkFileType, &networkNS); err != nil {
		return NetworkNamespace{}, err
	}

	return networkNS, nil
}

func (fs *filesystem) fetchHypervisorState(podID string, state interface{}) error {
	return fs.fetchResource(true, podID, "", hypervisorFileType, state)
}

func (fs *filesystem) fetchAgentState(podID string, state interface{}) error {
	return fs.fetchResource(true, podID, "", agentFileType, state)
}

func (fs *filesystem) storePodNetwork(podID string, networkNS NetworkNamespace) error {
	return fs.storePodResource(podID, networkFileType, networkNS)
}

func (fs *filesystem) storeHypervisorState(podID string, state interface{}) error {
	hypervisorFile, _, err := fs.resourceURI(true, podID, "", hypervisorFileType)
	if err != nil {
		return err
	}

	return fs.storeFile(hypervisorFile, state)
}

func (fs *filesystem) storeAgentState(podID string, state interface{}) error {
	agentFile, _, err := fs.resourceURI(true, podID, "", agentFileType)
	if err != nil {
		return err
	}

	return fs.storeFile(agentFile, state)
}

func (fs *filesystem) deletePodResources(podID string, resources []podResource) error {
	if resources == nil {
		resources = []podResource{configFileType, stateFileType}
	}

	for _, resource := range resources {
		_, dir, err := fs.podURI(podID, resource)
		if err != nil {
			return err
		}

		err = os.RemoveAll(dir)
		if err != nil {
			return err
		}
	}

	return nil
}

func (fs *filesystem) storeContainerResource(podID, containerID string, resource podResource, data interface{}) error {
	if podID == "" {
		return errNeedPodID
	}

	if containerID == "" {
		return errNeedContainerID
	}

	return fs.storeResource(false, podID, containerID, resource, data)
}

func (fs *filesystem) fetchContainerConfig(podID, containerID string) (ContainerConfig, error) {
	var config ContainerConfig

	if err := fs.fetchResource(false, podID, containerID, configFileType, &config); err != nil {
		return ContainerConfig{}, err
	}

	return config, nil
}

func (fs *filesystem) fetchContainerState(podID, containerID string) (State, error) {
	var state State

	if err := fs.fetchResource(false, podID, containerID, stateFileType, &state); err != nil {
		return State{}, err
	}

	return state, nil
}

func (fs *filesystem) fetchContainerProcess(podID, containerID string) (Process, error) {
	var process Process

	if err := fs.fetchResource(false, podID, containerID, processFileType, &process); err != nil {
		return Process{}, err
	}

	return process, nil
}

func (fs *filesystem) storeContainerProcess(podID, containerID string, process Process) error {
	return fs.storeContainerResource(podID, containerID, processFileType, process)
}

func (fs *filesystem) fetchContainerMounts(podID, containerID string) ([]Mount, error) {
	var mounts []Mount

	if err := fs.fetchResource(false, podID, containerID, mountsFileType, &mounts); err != nil {
		return []Mount{}, err
	}

	return mounts, nil
}

func (fs *filesystem) fetchContainerDevices(podID, containerID string) ([]Device, error) {
	var devices []Device

	if err := fs.fetchResource(false, podID, containerID, devicesFileType, &devices); err != nil {
		return []Device{}, err
	}

	return devices, nil
}

func (fs *filesystem) storeContainerMounts(podID, containerID string, mounts []Mount) error {
	return fs.storeContainerResource(podID, containerID, mountsFileType, mounts)
}

func (fs *filesystem) storeContainerDevices(podID, containerID string, devices []Device) error {
	return fs.storeContainerResource(podID, containerID, devicesFileType, devices)
}

func (fs *filesystem) deleteContainerResources(podID, containerID string, resources []podResource) error {
	if resources == nil {
		resources = []podResource{configFileType, stateFileType}
	}

	for _, resource := range resources {
		_, dir, err := fs.podURI(podID, resource)
		if err != nil {
			return err
		}

		containerDir := filepath.Join(dir, containerID, "/")

		err = os.RemoveAll(containerDir)
		if err != nil {
			return err
		}
	}

	return nil
}
