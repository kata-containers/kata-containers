// A simple proxy that multiplexes a unix socket connection
//
// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"bytes"
	"crypto/md5"
	"fmt"
	"io"
	"io/ioutil"
	"net"
	"os"
	"os/signal"
	"regexp"
	"strings"
	"sync"
	"syscall"
	"testing"

	"github.com/hashicorp/yamux"
	"github.com/stretchr/testify/assert"
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
	copyCh := make(chan error)
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

		_, err = f.Seek(0, io.SeekStart)
		if err != nil {
			return err
		}
		go func() {
			_, err := io.Copy(conn, f)
			copyCh <- err
		}()

	} else {
		sum1 = fmt.Sprintf("%x", md5.Sum(buf))

		size, err := conn.Write(buf)
		if err != nil {
			return err
		}
		expected = int64(size)
		copyCh <- nil
	}

	// read from server
	h := md5.New()
	var result []byte
	for {
		if expected >= 1024 {
			result = make([]byte, 1024)
		} else if expected > 0 {
			result = make([]byte, expected)
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
		return fmt.Errorf("unmatched checksum on file %s:\norig:\t%s\nnew:\t%s", file, sum1, sum2)
	}

	return <-copyCh
}

func server(listener net.Listener, closeCh chan bool) error {
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

	go func() {
		<-closeCh
		session.Close()
	}()

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
	closeCh := make(chan bool)
	go func() {
		server(l, closeCh)
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
	lp, s, err := serve(servConn, "unix", listenSock, results)
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		close(closeCh)
		s.Close()
	}()

	// run client tests
	files, err := ioutil.ReadDir(testDir)
	if err != nil {
		lp.Close()
		t.Fatal(err)
	}

	wg := &sync.WaitGroup{}
	cliRes := make(chan error)
	for _, file := range files {
		if file.IsDir() {
			continue
		}
		wg.Add(1)
		go func(filename string) {
			cliRes <- client(listenSock, filename)
			wg.Done()
		}(file.Name())
	}

	go func() {
		wg.Wait()
		close(cliRes)
	}()

	for err = range cliRes {
		if err != nil {
			t.Fatal(err)
		}
	}

	// closing the listener should result in an error in results channel
	lp.Close()
	err = <-results
	assert.NotNil(t, err, "closing listener should result in an error")
}

func TestSetupSigtermNotifier(t *testing.T) {
	sigCh := setupNotifier()
	assert.NotNil(t, sigCh, "Signal channel should not be nil")

	signal.Reset()
}

func TestHandleSigtermSignalNilSignalChannelFailure(t *testing.T) {
	err := handleExitSignal(nil, nil, nil)
	assert.NotNil(t, err, "Should throw an error because signal channel provided was nil")
}

func TestHandleSigtermSignalWrongSignalFailure(t *testing.T) {
	sig := syscall.SIGUSR1
	sigCh := make(chan os.Signal, 1)
	sigCh <- sig
	err := handleExitSignal(sigCh, nil, nil)
	assert.NotNil(t, err, "Should throw an error because signal sent was %q", sig.String())
}

func TestHandleSigtermSignalNilConnectionsSuccess(t *testing.T) {
	sigCh := make(chan os.Signal, 1)
	sigCh <- termSignal
	err := handleExitSignal(sigCh, nil, nil)
	assert.Nil(t, err, "Should not fail: %v", err)
}

func TestLogger(t *testing.T) {
	assert := assert.New(t)

	buf := &bytes.Buffer{}

	log := logger()

	savedLogOut := log.Logger.Out

	defer func() {
		log.Logger.Out = savedLogOut
	}()

	// save all output to a buffer
	log.Logger.Out = buf

	msg := "oh dear!"
	log.Error(msg)

	line := buf.String()

	assert.True(strings.Contains(line, fmt.Sprintf(`msg=%q`, msg)))
	assert.True(strings.Contains(line, "name="+proxyName))
	assert.True(strings.Contains(line, "source=proxy"))

	pidPattern := regexp.MustCompile(`pid=\d+`)
	matches := pidPattern.FindAllString(line, -1)
	assert.NotNil(matches)

	assert.False(strings.Contains(line, "sandbox="))
}

func TestLoggerWithSandbox(t *testing.T) {
	assert := assert.New(t)

	buf := &bytes.Buffer{}

	savedSandbox := sandboxID
	sandboxID = "a-sandbox-id"

	log := logger()
	savedLogOut := log.Logger.Out

	defer func() {
		log.Logger.Out = savedLogOut
		sandboxID = savedSandbox
	}()

	// save all output to a buffer
	log.Logger.Out = buf

	msg := "I've got a bad feeling about this!"
	log.Error(msg)

	line := buf.String()

	assert.True(strings.Contains(line, fmt.Sprintf(`msg=%q`, msg)))
	assert.True(strings.Contains(line, "name="+proxyName))
	assert.True(strings.Contains(line, "source=proxy"))

	pidPattern := regexp.MustCompile(`pid=\d+`)
	matches := pidPattern.FindAllString(line, -1)
	assert.NotNil(matches)

	assert.True(strings.Contains(line, "sandbox="+sandboxID))
}
