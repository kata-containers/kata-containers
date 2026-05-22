// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package containerdshim

import (
	"sync"
	"testing"

	taskAPI "github.com/containerd/containerd/api/runtime/task/v2"
	"github.com/stretchr/testify/assert"
)

func TestNewContainer(t *testing.T) {
	assert := assert.New(t)

	_, err := newContainer(nil, nil, "", nil, false)

	assert.Error(err)
}

func TestGetExec(t *testing.T) {
	assert := assert.New(t)

	r := &taskAPI.CreateTaskRequest{}

	c, err := newContainer(nil, r, "", nil, true)
	assert.NoError(err)

	// Exec not found on an initialized (but empty) map.
	_, err = c.getExec("")
	assert.Error(err)

	// Exec not found when the map is nil.
	c.execs = nil
	_, err = c.getExec("")
	assert.Error(err)

	// Restore the map and verify a set exec can be retrieved.
	c.execs = make(map[string]*exec)
	c.setExec(TestID, &exec{})
	_, err = c.getExec(TestID)
	assert.NoError(err)
}

// Regression test for #12825: concurrent map read and map write on c.execs.
// Run with -race to verify the fix (go test -race).
func TestConcurrentExecAccess(t *testing.T) {
	r := &taskAPI.CreateTaskRequest{}
	c, err := newContainer(nil, r, "", nil, true)
	if err != nil {
		t.Fatal(err)
	}

	const iterations = 500
	const execID = "concurrent-exec"

	var wg sync.WaitGroup
	wg.Add(3)

	// Writer: alternate between set and delete.
	go func() {
		defer wg.Done()
		for i := 0; i < iterations; i++ {
			c.setExec(execID, &exec{})
			c.deleteExec(execID)
		}
	}()

	// Reader 1: continuously try to get the exec.
	go func() {
		defer wg.Done()
		for i := 0; i < iterations; i++ {
			c.getExec(execID)
		}
	}()

	// Reader 2: another concurrent reader on the same ID.
	go func() {
		defer wg.Done()
		for i := 0; i < iterations; i++ {
			c.getExec(execID)
		}
	}()

	wg.Wait()
}
