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

	"github.com/golang/glog"
	"github.com/hashicorp/yamux"
)

func serve(servConn io.ReadWriteCloser, proto, addr string, results chan error) error {
	session, err := yamux.Client(servConn, nil)
	if err != nil {
		glog.Errorf("fail to create yamux client: %s", err)
		return err
	}

	// serving connection
	l, err := net.Listen(proto, addr)
	if err != nil {
		glog.Errorf("fail to listen on %s:%s: %s", proto, addr, err)
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
				glog.Errorf("fail to accept new connection: %s", err)
				return
			}

			stream, err = session.Open()
			if err != nil {
				glog.Errorf("fail to open yamux stream: %s", err)
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

func main() {
	var channel, proxyAddr string
	flag.StringVar(&channel, "mux-socket", "", "unix socket to multiplex on")
	flag.StringVar(&proxyAddr, "listen-socket", "", "unix socket to listen on")

	flag.Parse()

	muxAddr, err := unixAddr(channel)
	if err != nil {
		glog.Error("invalid mux socket address")
		return
	}
	listenAddr, err := unixAddr(proxyAddr)
	if err != nil {
		glog.Error("invalid listen socket address")
		return
	}

	// yamux connection
	servConn, err := net.Dial("unix", muxAddr)
	if err != nil {
		glog.Errorf("fail to dial channel(%s): %s", muxAddr, err)
		return
	}
	defer servConn.Close()

	result := make(chan error)
	err = serve(servConn, "unix", listenAddr, result)
	if err != nil {
		return
	}

	<-result
}
