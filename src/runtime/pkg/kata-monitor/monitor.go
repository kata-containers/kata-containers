// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"errors"
	"fmt"
	"net/http"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
	"github.com/sirupsen/logrus"
)

var monitorLog = logrus.WithField("source", "kata-monitor")

const (
	RuntimeContainerd           = "containerd"
	RuntimeCRIO                 = "cri-o"
	fsMonitorRetryDelaySeconds  = 60
	podCacheRefreshDelaySeconds = 5
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
	sbsWatcher, err := fsnotify.NewWatcher()
	if err != nil {
		monitorLog.WithError(err).Fatal("failed to setup sandbox events watcher")
		os.Exit(1)
	}
	defer sbsWatcher.Close()
	for {
		err = sbsWatcher.Add(getSandboxFS())
		if err != nil {
			// if there are no kata pods (yet), the kata /run/vc directory may not be there: retry later
			monitorLog.WithError(err).Warnf("cannot monitor %s, retry in %d sec.", getSandboxFS(), fsMonitorRetryDelaySeconds)
			time.Sleep(fsMonitorRetryDelaySeconds * time.Second)
			continue
		}
		monitorLog.Debugf("started fs monitoring @%s", getSandboxFS())
		break
	}
	// we refresh the pod cache once if we get multiple add/delete pod events in a short time (< podCacheRefreshDelaySeconds)
	cacheUpdateTimer := time.NewTimer(podCacheRefreshDelaySeconds * time.Second)
	cacheUpdateTimerWasSet := false
	for {
		select {
		case event, ok := <-sbsWatcher.Events:
			if !ok {
				monitorLog.WithError(err).Fatal("cannot watch sandboxes fs")
				os.Exit(1)
			}
			monitorLog.WithField("event", event).Debug("got sandbox event")
			switch event.Op {
			case fsnotify.Create:
				splitPath := strings.Split(event.Name, string(os.PathSeparator))
				id := splitPath[len(splitPath)-1]
				if !km.sandboxCache.putIfNotExists(id, true) {
					monitorLog.WithField("pod", id).Warn(
						"CREATE event but pod already present in the sandbox cache")
				}
				monitorLog.WithField("pod", id).Info("sandbox cache: added pod")

			case fsnotify.Remove:
				splitPath := strings.Split(event.Name, string(os.PathSeparator))
				id := splitPath[len(splitPath)-1]
				if !km.sandboxCache.deleteIfExists(id) {
					monitorLog.WithField("pod", id).Warn(
						"REMOVE event but pod was missing from the sandbox cache")
				}
				monitorLog.WithField("pod", id).Info("sandbox cache: removed pod")

			default:
				monitorLog.WithField("event", event).Warn("got unexpected fs event")
			}

			// While we process fs events directly to update the sandbox cache we need to sync with the
			// container engine to ensure we are on sync with it: we can get out of sync in environments
			// where kata workloads can be started by other processes than the container engine.
			cacheUpdateTimerWasSet = cacheUpdateTimer.Reset(podCacheRefreshDelaySeconds * time.Second)
			monitorLog.WithField("was reset", cacheUpdateTimerWasSet).Debugf(
				"cache update timer fires in %d secs", podCacheRefreshDelaySeconds)

		case <-cacheUpdateTimer.C:
			sandboxes, err := km.getSandboxes(km.sandboxCache.getAllSandboxes())
			if err != nil {
				monitorLog.WithError(err).Error("failed to get sandboxes")
				continue
			}
			monitorLog.WithField("count", len(sandboxes)).Info("synced sandbox cache with the container engine")
			monitorLog.WithField("sandboxes", sandboxes).Debug("dump sandbox cache")
			km.sandboxCache.set(sandboxes)
		}
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
