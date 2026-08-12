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
	"path/filepath"
	"strings"
	"sync"
	"time"

	containerdshim "github.com/kata-containers/kata-containers/src/runtime/pkg/containerd-shim-v2"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils/shimclient"

	"github.com/fsnotify/fsnotify"
	"github.com/sirupsen/logrus"
)

var monitorLog = logrus.WithField("source", "kata-monitor")

const (
	RuntimeContainerd           = "containerd"
	RuntimeCRIO                 = "cri-o"
	fsMonitorRetryDelaySeconds  = 60
	podCacheRefreshDelaySeconds = 5
	contentTypeHtml             = "text/html"
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
			sandboxes: make(map[string]sandboxCRIMetadata),
		},
	}

	// register metrics
	registerMetrics()

	go km.startPodCacheUpdater()

	return km, nil
}

func removeFromSandboxList(sandboxList []string, sandboxToRemove string) []string {
	for i, sandbox := range sandboxList {
		if sandbox == sandboxToRemove {
			return append(sandboxList[:i], sandboxList[i+1:]...)
		}
	}
	return sandboxList
}

// syncSandboxFSPath registers a watch on path (if not already watched) and
// returns sandbox IDs found during the initial directory sync.
func (km *KataMonitor) syncSandboxFSPath(watcher *fsnotify.Watcher, path string, watched map[string]struct{}) ([]string, error) {
	if _, ok := watched[path]; ok {
		return nil, nil
	}

	if err := watcher.Add(path); err != nil {
		return nil, err
	}

	entries, err := os.ReadDir(path)
	if err != nil {
		_ = watcher.Remove(path)
		return nil, err
	}

	watched[path] = struct{}{}
	monitorLog.Debugf("started fs monitoring @%s", path)

	var sandboxes []string
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		id := entry.Name()
		if km.sandboxCache.putIfNotExists(id, sandboxCRIMetadata{}) {
			sandboxes = append(sandboxes, id)
		}
	}
	monitorLog.WithField("path", path).WithField("sandboxes", sandboxes).Debug("initial sync of sandbox directory completed")
	return sandboxes, nil
}

// startPodCacheUpdater will boot a thread to manage sandbox cache
func (km *KataMonitor) startPodCacheUpdater() {
	sbsWatcher, err := fsnotify.NewWatcher()
	if err != nil {
		monitorLog.WithError(err).Fatal("failed to setup sandbox events watcher")
		os.Exit(1)
	}
	defer sbsWatcher.Close()

	sandboxFSPaths := getSandboxFSPaths()
	watchedPaths := make(map[string]struct{}, len(sandboxFSPaths))
	sandboxList := []string{}

	tryWatchAll := func() {
		for _, path := range sandboxFSPaths {
			added, syncErr := km.syncSandboxFSPath(sbsWatcher, path, watchedPaths)
			if syncErr != nil {
				// Path may not exist yet when no sandboxes of that runtime are present.
				monitorLog.WithError(syncErr).Warnf("cannot monitor %s, retry in %d sec.", path, fsMonitorRetryDelaySeconds)
				continue
			}
			sandboxList = append(sandboxList, added...)
		}
	}

	tryWatchAll()
	if len(watchedPaths) == 0 {
		monitorLog.Warnf("no sandbox storage paths available yet; will retry every %d sec.", fsMonitorRetryDelaySeconds)
	}

	// We try to get CRI (kubernetes) metadata from the container manager for each new kata sandbox we detect.
	// It may take a while for data to be available, so we always wait podCacheRefreshDelaySeconds before checking.
	cacheUpdateTimer := time.NewTimer(podCacheRefreshDelaySeconds * time.Second)
	cacheUpdateTimerIsSet := true
	watchRetryTicker := time.NewTicker(fsMonitorRetryDelaySeconds * time.Second)
	defer watchRetryTicker.Stop()

	for {
		select {
		case event, ok := <-sbsWatcher.Events:
			if !ok {
				monitorLog.Fatal("cannot watch sandboxes fs")
				os.Exit(1)
			}
			monitorLog.WithField("event", event).Debug("got sandbox event")
			switch {
			case event.Op&fsnotify.Create == fsnotify.Create:
				id := filepath.Base(event.Name)
				info, statErr := os.Stat(event.Name)
				if statErr != nil || !info.IsDir() {
					continue
				}
				if !km.sandboxCache.putIfNotExists(id, sandboxCRIMetadata{}) {
					monitorLog.WithField("pod", id).Warn(
						"CREATE event but pod already present in the sandbox cache")
					continue
				}
				sandboxList = append(sandboxList, id)
				monitorLog.WithField("pod", id).Info("sandbox cache: added pod")
				if !cacheUpdateTimerIsSet {
					cacheUpdateTimer.Reset(podCacheRefreshDelaySeconds * time.Second)
					cacheUpdateTimerIsSet = true
					monitorLog.Debugf(
						"cache update timer fires in %d secs", podCacheRefreshDelaySeconds)
				}

			case event.Op&fsnotify.Remove == fsnotify.Remove:
				// A watched root may disappear (e.g. last sandbox cleaned up the tree).
				// Drop it from watchedPaths so the retry ticker can re-attach later.
				if _, ok := watchedPaths[event.Name]; ok {
					delete(watchedPaths, event.Name)
					monitorLog.WithField("path", event.Name).Warn("sandbox storage path removed; will retry watch")
					continue
				}
				id := filepath.Base(event.Name)
				if !km.sandboxCache.deleteIfExists(id) {
					monitorLog.WithField("pod", id).Warn(
						"REMOVE event but pod was missing from the sandbox cache")
				}
				sandboxList = removeFromSandboxList(sandboxList, id)
				monitorLog.WithField("pod", id).Info("sandbox cache: removed pod")
			}

		case <-watchRetryTicker.C:
			if len(watchedPaths) < len(sandboxFSPaths) {
				tryWatchAll()
			}

		case <-cacheUpdateTimer.C:
			cacheUpdateTimerIsSet = false
			monitorLog.WithField("pod list", sandboxList).Debugf(
				"retrieve pods metadata from the container manager")
			sandboxList, err = km.syncSandboxes(sandboxList)
			if err != nil {
				monitorLog.WithError(err).Error("failed to get sandboxes metadata")
				continue
			}
			if len(sandboxList) > 0 {
				monitorLog.WithField("sandboxes", sandboxList).Debugf(
					"%d sandboxes still miss metadata", len(sandboxList))
				cacheUpdateTimer.Reset(podCacheRefreshDelaySeconds * time.Second)
				cacheUpdateTimerIsSet = true
			}

			monitorLog.WithField("sandboxes", km.sandboxCache.getSandboxList()).Trace("dump sandbox cache")
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

	data, err := shimclient.DoGet(sandboxID, defaultTimeout, containerdshim.AgentURL)
	if err != nil {
		commonServeError(w, http.StatusBadRequest, err)
		return
	}

	fmt.Fprintln(w, string(data))
}

// ListSandboxes list all sandboxes running in Kata
func (km *KataMonitor) ListSandboxes(w http.ResponseWriter, r *http.Request) {
	sandboxes := km.sandboxCache.getSandboxList()
	htmlResponse := IfReturnHTMLResponse(w, r)
	if htmlResponse {
		listSandboxesHtml(sandboxes, w)
	} else {
		listSandboxesText(sandboxes, w)
	}
}

func listSandboxesText(sandboxes []string, w http.ResponseWriter) {
	for _, s := range sandboxes {
		fmt.Fprintf(w, "%s\n", s)
	}
}
func listSandboxesHtml(sandboxes []string, w http.ResponseWriter) {
	w.Write([]byte("<h1>Sandbox list</h1>\n"))
	w.Write([]byte("<ul>\n"))
	for _, s := range sandboxes {
		fmt.Fprintf(w, "<li>%s: <a href='/debug/pprof/?sandbox=%s'>pprof</a>, <a href='/metrics?sandbox=%s'>metrics</a>, <a href='/agent-url?sandbox=%s'>agent-url</a></li>\n", s, s, s, s)
	}
	w.Write([]byte("</ul>\n"))
}

// IfReturnHTMLResponse returns true if request accepts html response
// NOTE: IfReturnHTMLResponse will also set response header to `text/html`
func IfReturnHTMLResponse(w http.ResponseWriter, r *http.Request) bool {
	accepts := r.Header["Accept"]
	for _, accept := range accepts {
		fields := strings.Split(accept, ",")
		for _, field := range fields {
			if field == contentTypeHtml {
				w.Header().Set("Content-Type", contentTypeHtml)
				return true
			}
		}
	}

	return false
}
