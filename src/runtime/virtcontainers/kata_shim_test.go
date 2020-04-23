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
	"github.com/kata-containers/runtime/virtcontainers/utils"
	"github.com/stretchr/testify/assert"
)

const (
	testContainer = "testContainer"
)

var (
	testKataShimPath     = "/usr/bin/virtcontainers/bin/test/kata-shim"
	testKataShimProxyURL = "foo:///foo/kata-containers/proxy.sock"
	testWrongConsolePath = "/foo/wrong-console"
)

func getMockKataShimBinPath() string {
	if DefaultMockKataShimBinPath == "" {
		return testKataShimPath
	}

	return DefaultMockKataShimBinPath
}

func testKataShimStart(t *testing.T, sandbox *Sandbox, params ShimParams, expectFail bool) {
	s := &kataShim{}
	assert := assert.New(t)

	pid, err := s.start(sandbox, params)
	if expectFail {
		assert.Error(err)
		assert.Equal(pid, -1)
	} else {
		assert.NoError(err)
		assert.NotEqual(pid, -1)
	}
}

func TestKataShimStartNilSandboxConfigFailure(t *testing.T) {
	testKataShimStart(t, &Sandbox{}, ShimParams{}, true)
}

func TestKataShimStartNilShimConfigFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{},
	}

	testKataShimStart(t, sandbox, ShimParams{}, true)
}

func TestKataShimStartShimPathEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType:   KataShimType,
			ShimConfig: ShimConfig{},
		},
	}

	testKataShimStart(t, sandbox, ShimParams{}, true)
}

func TestKataShimStartShimTypeInvalid(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType:   "foo",
			ShimConfig: ShimConfig{},
		},
	}

	testKataShimStart(t, sandbox, ShimParams{}, true)
}

func TestKataShimStartParamsTokenEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: getMockKataShimBinPath(),
			},
		},
	}

	testKataShimStart(t, sandbox, ShimParams{}, true)
}

func TestKataShimStartParamsURLEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: getMockKataShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Token: "testToken",
	}

	testKataShimStart(t, sandbox, params, true)
}

func TestKataShimStartParamsContainerEmptyFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: getMockKataShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Token: "testToken",
		URL:   "unix://is/awesome",
	}

	testKataShimStart(t, sandbox, params, true)
}

func TestKataShimStartParamsInvalidCommand(t *testing.T) {
	dir, err := ioutil.TempDir("", "")
	assert.NoError(t, err)
	defer os.RemoveAll(dir)

	cmd := filepath.Join(dir, "does-not-exist")

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: cmd,
			},
		},
	}

	params := ShimParams{
		Token: "testToken",
		URL:   "http://foo",
	}

	testKataShimStart(t, sandbox, params, true)
}

func startKataShimStartWithoutConsoleSuccessful(t *testing.T, detach bool) (*os.File, *os.File, *os.File, *Sandbox, ShimParams, error) {
	saveStdout := os.Stdout
	rStdout, wStdout, err := os.Pipe()
	if err != nil {
		return nil, nil, nil, &Sandbox{}, ShimParams{}, err
	}

	os.Stdout = wStdout

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: getMockKataShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Container: testContainer,
		Token:     "testToken",
		URL:       testKataShimProxyURL,
		Detach:    detach,
	}

	return rStdout, wStdout, saveStdout, sandbox, params, nil
}

func TestKataShimStartSuccessful(t *testing.T) {
	rStdout, wStdout, saveStdout, sandbox, params, err := startKataShimStartWithoutConsoleSuccessful(t, false)
	assert := assert.New(t)
	assert.NoError(err)

	defer func() {
		os.Stdout = saveStdout
		rStdout.Close()
		wStdout.Close()
	}()

	testKataShimStart(t, sandbox, params, false)

	bufStdout := make([]byte, 1024)
	_, err = rStdout.Read(bufStdout)
	assert.NoError(err)
	assert.True(strings.Contains(string(bufStdout), ShimStdoutOutput))
}

func TestKataShimStartDetachSuccessful(t *testing.T) {
	rStdout, wStdout, saveStdout, sandbox, params, err := startKataShimStartWithoutConsoleSuccessful(t, true)
	assert.NoError(t, err)

	defer func() {
		os.Stdout = saveStdout
		wStdout.Close()
		rStdout.Close()
	}()

	testKataShimStart(t, sandbox, params, false)

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
		assert.NoError(t, err)
	case <-time.After(time.Duration(20) * time.Millisecond):
		return
	}
}

func TestKataShimStartWithConsoleNonExistingFailure(t *testing.T) {
	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: getMockKataShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Token:   "testToken",
		URL:     testKataShimProxyURL,
		Console: testWrongConsolePath,
	}

	testKataShimStart(t, sandbox, params, true)
}

// unlockpt unlocks the slave pseudoterminal device corresponding to the master pseudoterminal referred to by f.
func unlockpt(f *os.File) error {
	var u int32

	return utils.Ioctl(f.Fd(), syscall.TIOCSPTLCK, uintptr(unsafe.Pointer(&u)))
}

// ptsname retrieves the name of the first available pts for the given master.
func ptsname(f *os.File) (string, error) {
	var n int32

	if err := utils.Ioctl(f.Fd(), syscall.TIOCGPTN, uintptr(unsafe.Pointer(&n))); err != nil {
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

func TestKataShimStartWithConsoleSuccessful(t *testing.T) {
	defer cleanUp()

	master, console, err := newConsole()
	t.Logf("Console created for tests:%s\n", console)
	assert.NoError(t, err)

	sandbox := &Sandbox{
		config: &SandboxConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: getMockKataShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Container: testContainer,
		Token:     "testToken",
		URL:       testKataShimProxyURL,
		Console:   console,
	}

	testKataShimStart(t, sandbox, params, false)
	master.Close()
}
