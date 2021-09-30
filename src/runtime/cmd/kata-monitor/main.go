// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"flag"
	"fmt"
	"net/http"
	"os"
	goruntime "runtime"
	"text/template"
	"time"

	kataMonitor "github.com/kata-containers/kata-containers/src/runtime/pkg/kata-monitor"
	"github.com/sirupsen/logrus"
)

var monitorListenAddr = flag.String("listen-address", ":8090", "The address to listen on for HTTP requests.")
var runtimeEndpoint = flag.String("runtime-endpoint", "/run/containerd/containerd.sock", `Endpoint of CRI container runtime service. (default: "/run/containerd/containerd.sock")`)
var logLevel = flag.String("log-level", "info", "Log level of logrus(trace/debug/info/warn/error/fatal/panic).")

// These values are overridden via ldflags
var (
	appName = "kata-monitor"
	// version is the kata monitor version.
	version = "0.2.0"

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

type endpoint struct {
	handler http.HandlerFunc
	path    string
	desc    string
}

// global variable endpoints contains all available endpoints
var endpoints []endpoint

func main() {
	ver := versionInfo{
		AppName:   appName,
		Version:   version,
		GoVersion: goruntime.Version(),
		Os:        goruntime.GOOS,
		Arch:      goruntime.GOARCH,
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
		"listen-address":   *monitorListenAddr,
		"runtime-endpoint": *runtimeEndpoint,
		"log-level":        *logLevel,
	}

	logrus.WithFields(announceFields).Info("announce")

	// create new kataMonitor
	km, err := kataMonitor.NewKataMonitor(*runtimeEndpoint)
	if err != nil {
		panic(err)
	}

	// setup handlers, currently only metrics are supported
	m := http.NewServeMux()
	endpoints = []endpoint{
		{
			path:    "/metrics",
			desc:    "Get metrics from sandboxes.",
			handler: km.ProcessMetricsRequest,
		},
		{
			path:    "/sandboxes",
			desc:    "List all Kata Containers sandboxes.",
			handler: km.ListSandboxes,
		},
		{
			path:    "/agent-url",
			desc:    "Get sandbox agent URL.",
			handler: km.GetAgentURL,
		},
		{
			path:    "/debug/vars",
			desc:    "Golang pprof `/debug/vars` endpoint for kata runtime shim process.",
			handler: km.ExpvarHandler,
		},
		{
			path:    "/debug/pprof/",
			desc:    "Golang pprof `/debug/pprof/` endpoint for kata runtime shim process.",
			handler: km.PprofIndex,
		},
		{
			path:    "/debug/pprof/cmdline",
			desc:    "Golang pprof `/debug/pprof/cmdline` endpoint for kata runtime shim process.",
			handler: km.PprofCmdline,
		},
		{
			path:    "/debug/pprof/profile",
			desc:    "Golang pprof `/debug/pprof/profile` endpoint for kata runtime shim process.",
			handler: km.PprofProfile,
		},
		{
			path:    "/debug/pprof/symbol",
			desc:    "Golang pprof `/debug/pprof/symbol` endpoint for kata runtime shim process.",
			handler: km.PprofSymbol,
		},
		{
			path:    "/debug/pprof/trace",
			desc:    "Golang pprof `/debug/pprof/trace` endpoint for kata runtime shim process.",
			handler: km.PprofTrace,
		},
	}

	for _, endpoint := range endpoints {
		m.Handle(endpoint.path, endpoint.handler)
	}

	// root index page to show all endpoints in kata-monitor
	m.Handle("/", http.HandlerFunc(indexPage))

	// listening on the server
	svr := &http.Server{
		Handler: m,
		Addr:    *monitorListenAddr,
	}
	logrus.Fatal(svr.ListenAndServe())
}

func indexPage(w http.ResponseWriter, r *http.Request) {
	w.Write([]byte("Available HTTP endpoints:\n"))

	spacing := 0
	for _, endpoint := range endpoints {
		if len(endpoint.path) > spacing {
			spacing = len(endpoint.path)
		}
	}
	spacing = spacing + 3

	formattedString := fmt.Sprintf("%%-%ds: %%s\n", spacing)
	for _, endpoint := range endpoints {
		w.Write([]byte(fmt.Sprintf(formattedString, endpoint.path, endpoint.desc)))
	}
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
