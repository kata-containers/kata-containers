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

	"github.com/stretchr/testify/assert"
)

func TestCCProxyStart(t *testing.T) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)
	defer os.RemoveAll(tmpdir)

	proxy := &ccProxy{}

	type testData struct {
		sandbox     Sandbox
		expectedURI string
		expectError bool
	}

	invalidPath := filepath.Join(tmpdir, "enoent")
	expectedSocketPath := filepath.Join(runStoragePath, testSandboxID, "proxy.sock")
	expectedURI := fmt.Sprintf("unix://%s", expectedSocketPath)

	data := []testData{
		{Sandbox{}, "", true},
		{
			Sandbox{
				config: &SandboxConfig{
					ProxyType: "invalid",
				},
			}, "", true,
		},
		{
			Sandbox{
				config: &SandboxConfig{
					ProxyType: CCProxyType,
					// invalid - no path
					ProxyConfig: ProxyConfig{},
				},
			}, "", true,
		},
		{
			Sandbox{
				config: &SandboxConfig{
					ProxyType: CCProxyType,
					ProxyConfig: ProxyConfig{
						Path: invalidPath,
					},
				},
			}, "", true,
		},
		{
			Sandbox{
				id: testSandboxID,
				config: &SandboxConfig{
					ProxyType: CCProxyType,
					ProxyConfig: ProxyConfig{
						Path: "echo",
					},
				},
			}, expectedURI, false,
		},
	}

	for _, d := range data {
		pid, uri, err := proxy.start(d.sandbox, proxyParams{})
		if d.expectError {
			assert.Error(err)
			continue
		}

		assert.NoError(err)
		assert.True(pid > 0)
		assert.Equal(d.expectedURI, uri)
	}
}
