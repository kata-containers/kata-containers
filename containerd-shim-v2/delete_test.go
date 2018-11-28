// Copyright (c) 2017 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"io/ioutil"
	"os"
	"path/filepath"
	"testing"

	taskAPI "github.com/containerd/containerd/runtime/v2/task"
	"github.com/kata-containers/runtime/virtcontainers/pkg/vcmock"
	"github.com/stretchr/testify/assert"
)

func TestDeleteContainerSuccessAndFail(t *testing.T) {
	assert := assert.New(t)

	sandbox := &vcmock.Sandbox{
		MockID: testSandboxID,
	}

	rootPath, configPath := testConfigSetup(t)
	defer os.RemoveAll(rootPath)
	_, err := readOCIConfigJSON(configPath)
	assert.NoError(err)

	s := &service{
		id:         testSandboxID,
		sandbox:    sandbox,
		containers: make(map[string]*container),
	}

	reqCreate := &taskAPI.CreateTaskRequest{
		ID: testContainerID,
	}
	s.containers[testContainerID], err = newContainer(s, reqCreate, "", nil)
	assert.NoError(err)
}

func testConfigSetup(t *testing.T) (rootPath string, configPath string) {
	assert := assert.New(t)

	tmpdir, err := ioutil.TempDir("", "")
	assert.NoError(err)

	bundlePath := filepath.Join(tmpdir, "bundle")
	err = os.MkdirAll(bundlePath, testDirMode)
	assert.NoError(err)

	err = createOCIConfig(bundlePath)
	assert.NoError(err)

	// config json path
	configPath = filepath.Join(bundlePath, "config.json")
	return tmpdir, configPath
}
