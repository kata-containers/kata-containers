// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"os/exec"
	"syscall"
	"testing"

	"github.com/stretchr/testify/assert"
)

const (
	testRunningProcess = "sleep"
)

func testSetShimType(t *testing.T, value string, expected ShimType) {
	var shimType ShimType
	assert := assert.New(t)

	err := (&shimType).Set(value)
	assert.NoError(err)
	assert.Equal(shimType, expected)
}

func TestSetKataShimType(t *testing.T) {
	testSetShimType(t, "kataShim", KataShimType)
}

func TestSetNoopShimType(t *testing.T) {
	testSetShimType(t, "noopShim", NoopShimType)
}

func TestSetUnknownShimType(t *testing.T) {
	var shimType ShimType
	assert := assert.New(t)

	unknownType := "unknown"

	err := (&shimType).Set(unknownType)
	assert.Error(err)
	assert.NotEqual(shimType, NoopShimType)
}

func testStringFromShimType(t *testing.T, shimType ShimType, expected string) {
	shimTypeStr := (&shimType).String()
	assert := assert.New(t)
	assert.Equal(shimTypeStr, expected)
}

func TestStringFromKataShimType(t *testing.T) {
	shimType := KataShimType
	testStringFromShimType(t, shimType, "kataShim")
}

func TestStringFromNoopShimType(t *testing.T) {
	shimType := NoopShimType
	testStringFromShimType(t, shimType, "noopShim")
}

func TestStringFromKataBuiltInShimType(t *testing.T) {
	shimType := KataBuiltInShimType
	testStringFromShimType(t, shimType, "kataBuiltInShim")
}

func TestStringFromUnknownShimType(t *testing.T) {
	var shimType ShimType
	testStringFromShimType(t, shimType, "")
}

func testNewShimFromShimType(t *testing.T, shimType ShimType, expected shim) {
	assert := assert.New(t)
	result, err := newShim(shimType)
	assert.NoError(err)
	assert.Exactly(result, expected)
}

func TestNewShimFromKataShimType(t *testing.T) {
	shimType := KataShimType
	expectedShim := &kataShim{}
	testNewShimFromShimType(t, shimType, expectedShim)
}

func TestNewShimFromNoopShimType(t *testing.T) {
	shimType := NoopShimType
	expectedShim := &noopShim{}
	testNewShimFromShimType(t, shimType, expectedShim)
}

func TestNewShimFromKataBuiltInShimType(t *testing.T) {
	shimType := KataBuiltInShimType
	expectedShim := &kataBuiltInShim{}
	testNewShimFromShimType(t, shimType, expectedShim)
}

func TestNewShimFromUnknownShimType(t *testing.T) {
	var shimType ShimType
	assert := assert.New(t)

	_, err := newShim(shimType)
	assert.NoError(err)
}

func testNewShimConfigFromSandboxConfig(t *testing.T, sandboxConfig SandboxConfig, expected interface{}) {
	assert := assert.New(t)
	result := newShimConfig(sandboxConfig)
	assert.Exactly(result, expected)
}

func TestNewShimConfigFromKataShimSandboxConfig(t *testing.T) {
	shimConfig := ShimConfig{}

	sandboxConfig := SandboxConfig{
		ShimType:   KataShimType,
		ShimConfig: shimConfig,
	}

	testNewShimConfigFromSandboxConfig(t, sandboxConfig, shimConfig)
}

func TestNewShimConfigFromNoopShimSandboxConfig(t *testing.T) {
	sandboxConfig := SandboxConfig{
		ShimType: NoopShimType,
	}

	testNewShimConfigFromSandboxConfig(t, sandboxConfig, nil)
}

func TestNewShimConfigFromKataBuiltInShimSandboxConfig(t *testing.T) {
	sandboxConfig := SandboxConfig{
		ShimType: KataBuiltInShimType,
	}

	testNewShimConfigFromSandboxConfig(t, sandboxConfig, nil)
}

func TestNewShimConfigFromUnknownShimSandboxConfig(t *testing.T) {
	var shimType ShimType

	sandboxConfig := SandboxConfig{
		ShimType: shimType,
	}

	testNewShimConfigFromSandboxConfig(t, sandboxConfig, nil)
}

func testRunSleep0AndGetPid(t *testing.T) int {
	assert := assert.New(t)

	binPath, err := exec.LookPath(testRunningProcess)
	assert.NoError(err)

	cmd := exec.Command(binPath, "0")
	err = cmd.Start()
	assert.NoError(err)

	pid := cmd.Process.Pid
	err = cmd.Wait()
	assert.NoError(err)

	return pid
}

func testRunSleep999AndGetCmd(t *testing.T) *exec.Cmd {
	assert := assert.New(t)

	binPath, err := exec.LookPath(testRunningProcess)
	assert.NoError(err)

	cmd := exec.Command(binPath, "999")
	err = cmd.Start()
	assert.NoError(err)
	return cmd
}

func TestStopShimSuccessfulProcessNotRunning(t *testing.T) {
	assert := assert.New(t)
	pid := testRunSleep0AndGetPid(t)
	assert.NoError(stopShim(pid))
}

func TestStopShimSuccessfulProcessRunning(t *testing.T) {
	assert := assert.New(t)
	cmd := testRunSleep999AndGetCmd(t)
	assert.NoError(stopShim(cmd.Process.Pid))
}

func testIsShimRunning(t *testing.T, pid int, expected bool) {
	assert := assert.New(t)
	running, err := isShimRunning(pid)
	assert.NoError(err)
	assert.Equal(running, expected)
}

func TestIsShimRunningFalse(t *testing.T) {
	pid := testRunSleep0AndGetPid(t)

	testIsShimRunning(t, pid, false)
}

func TestIsShimRunningTrue(t *testing.T) {
	cmd := testRunSleep999AndGetCmd(t)
	assert := assert.New(t)

	testIsShimRunning(t, cmd.Process.Pid, true)

	err := syscall.Kill(cmd.Process.Pid, syscall.SIGKILL)
	assert.NoError(err)
}

func TestWaitForShimInvalidPidSuccessful(t *testing.T) {
	wrongValuesList := []int{0, -1, -100}
	assert := assert.New(t)

	for _, val := range wrongValuesList {
		err := waitForShim(val)
		assert.NoError(err)
	}
}

func TestWaitForShimNotRunningSuccessful(t *testing.T) {
	pid := testRunSleep0AndGetPid(t)
	assert := assert.New(t)
	assert.NoError(waitForShim(pid))
}

func TestWaitForShimRunningForTooLongFailure(t *testing.T) {
	cmd := testRunSleep999AndGetCmd(t)
	assert := assert.New(t)

	waitForShimTimeout = 0.1
	assert.Error(waitForShim(cmd.Process.Pid))
	assert.NoError(syscall.Kill(cmd.Process.Pid, syscall.SIGKILL))
}
