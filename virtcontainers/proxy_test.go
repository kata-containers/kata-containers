//
// Copyright (c) 2017 Intel Corporation
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
	"path/filepath"
	"reflect"
	"testing"
)

func testSetProxyType(t *testing.T, value string, expected ProxyType) {
	var proxyType ProxyType

	err := (&proxyType).Set(value)
	if err != nil {
		t.Fatal(err)
	}

	if proxyType != expected {
		t.Fatalf("Got %s\nExpecting %s", proxyType, expected)
	}
}

func TestSetCCProxyType(t *testing.T) {
	testSetProxyType(t, "ccProxy", CCProxyType)
}

func TestSetKataProxyType(t *testing.T) {
	testSetProxyType(t, "kataProxy", KataProxyType)
}

func TestSetNoopProxyType(t *testing.T) {
	testSetProxyType(t, "noopProxy", NoopProxyType)
}

func TestSetNoProxyType(t *testing.T) {
	testSetProxyType(t, "noProxy", NoProxyType)
}

func TestSetKataBuiltInProxyType(t *testing.T) {
	testSetProxyType(t, "kataBuiltInProxy", KataBuiltInProxyType)
}

func TestSetUnknownProxyType(t *testing.T) {
	var proxyType ProxyType

	unknownType := "unknown"

	err := (&proxyType).Set(unknownType)
	if err == nil {
		t.Fatalf("Should fail because %s type used", unknownType)
	}

	if proxyType == CCProxyType ||
		proxyType == NoopProxyType ||
		proxyType == NoProxyType ||
		proxyType == KataProxyType {
		t.Fatalf("%s proxy type was not expected", proxyType)
	}
}

func testStringFromProxyType(t *testing.T, proxyType ProxyType, expected string) {
	proxyTypeStr := (&proxyType).String()
	if proxyTypeStr != expected {
		t.Fatalf("Got %s\nExpecting %s", proxyTypeStr, expected)
	}
}

func TestStringFromCCProxyType(t *testing.T) {
	proxyType := CCProxyType
	testStringFromProxyType(t, proxyType, "ccProxy")
}

func TestStringFromKataProxyType(t *testing.T) {
	proxyType := KataProxyType
	testStringFromProxyType(t, proxyType, "kataProxy")
}

func TestStringFromNoProxyType(t *testing.T) {
	proxyType := NoProxyType
	testStringFromProxyType(t, proxyType, "noProxy")
}

func TestStringFromNoopProxyType(t *testing.T) {
	proxyType := NoopProxyType
	testStringFromProxyType(t, proxyType, "noopProxy")
}

func TestStringFromKataBuiltInProxyType(t *testing.T) {
	proxyType := KataBuiltInProxyType
	testStringFromProxyType(t, proxyType, "kataBuiltInProxy")
}

func TestStringFromUnknownProxyType(t *testing.T) {
	var proxyType ProxyType
	testStringFromProxyType(t, proxyType, "")
}

func testNewProxyFromProxyType(t *testing.T, proxyType ProxyType, expected proxy) {
	result, err := newProxy(proxyType)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(result, expected) == false {
		t.Fatalf("Got %+v\nExpecting %+v", result, expected)
	}
}

func TestNewProxyFromCCProxyType(t *testing.T) {
	proxyType := CCProxyType
	expectedProxy := &ccProxy{}
	testNewProxyFromProxyType(t, proxyType, expectedProxy)
}

func TestNewProxyFromKataProxyType(t *testing.T) {
	proxyType := KataProxyType
	expectedProxy := &kataProxy{}
	testNewProxyFromProxyType(t, proxyType, expectedProxy)
}

func TestNewProxyFromNoProxyType(t *testing.T) {
	proxyType := NoProxyType
	expectedProxy := &noProxy{}
	testNewProxyFromProxyType(t, proxyType, expectedProxy)
}

func TestNewProxyFromNoopProxyType(t *testing.T) {
	proxyType := NoopProxyType
	expectedProxy := &noopProxy{}
	testNewProxyFromProxyType(t, proxyType, expectedProxy)
}

func TestNewProxyFromKataBuiltInProxyType(t *testing.T) {
	proxyType := KataBuiltInProxyType
	expectedProxy := &kataBuiltInProxy{}
	testNewProxyFromProxyType(t, proxyType, expectedProxy)
}

func TestNewProxyFromUnknownProxyType(t *testing.T) {
	var proxyType ProxyType

	_, err := newProxy(proxyType)
	if err != nil {
		t.Fatal(err)
	}
}

func testNewProxyConfigFromPodConfig(t *testing.T, podConfig PodConfig, expected ProxyConfig) {
	result, err := newProxyConfig(&podConfig)
	if err != nil {
		t.Fatal(err)
	}

	if reflect.DeepEqual(result, expected) == false {
		t.Fatalf("Got %+v\nExpecting %+v", result, expected)
	}
}

var testProxyPath = "proxy-path"

func TestNewProxyConfigFromCCProxyPodConfig(t *testing.T) {
	proxyConfig := ProxyConfig{
		Path: testProxyPath,
	}

	podConfig := PodConfig{
		ProxyType:   CCProxyType,
		ProxyConfig: proxyConfig,
	}

	testNewProxyConfigFromPodConfig(t, podConfig, proxyConfig)
}

func TestNewProxyConfigFromKataProxyPodConfig(t *testing.T) {
	proxyConfig := ProxyConfig{
		Path: testProxyPath,
	}

	podConfig := PodConfig{
		ProxyType:   KataProxyType,
		ProxyConfig: proxyConfig,
	}

	testNewProxyConfigFromPodConfig(t, podConfig, proxyConfig)
}

func TestNewProxyConfigNilPodConfigFailure(t *testing.T) {
	if _, err := newProxyConfig(nil); err == nil {
		t.Fatal("Should fail because PodConfig provided is nil")
	}
}

func TestNewProxyConfigNoPathFailure(t *testing.T) {
	podConfig := &PodConfig{
		ProxyType:   CCProxyType,
		ProxyConfig: ProxyConfig{},
	}

	if _, err := newProxyConfig(podConfig); err == nil {
		t.Fatal("Should fail because ProxyConfig has no Path")
	}
}

const podID = "123456789"

func testDefaultProxyURL(expectedURL string, socketType string, podID string) error {
	pod := &Pod{
		id: podID,
	}

	url, err := defaultProxyURL(*pod, socketType)
	if err != nil {
		return err
	}

	if url != expectedURL {
		return fmt.Errorf("Mismatched URL: %s vs %s", url, expectedURL)
	}

	return nil
}

func TestDefaultProxyURLUnix(t *testing.T) {
	path := filepath.Join(runStoragePath, podID, "proxy.sock")
	socketPath := fmt.Sprintf("unix://%s", path)

	if err := testDefaultProxyURL(socketPath, SocketTypeUNIX, podID); err != nil {
		t.Fatal(err)
	}
}

func TestDefaultProxyURLVSock(t *testing.T) {
	if err := testDefaultProxyURL("", SocketTypeVSOCK, podID); err != nil {
		t.Fatal(err)
	}
}

func TestDefaultProxyURLUnknown(t *testing.T) {
	path := filepath.Join(runStoragePath, podID, "proxy.sock")
	socketPath := fmt.Sprintf("unix://%s", path)

	if err := testDefaultProxyURL(socketPath, "foobar", podID); err == nil {
		t.Fatal()
	}
}
