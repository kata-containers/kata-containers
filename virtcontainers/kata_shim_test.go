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
	"testing"
	"time"

	. "github.com/kata-containers/runtime/virtcontainers/pkg/mock"
)

var testKataShimPath = "/usr/bin/virtcontainers/bin/test/kata-shim"
var testKataShimProxyURL = "foo:///foo/kata-containers/proxy.sock"

func getMockKataShimBinPath() string {
	if DefaultMockKataShimBinPath == "" {
		return testKataShimPath
	}

	return DefaultMockKataShimBinPath
}

func testKataShimStart(t *testing.T, sandbox *Sandbox, params ShimParams, expectFail bool) {
	s := &kataShim{}

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
	if err != nil {
		t.Fatal(err)
	}
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
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		os.Stdout = saveStdout
		rStdout.Close()
		wStdout.Close()
	}()

	testKataShimStart(t, sandbox, params, false)

	bufStdout := make([]byte, 1024)
	if _, err := rStdout.Read(bufStdout); err != nil {
		t.Fatal(err)
	}

	if !strings.Contains(string(bufStdout), ShimStdoutOutput) {
		t.Fatalf("Substring %q not found in %q", ShimStdoutOutput, string(bufStdout))
	}
}

func TestKataShimStartDetachSuccessful(t *testing.T) {
	rStdout, wStdout, saveStdout, sandbox, params, err := startKataShimStartWithoutConsoleSuccessful(t, true)
	if err != nil {
		t.Fatal(err)
	}

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
		if err != nil {
			t.Fatal(err)
		}
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

func TestKataShimStartWithConsoleSuccessful(t *testing.T) {
	defer cleanUp()

	master, console, err := newConsole()
	t.Logf("Console created for tests:%s\n", console)

	if err != nil {
		t.Fatal(err)
	}

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
