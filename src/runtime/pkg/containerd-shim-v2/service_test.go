// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"testing"

	taskAPI "github.com/containerd/containerd/runtime/v2/task"
	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/stretchr/testify/assert"
)

func newService(id string) (*service, error) {
	ctx := context.Background()

	ctx, cancel := context.WithCancel(ctx)

	s := &service{
		id:         id,
		pid:        uint32(os.Getpid()),
		ctx:        ctx,
		containers: make(map[string]*container),
		events:     make(chan interface{}, chSize),
		ec:         make(chan exit, bufferSize),
		cancel:     cancel,
	}

	return s, nil
}

func TestServiceCreate(t *testing.T) {
	const badCIDErrorPrefix = "invalid container/sandbox ID"
	const blankCIDError = "ID cannot be blank"

	assert := assert.New(t)

	tmpdir, _ := ioutil.TempDir("", "")
	defer os.RemoveAll(tmpdir)

	bundleDir := filepath.Join(tmpdir, "bundle")
	err := makeOCIBundle(bundleDir)
	assert.NoError(err)

	ctx := context.Background()

	s, err := newService("foo")
	assert.NoError(err)

	for i, d := range ktu.ContainerIDTestData {
		msg := fmt.Sprintf("test[%d]: %+v", i, d)

		// Only consider error scenarios as we are only testing invalid CIDs here.
		if d.Valid {
			continue
		}

		task := taskAPI.CreateTaskRequest{
			ID:     d.ID,
			Bundle: bundleDir,
		}

		_, err = s.Create(ctx, &task)
		assert.Error(err, msg)

		if d.ID == "" {
			assert.Equal(err.Error(), blankCIDError, msg)
		} else {
			assert.True(strings.HasPrefix(err.Error(), badCIDErrorPrefix), msg)
		}
	}
}
