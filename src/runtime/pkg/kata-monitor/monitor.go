// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package katamonitor

import (
	"fmt"
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
		return nil, fmt.Errorf("Containerd serve address missing.")
	}

	containerdConf := &srvconfig.Config{
		State: defaults.DefaultStateDir,
	}

	if err := srvconfig.LoadConfig(containerdConfigFile, containerdConf); err != nil && !os.IsNotExist(err) {
		return nil, err
	}

	ka := &KataMonitor{
		containerdAddr:       containerdAddr,
		containerdConfigFile: containerdConfigFile,
		containerdStatePath:  containerdConf.State,
		sandboxCache: &sandboxCache{
			Mutex:     &sync.Mutex{},
			sandboxes: make(map[string]string),
		},
	}

	if err := ka.initSandboxCache(); err != nil {
		return nil, err
	}

	// register metrics
	registerMetrics()

	go ka.sandboxCache.startEventsListener(ka.containerdAddr)

	return ka, nil
}

func (ka *KataMonitor) initSandboxCache() error {
	sandboxes, err := ka.getSandboxes()
	if err != nil {
		return err
	}
	ka.sandboxCache.init(sandboxes)
	return nil
}
