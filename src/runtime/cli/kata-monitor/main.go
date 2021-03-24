// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"net/http"
	"os"
	"runtime"
	"text/template"
	"time"

	kataMonitor "github.com/kata-containers/kata-containers/src/runtime/pkg/kata-monitor"
	"github.com/sirupsen/logrus"
)

var monitorListenAddr = flag.String("listen-address", ":8090", "The address to listen on for HTTP requests.")
var containerdAddr = flag.String("containerd-address", "/run/containerd/containerd.sock", "Containerd address to accept client requests.")
var containerdConfig = flag.String("containerd-conf", "/etc/containerd/config.toml", "Containerd config file.")
var logLevel = flag.String("log-level", "info", "Log level of logrus(trace/debug/info/warn/error/fatal/panic).")

// These values are overridden via ldflags
var (
	appName = "kata-monitor"
	// version is the kata monitor version.
	version = "0.1.0"

	GitCommit = "unknown-commit"
)

type versionInfo struct {
	AppName   string
	Version   string
	GitCommit string
	GoVersion string
	Os        string
	Arch      string
}

var versionTemplate = `{{.AppName}}
 Version:	{{.Version}}
 Go version:	{{.GoVersion}}
 Git commit:	{{.GitCommit}}
 OS/Arch:	{{.Os}}/{{.Arch}}
`

func printVersion(ver versionInfo) {
	t, _ := template.New("version").Parse(versionTemplate)

	if err := t.Execute(os.Stdout, ver); err != nil {
		panic(err)
	}
}

func main() {
	ver := versionInfo{
		AppName:   appName,
		Version:   version,
		GoVersion: runtime.Version(),
		Os:        runtime.GOOS,
		Arch:      runtime.GOARCH,
		GitCommit: GitCommit,
	}

	if len(os.Args) == 2 && (os.Args[1] == "--version" || os.Args[1] == "version") {
		printVersion(ver)
		return
	}

	flag.Parse()

	// init logrus
	initLog()

	announceFields := logrus.Fields{
		// properties from version info
		"app":        ver.AppName,
		"version":    ver.Version,
		"go-version": ver.GoVersion,
		"os":         ver.Os,
		"arch":       ver.Arch,
		"git-commit": ver.GitCommit,

		// properties from command-line options
		"listen-address":     *monitorListenAddr,
		"containerd-address": *containerdAddr,
		"containerd-conf":    *containerdConfig,
		"log-level":          *logLevel,
	}

	logrus.WithFields(announceFields).Info("announce")

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
