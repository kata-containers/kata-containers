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
	"sync"

	"github.com/hashicorp/yamux"
	"github.com/sirupsen/logrus"
	lSyslog "github.com/sirupsen/logrus/hooks/syslog"
)

const proxyName = "kata-proxy"

// version is the proxy version. This variable is populated at build time.
var version = "unknown"

var proxyLog = logrus.New()

func serve(servConn io.ReadWriteCloser, proto, addr string, results chan error) error {
	session, err := yamux.Client(servConn, nil)
	if err != nil {
		return err
	}

	// serving connection
	l, err := net.Listen(proto, addr)
	if err != nil {
		return err
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

	return nil
}

func proxyConn(conn1 net.Conn, conn2 net.Conn) {
	wg := &sync.WaitGroup{}
	once := &sync.Once{}
	cleanup := func() {
		conn1.Close()
		conn2.Close()
	}
	copyStream := func(dst io.Writer, src io.Reader) {
		_, err := io.Copy(dst, src)
		if err != nil {
			once.Do(cleanup)
		}
		wg.Done()
	}

	wg.Add(2)
	go copyStream(conn1, conn2)
	go copyStream(conn2, conn1)
	go func() {
		wg.Wait()
		once.Do(cleanup)
	}()
}

func unixAddr(uri string) (string, error) {
	if len(uri) == 0 {
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
		"name": proxyName,
		"pid":  os.Getpid(),
	})
}

func setupLogger(logLevel string) error {
	level, err := logrus.ParseLevel(logLevel)
	if err != nil {
		return err
	}

	proxyLog.SetLevel(level)

	hook, err := lSyslog.NewSyslogHook("", "", syslog.LOG_INFO, proxyName)
	if err == nil {
		proxyLog.AddHook(hook)
	}

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
		logger().WithField("socket-address", sock).Fatal("invalid agent logs socket address")
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

	go func() {
		scanner := bufio.NewScanner(conn)
		for scanner.Scan() {
			logger().Infof("%s\n", scanner.Text())
		}

		if err := scanner.Err(); err != nil {
			logger().Errorf("Failed reading agent logs from socket: %v", err)
		}
	}()

	return nil
}

func main() {
	var channel, proxyAddr, agentLogsSocket, logLevel string
	var showVersion bool

	flag.BoolVar(&showVersion, "version", false, "display program version and exit")
	flag.StringVar(&channel, "mux-socket", "", "unix socket to multiplex on")
	flag.StringVar(&proxyAddr, "listen-socket", "", "unix socket to listen on")
	flag.StringVar(&agentLogsSocket, "agent-logs-socket", "", "socket to listen on to retrieve agent logs")

	flag.StringVar(&logLevel, "log", "warn",
		"log messages above specified level: debug, warn, error, fatal or panic")

	flag.Parse()

	if showVersion {
		fmt.Printf("%v version %v\n", proxyName, version)
		os.Exit(0)
	}

	if err := setupLogger(logLevel); err != nil {
		logger().Fatal(err)
	}

	if err := printAgentLogs(agentLogsSocket); err != nil {
		logger().Fatal(err)
		return
	}

	muxAddr, err := unixAddr(channel)
	if err != nil {
		logger().Fatal("invalid mux socket address")
	}
	listenAddr, err := unixAddr(proxyAddr)
	if err != nil {
		logger().Fatal("invalid listen socket address")
		return
	}

	// yamux connection
	servConn, err := net.Dial("unix", muxAddr)
	if err != nil {
		logger().Fatalf("failed to dial channel(%q): %s", muxAddr, err)
		return
	}
	defer servConn.Close()

	results := make(chan error)
	err = serve(servConn, "unix", listenAddr, results)
	if err != nil {
		logger().Fatal(err)
	}

	for err = range results {
		if err != nil {
			logger().Fatal(err)
		}
	}
}
