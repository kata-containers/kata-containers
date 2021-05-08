// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"fmt"
	"net/http"
	"os"
	"sync"

	"github.com/containerd/containerd/defaults"
	srvconfig "github.com/containerd/containerd/services/server/config"
	"github.com/sirupsen/logrus"

	// register grpc event types
	_ "github.com/containerd/containerd/api/events"
)

var monitorLog = logrus.WithField("source", "kata-monitor")

// SetLogger sets the logger for katamonitor package.
func SetLogger(logger *logrus.Entry) {
	fields := monitorLog.Data
	monitorLog = logger.WithFields(fields)
}

// KataMonitor is monitor agent
type KataMonitor struct {
	containerdAddr       string
	containerdConfigFile string
	containerdStatePath  string
	sandboxCache         *sandboxCache
}

// NewKataMonitor create and return a new KataMonitor instance
func NewKataMonitor(containerdAddr, containerdConfigFile string) (*KataMonitor, error) {
	if containerdAddr == "" {
		return nil, fmt.Errorf("containerd serve address missing")
	}

	containerdConf := &srvconfig.Config{
		State: defaults.DefaultStateDir,
	}

	if err := srvconfig.LoadConfig(containerdConfigFile, containerdConf); err != nil && !os.IsNotExist(err) {
		return nil, err
	}

	km := &KataMonitor{
		containerdAddr:       containerdAddr,
		containerdConfigFile: containerdConfigFile,
		containerdStatePath:  containerdConf.State,
		sandboxCache: &sandboxCache{
			Mutex:     &sync.Mutex{},
			sandboxes: make(map[string]string),
		},
	}

	if err := km.initSandboxCache(); err != nil {
		return nil, err
	}

	// register metrics
	registerMetrics()

	go km.sandboxCache.startEventsListener(km.containerdAddr)

	return km, nil
}

func (km *KataMonitor) initSandboxCache() error {
	sandboxes, err := km.getSandboxes()
	if err != nil {
		return err
	}
	km.sandboxCache.init(sandboxes)
	return nil
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
	sandboxes := km.getSandboxList()
	for _, s := range sandboxes {
		w.Write([]byte(fmt.Sprintf("%s\n", s)))
	}
}

func (km *KataMonitor) getSandboxList() []string {
	sn := km.sandboxCache.getAllSandboxes()
	result := make([]string, len(sn))

	i := 0
	for k := range sn {
		result[i] = k
		i++
	}
	return result
}

func (km *KataMonitor) getSandboxNamespace(sandbox string) (string, error) {
	return km.sandboxCache.getSandboxNamespace(sandbox)
}
