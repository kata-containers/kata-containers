// Copyright (c) 2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package nsenter

import (
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"syscall"
	"testing"

	"github.com/stretchr/testify/assert"
	"golang.org/x/sys/unix"

	ktu "github.com/kata-containers/kata-containers/src/runtime/pkg/katatestutils"
)

const testPID = 12345

var tu = ktu.NewTestConstraint(true)

func TestGetNSPathFromPID(t *testing.T) {
	for nsType := range CloneFlagsTable {
		expectedPath := fmt.Sprintf("/proc/%d/ns/%s", testPID, nsType)
		path := getNSPathFromPID(testPID, nsType)
		assert.Equal(t, path, expectedPath)
	}
}

func TestGetCurrentThreadNSPath(t *testing.T) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	currentPID := os.Getpid()
	currentTID := unix.Gettid()
	for nsType := range CloneFlagsTable {
		expectedPath := fmt.Sprintf("/proc/%d/task/%d/ns/%s", currentPID, currentTID, nsType)
		path := getCurrentThreadNSPath(nsType)
		assert.Equal(t, path, expectedPath)
	}
}

func TestGetFileFromNSEmptyNSPathFailure(t *testing.T) {
	nsFile, err := getFileFromNS("")
	assert.NotNil(t, err, "Empty path should result as a failure")
	assert.Nil(t, nsFile, "The file handler returned should be nil")
}

func TestGetFileFromNSNotExistingNSPathFailure(t *testing.T) {
	nsFile, err := ioutil.TempFile("", "not-existing-ns-path")
	assert.NoError(t, err)
	nsFilePath := nsFile.Name()
	nsFile.Close()

	assert.NoError(t, os.Remove(nsFilePath))

	nsFile, err = getFileFromNS(nsFilePath)
	assert.NotNil(t, err, "Not existing path should result as a failure")
	assert.Nil(t, nsFile, "The file handler returned should be nil")
}

func TestGetFileFromNSWrongNSPathFailure(t *testing.T) {
	nsFile, err := ioutil.TempFile("", "wrong-ns-path")
	assert.NoError(t, err)
	nsFilePath := nsFile.Name()
	nsFile.Close()

	defer os.Remove(nsFilePath)

	nsFile, err = getFileFromNS(nsFilePath)
	assert.NotNil(t, err, "Should fail because wrong filesystem")
	assert.Nil(t, nsFile, "The file handler returned should be nil")
}

func TestGetFileFromNSSuccessful(t *testing.T) {
	for nsType := range CloneFlagsTable {
		nsFilePath := fmt.Sprintf("/proc/self/ns/%s", string(nsType))
		nsFile, err := getFileFromNS(nsFilePath)
		assert.Nil(t, err, "Should have succeeded: %v", err)
		assert.NotNil(t, nsFile, "The file handler should not be nil")
		if nsFile != nil {
			nsFile.Close()
		}
	}
}

func startSleepBinary(duration int, cloneFlags int) (int, error) {
	sleepBinName := "sleep"
	sleepPath, err := exec.LookPath(sleepBinName)
	if err != nil {
		return -1, fmt.Errorf("Could not find %q: %v", sleepBinName, err)
	}

	cmd := exec.Command(sleepPath, strconv.Itoa(duration))
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Cloneflags: uintptr(cloneFlags),
	}

	if err := cmd.Start(); err != nil {
		return -1, err
	}

	return cmd.Process.Pid, nil
}

func TestSetNSNilFileHandlerFailure(t *testing.T) {
	err := setNS(nil, "")
	assert.NotNil(t, err, "Should fail because file handler is nil")
}

func TestSetNSUnknownNSTypeFailure(t *testing.T) {
	file := &os.File{}
	err := setNS(file, "")
	assert.NotNil(t, err, "Should fail because unknown ns type")
}

func TestSetNSWrongFileFailure(t *testing.T) {
	nsFile, err := ioutil.TempFile("", "wrong-ns-path")
	assert.NoError(t, err)
	defer func() {
		nsFilePath := nsFile.Name()
		nsFile.Close()
		os.Remove(nsFilePath)
	}()

	err = setNS(nsFile, NSTypeIPC)
	assert.NotNil(t, err, "Should fail because file is not a namespace")
}

func supportedNamespaces() []Namespace {
	var list []Namespace
	var ns = []Namespace{
		{Type: NSTypeCGroup},
		{Type: NSTypeIPC},
		{Type: NSTypeNet},
		{Type: NSTypePID},
		{Type: NSTypeUTS},
	}

	for _, n := range ns {
		if _, err := os.Stat(fmt.Sprint("/proc/self/ns/", string(n.Type))); err == nil {
			list = append(list, n)
		}
	}

	return list
}

func testToRunNil() error {
	return nil
}

func TestNsEnterEmptyPathAndPIDFromNSListFailure(t *testing.T) {
	err := NsEnter(supportedNamespaces(), testToRunNil)
	assert.NotNil(t, err, "Should fail because neither a path nor a PID"+
		" has been provided by every namespace of the list")
}

func TestNsEnterEmptyNamespaceListSuccess(t *testing.T) {
	err := NsEnter([]Namespace{}, testToRunNil)
	assert.Nil(t, err, "Should not fail since closure should return nil: %v", err)
}

func TestNsEnterSuccessful(t *testing.T) {
	if tu.NotValid(ktu.NeedRoot()) {
		t.Skip(ktu.TestDisabledNeedRoot)
	}
	nsList := supportedNamespaces()
	sleepDuration := 60

	cloneFlags := 0
	for _, ns := range nsList {
		cloneFlags |= CloneFlagsTable[ns.Type]
	}

	sleepPID, err := startSleepBinary(sleepDuration, cloneFlags)
	assert.NoError(t, err)
	defer func() {
		if sleepPID > 1 {
			unix.Kill(sleepPID, syscall.SIGKILL)
		}
	}()

	for idx := range nsList {
		nsList[idx].Path = getNSPathFromPID(sleepPID, nsList[idx].Type)
		nsList[idx].PID = sleepPID
	}

	var sleepPIDFromNsEnter int

	testToRun := func() error {
		sleepPIDFromNsEnter, err = startSleepBinary(sleepDuration, 0)
		if err != nil {
			return err
		}

		return nil
	}

	err = NsEnter(nsList, testToRun)
	assert.Nil(t, err, "%v", err)

	defer func() {
		if sleepPIDFromNsEnter > 1 {
			unix.Kill(sleepPIDFromNsEnter, syscall.SIGKILL)
		}
	}()

	for _, ns := range nsList {
		nsPathEntered := getNSPathFromPID(sleepPIDFromNsEnter, ns.Type)

		// Here we are trying to resolve the path but it fails because
		// namespaces links don't really exist. For this reason, the
		// call to EvalSymlinks will fail when it will try to stat the
		// resolved path found. As we only care about the path, we can
		// retrieve it from the PathError structure.
		evalExpectedNSPath, err := filepath.EvalSymlinks(ns.Path)
		if err != nil {
			evalExpectedNSPath = err.(*os.PathError).Path
		}

		// Same thing here, resolving the namespace path.
		evalNSEnteredPath, err := filepath.EvalSymlinks(nsPathEntered)
		if err != nil {
			evalNSEnteredPath = err.(*os.PathError).Path
		}

		_, evalExpectedNS := filepath.Split(evalExpectedNSPath)
		_, evalNSEntered := filepath.Split(evalNSEnteredPath)

		assert.Equal(t, evalExpectedNS, evalNSEntered)
	}
}
