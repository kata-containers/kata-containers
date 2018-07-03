// A simple proxy that multiplexes a unix socket connection
//
// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bufio"
	"errors"
	"flag"
	"fmt"
	"io"
	"log/syslog"
	"net"
	"net/url"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"github.com/hashicorp/yamux"
	"github.com/sirupsen/logrus"
	lSyslog "github.com/sirupsen/logrus/hooks/syslog"
)

const (
	proxyName  = "kata-proxy"
	termSignal = syscall.SIGTERM
)

var debug = false

// version is the proxy version. This variable is populated at build time.
var version = "unknown"

// if true, coredump when an internal error occurs or a fatal signal is received
var crashOnError = false

var proxyLog = logrus.New()

func serve(servConn io.ReadWriteCloser, proto, addr string, results chan error) (net.Listener, error) {
	sessionConfig := yamux.DefaultConfig()
	// Disable keepAlive since we don't know how much time a container can be paused
	sessionConfig.EnableKeepAlive = false
	session, err := yamux.Client(servConn, sessionConfig)
	if err != nil {
		return nil, err
	}

	// serving connection
	l, err := net.Listen(proto, addr)
	if err != nil {
		return nil, err
	}

	go func() {
		var err error
		defer func() {
			l.Close()
			results <- err
		}()

		for {
			var conn, stream net.Conn
			conn, err = l.Accept()
			if err != nil {
				return
			}

			stream, err = session.Open()
			if err != nil {
				return
			}

			go proxyConn(conn, stream)
		}
	}()

	return l, nil
}

func proxyConn(conn1 net.Conn, conn2 net.Conn) {
	once := &sync.Once{}
	cleanup := func() {
		conn1.Close()
		conn2.Close()
	}
	copyStream := func(dst io.Writer, src io.Reader) {
		_, err := io.Copy(dst, src)
		if err != nil {
			logger().Debug("Copy stream error: %v", err)
		}

		once.Do(cleanup)
	}

	go copyStream(conn1, conn2)
	go copyStream(conn2, conn1)
}

func unixAddr(uri string) (string, error) {
	if uri == "" {
		return "", errors.New("empty uri")

	}
	addr, err := url.Parse(uri)
	if err != nil {
		return "", err
	}
	if addr.Scheme != "" && addr.Scheme != "unix" {
		return "", errors.New("invalid address scheme")
	}
	return addr.Host + addr.Path, nil
}

func logger() *logrus.Entry {
	return proxyLog.WithFields(logrus.Fields{
		"name":   proxyName,
		"pid":    os.Getpid(),
		"source": "proxy",
	})
}

func setupLogger(logLevel string) error {
	level, err := logrus.ParseLevel(logLevel)
	if err != nil {
		return err
	}

	proxyLog.SetLevel(level)

	proxyLog.Formatter = &logrus.TextFormatter{TimestampFormat: time.RFC3339Nano}

	hook, err := lSyslog.NewSyslogHook("", "", syslog.LOG_INFO|syslog.LOG_USER, proxyName)
	if err != nil {
		return err
	}

	proxyLog.AddHook(hook)

	logger().WithField("version", version).Info()

	return nil
}

func printAgentLogs(sock string) error {
	// Don't return an error if nothing has been provided.
	// This flag is optional.
	if sock == "" {
		return nil
	}

	agentLogsAddr, err := unixAddr(sock)
	if err != nil {
		logger().WithField("socket-address", sock).WithError(err).Fatal("invalid agent logs socket address")
		return err
	}

	// Check permissions socket for "other" is 0.
	fileInfo, err := os.Stat(agentLogsAddr)
	if err != nil {
		return err
	}

	otherMask := 0007
	other := int(fileInfo.Mode().Perm()) & otherMask
	if other != 0 {
		return fmt.Errorf("All socket permissions for 'other' should be disabled, got %3.3o", other)
	}

	conn, err := net.Dial("unix", agentLogsAddr)
	if err != nil {
		return err
	}

	// Allow log messages coming from the agent to be distinguished from
	// messages originating from the proxy itself.
	agentLogger := logger().WithFields(logrus.Fields{
		"source": "agent",
	})

	go func() {
		scanner := bufio.NewScanner(conn)
		for scanner.Scan() {
			agentLogger.Infof("%s\n", scanner.Text())
		}

		if err := scanner.Err(); err != nil {
			logger().WithError(err).Error("Failed reading agent logs from socket")
		}
	}()

	return nil
}

func setupNotifier() chan os.Signal {
	sigmax := 8
	sigCh := make(chan os.Signal, sigmax)
	ch := make(chan os.Signal, sigmax)

	signal.Notify(sigCh, termSignal)

	for _, sig := range handledSignals() {
		signal.Notify(sigCh, sig)
	}

	go func() {
		for {
			// Block waiting for a signal
			sig := <-sigCh

			nativeSignal, ok := sig.(syscall.Signal)
			if !ok {
				err := errors.New("unknown signal")
				logger().WithError(err).WithField("signal", sig.String()).Error()
				continue
			}

			if fatalSignal(nativeSignal) {
				logger().WithField("signal", sig).Error("received fatal signal")
				die()
			} else if nonFatalSignal(nativeSignal) {
				if debug {
					logger().WithField("signal", sig).Debug("handling signal")
					backtrace()
				}
			} else {
				// let the caller handle other signals
				ch <- sig
			}
		}
	}()

	return ch
}

// Blocking function waiting for a SIGTERM signal.
func handleExitSignal(sigCh chan os.Signal, vmConn *net.Conn, proxyListener *net.Listener) error {
	if sigCh == nil {
		return fmt.Errorf("Signal channel cannot be nil, it has to be initialized")
	}

	// Blocking here waiting for the signal to be received.
	sig := <-sigCh

	_, ok := sig.(syscall.Signal)
	if !ok {
		proxyLog.WithField("signal", sig).Error("unable to handle signal")
	}

	if sig != termSignal {
		return fmt.Errorf("Signal received should be %q, got %q instead", termSignal.String(), sig.String())
	}

	// Let's first close the connection with the shim/runtime so that we
	// don't get any more requests.
	if proxyListener != nil {
		if err := (*proxyListener).Close(); err != nil {
			return err
		}
		*proxyListener = nil
	}

	// Now let's close the connection with the VM/agent.
	if vmConn != nil {
		if err := (*vmConn).Close(); err != nil {
			return err
		}
		*vmConn = nil
	}

	return nil
}

func handleVersion(showVersion bool) {
	if showVersion {
		fmt.Printf("%v version %v\n", proxyName, version)
		os.Exit(0)
	}
}

func realMain() {
	var channel, proxyAddr, agentLogsSocket, logLevel string
	var showVersion bool

	flag.BoolVar(&debug, "debug", false, "enable debug mode")
	flag.BoolVar(&showVersion, "version", false, "display program version and exit")
	flag.StringVar(&channel, "mux-socket", "", "unix socket to multiplex on")
	flag.StringVar(&proxyAddr, "listen-socket", "", "unix socket to listen on")
	flag.StringVar(&agentLogsSocket, "agent-logs-socket", "", "socket to listen on to retrieve agent logs")

	flag.StringVar(&logLevel, "log", "warn",
		"log messages above specified level: debug, warn, error, fatal or panic")

	flag.Parse()

	handleVersion(showVersion)

	if logLevel == "debug" {
		debug = true
		crashOnError = true
	}

	if channel == "" || proxyAddr == "" {
		fmt.Printf("Option -mux-socket and -listen-socket required\n")
		os.Exit(0)
	}

	sigCh := setupNotifier()

	if err := setupLogger(logLevel); err != nil {
		logger().WithError(err).Fatal("unable to setup logger")
		os.Exit(1)
	}

	if err := printAgentLogs(agentLogsSocket); err != nil {
		logger().WithError(err).Fatal("failed to print agent logs")
		os.Exit(1)
	}

	muxAddr, err := unixAddr(channel)
	if err != nil {
		logger().WithError(err).Fatal("invalid mux socket address")
		os.Exit(1)
	}
	listenAddr, err := unixAddr(proxyAddr)
	if err != nil {
		logger().WithError(err).Fatal("invalid listen socket address")
		os.Exit(1)
	}

	// yamux connection
	servConn, err := net.Dial("unix", muxAddr)
	if err != nil {
		logger().WithError(err).WithField("channel", muxAddr).Fatal("failed to dial channel")
		os.Exit(1)
	}
	defer func() {
		if servConn != nil {
			servConn.Close()
		}
	}()

	results := make(chan error)
	l, err := serve(servConn, "unix", listenAddr, results)
	if err != nil {
		logger().WithError(err).Fatal("failed to serve")
		os.Exit(1)
	}
	defer func() {
		if l != nil {
			l.Close()
		}
	}()

	go func() {
		for err := range results {
			if err != nil {
				logger().WithError(err).Fatal("channel error")
			}
		}
	}()

	if err := handleExitSignal(sigCh, &servConn, &l); err != nil {
		logger().WithError(err).Fatal("failed to handle exit signal")
		os.Exit(1)
	}

	logger().Debug("shutting down")
}

func main() {
	defer handlePanic()
	realMain()
}
