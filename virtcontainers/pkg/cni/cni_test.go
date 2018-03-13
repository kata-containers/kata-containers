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

package cni

import (
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/containernetworking/cni/libcni"
	"github.com/containernetworking/plugins/pkg/ns"
)

const (
	dirMode = os.FileMode(0750)
)

var testDirBase = "../../utils/supportfiles/cni"
var testConfDir = testDirBase + "/net.d"
var testBinDir = testDirBase + "/bin"
var testWrongConfDir = testDirBase + "/wrong"
var testDefFile = "10-test_network.conf"
var testLoFile = "99-test_loopback.conf"
var testWrongFile = "100-test_error.conf"

var testLoFileContent = []byte(`{
    "cniVersion": "0.3.0",
    "name": "testlonetwork",
    "type": "loopback"
}`)

var testLoFileContentNoName = []byte(`{
    "cniVersion": "0.3.0",
    "type": "loopback"
}`)

var testDefFileContent = []byte(`{
    "cniVersion": "0.3.0",
    "name": "testdefnetwork",
    "type": "cni-bridge",
    "bridge": "cni0",
    "isGateway": true,
    "ipMasq": true,
    "ipam": {
        "type": "host-local",
        "subnet": "10.88.0.0/16",
        "routes": [
            { "dst": "0.0.0.0/0" }
        ]
    }
}`)

var testDefFileContentNoName = []byte(`{
    "cniVersion": "0.3.0",
    "type": "cni-bridge",
    "bridge": "cni0",
    "isGateway": true,
    "ipMasq": true,
    "ipam": {
        "type": "host-local",
        "subnet": "10.88.0.0/16",
        "routes": [
            { "dst": "0.0.0.0/0" }
        ]
    }
}`)

var testWrongFileContent = []byte(`{
    "cniVersion "0.3.0",
    "type": "loopback"
}`)

func createLoNetwork(t *testing.T) {
	loFile := filepath.Join(testConfDir, testLoFile)

	f, err := os.Create(loFile)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	_, err = f.Write(testLoFileContent)
	if err != nil {
		t.Fatal(err)
	}
}

func createLoNetworkNoName(t *testing.T) {
	loFile := filepath.Join(testConfDir, testLoFile)

	f, err := os.Create(loFile)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	_, err = f.Write(testLoFileContentNoName)
	if err != nil {
		t.Fatal(err)
	}
}

func createDefNetwork(t *testing.T) {
	defFile := filepath.Join(testConfDir, testDefFile)

	f, err := os.Create(defFile)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	_, err = f.Write(testDefFileContent)
	if err != nil {
		t.Fatal(err)
	}
}

func createDefNetworkNoName(t *testing.T) {
	defFile := filepath.Join(testConfDir, testDefFile)

	f, err := os.Create(defFile)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	_, err = f.Write(testDefFileContentNoName)
	if err != nil {
		t.Fatal(err)
	}
}

func createWrongNetwork(t *testing.T) {
	wrongFile := filepath.Join(testConfDir, testWrongFile)

	f, err := os.Create(wrongFile)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	_, err = f.Write(testWrongFileContent)
	if err != nil {
		t.Fatal(err)
	}
}

func removeLoNetwork(t *testing.T) {
	loFile := filepath.Join(testConfDir, testLoFile)
	err := os.Remove(loFile)
	if err != nil {
		t.Fatal(err)
	}
}

func removeDefNetwork(t *testing.T) {
	defFile := filepath.Join(testConfDir, testDefFile)
	err := os.Remove(defFile)
	if err != nil {
		t.Fatal(err)
	}
}

func removeWrongNetwork(t *testing.T) {
	wrongFile := filepath.Join(testConfDir, testWrongFile)
	err := os.Remove(wrongFile)
	if err != nil {
		t.Fatal(err)
	}
}

func TestNewNetworkPluginSuccessful(t *testing.T) {
	createLoNetwork(t)
	defer removeLoNetwork(t)
	createDefNetwork(t)
	defer removeDefNetwork(t)

	netPlugin, err := NewNetworkPluginWithArgs(testConfDir, testBinDir)
	if err != nil {
		t.Fatal(err)
	}

	if netPlugin.loNetwork == nil {
		t.Fatal("Invalid local network")
	}

	if netPlugin.defNetwork == nil {
		t.Fatal("Invalid default network")
	}

	if netPlugin.loNetwork.name != "testlonetwork" {
		t.Fatal("Invalid local network name")
	}

	if netPlugin.defNetwork.name != "testdefnetwork" {
		t.Fatal("Invalid default network name")
	}
}

func TestNewNetworkPluginSuccessfulNoName(t *testing.T) {
	createLoNetworkNoName(t)
	defer removeLoNetwork(t)
	createDefNetworkNoName(t)
	defer removeDefNetwork(t)

	netPlugin, err := NewNetworkPluginWithArgs(testConfDir, testBinDir)
	if err != nil {
		t.Fatal(err)
	}

	if netPlugin.loNetwork == nil {
		t.Fatal("Invalid local network")
	}

	if netPlugin.defNetwork == nil {
		t.Fatal("Invalid default network")
	}

	if netPlugin.loNetwork.name != "lo" {
		t.Fatal("Invalid local network name")
	}

	if netPlugin.defNetwork.name != "net" {
		t.Fatal("Invalid default network name")
	}
}

func TestNewNetworkPluginFailureNoNetwork(t *testing.T) {
	netPlugin, err := NewNetworkPluginWithArgs(testConfDir, testBinDir)
	if err == nil || netPlugin != nil {
		t.Fatal("Should fail because no available network")
	}
}

func TestNewNetworkPluginFailureNoConfDir(t *testing.T) {
	netPlugin, err := NewNetworkPluginWithArgs(testWrongConfDir, testBinDir)
	if err == nil || netPlugin != nil {
		t.Fatal("Should fail because configuration directory does not exist")
	}
}

func TestNewNetworkPluginFailureWrongNetwork(t *testing.T) {
	createWrongNetwork(t)
	defer removeWrongNetwork(t)

	netPlugin, err := NewNetworkPluginWithArgs(testConfDir, testBinDir)
	if err == nil || netPlugin != nil {
		t.Fatal("Should fail because of wrong network definition")
	}
}

func TestBuildRuntimeConf(t *testing.T) {
	expected := libcni.RuntimeConf{
		ContainerID: "testPodID",
		NetNS:       "testPodNetNSPath",
		IfName:      "testIfName",
	}

	runtimeConf := buildRuntimeConf("testPodID", "testPodNetNSPath", "testIfName")

	if reflect.DeepEqual(*runtimeConf, expected) == false {
		t.Fatal("Runtime configuration different from expected one")
	}
}

func TestAddNetworkSuccessful(t *testing.T) {
	createLoNetworkNoName(t)
	defer removeLoNetwork(t)
	createDefNetworkNoName(t)
	defer removeDefNetwork(t)

	netNsHandle, err := ns.NewNS()
	if err != nil {
		t.Fatal(err)
	}
	defer netNsHandle.Close()

	testNetNsPath := netNsHandle.Path()

	netPlugin, err := NewNetworkPluginWithArgs(testConfDir, testBinDir)
	if err != nil {
		t.Fatal(err)
	}

	_, err = netPlugin.AddNetwork("testPodID", testNetNsPath, "testIfName")
	if err != nil {
		t.Fatal(err)
	}
}

func TestAddNetworkFailureUnknownNetNs(t *testing.T) {
	createLoNetworkNoName(t)
	defer removeLoNetwork(t)
	createDefNetworkNoName(t)
	defer removeDefNetwork(t)

	const invalidNetNsPath = "/this/path/does/not/exist"

	// ensure it really is invalid
	_, err := os.Stat(invalidNetNsPath)
	if err == nil {
		t.Fatalf("directory %v unexpectedly exists", invalidNetNsPath)
	}

	netPlugin, err := NewNetworkPluginWithArgs(testConfDir, testBinDir)
	if err != nil {
		t.Fatal(err)
	}

	_, err = netPlugin.AddNetwork("testPodID", invalidNetNsPath, "testIfName")
	if err == nil {
		t.Fatalf("Should fail because netns %s does not exist", invalidNetNsPath)
	}
}

func TestRemoveNetworkSuccessful(t *testing.T) {
	createLoNetworkNoName(t)
	defer removeLoNetwork(t)
	createDefNetworkNoName(t)
	defer removeDefNetwork(t)

	netNsHandle, err := ns.NewNS()
	if err != nil {
		t.Fatal(err)
	}
	defer netNsHandle.Close()

	testNetNsPath := netNsHandle.Path()

	netPlugin, err := NewNetworkPluginWithArgs(testConfDir, testBinDir)
	if err != nil {
		t.Fatal(err)
	}

	_, err = netPlugin.AddNetwork("testPodID", testNetNsPath, "testIfName")
	if err != nil {
		t.Fatal(err)
	}

	err = netPlugin.RemoveNetwork("testPodID", testNetNsPath, "testIfName")
	if err != nil {
		t.Fatal(err)
	}
}

func TestRemoveNetworkSuccessfulNetworkDoesNotExist(t *testing.T) {
	createLoNetworkNoName(t)
	defer removeLoNetwork(t)
	createDefNetworkNoName(t)
	defer removeDefNetwork(t)

	netNsHandle, err := ns.NewNS()
	if err != nil {
		t.Fatal(err)
	}
	defer netNsHandle.Close()

	testNetNsPath := netNsHandle.Path()

	netPlugin, err := NewNetworkPluginWithArgs(testConfDir, testBinDir)
	if err != nil {
		t.Fatal(err)
	}

	err = netPlugin.RemoveNetwork("testPodID", testNetNsPath, "testIfName")
	if err != nil {
		// CNI specification says that no error should be returned
		// in case we try to tear down a non-existing network.
		t.Fatalf("Should pass because network not previously added: %s", err)
	}
}

func TestMain(m *testing.M) {
	err := os.MkdirAll(testConfDir, dirMode)
	if err != nil {
		fmt.Println("Could not create test configuration directory:", err)
		os.Exit(1)
	}

	_, err = os.Stat(testBinDir)
	if err != nil {
		fmt.Println("Test binary directory should exist:", err)
		os.RemoveAll(testConfDir)
		os.Exit(1)
	}

	ret := m.Run()

	os.RemoveAll(testConfDir)

	os.Exit(ret)
}
