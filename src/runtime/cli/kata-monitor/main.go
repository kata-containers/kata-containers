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

var monitorListenAddr = flag.String("listen-address", ":8090", "The address to listen on for HTTP requests.")
var containerdAddr = flag.String("containerd-address", "/run/containerd/containerd.sock", "Containerd address to accept client requests.")
var containerdConfig = flag.String("containerd-conf", "/etc/containerd/config.toml", "Containerd config file.")
var logLevel = flag.String("log-level", "info", "Log level of logrus(trace/debug/info/warn/error/fatal/panic).")

func main() {
	flag.Parse()

	// init logrus
	initLog()

	// create new kataMonitor
	km, err := kataMonitor.NewKataMonitor(*containerdAddr, *containerdConfig)
	if err != nil {
		panic(err)
	}

	// setup handlers, now only metrics is supported
	m := http.NewServeMux()
	m.Handle("/metrics", http.HandlerFunc(km.ProcessMetricsRequest))
	m.Handle("/sandboxes", http.HandlerFunc(km.ListSandboxes))
	m.Handle("/agent-url", http.HandlerFunc(km.GetAgentURL))

	// for debug shim process
	m.Handle("/debug/vars", http.HandlerFunc(km.ExpvarHandler))
	m.Handle("/debug/pprof/", http.HandlerFunc(km.PprofIndex))
	m.Handle("/debug/pprof/cmdline", http.HandlerFunc(km.PprofCmdline))
	m.Handle("/debug/pprof/profile", http.HandlerFunc(km.PprofProfile))
	m.Handle("/debug/pprof/symbol", http.HandlerFunc(km.PprofSymbol))
	m.Handle("/debug/pprof/trace", http.HandlerFunc(km.PprofTrace))

	// listening on the server
	svr := &http.Server{
		Handler: m,
		Addr:    *monitorListenAddr,
	}
	logrus.Fatal(svr.ListenAndServe())
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
