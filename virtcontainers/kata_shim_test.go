//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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

	. "github.com/containers/virtcontainers/pkg/mock"
)

// These tests don't care about the format of the container ID
const testKataContainer = "testContainer"

var testKataShimPath = "/usr/bin/virtcontainers/bin/test/kata-shim"
var testKataShimProxyURL = "foo:///foo/kata-containers/proxy.sock"

func getMockKataShimBinPath() string {
	if DefaultMockKataShimBinPath == "" {
		return testKataShimPath
	}

	return DefaultMockKataShimBinPath
}

func testKataShimStart(t *testing.T, pod Pod, params ShimParams, expectFail bool) {
	s := &kataShim{}

	pid, err := s.start(pod, params)
	if expectFail {
		if err == nil || pid != -1 {
			t.Fatalf("This test should fail (pod %+v, params %+v, expectFail %t)",
				pod, params, expectFail)
		}
	} else {
		if err != nil {
			t.Fatalf("This test should pass (pod %+v, params %+v, expectFail %t): %s",
				pod, params, expectFail, err)
		}

		if pid == -1 {
			t.Fatalf("This test should pass (pod %+v, params %+v, expectFail %t)",
				pod, params, expectFail)
		}
	}
}

func TestKataShimStartNilPodConfigFailure(t *testing.T) {
	testKataShimStart(t, Pod{}, ShimParams{}, true)
}

func TestKataShimStartNilShimConfigFailure(t *testing.T) {
	pod := Pod{
		config: &PodConfig{},
	}

	testKataShimStart(t, pod, ShimParams{}, true)
}

func TestKataShimStartShimPathEmptyFailure(t *testing.T) {
	pod := Pod{
		config: &PodConfig{
			ShimType:   KataShimType,
			ShimConfig: ShimConfig{},
		},
	}

	testKataShimStart(t, pod, ShimParams{}, true)
}

func TestKataShimStartShimTypeInvalid(t *testing.T) {
	pod := Pod{
		config: &PodConfig{
			ShimType:   "foo",
			ShimConfig: ShimConfig{},
		},
	}

	testKataShimStart(t, pod, ShimParams{}, true)
}

func TestKataShimStartParamsTokenEmptyFailure(t *testing.T) {
	pod := Pod{
		config: &PodConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: getMockKataShimBinPath(),
			},
		},
	}

	testKataShimStart(t, pod, ShimParams{}, true)
}

func TestKataShimStartParamsURLEmptyFailure(t *testing.T) {
	pod := Pod{
		config: &PodConfig{
			ShimType: KataShimType,
			ShimConfig: ShimConfig{
				Path: getMockKataShimBinPath(),
			},
		},
	}

	params := ShimParams{
		Token: "testToken",
	}

	testKataShimStart(t, pod, params, true)
}

func TestKataShimStartParamsContainerEmptyFailure(t *testing.T) {
	pod := Pod{
		config: &PodConfig{
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

	testKataShimStart(t, pod, params, true)
}

func TestKataShimStartParamsInvalidCommand(t *testing.T) {
	dir, err := ioutil.TempDir("", "")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)

	cmd := filepath.Join(dir, "does-not-exist")

	pod := Pod{
		config: &PodConfig{
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

	testKataShimStart(t, pod, params, true)
}

func startKataShimStartWithoutConsoleSuccessful(t *testing.T, detach bool) (*os.File, *os.File, *os.File, Pod, ShimParams, error) {
	saveStdout := os.Stdout
	rStdout, wStdout, err := os.Pipe()
	if err != nil {
		return nil, nil, nil, Pod{}, ShimParams{}, err
	}

	os.Stdout = wStdout

	pod := Pod{
		config: &PodConfig{
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

	return rStdout, wStdout, saveStdout, pod, params, nil
}

func TestKataShimStartSuccessful(t *testing.T) {
	rStdout, wStdout, saveStdout, pod, params, err := startKataShimStartWithoutConsoleSuccessful(t, false)
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		os.Stdout = saveStdout
		rStdout.Close()
		wStdout.Close()
	}()

	testKataShimStart(t, pod, params, false)

	bufStdout := make([]byte, 1024)
	if _, err := rStdout.Read(bufStdout); err != nil {
		t.Fatal(err)
	}

	if !strings.Contains(string(bufStdout), ShimStdoutOutput) {
		t.Fatalf("Substring %q not found in %q", ShimStdoutOutput, string(bufStdout))
	}
}

func TestKataShimStartDetachSuccessful(t *testing.T) {
	rStdout, wStdout, saveStdout, pod, params, err := startKataShimStartWithoutConsoleSuccessful(t, true)
	if err != nil {
		t.Fatal(err)
	}

	defer func() {
		os.Stdout = saveStdout
		wStdout.Close()
		rStdout.Close()
	}()

	testKataShimStart(t, pod, params, false)

	readCh := make(chan error)
	go func() {
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
	pod := Pod{
		config: &PodConfig{
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

	testKataShimStart(t, pod, params, true)
}

func TestKataShimStartWithConsoleSuccessful(t *testing.T) {
	cleanUp()

	master, console, err := newConsole()
	t.Logf("Console created for tests:%s\n", console)

	if err != nil {
		t.Fatal(err)
	}

	pod := Pod{
		config: &PodConfig{
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

	testKataShimStart(t, pod, params, false)
	master.Close()
}
