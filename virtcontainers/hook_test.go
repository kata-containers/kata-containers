// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"context"
	"io/ioutil"
	"os"
	"path/filepath"
	"reflect"
	"sync"
	"testing"

	vcAnnotations "github.com/kata-containers/runtime/virtcontainers/pkg/annotations"
	. "github.com/kata-containers/runtime/virtcontainers/pkg/mock"
	specs "github.com/opencontainers/runtime-spec/specs-go"
)

// Important to keep these values in sync with hook test binary
var testKeyHook = "test-key"
var testContainerIDHook = "test-container-id"
var testControllerIDHook = "test-controller-id"
var testProcessIDHook = 12345
var testBinHookPath = "/usr/bin/virtcontainers/bin/test/hook"
var testBundlePath = "/test/bundle"

func getMockHookBinPath() string {
	if DefaultMockHookBinPath == "" {
		return testBinHookPath
	}

	return DefaultMockHookBinPath
}

func TestBuildHookState(t *testing.T) {
	t.Skip()
	expected := specs.State{
		Pid: testProcessIDHook,
	}

	s := &Sandbox{}

	hookState := buildHookState(testProcessIDHook, s)

	if reflect.DeepEqual(hookState, expected) == false {
		t.Fatal()
	}

	s = createTestSandbox()
	hookState = buildHookState(testProcessIDHook, s)

	expected = specs.State{
		Pid:    testProcessIDHook,
		Bundle: testBundlePath,
		ID:     testSandboxID,
	}

	if reflect.DeepEqual(hookState, expected) == false {
		t.Fatal()
	}

}

func createHook(timeout int) *Hook {
	return &Hook{
		Path:    getMockHookBinPath(),
		Args:    []string{testKeyHook, testContainerIDHook, testControllerIDHook},
		Env:     os.Environ(),
		Timeout: timeout,
	}
}

func createWrongHook() *Hook {
	return &Hook{
		Path: getMockHookBinPath(),
		Args: []string{"wrong-args"},
		Env:  os.Environ(),
	}
}

func createTestSandbox() *Sandbox {
	c := &SandboxConfig{
		Annotations: map[string]string{
			vcAnnotations.BundlePathKey: testBundlePath,
		},
	}
	return &Sandbox{
		annotationsLock: &sync.RWMutex{},
		config:          c,
		id:              testSandboxID,
		ctx:             context.Background(),
	}
}

func testRunHookFull(t *testing.T, timeout int, expectFail bool) {
	hook := createHook(timeout)

	s := createTestSandbox()
	err := hook.runHook(s)
	if expectFail {
		if err == nil {
			t.Fatal("unexpected success")
		}
	} else {
		if err != nil {
			t.Fatalf("unexpected failure: %v", err)
		}
	}
}

func testRunHook(t *testing.T, timeout int) {
	testRunHookFull(t, timeout, false)
}

func TestRunHook(t *testing.T) {
	cleanUp()

	testRunHook(t, 0)
}

func TestRunHookTimeout(t *testing.T) {
	testRunHook(t, 1)
}

func TestRunHookExitFailure(t *testing.T) {
	hook := createWrongHook()
	s := createTestSandbox()

	err := hook.runHook(s)
	if err == nil {
		t.Fatal()
	}
}

func TestRunHookTimeoutFailure(t *testing.T) {
	hook := createHook(1)

	hook.Args = append(hook.Args, "2")

	s := createTestSandbox()

	err := hook.runHook(s)
	if err == nil {
		t.Fatal()
	}
}

func TestRunHookWaitFailure(t *testing.T) {
	hook := createHook(60)

	hook.Args = append(hook.Args, "1", "panic")
	s := createTestSandbox()

	err := hook.runHook(s)
	if err == nil {
		t.Fatal()
	}
}

func testRunHookInvalidCommand(t *testing.T, timeout int) {
	cleanUp()

	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}

	defer os.RemoveAll(dir)

	cmd := filepath.Join(dir, "does-not-exist")

	savedDefaultMockHookBinPath := DefaultMockHookBinPath
	DefaultMockHookBinPath = cmd

	defer func() {
		DefaultMockHookBinPath = savedDefaultMockHookBinPath
	}()

	testRunHookFull(t, timeout, true)
}

func TestRunHookInvalidCommand(t *testing.T) {
	testRunHookInvalidCommand(t, 0)
}

func TestRunHookTimeoutInvalidCommand(t *testing.T) {
	testRunHookInvalidCommand(t, 1)
}

func testHooks(t *testing.T, hook *Hook) {
	hooks := &Hooks{
		PreStartHooks:  []Hook{*hook},
		PostStartHooks: []Hook{*hook},
		PostStopHooks:  []Hook{*hook},
	}
	s := createTestSandbox()

	err := hooks.preStartHooks(s)
	if err != nil {
		t.Fatal(err)
	}

	err = hooks.postStartHooks(s)
	if err != nil {
		t.Fatal(err)
	}

	err = hooks.postStopHooks(s)
	if err != nil {
		t.Fatal(err)
	}
}

func testFailingHooks(t *testing.T, hook *Hook) {
	hooks := &Hooks{
		PreStartHooks:  []Hook{*hook},
		PostStartHooks: []Hook{*hook},
		PostStopHooks:  []Hook{*hook},
	}
	s := createTestSandbox()

	err := hooks.preStartHooks(s)
	if err == nil {
		t.Fatal(err)
	}

	err = hooks.postStartHooks(s)
	if err != nil {
		t.Fatal(err)
	}

	err = hooks.postStopHooks(s)
	if err != nil {
		t.Fatal(err)
	}
}

func TestHooks(t *testing.T) {
	testHooks(t, createHook(0))
}

func TestHooksTimeout(t *testing.T) {
	testHooks(t, createHook(1))
}

func TestFailingHooks(t *testing.T) {
	testFailingHooks(t, createWrongHook())
}

func TestEmptyHooks(t *testing.T) {
	hooks := &Hooks{}
	s := createTestSandbox()

	err := hooks.preStartHooks(s)
	if err != nil {
		t.Fatal(err)
	}

	err = hooks.postStartHooks(s)
	if err != nil {
		t.Fatal(err)
	}

	err = hooks.postStopHooks(s)
	if err != nil {
		t.Fatal(err)
	}
}
