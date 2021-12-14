// Copyright (c) 2020-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"time"

	cdshim "github.com/containerd/containerd/runtime/v2/shim"

	shim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
)

const (
	defaultTimeout = 3 * time.Second
)

func commonServeError(w http.ResponseWriter, status int, err error) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	w.WriteHeader(status)
	if err != nil {
		fmt.Fprintln(w, err.Error())
	}
}

func getSandboxIDFromReq(r *http.Request) (string, error) {
	sandbox := r.URL.Query().Get("sandbox")
	if sandbox != "" {
		return sandbox, nil
	}
	return "", fmt.Errorf("sandbox not found in %+v", r.URL.Query())
}

func getSandboxFS() string {
	return shim.GetSandboxesStoragePath()
}

func checkSandboxFSExists(sandboxID string) bool {
	sbsPath := filepath.Join(string(filepath.Separator), getSandboxFS(), sandboxID)
	_, err := os.Stat(sbsPath)

	return !os.IsNotExist(err)
}

// BuildShimClient builds and returns an http client for communicating with the provided sandbox
func BuildShimClient(sandboxID string, timeout time.Duration) (*http.Client, error) {
	return buildUnixSocketClient(shim.SocketAddress(sandboxID), timeout)
}

// buildUnixSocketClient build http client for Unix socket
func buildUnixSocketClient(socketAddr string, timeout time.Duration) (*http.Client, error) {
	transport := &http.Transport{
		DisableKeepAlives: true,
		Dial: func(proto, addr string) (conn net.Conn, err error) {
			return cdshim.AnonDialer(socketAddr, timeout)
		},
	}

	client := &http.Client{
		Transport: transport,
	}

	if timeout > 0 {
		client.Timeout = timeout
	}

	return client, nil
}

func doGet(sandboxID string, timeoutInSeconds time.Duration, urlPath string) ([]byte, error) {
	client, err := BuildShimClient(sandboxID, timeoutInSeconds)
	if err != nil {
		return nil, err
	}

	resp, err := client.Get(fmt.Sprintf("http://shim/%s", urlPath))
	if err != nil {
		return nil, err
	}

	defer func() {
		resp.Body.Close()
	}()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	return body, nil
}
