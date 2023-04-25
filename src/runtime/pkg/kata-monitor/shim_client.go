// Copyright (c) 2020-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"fmt"
	"net/http"
	"strings"
	"time"

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

func getFilterFamilyFromReq(r *http.Request) ([]string, error) {
	filterFamilies := r.URL.Query().Get("filter_family")
	if filterFamilies != "" {
		return strings.Split(filterFamilies, ","), nil
	}
	return nil, nil
}
