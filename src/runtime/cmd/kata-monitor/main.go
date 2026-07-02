// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"crypto/tls"
	"flag"
	"fmt"
	"net/http"
	"os"
	goruntime "runtime"
	"strings"
	"text/template"
	"time"

	kataMonitor "github.com/kata-containers/kata-containers/src/runtime/pkg/kata-monitor"
	"github.com/sirupsen/logrus"
)

const defaultListenAddress = "127.0.0.1:8090"

var monitorListenAddr = flag.String("listen-address", defaultListenAddress, "The address to listen on for HTTP requests.")
var runtimeEndpoint = flag.String("runtime-endpoint", "/run/containerd/containerd.sock", "Endpoint of CRI container runtime service.")
var logLevel = flag.String("log-level", "info", "Log level of logrus(trace/debug/info/warn/error/fatal/panic).")
var tlsCertFile = flag.String("tls-cert-file", "", "Path to TLS certificate file. Enables TLS when set (requires --tls-key-file).")
var tlsKeyFile = flag.String("tls-key-file", "", "Path to TLS private key file. Enables TLS when set (requires --tls-cert-file).")
var tlsMinVersion = flag.String("tls-min-version", "", "Minimum TLS version (VersionTLS12, VersionTLS13). Defaults to VersionTLS12 when TLS is enabled.")
var tlsCipherSuites = flag.String("tls-cipher-suites", "", "Comma-separated list of TLS cipher suite names (IANA format). Applies to TLS 1.2 and below.")

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

	if (*tlsCertFile == "") != (*tlsKeyFile == "") {
		logrus.Fatal("--tls-cert-file and --tls-key-file must be set together")
	}

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
		"listen-address":    *monitorListenAddr,
		"runtime-endpoint":  *runtimeEndpoint,
		"log-level":         *logLevel,
		"tls-cert-file":     *tlsCertFile,
		"tls-min-version":   *tlsMinVersion,
		"tls-cipher-suites": *tlsCipherSuites,
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

	if *tlsCertFile != "" {
		tlsConfig, err := buildTLSConfig(*tlsMinVersion, *tlsCipherSuites)
		if err != nil {
			logrus.WithError(err).Fatal("failed to build TLS config")
		}
		svr.TLSConfig = tlsConfig
		logrus.Fatal(svr.ListenAndServeTLS(*tlsCertFile, *tlsKeyFile))
	} else {
		logrus.Fatal(svr.ListenAndServe())
	}
}

// buildTLSConfig constructs a tls.Config from the given version string and
// comma-separated IANA cipher suite names. Both arguments are optional; when
// empty the function returns a config with secure defaults (TLS 1.2 minimum).
func buildTLSConfig(minVersion, cipherSuites string) (*tls.Config, error) {
	cfg := &tls.Config{
		MinVersion: tls.VersionTLS12,
	}

	if minVersion != "" {
		v, err := parseTLSVersion(minVersion)
		if err != nil {
			return nil, err
		}
		cfg.MinVersion = v
	}

	if cipherSuites != "" {
		ids, err := parseCipherSuites(cipherSuites)
		if err != nil {
			return nil, err
		}
		cfg.CipherSuites = ids
	}

	return cfg, nil
}

func parseTLSVersion(s string) (uint16, error) {
	switch s {
	case "VersionTLS12":
		return tls.VersionTLS12, nil
	case "VersionTLS13":
		return tls.VersionTLS13, nil
	default:
		return 0, fmt.Errorf("unsupported TLS version %q (supported: VersionTLS12, VersionTLS13)", s)
	}
}

func parseCipherSuites(s string) ([]uint16, error) {
	known := make(map[string]uint16)
	for _, cs := range tls.CipherSuites() {
		known[cs.Name] = cs.ID
	}

	// Build a set of insecure names for a better error message.
	insecure := make(map[string]struct{})
	for _, cs := range tls.InsecureCipherSuites() {
		insecure[cs.Name] = struct{}{}
	}

	names := strings.Split(s, ",")
	ids := make([]uint16, 0, len(names))
	for _, name := range names {
		name = strings.TrimSpace(name)
		if name == "" {
			continue
		}
		if _, bad := insecure[name]; bad {
			return nil, fmt.Errorf("cipher suite %q is insecure and not allowed", name)
		}
		id, ok := known[name]
		if !ok {
			return nil, fmt.Errorf("unknown cipher suite %q", name)
		}
		ids = append(ids, id)
	}
	return ids, nil
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
		fmt.Fprintf(w, formatter, endpoint.path, endpoint.desc)
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
