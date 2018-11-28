// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	taskAPI "github.com/containerd/containerd/runtime/v2/task"
	"github.com/stretchr/testify/assert"
	"testing"
)

func TestNewContainer(t *testing.T) {
	assert := assert.New(t)

	_, err := newContainer(nil, nil, "", nil)

	assert.Error(err)
}

func TestGetExec(t *testing.T) {
	assert := assert.New(t)

	r := &taskAPI.CreateTaskRequest{}

	c, err := newContainer(nil, r, "", nil)
	assert.NoError(err)

	_, err = c.getExec("")
	assert.Error(err)

	c.execs = make(map[string]*exec)
	_, err = c.getExec("")
	assert.Error(err)

	c.execs[TestID] = &exec{}
	_, err = c.getExec(TestID)
	assert.NoError(err)
}
