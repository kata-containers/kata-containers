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
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestFilesystemCreateAllResourcesSuccessful(t *testing.T) {
	fs := &filesystem{}

	contConfigs := []ContainerConfig{
		{ID: "1"},
		{ID: "10"},
		{ID: "100"},
	}

	podConfig := &PodConfig{
		Containers: contConfigs,
	}

	pod := Pod{
		id:      testPodID,
		storage: fs,
		config:  podConfig,
	}

	if err := pod.newContainers(); err != nil {
		t.Fatal(err)
	}

	podConfigPath := filepath.Join(configStoragePath, testPodID)
	podRunPath := filepath.Join(runStoragePath, testPodID)

	os.RemoveAll(podConfigPath)
	os.RemoveAll(podRunPath)

	for _, container := range contConfigs {
		configPath := filepath.Join(configStoragePath, testPodID, container.ID)
		os.RemoveAll(configPath)

		runPath := filepath.Join(runStoragePath, testPodID, container.ID)
		os.RemoveAll(runPath)
	}

	err := fs.createAllResources(pod)
	if err != nil {
		t.Fatal(err)
	}

	// Check resources
	_, err = os.Stat(podConfigPath)
	if err != nil {
		t.Fatal(err)
	}

	_, err = os.Stat(podRunPath)
	if err != nil {
		t.Fatal(err)
	}

	for _, container := range contConfigs {
		configPath := filepath.Join(configStoragePath, testPodID, container.ID)
		s, err := os.Stat(configPath)
		if err != nil {
			t.Fatal(err)
		}

		// Check we created the dirs with the correct mode
		if s.Mode() != dirMode {
			t.Fatal(fmt.Errorf("dirmode [%v] != expected [%v]", s.Mode(), dirMode))
		}

		runPath := filepath.Join(runStoragePath, testPodID, container.ID)
		s, err = os.Stat(runPath)
		if err != nil {
			t.Fatal(err)
		}

		// Check we created the dirs with the correct mode
		if s.Mode() != dirMode {
			t.Fatal(fmt.Errorf("dirmode [%v] != expected [%v]", s.Mode(), dirMode))
		}

	}
}

func TestFilesystemCreateAllResourcesFailingPodIDEmpty(t *testing.T) {
	fs := &filesystem{}

	pod := Pod{}

	err := fs.createAllResources(pod)
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemCreateAllResourcesFailingContainerIDEmpty(t *testing.T) {
	fs := &filesystem{}

	containers := []*Container{
		{id: ""},
	}

	pod := Pod{
		id:         testPodID,
		containers: containers,
	}

	err := fs.createAllResources(pod)
	if err == nil {
		t.Fatal()
	}
}

type TestNoopStructure struct {
	Field1 string
	Field2 string
}

func TestFilesystemStoreFileSuccessfulNotExisting(t *testing.T) {
	fs := &filesystem{}

	path := filepath.Join(testDir, "testFilesystem")
	os.Remove(path)

	data := TestNoopStructure{
		Field1: "value1",
		Field2: "value2",
	}

	expected := "{\"Field1\":\"value1\",\"Field2\":\"value2\"}"

	err := fs.storeFile(path, data)
	if err != nil {
		t.Fatal(err)
	}

	fileData, err := ioutil.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}

	if string(fileData) != expected {
		t.Fatal()
	}
}

func TestFilesystemStoreFileSuccessfulExisting(t *testing.T) {
	fs := &filesystem{}

	path := filepath.Join(testDir, "testFilesystem")
	os.Remove(path)

	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	f.Close()

	data := TestNoopStructure{
		Field1: "value1",
		Field2: "value2",
	}

	expected := "{\"Field1\":\"value1\",\"Field2\":\"value2\"}"

	err = fs.storeFile(path, data)
	if err != nil {
		t.Fatal(err)
	}

	fileData, err := ioutil.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}

	if string(fileData) != expected {
		t.Fatal()
	}
}

func TestFilesystemStoreFileFailingMarshalling(t *testing.T) {
	fs := &filesystem{}

	path := filepath.Join(testDir, "testFilesystem")
	os.Remove(path)

	data := make(chan bool)

	err := fs.storeFile(path, data)
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemFetchFileSuccessful(t *testing.T) {
	fs := &filesystem{}
	data := TestNoopStructure{}

	path := filepath.Join(testDir, "testFilesystem")
	os.Remove(path)

	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}

	dataToWrite := "{\"Field1\":\"value1\",\"Field2\":\"value2\"}"
	n, err := f.WriteString(dataToWrite)
	if err != nil || n != len(dataToWrite) {
		f.Close()
		t.Fatal(err)
	}
	f.Close()

	err = fs.fetchFile(path, podResource(-1), &data)
	if err != nil {
		t.Fatal(err)
	}

	expected := TestNoopStructure{
		Field1: "value1",
		Field2: "value2",
	}

	if reflect.DeepEqual(data, expected) == false {
		t.Fatal()
	}
}

func TestFilesystemFetchFileFailingNoFile(t *testing.T) {
	fs := &filesystem{}
	data := TestNoopStructure{}

	path := filepath.Join(testDir, "testFilesystem")
	os.Remove(path)

	err := fs.fetchFile(path, podResource(-1), &data)
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemFetchFileFailingUnMarshalling(t *testing.T) {
	fs := &filesystem{}
	data := TestNoopStructure{}

	path := filepath.Join(testDir, "testFilesystem")
	os.Remove(path)

	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	f.Close()

	err = fs.fetchFile(path, podResource(-1), data)
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemFetchContainerConfigSuccessful(t *testing.T) {
	fs := &filesystem{}
	contID := "100"
	rootFs := "rootfs"

	contConfigDir := filepath.Join(configStoragePath, testPodID, contID)
	os.MkdirAll(contConfigDir, dirMode)

	path := filepath.Join(contConfigDir, configFile)
	os.Remove(path)

	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}

	configData := fmt.Sprintf("{\"ID\":\"%s\",\"RootFs\":\"%s\"}", contID, rootFs)
	n, err := f.WriteString(configData)
	if err != nil || n != len(configData) {
		f.Close()
		t.Fatal(err)
	}
	f.Close()

	data, err := fs.fetchContainerConfig(testPodID, contID)
	if err != nil {
		t.Fatal(err)
	}

	expected := ContainerConfig{
		ID:     contID,
		RootFs: rootFs,
	}

	if reflect.DeepEqual(data, expected) == false {
		t.Fatal()
	}
}

func TestFilesystemFetchContainerConfigFailingContIDEmpty(t *testing.T) {
	fs := &filesystem{}

	_, err := fs.fetchContainerConfig(testPodID, "")
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemFetchContainerConfigFailingPodIDEmpty(t *testing.T) {
	fs := &filesystem{}

	_, err := fs.fetchContainerConfig("", "100")
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemFetchContainerMountsSuccessful(t *testing.T) {
	fs := &filesystem{}
	contID := "100"

	contMountsDir := filepath.Join(runStoragePath, testPodID, contID)
	os.MkdirAll(contMountsDir, dirMode)

	path := filepath.Join(contMountsDir, mountsFile)
	os.Remove(path)

	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}

	source := "/dev/sda1"
	dest := "/root"
	mntType := "ext4"
	options := "rw"
	hostPath := "/tmp/root"

	mountData := fmt.Sprintf(`
		[
		  {
			"Source":"%s",
			"Destination":"%s",
			"Type":"%s",
			"Options": ["%s"],
			"HostPath":"%s"
		  }
		]
	`, source, dest, mntType, options, hostPath)

	n, err := f.WriteString(mountData)
	if err != nil || n != len(mountData) {
		f.Close()
		t.Fatal(err)
	}
	f.Close()

	data, err := fs.fetchContainerMounts(testPodID, contID)
	if err != nil {
		data, _ := ioutil.ReadFile(path)
		t.Logf("Data from file : %s", string(data[:]))
		t.Fatal(err)
	}

	expected := []Mount{
		{
			Source:      source,
			Destination: dest,
			Type:        mntType,
			Options:     []string{"rw"},
			HostPath:    hostPath,
		},
	}

	if reflect.DeepEqual(data, expected) == false {
		t.Fatalf("Expected : [%v]\n, Got : [%v]\n", expected, data)
	}
}

func TestFilesystemFetchContainerMountsInvalidType(t *testing.T) {
	fs := &filesystem{}
	contID := "100"

	contMountsDir := filepath.Join(runStoragePath, testPodID, contID)
	os.MkdirAll(contMountsDir, dirMode)

	path := filepath.Join(contMountsDir, mountsFile)
	os.Remove(path)

	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}

	configData := fmt.Sprintf("{\"ID\":\"%s\",\"RootFs\":\"rootfs\"}", contID)
	n, err := f.WriteString(configData)
	if err != nil || n != len(configData) {
		f.Close()
		t.Fatal(err)
	}
	f.Close()

	_, err = fs.fetchContainerMounts(testPodID, contID)
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemFetchContainerMountsFailingContIDEmpty(t *testing.T) {
	fs := &filesystem{}

	_, err := fs.fetchContainerMounts(testPodID, "")
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemFetchContainerMountsFailingPodIDEmpty(t *testing.T) {
	fs := &filesystem{}

	_, err := fs.fetchContainerMounts("", "100")
	if err == nil {
		t.Fatal()
	}
}

func TestFilesystemResourceDirFailingPodIDEmpty(t *testing.T) {
	for _, b := range []bool{true, false} {
		_, err := resourceDir(b, "", "", configFileType)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemResourceDirFailingInvalidResource(t *testing.T) {
	for _, b := range []bool{true, false} {
		_, err := resourceDir(b, testPodID, "100", podResource(-1))
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemResourceURIFailingResourceDir(t *testing.T) {
	fs := &filesystem{}

	for _, b := range []bool{true, false} {
		_, _, err := fs.resourceURI(b, testPodID, "100", podResource(-1))
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemStoreResourceFailingPodConfigStateFileType(t *testing.T) {
	fs := &filesystem{}
	data := PodConfig{}

	for _, b := range []bool{true, false} {
		err := fs.storeResource(b, testPodID, "100", stateFileType, data)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemStoreResourceFailingContainerConfigStateFileType(t *testing.T) {
	fs := &filesystem{}
	data := ContainerConfig{}

	for _, b := range []bool{true, false} {
		err := fs.storeResource(b, testPodID, "100", stateFileType, data)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemStoreResourceFailingPodConfigResourceURI(t *testing.T) {
	fs := &filesystem{}
	data := PodConfig{}

	for _, b := range []bool{true, false} {
		err := fs.storeResource(b, "", "100", configFileType, data)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemStoreResourceFailingContainerConfigResourceURI(t *testing.T) {
	fs := &filesystem{}
	data := ContainerConfig{}

	for _, b := range []bool{true, false} {
		err := fs.storeResource(b, "", "100", configFileType, data)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemStoreResourceFailingStateConfigFileType(t *testing.T) {
	fs := &filesystem{}
	data := State{}

	for _, b := range []bool{true, false} {
		err := fs.storeResource(b, testPodID, "100", configFileType, data)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemStoreResourceFailingStateResourceURI(t *testing.T) {
	fs := &filesystem{}
	data := State{}

	for _, b := range []bool{true, false} {
		err := fs.storeResource(b, "", "100", stateFileType, data)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemStoreResourceFailingWrongDataType(t *testing.T) {
	fs := &filesystem{}
	data := TestNoopStructure{}

	for _, b := range []bool{true, false} {
		err := fs.storeResource(b, testPodID, "100", configFileType, data)
		if err == nil {
			t.Fatal()
		}
	}
}

func TestFilesystemFetchResourceFailingWrongResourceType(t *testing.T) {
	fs := &filesystem{}

	for _, b := range []bool{true, false} {
		if err := fs.fetchResource(b, testPodID, "100", lockFileType, nil); err == nil {
			t.Fatal()
		}
	}
}
