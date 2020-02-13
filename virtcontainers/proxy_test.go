// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	"github.com/kata-containers/runtime/virtcontainers/persist/fs"
	"github.com/sirupsen/logrus"
	"github.com/stretchr/testify/assert"
)

var testDefaultLogger = logrus.WithField("proxy", "test")

func testSetProxyType(t *testing.T, value string, expected ProxyType) {
	var proxyType ProxyType
	assert := assert.New(t)

	err := (&proxyType).Set(value)
	assert.NoError(err)
	assert.Equal(proxyType, expected)
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
	assert := assert.New(t)

	unknownType := "unknown"

	err := (&proxyType).Set(unknownType)
	assert.Error(err)
	assert.NotEqual(proxyType, NoopProxyType)
	assert.NotEqual(proxyType, NoProxyType)
	assert.NotEqual(proxyType, KataProxyType)
}

func testStringFromProxyType(t *testing.T, proxyType ProxyType, expected string) {
	proxyTypeStr := (&proxyType).String()
	assert.Equal(t, proxyTypeStr, expected)
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
	assert := assert.New(t)
	assert.NoError(err)
	assert.Exactly(result, expected)
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
	assert.NoError(t, err)
}

func testNewProxyFromSandboxConfig(t *testing.T, sandboxConfig SandboxConfig) {
	assert := assert.New(t)

	_, err := newProxy(sandboxConfig.ProxyType)
	assert.NoError(err)

	err = validateProxyConfig(sandboxConfig.ProxyConfig)
	assert.NoError(err)
}

var testProxyPath = "proxy-path"

func TestNewProxyConfigFromKataProxySandboxConfig(t *testing.T) {
	proxyConfig := ProxyConfig{
		Path: testProxyPath,
	}

	sandboxConfig := SandboxConfig{
		ProxyType:   KataProxyType,
		ProxyConfig: proxyConfig,
	}

	testNewProxyFromSandboxConfig(t, sandboxConfig)
}

func TestNewProxyConfigNoPathFailure(t *testing.T) {
	assert.Error(t, validateProxyConfig(ProxyConfig{}))
}

const sandboxID = "123456789"

func testDefaultProxyURL(expectedURL string, socketType string, sandboxID string) error {
	sandbox := &Sandbox{
		id: sandboxID,
	}

	url, err := defaultProxyURL(sandbox.id, socketType)
	if err != nil {
		return err
	}

	if url != expectedURL {
		return fmt.Errorf("Mismatched URL: %s vs %s", url, expectedURL)
	}

	return nil
}

func TestDefaultProxyURLUnix(t *testing.T) {
	path := filepath.Join(filepath.Join(fs.MockRunStoragePath(), sandboxID), "proxy.sock")
	socketPath := fmt.Sprintf("unix://%s", path)
	assert.NoError(t, testDefaultProxyURL(socketPath, SocketTypeUNIX, sandboxID))
}

func TestDefaultProxyURLVSock(t *testing.T) {
	assert.NoError(t, testDefaultProxyURL("", SocketTypeVSOCK, sandboxID))
}

func TestDefaultProxyURLUnknown(t *testing.T) {
	path := filepath.Join(filepath.Join(fs.MockRunStoragePath(), sandboxID), "proxy.sock")
	socketPath := fmt.Sprintf("unix://%s", path)
	assert.Error(t, testDefaultProxyURL(socketPath, "foobar", sandboxID))
}

func testProxyStart(t *testing.T, agent agent, proxy proxy) {
	assert := assert.New(t)

	assert.NotNil(proxy)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	type testData struct {
		params      proxyParams
		expectedURI string
		expectError bool
	}

	invalidPath := filepath.Join(tmpdir, "enoent")
	expectedSocketPath := filepath.Join(filepath.Join(fs.MockRunStoragePath(), testSandboxID), "proxy.sock")
	expectedURI := fmt.Sprintf("unix://%s", expectedSocketPath)

	data := []testData{
		{proxyParams{}, "", true},
		{
			// no path
			proxyParams{
				id:         "foobar",
				agentURL:   "agentURL",
				consoleURL: "consoleURL",
				logger:     testDefaultLogger,
			},
			"", true,
		},
		{
			// invalid path
			proxyParams{
				id:         "foobar",
				path:       invalidPath,
				agentURL:   "agentURL",
				consoleURL: "consoleURL",
				logger:     testDefaultLogger,
			},
			"", true,
		},
		{
			// good case
			proxyParams{
				id:         testSandboxID,
				path:       "echo",
				agentURL:   "agentURL",
				consoleURL: "consoleURL",
				logger:     testDefaultLogger,
			},
			expectedURI, false,
		},
	}

	for _, d := range data {
		pid, uri, err := proxy.start(d.params)
		if d.expectError {
			assert.Error(err)
			continue
		}

		assert.NoError(err)
		assert.True(pid > 0)
		assert.Equal(d.expectedURI, uri)
	}
}

func TestValidateProxyConfig(t *testing.T) {
	assert := assert.New(t)

	config := ProxyConfig{}
	err := validateProxyConfig(config)
	assert.Error(err)

	config.Path = "foobar"
	err = validateProxyConfig(config)
	assert.Nil(err)
}

func TestValidateProxyParams(t *testing.T) {
	assert := assert.New(t)

	p := proxyParams{}
	err := validateProxyParams(p)
	assert.Error(err)

	p.path = "foobar"
	err = validateProxyParams(p)
	assert.Error(err)

	p.id = "foobar1"
	err = validateProxyParams(p)
	assert.Error(err)

	p.agentURL = "foobar2"
	err = validateProxyParams(p)
	assert.Error(err)

	p.consoleURL = "foobar3"
	err = validateProxyParams(p)
	assert.Error(err)

	p.logger = &logrus.Entry{}
	err = validateProxyParams(p)
	assert.Nil(err)
}
