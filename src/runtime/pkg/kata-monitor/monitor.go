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
	// Initial sync with the kata sandboxes already running
	sbsFile, err := os.Open(getSandboxFS())
	if err != nil {
		monitorLog.WithError(err).Fatal("cannot open sandboxes fs")
		os.Exit(1)
	}
	sandboxList, err := sbsFile.Readdirnames(0)
	if err != nil {
		monitorLog.WithError(err).Fatal("cannot read sandboxes fs")
		os.Exit(1)
	}
	for _, sandbox := range sandboxList {
		km.sandboxCache.putIfNotExists(sandbox, sandboxCRIMetadata{})
	}

	monitorLog.Debug("initial sync of sbs directory completed")
	monitorLog.Tracef("pod list from sbs: %v", sandboxList)

	// We try to get CRI (kubernetes) metadata from the container manager for each new kata sandbox we detect.
	// It may take a while for data to be available, so we always wait podCacheRefreshDelaySeconds before checking.
	cacheUpdateTimer := time.NewTimer(podCacheRefreshDelaySeconds * time.Second)
	cacheUpdateTimerIsSet := true
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
				if !km.sandboxCache.putIfNotExists(id, sandboxCRIMetadata{}) {
					monitorLog.WithField("pod", id).Warn(
						"CREATE event but pod already present in the sandbox cache")
				}
				sandboxList = append(sandboxList, id)
				monitorLog.WithField("pod", id).Info("sandbox cache: added pod")
				if !cacheUpdateTimerIsSet {
					cacheUpdateTimer.Reset(podCacheRefreshDelaySeconds * time.Second)
					cacheUpdateTimerIsSet = true
					monitorLog.Debugf(
						"cache update timer fires in %d secs", podCacheRefreshDelaySeconds)
				}

			case fsnotify.Remove:
				splitPath := strings.Split(event.Name, string(os.PathSeparator))
				id := splitPath[len(splitPath)-1]
				if !km.sandboxCache.deleteIfExists(id) {
					monitorLog.WithField("pod", id).Warn(
						"REMOVE event but pod was missing from the sandbox cache")
				}
				sandboxList = removeFromSandboxList(sandboxList, id)
				monitorLog.WithField("pod", id).Info("sandbox cache: removed pod")
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

	data, err := shimclient.DoGet(sandboxID, defaultTimeout, containerdshim.AgentUrl)
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
		w.Write([]byte(fmt.Sprintf("%s\n", s)))
	}
}
func listSandboxesHtml(sandboxes []string, w http.ResponseWriter) {
	w.Write([]byte("<h1>Sandbox list</h1>\n"))
	w.Write([]byte("<ul>\n"))
	for _, s := range sandboxes {
		w.Write([]byte(fmt.Sprintf("<li>%s: <a href='/debug/pprof/?sandbox=%s'>pprof</a>, <a href='/metrics?sandbox=%s'>metrics</a>, <a href='/agent-url?sandbox=%s'>agent-url</a></li>\n", s, s, s, s)))
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
