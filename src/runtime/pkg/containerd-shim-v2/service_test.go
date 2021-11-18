// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"context"
	"fmt"
	"os"
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
	const serviceErrorPrefix = "Cause: "
	const badCIDErrorPrefix = serviceErrorPrefix + "invalid container/sandbox ID"
	const blankCIDError = serviceErrorPrefix + "ID cannot be blank"

	assert := assert.New(t)

	tmpdir, bundleDir, _ := ktu.SetupOCIConfigFile(t)
	defer os.RemoveAll(tmpdir)

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

		var expectedErrorPrefix string

		if d.ID == "" {
			expectedErrorPrefix = blankCIDError
		} else {
			expectedErrorPrefix = badCIDErrorPrefix
		}
		msg += "\nerror has not prefix:\n'"
		msg += expectedErrorPrefix
		msg += "'\nbut has:\n'"
		msg += err.Error() + "'"
		assert.True(strings.HasPrefix(err.Error(), expectedErrorPrefix), msg)
	}
}
