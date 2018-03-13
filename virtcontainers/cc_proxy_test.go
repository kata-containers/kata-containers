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
		pod         Pod
		expectedURI string
		expectError bool
	}

	invalidPath := filepath.Join(tmpdir, "enoent")
	expectedSocketPath := filepath.Join(runStoragePath, testPodID, "proxy.sock")
	expectedURI := fmt.Sprintf("unix://%s", expectedSocketPath)

	data := []testData{
		{Pod{}, "", true},
		{
			Pod{
				config: &PodConfig{
					ProxyType: "invalid",
				},
			}, "", true,
		},
		{
			Pod{
				config: &PodConfig{
					ProxyType:   CCProxyType,
					ProxyConfig: ProxyConfig{
					// invalid - no path
					},
				},
			}, "", true,
		},
		{
			Pod{
				config: &PodConfig{
					ProxyType: CCProxyType,
					ProxyConfig: ProxyConfig{
						Path: invalidPath,
					},
				},
			}, "", true,
		},
		{
			Pod{
				id: testPodID,
				config: &PodConfig{
					ProxyType: CCProxyType,
					ProxyConfig: ProxyConfig{
						Path: "echo",
					},
				},
			}, expectedURI, false,
		},
	}

	for _, d := range data {
		pid, uri, err := proxy.start(d.pod, proxyParams{})
		if d.expectError {
			assert.Error(err)
			continue
		}

		assert.NoError(err)
		assert.True(pid > 0)
		assert.Equal(d.expectedURI, uri)
	}
}
