// Copyright (c) 2017 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"
	"io"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"
	"unsafe"

	. "github.com/kata-containers/runtime/virtcontainers/pkg/mock"
)

// These tests don't care about the format of the container ID
const testContainer = "testContainer"

var testCCShimPath = "/usr/bin/virtcontainers/bin/test/cc-shim"
var testProxyURL = "foo:///foo/clear-containers/proxy.sock"
var testWrongConsolePath = "/foo/wrong-console"

func getMockCCShimBinPath() string {
	if DefaultMockCCShimBinPath == "" {
		return testCCShimPath
	}

	return DefaultMockCCShimBinPath
}

func testCCShimStart(t *testing.T, sandbox *Sandbox, params ShimParams, expectFail bool) {
	s := &ccShim{}

	pid, err := s.start(sandbox, params)
	if expectFail {
		if err == nil || pid != -1 {
			t.Fatalf("This test should fail (sandbox %+v, params %+v, expectFail %t)",
				sandbox, params, expectFail)
		}
	} else {
		if err != nil {
			t.Fatalf("This test should pass (sandbox %+v, params %+v, expectFail %t): %s",
				sandbox, params, expectFail, err)
		}

		if pid == -1 {
			t.Fatalf("This test should pass (sandbox %+v, params %+v, expectFail %t)",
				sandbox, params, expectFail)
		}
	}
}

func TestCCShimStartNilSandboxConfigFailure(t *testing.T) {
	testCCShimStart(t, &Sandbox{}, ShimParams{}, true)
}

func TestCCShimStartNilShimConfigFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{},
	}

	testCCShimStart(t, sandbox, ShimParams{}, true)
}

func TestCCShimStartShimPathEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType:   CCShimType,
			ShimConfig: ShimConfig{},
		},
	}

	testCCShimStart(t, sandbox, ShimParams{}, true)
}

func TestCCShimStartShimTypeInvalid(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType:   "foo",
			ShimConfig: ShimConfig{},
		},
	}

	testCCShimStart(t, sandbox, ShimParams{}, true)
}

func TestCCShimStartParamsTokenEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: CCShimType,
			ShimConfig: ShimConfig{
				Path: getMockCCShimBinPath(),
			},
		},
	}

	testCCShimStart(t, sandbox, ShimParams{}, true)
}

func TestCCShimStartParamsURLEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: CCShimType,
			ShimConfig: ShimConfig{
				Path: getMockCCShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Token: "testToken",
	}

	testCCShimStart(t, sandbox, params, true)
}

func TestCCShimStartParamsContainerEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: CCShimType,
			ShimConfig: ShimConfig{
				Path: getMockCCShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Token: "testToken",
		URL:   "unix://is/awesome",
	}

	testCCShimStart(t, sandbox, params, true)
}

func TestCCShimStartParamsInvalidCommand(t *testing.T) {
	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	cmd := filepath.Join(dir, "does-not-exist")

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: CCShimType,
			ShimConfig: ShimConfig{
				Path: cmd,
			},
		},
	}

	params := ShimParams{
		Token: "testToken",
		URL:   "http://foo",
	}

	testCCShimStart(t, sandbox, params, true)
}

func startCCShimStartWithoutConsoleSuccessful(t *testing.T, detach bool) (*os.File, *os.File, *os.File, *Sandbox, ShimParams, error) {
	saveStdout := os.Stdout
	rStdout, wStdout, err := os.Pipe()
	if err != nil {
		return nil, nil, nil, &Sandbox{}, ShimParams{}, err
	}

	os.Stdout = wStdout

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: CCShimType,
			ShimConfig: ShimConfig{
				Path: getMockCCShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Container: testContainer,
		Token:     "testToken",
		URL:       testProxyURL,
		Detach:    detach,
	}

	return rStdout, wStdout, saveStdout, sandbox, params, nil
}

func TestCCShimStartSuccessful(t *testing.T) {
	rStdout, wStdout, saveStdout, sandbox, params, err := startCCShimStartWithoutConsoleSuccessful(t, false)
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		os.Stdout = saveStdout
		rStdout.Close()
		wStdout.Close()
	}()

	testCCShimStart(t, sandbox, params, false)

	bufStdout := make([]byte, 1024)
	if _, err := rStdout.Read(bufStdout); err != nil {
		t.Fatal(err)
	}

	if !strings.Contains(string(bufStdout), ShimStdoutOutput) {
		t.Fatalf("Substring %q not found in %q", ShimStdoutOutput, string(bufStdout))
	}
}

func TestCCShimStartDetachSuccessful(t *testing.T) {
	rStdout, wStdout, saveStdout, sandbox, params, err := startCCShimStartWithoutConsoleSuccessful(t, true)
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		os.Stdout = saveStdout
		wStdout.Close()
		rStdout.Close()
	}()

	testCCShimStart(t, sandbox, params, false)

	readCh := make(chan error, 1)
	go func() {
		defer close(readCh)
		bufStdout := make([]byte, 1024)
		n, err := rStdout.Read(bufStdout)
		if err != nil && err != io.EOF {
			readCh <- err
			return
		}

		if n > 0 {
			readCh <- fmt.Errorf("Not expecting to read anything, Got %q", string(bufStdout))
			return
		}

		readCh <- nil
	}()

	select {
	case err := <-readCh:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Duration(20) * time.Millisecond):
		return
	}
}

func TestCCShimStartWithConsoleNonExistingFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: CCShimType,
			ShimConfig: ShimConfig{
				Path: getMockCCShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Token:   "testToken",
		URL:     testProxyURL,
		Console: testWrongConsolePath,
	}

	testCCShimStart(t, sandbox, params, true)
}

func ioctl(fd uintptr, flag, data uintptr) error {
	if _, _, err := syscall.Syscall(syscall.SYS_IOCTL, fd, flag, data); err != 0 {
		return err
	}

	return nil
}

// unlockpt unlocks the slave pseudoterminal device corresponding to the master pseudoterminal referred to by f.
func unlockpt(f *os.File) error {
	var u int32

	return ioctl(f.Fd(), syscall.TIOCSPTLCK, uintptr(unsafe.Pointer(&u)))
}

// ptsname retrieves the name of the first available pts for the given master.
func ptsname(f *os.File) (string, error) {
	var n int32

	if err := ioctl(f.Fd(), syscall.TIOCGPTN, uintptr(unsafe.Pointer(&n))); err != nil {
		return "", err
	}

	return fmt.Sprintf("/dev/pts/%d", n), nil
}

func newConsole() (*os.File, string, error) {
	master, err := os.OpenFile("/dev/ptmx", syscall.O_RDWR|syscall.O_NOCTTY|syscall.O_CLOEXEC, 0)
	if err != nil {
		return nil, "", err
	}

	console, err := ptsname(master)
	if err != nil {
		return nil, "", err
	}

	if err := unlockpt(master); err != nil {
		return nil, "", err
	}

	if err := os.Chmod(console, 0600); err != nil {
		return nil, "", err
	}

	return master, console, nil
}

func TestCCShimStartWithConsoleSuccessful(t *testing.T) {
	defer cleanUp()

	master, console, err := newConsole()
	t.Logf("Console created for tests:%s\n", console)

	if err != nil {
		t.Fatal(err)
	}

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: CCShimType,
			ShimConfig: ShimConfig{
				Path: getMockCCShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Container: testContainer,
		Token:     "testToken",
		URL:       testProxyURL,
		Console:   console,
	}

	testCCShimStart(t, sandbox, params, false)
	master.Close()
}
