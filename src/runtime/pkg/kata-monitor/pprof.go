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
	"regexp"
	"strings"

	cdshim "github.com/containerd/containerd/runtime/v2/shim"

	shim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
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

	return shim.ClientSocketAddress(sandbox)
}

func (km *KataMonitor) proxyRequest(w http.ResponseWriter, r *http.Request,
	proxyResponse func(req *http.Request, w io.Writer, r io.Reader) error) {

	if proxyResponse == nil {
		proxyResponse = copyResponse
	}

	w.Header().Set("X-Content-Type-Options", "nosniff")

	socketAddress, err := km.composeSocketAddress(r)
	if err != nil {
		monitorLog.WithError(err).Error("failed to get shim monitor address")
		serveError(w, http.StatusBadRequest, "sandbox may be stopped or deleted")
		return
	}

	transport := &http.Transport{
		DisableKeepAlives: true,
		Dial: func(proto, addr string) (conn net.Conn, err error) {
			return cdshim.AnonDialer(socketAddress, defaultTimeout)
		},
	}

	client := http.Client{
		Transport: transport,
	}

	uri := fmt.Sprintf("http://shim%s", r.URL.String())
	monitorLog.Debugf("proxyRequest to: %s, uri: %s", socketAddress, uri)
	resp, err := client.Get(uri)
	if err != nil {
		serveError(w, http.StatusInternalServerError, fmt.Sprintf("failed to request %s through %s", uri, socketAddress))
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

	err = proxyResponse(r, w, output)
	if err != nil {
		monitorLog.WithError(err).Errorf("failed proxying %s from %s", uri, socketAddress)
		serveError(w, http.StatusInternalServerError, "error retrieving resource")
	}
}

// ExpvarHandler handles other `/debug/vars` requests
func (km *KataMonitor) ExpvarHandler(w http.ResponseWriter, r *http.Request) {
	km.proxyRequest(w, r, nil)
}

// PprofIndex handles other `/debug/pprof/` requests
func (km *KataMonitor) PprofIndex(w http.ResponseWriter, r *http.Request) {
	if len(strings.TrimPrefix(r.URL.Path, "/debug/pprof/")) == 0 {
		km.proxyRequest(w, r, copyResponseAddingSandboxIdToHref)
	} else {
		km.proxyRequest(w, r, nil)
	}
}

// PprofCmdline handles other `/debug/cmdline` requests
func (km *KataMonitor) PprofCmdline(w http.ResponseWriter, r *http.Request) {
	km.proxyRequest(w, r, nil)
}

// PprofProfile handles other `/debug/profile` requests
func (km *KataMonitor) PprofProfile(w http.ResponseWriter, r *http.Request) {
	km.proxyRequest(w, r, nil)
}

// PprofSymbol handles other `/debug/symbol` requests
func (km *KataMonitor) PprofSymbol(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	km.proxyRequest(w, r, nil)
}

// PprofTrace handles other `/debug/trace` requests
func (km *KataMonitor) PprofTrace(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/octet-stream")
	w.Header().Set("Content-Disposition", `attachment; filename="trace"`)
	km.proxyRequest(w, r, nil)
}

func copyResponse(req *http.Request, w io.Writer, r io.Reader) error {
	_, err := io.Copy(w, r)
	return err
}

func copyResponseAddingSandboxIdToHref(req *http.Request, w io.Writer, r io.Reader) error {
	sb, err := getSandboxIDFromReq(req)
	if err != nil {
		monitorLog.WithError(err).Warning("missing sandbox query in pprof url")
		return copyResponse(req, w, r)
	}
	buf, err := io.ReadAll(r)
	if err != nil {
		return err
	}

	re := regexp.MustCompile(`<a href=(['"])(\w+)\?(\w+=\w+)['"]>`)
	outHtml := re.ReplaceAllString(string(buf), fmt.Sprintf("<a href=$1$2?sandbox=%s&$3$1>", sb))
	w.Write([]byte(outHtml))
	return nil
}
