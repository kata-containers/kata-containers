// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"net/http"
	"os"
	"time"

	kataMonitor "github.com/kata-containers/kata-containers/src/runtime/pkg/kata-monitor"
	"github.com/sirupsen/logrus"
)

var metricListenAddr = flag.String("listen-address", ":8090", "The address to listen on for HTTP requests.")
var containerdAddr = flag.String("containerd-address", "/run/containerd/containerd.sock", "Containerd address to accept client requests.")
var containerdConfig = flag.String("containerd-conf", "/etc/containerd/config.toml", "Containerd config file.")
var logLevel = flag.String("log-level", "info", "Log level of logrus(trace/debug/info/warn/error/fatal/panic).")

func main() {
	flag.Parse()

	// init logrus
	initLog()

	// create new MAgent
	ma, err := kataMonitor.NewKataMonitor(*containerdAddr, *containerdConfig)
	if err != nil {
		panic(err)
	}

	// setup handlers, now only metrics is supported
	http.HandleFunc("/metrics", ma.ProcessMetricsRequest)

	// listening on the server
	logrus.Fatal(http.ListenAndServe(*metricListenAddr, nil))
}

// initLog setup logger
func initLog() {
	kataMonitorLog := logrus.WithFields(logrus.Fields{
		"name": "kata-monitor",
		"pid":  os.Getpid(),
	})

	// set log level, default to warn
	level, err := logrus.ParseLevel(*logLevel)
	if err != nil {
		level = logrus.WarnLevel
	}

	kataMonitorLog.Logger.SetLevel(level)
	kataMonitorLog.Logger.Formatter = &logrus.TextFormatter{TimestampFormat: time.RFC3339Nano}

	kataMonitor.SetLogger(kataMonitorLog)
}
