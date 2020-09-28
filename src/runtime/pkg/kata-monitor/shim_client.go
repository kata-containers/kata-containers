// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"fmt"
	"io/ioutil"
	"net"
	"net/http"
	"time"
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

func getSandboxIdFromReq(r *http.Request) (string, error) {
	sandbox := r.URL.Query().Get("sandbox")
	if sandbox != "" {
		return sandbox, nil
	}
	return "", fmt.Errorf("sandbox not found in %+v", r.URL.Query())
}

func (km *KataMonitor) buildShimClient(sandboxID, namespace string, timeout time.Duration) (*http.Client, error) {
	socket, err := km.getMonitorAddress(sandboxID, namespace)
	if err != nil {
		return nil, err
	}

	transport := &http.Transport{
		DisableKeepAlives: true,
		Dial: func(proto, addr string) (conn net.Conn, err error) {
			return net.Dial("unix", "\x00"+socket)
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

func (km *KataMonitor) doGet(sandboxID, namespace string, timeoutInSeconds time.Duration, urlPath string) ([]byte, error) {
	client, err := km.buildShimClient(sandboxID, namespace, timeoutInSeconds)
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

	body, err := ioutil.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	return body, nil
}
