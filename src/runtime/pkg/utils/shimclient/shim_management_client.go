// Copyright (c) 2022 Databricks Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package shimclient

import (
	"bytes"
	"fmt"
	"io"
	"net"
	"net/http"
	"time"

	cdshim "github.com/containerd/containerd/runtime/v2/shim"
	shim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
)

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

func DoGet(sandboxID string, timeoutInSeconds time.Duration, urlPath string) ([]byte, error) {
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

func DoPost(sandboxID string, timeoutInSeconds time.Duration, urlPath string, payload []byte) error {
	client, err := BuildShimClient(sandboxID, timeoutInSeconds)
	if err != nil {
		return err
	}

	resp, err := client.Post(fmt.Sprintf("http://shim/%s", urlPath), "application/json", bytes.NewBuffer(payload))
	defer func() {
		resp.Body.Close()
	}()
	return err
}
