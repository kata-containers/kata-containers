// A simple proxy that multiplexes a unix socket connection
//
// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"flag"
	"io"
	"net"
	"net/url"
	"sync"

	"github.com/hashicorp/yamux"
	"github.com/sirupsen/logrus"
)

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

func setupLoger(logLevel string) error {

	level, err := logrus.ParseLevel(logLevel)
	if err != nil {
		return err
	}

	logrus.SetLevel(level)
	return nil
}

func main() {
	var channel, proxyAddr, logLevel string
	flag.StringVar(&channel, "mux-socket", "", "unix socket to multiplex on")
	flag.StringVar(&proxyAddr, "listen-socket", "", "unix socket to listen on")

	flag.StringVar(&logLevel, "log", "warn",
		"log messages above specified level: debug, warn, error, fatal or panic")

	flag.Parse()

	err := setupLoger(logLevel)
	if err != nil {
		logrus.Fatal(err)
	}

	muxAddr, err := unixAddr(channel)
	if err != nil {
		logrus.Fatal("invalid mux socket address")
	}
	listenAddr, err := unixAddr(proxyAddr)
	if err != nil {
		logrus.Fatal("invalid listen socket address")
		return
	}

	// yamux connection
	servConn, err := net.Dial("unix", muxAddr)
	if err != nil {
		logrus.Fatalf("failed to dial channel(%q): %s", muxAddr, err)
		return
	}
	defer servConn.Close()

	results := make(chan error)
	err = serve(servConn, "unix", listenAddr, results)
	if err != nil {
		logrus.Fatal(err)
	}

	for err = range results {
		if err != nil {
			logrus.Fatal(err)
		}
	}
}
