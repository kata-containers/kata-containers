// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"fmt"
	"io"
	"net"
	"net/http"
)

func serveError(w http.ResponseWriter, status int, txt string) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	w.Header().Set("X-Go-Pprof", "1")
	w.Header().Del("Content-Disposition")
	w.WriteHeader(status)
	fmt.Fprintln(w, txt)
}

func (km *KataMonitor) composeSocketAddress(r *http.Request) (string, error) {
	sandbox, err := getSandboxIDFromReq(r)
	if err != nil {
		return "", err
	}

	namespace, err := km.getSandboxNamespace(sandbox)
	if err != nil {
		return "", err
	}

	return km.getMonitorAddress(sandbox, namespace)
}

func (km *KataMonitor) proxyRequest(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("X-Content-Type-Options", "nosniff")

	socket, err := km.composeSocketAddress(r)
	if err != nil {
		monitorLog.WithError(err).Error("failed to get shim monitor address")
		serveError(w, http.StatusBadRequest, "sandbox may be stopped or deleted")
		return
	}

	transport := &http.Transport{
		DisableKeepAlives: true,
		Dial: func(proto, addr string) (conn net.Conn, err error) {
			return net.Dial("unix", "\x00"+socket)
		},
	}

	client := http.Client{
		Transport: transport,
	}

	uri := fmt.Sprintf("http://shim%s", r.URL.String())
	resp, err := client.Get(uri)
	if err != nil {
		return
	}

	output := resp.Body
	defer output.Close()

	contentType := resp.Header.Get("Content-Type")
	if contentType != "" {
		w.Header().Set("Content-Type", contentType)
	}

	contentDisposition := resp.Header.Get("Content-Disposition")
	if contentDisposition != "" {
		w.Header().Set("Content-Disposition", contentDisposition)
	}

	io.Copy(w, output)
}

// ExpvarHandler handles other `/debug/vars` requests
func (km *KataMonitor) ExpvarHandler(w http.ResponseWriter, r *http.Request) {
	km.proxyRequest(w, r)
}

// PprofIndex handles other `/debug/pprof/` requests
func (km *KataMonitor) PprofIndex(w http.ResponseWriter, r *http.Request) {
	km.proxyRequest(w, r)
}

// PprofCmdline handles other `/debug/cmdline` requests
func (km *KataMonitor) PprofCmdline(w http.ResponseWriter, r *http.Request) {
	km.proxyRequest(w, r)
}

// PprofProfile handles other `/debug/profile` requests
func (km *KataMonitor) PprofProfile(w http.ResponseWriter, r *http.Request) {
	km.proxyRequest(w, r)
}

// PprofSymbol handles other `/debug/symbol` requests
func (km *KataMonitor) PprofSymbol(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	km.proxyRequest(w, r)
}

// PprofTrace handles other `/debug/trace` requests
func (km *KataMonitor) PprofTrace(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/octet-stream")
	w.Header().Set("Content-Disposition", `attachment; filename="trace"`)
	km.proxyRequest(w, r)
}
