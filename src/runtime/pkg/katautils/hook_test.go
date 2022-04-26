// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package katautils

import (
	"context"
	"os"
	"testing"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
	"github.com/opencontainers/runtime-spec/specs-go"
	"github.com/stretchr/testify/assert"
)

// Important to keep these values in sync with hook test binary
var testKeyHook = "test-key"
var testContainerIDHook = "test-container-id"
var testControllerIDHook = "test-controller-id"
var testBinHookPath = "mockhook/hook"
var testBundlePath = "/test/bundle"
var mockHookLogFile = "/tmp/mock_hook.log"

func getMockHookBinPath() string {
	return testBinHookPath
}

func createHook(timeout int) specs.Hook {
	to := &timeout
	if timeout == 0 {
		to = nil
	}

	return specs.Hook{
		Path:    getMockHookBinPath(),
		Args:    []string{testKeyHook, testContainerIDHook, testControllerIDHook},
		Env:     os.Environ(),
		Timeout: to,
	}
}

func createWrongHook() specs.Hook {
	return specs.Hook{
		Path: getMockHookBinPath(),
		Args: []string{"wrong-args"},
		Env:  os.Environ(),
	}
}

func cleanMockHookLogFile() {
	_ = os.Remove(mockHookLogFile)
}

func TestRunHook(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)
	t.Cleanup(cleanMockHookLogFile)

	ctx := context.Background()
	spec := specs.Spec{}

	// Run with timeout 0
	hook := createHook(0)
	err := runHook(ctx, spec, hook, testSandboxID, testBundlePath)
	assert.NoError(err)

	// Run with timeout 1
	hook = createHook(1)
	err = runHook(ctx, spec, hook, testSandboxID, testBundlePath)
	assert.NoError(err)

	// Run timeout failure
	hook = createHook(1)
	hook.Args = append(hook.Args, "2")
	err = runHook(ctx, spec, hook, testSandboxID, testBundlePath)
	assert.Error(err)

	// Failure due to wrong hook
	hook = createWrongHook()
	err = runHook(ctx, spec, hook, testSandboxID, testBundlePath)
	assert.Error(err)
}

func TestPreStartHooks(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)
	t.Cleanup(cleanMockHookLogFile)

	ctx := context.Background()

	// Hooks field is nil
	spec := specs.Spec{}
	err := PreStartHooks(ctx, spec, "", "")
	assert.NoError(err)

	// Hooks list is empty
	spec = specs.Spec{
		Hooks: &specs.Hooks{},
	}
	err = PreStartHooks(ctx, spec, "", "")
	assert.NoError(err)

	// Run with timeout 0
	hook := createHook(0)
	spec = specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{hook},
		},
	}
	err = PreStartHooks(ctx, spec, testSandboxID, testBundlePath)
	assert.NoError(err)

	// Failure due to wrong hook
	hook = createWrongHook()
	spec = specs.Spec{
		Hooks: &specs.Hooks{
			Prestart: []specs.Hook{hook},
		},
	}
	err = PreStartHooks(ctx, spec, testSandboxID, testBundlePath)
	assert.Error(err)
}

func TestPostStartHooks(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)
	t.Cleanup(cleanMockHookLogFile)

	ctx := context.Background()

	// Hooks field is nil
	spec := specs.Spec{}
	err := PostStartHooks(ctx, spec, "", "")
	assert.NoError(err)

	// Hooks list is empty
	spec = specs.Spec{
		Hooks: &specs.Hooks{},
	}
	err = PostStartHooks(ctx, spec, "", "")
	assert.NoError(err)

	// Run with timeout 0
	hook := createHook(0)
	spec = specs.Spec{
		Hooks: &specs.Hooks{
			Poststart: []specs.Hook{hook},
		},
	}
	err = PostStartHooks(ctx, spec, testSandboxID, testBundlePath)
	assert.NoError(err)

	// Failure due to wrong hook
	hook = createWrongHook()
	spec = specs.Spec{
		Hooks: &specs.Hooks{
			Poststart: []specs.Hook{hook},
		},
	}
	err = PostStartHooks(ctx, spec, testSandboxID, testBundlePath)
	assert.Error(err)
}

func TestPostStopHooks(t *testing.T) {
	if tc.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}

	assert := assert.New(t)

	ctx := context.Background()
	t.Cleanup(cleanMockHookLogFile)

	// Hooks field is nil
	spec := specs.Spec{}
	err := PostStopHooks(ctx, spec, "", "")
	assert.NoError(err)

	// Hooks list is empty
	spec = specs.Spec{
		Hooks: &specs.Hooks{},
	}
	err = PostStopHooks(ctx, spec, "", "")
	assert.NoError(err)

	// Run with timeout 0
	hook := createHook(0)
	spec = specs.Spec{
		Hooks: &specs.Hooks{
			Poststop: []specs.Hook{hook},
		},
	}
	err = PostStopHooks(ctx, spec, testSandboxID, testBundlePath)
	assert.NoError(err)

	// Failure due to wrong hook
	hook = createWrongHook()
	spec = specs.Spec{
		Hooks: &specs.Hooks{
			Poststop: []specs.Hook{hook},
		},
	}
	err = PostStopHooks(ctx, spec, testSandboxID, testBundlePath)
	assert.Error(err)
}
