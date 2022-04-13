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

const defaultListenAddress = "127.0.0.1:8090"

var monitorListenAddr = flag.String("listen-address", defaultListenAddress, "The address to listen on for HTTP requests.")
var runtimeEndpoint = flag.String("runtime-endpoint", "/run/containerd/containerd.sock", "Endpoint of CRI container runtime service.")
var logLevel = flag.String("log-level", "info", "Log level of logrus(trace/debug/info/warn/error/fatal/panic).")

// These values are overridden via ldflags
var (
	appName = "kata-monitor"
	// version is the kata monitor version.
	version = "0.3.0"

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
	htmlResponse := kataMonitor.IfReturnHTMLResponse(w, r)
	if htmlResponse {
		indexPageHTML(w, r)
	} else {
		indexPageText(w, r)
	}
}

func indexPageText(w http.ResponseWriter, r *http.Request) {
	w.Write([]byte("Available HTTP endpoints:\n"))

	spacing := 0
	for _, endpoint := range endpoints {
		if len(endpoint.path) > spacing {
			spacing = len(endpoint.path)
		}
	}
	spacing = spacing + 3
	formatter := fmt.Sprintf("%%-%ds: %%s\n", spacing)

	for _, endpoint := range endpoints {
		w.Write([]byte(fmt.Sprintf(formatter, endpoint.path, endpoint.desc)))
	}
}

func indexPageHTML(w http.ResponseWriter, r *http.Request) {

	w.Write([]byte("<h1>Available HTTP endpoints:</h1>\n"))

	var formattedString string
	needLinkPaths := []string{"/metrics", "/sandboxes"}

	w.Write([]byte("<ul>"))
	for _, endpoint := range endpoints {
		formattedString = fmt.Sprintf("<b>%s</b>: %s\n", endpoint.path, endpoint.desc)
		for _, linkPath := range needLinkPaths {
			if linkPath == endpoint.path {
				formattedString = fmt.Sprintf("<b><a href='%s'>%s</a></b>: %s\n", endpoint.path, endpoint.path, endpoint.desc)
				break
			}
		}
		formattedString = fmt.Sprintf("<li>%s</li>", formattedString)
		w.Write([]byte(formattedString))
	}
	w.Write([]byte("</ul>"))
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
