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
	socketAddress, err := shim.ClientSocketAddress(sandboxID)
	if err != nil {
		return nil, err
	}

	return buildUnixSocketClient(socketAddress, timeout)
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

	resp, err := client.Get(fmt.Sprintf("http://shim%s", urlPath))
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

// DoPut will make a PUT request to the shim endpoint that handles the given sandbox ID
func DoPut(sandboxID string, timeoutInSeconds time.Duration, urlPath, contentType string, payload []byte) error {
	client, err := BuildShimClient(sandboxID, timeoutInSeconds)
	if err != nil {
		return err
	}

	req, err := http.NewRequest(http.MethodPut, fmt.Sprintf("http://shim%s", urlPath), bytes.NewBuffer(payload))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", contentType)

	resp, err := client.Do(req)
	if err != nil {
		return err
	}

	defer func() {
		if resp != nil {
			resp.Body.Close()
		}
	}()

	if resp.StatusCode != http.StatusOK {
		data, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("error sending put: url: %s, status code: %d, response data: %s", urlPath, resp.StatusCode, string(data))
	}

	return nil
}

// DoPost will make a POST request to the shim endpoint that handles the given sandbox ID
func DoPost(sandboxID string, timeoutInSeconds time.Duration, urlPath, contentType string, payload []byte) error {
	client, err := BuildShimClient(sandboxID, timeoutInSeconds)
	if err != nil {
		return err
	}

	resp, err := client.Post(fmt.Sprintf("http://shim%s", urlPath), contentType, bytes.NewBuffer(payload))
	if err != nil {
		return err
	}

	defer func() {
		if resp != nil {
			resp.Body.Close()
		}
	}()

	if resp.StatusCode != http.StatusOK {
		data, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("error sending post: url: %s, status code: %d, response data: %s", urlPath, resp.StatusCode, string(data))
	}

	return nil
}
