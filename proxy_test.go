// A simple proxy that multiplexes a unix socket connection
//
// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"crypto/md5"
	"fmt"
	"io"
	"io/ioutil"
	"net"
	"os"
	"sync"
	"testing"

	"github.com/hashicorp/yamux"
)

func client(proxyAddr, file string) error {
	buf := []byte("hello proxy")

	conn, err := net.Dial("unix", proxyAddr)
	if err != nil {
		return err
	}
	defer conn.Close()

	var sum1 string
	var expected int64
	if file != "" {
		f, err := os.Open(file)
		if err != nil {
			return err
		}
		defer f.Close()

		h := md5.New()
		expected, err = io.Copy(h, f)
		if err != nil {
			return err
		}
		sum1 = fmt.Sprintf("%x", h.Sum(nil))

		_, err = f.Seek(0, os.SEEK_SET)
		if err != nil {
			return err
		}
		go io.Copy(conn, f)

	} else {
		sum1 = fmt.Sprintf("%x", md5.Sum(buf))

		size, err := conn.Write(buf)
		if err != nil {
			return err
		}
		expected = int64(size)
	}

	// read from server
	h := md5.New()
	var result []byte
	for {
		if expected >= 1024 {
			result = make([]byte, 1024, 1024)
		} else if expected > 0 {
			result = make([]byte, expected, expected)
		} else {
			break
		}
		size, err := conn.Read(result)
		if err != nil {
			return err
		}
		_, err = h.Write(result[:size])
		if err != nil {
			return err
		}
		expected -= int64(size)
	}

	sum2 := fmt.Sprintf("%x", h.Sum(nil))

	if sum1 != sum2 {
		return fmt.Errorf("unmatched checksum on file %s:\norig:\t%s\nnew:\t%s\n", file, sum1, sum2)
	}

	return nil
}

func server(listener net.Listener) error {
	// Accept once
	conn, err := listener.Accept()
	if err != nil {
		return err
	}
	defer conn.Close()

	session, err := yamux.Server(conn, nil)
	if err != nil {
		return err
	}
	defer session.Close()

	for {
		stream, err := session.Accept()
		if err != nil {
			return err
		}
		go func() {
			io.Copy(stream, stream)
			stream.Close()
		}()
	}

	return nil
}

func TestUnixAddrParsing(T *testing.T) {
	buf := "unix://foo/bar"
	addr, err := unixAddr(buf)
	if err != nil {
		T.Fatalf("failed to parse %s", buf)
	}
	if addr != "foo/bar" {
		T.Fatalf("unexpected parsing result: %s", addr)
	}

	buf = "unix://////foo/bar"
	_, err = unixAddr(buf)
	if err != nil {
		T.Fatalf("failed to parse %s", buf)
	}

	buf = "foo/bar"
	_, err = unixAddr(buf)
	if err != nil {
		T.Fatalf("failed to parse %s", buf)
	}

	buf = "/foo/bar"
	_, err = unixAddr(buf)
	if err != nil {
		T.Fatalf("failed to parse %s", buf)
	}

	buf = ""
	_, err = unixAddr(buf)
	if err == nil {
		T.Fatal("unexpected success parsing empty string")
	}

	buf = "vsock://foo/bar"
	_, err = unixAddr(buf)
	if err == nil {
		T.Fatalf("unexpected success parsing %s", buf)
	}
}

func TestProxy(t *testing.T) {
	muxSock := "/tmp/proxy-mux.sock"
	listenSock := "/tmp/proxy-listen.sock"
	testDir := "."
	os.Remove(muxSock)
	os.Remove(listenSock)

	// start yamux server
	l, err := net.Listen("unix", muxSock)
	if err != nil {
		t.Fatal(err)
	}
	go func() {
		server(l)
		l.Close()
	}()

	// start proxy
	servConn, err := net.Dial("unix", muxSock)
	if err != nil {
		t.Fatalf("fail to dial channel(%s): %s", muxSock, err)
		return
	}
	defer servConn.Close()

	results := make(chan error)
	err = serve(servConn, "unix", listenSock, results)
	if err != nil {
		t.Fatal(err)
	}

	// run client tests
	files, err := ioutil.ReadDir(testDir)
	if err != nil {
		t.Fatal(err)
	}

	wg := &sync.WaitGroup{}
	for _, file := range files {
		if file.IsDir() {
			continue
		}
		wg.Add(1)
		go func(filename string) {
			results <- client(listenSock, filename)
			wg.Done()
		}(file.Name())
	}

	go func() {
		wg.Wait()
		close(results)
	}()

	for err = range results {
		if err != nil {
			t.Fatal(err)
		}
	}
}
