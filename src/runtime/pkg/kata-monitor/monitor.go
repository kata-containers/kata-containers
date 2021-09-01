// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"errors"
	"fmt"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/sirupsen/logrus"
)

var monitorLog = logrus.WithField("source", "kata-monitor")

const (
	RuntimeContainerd          = "containerd"
	RuntimeCRIO                = "cri-o"
	podCacheRefreshTimeSeconds = 15
)

// SetLogger sets the logger for katamonitor package.
func SetLogger(logger *logrus.Entry) {
	fields := monitorLog.Data
	monitorLog = logger.WithFields(fields)
}

// KataMonitor is monitor agent
type KataMonitor struct {
	sandboxCache    *sandboxCache
	runtimeEndpoint string
}

// NewKataMonitor create and return a new KataMonitor instance
func NewKataMonitor(runtimeEndpoint string) (*KataMonitor, error) {
	if runtimeEndpoint == "" {
		return nil, errors.New("runtime endpoint missing")
	}

	if !strings.HasPrefix(runtimeEndpoint, "unix") {
		runtimeEndpoint = "unix://" + runtimeEndpoint
	}

	km := &KataMonitor{
		runtimeEndpoint: runtimeEndpoint,
		sandboxCache: &sandboxCache{
			Mutex:     &sync.Mutex{},
			sandboxes: make(map[string]bool),
		},
	}

	// register metrics
	registerMetrics()

	go km.startPodCacheUpdater()

	return km, nil
}

// startPodCacheUpdater will boot a thread to manage sandbox cache
func (km *KataMonitor) startPodCacheUpdater() {
	for {
		time.Sleep(podCacheRefreshTimeSeconds * time.Second)
		sandboxes, err := km.getSandboxes(km.sandboxCache.getAllSandboxes())
		if err != nil {
			monitorLog.WithError(err).Error("failed to get sandboxes")
			continue
		}
		monitorLog.WithField("count", len(sandboxes)).Debug("update sandboxes list")
		km.sandboxCache.set(sandboxes)
	}
}

// GetAgentURL returns agent URL
func (km *KataMonitor) GetAgentURL(w http.ResponseWriter, r *http.Request) {
	sandboxID, err := getSandboxIDFromReq(r)
	if err != nil {
		commonServeError(w, http.StatusBadRequest, err)
		return
	}

	data, err := doGet(sandboxID, defaultTimeout, "agent-url")
	if err != nil {
		commonServeError(w, http.StatusBadRequest, err)
		return
	}

	fmt.Fprintln(w, string(data))
}

// ListSandboxes list all sandboxes running in Kata
func (km *KataMonitor) ListSandboxes(w http.ResponseWriter, r *http.Request) {
	sandboxes := km.sandboxCache.getKataSandboxes()
	for _, s := range sandboxes {
		w.Write([]byte(fmt.Sprintf("%s\n", s)))
	}
}
