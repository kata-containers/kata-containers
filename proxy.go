// A simple proxy that multiplexes a unix socket connection
//
// Copyright 2017 HyperHQ Inc.

package main

import (
	"flag"
	"io"
	"net"
	"sync"

	"github.com/golang/glog"
	"github.com/hashicorp/yamux"
)

func serv(servConn io.ReadWriteCloser, proto, addr string) error {
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
	defer l.Close()

	for {
		conn, err := l.Accept()
		if err != nil {
			glog.Errorf("fail to accept new connection: %s", err)
			return err
		}
		stream, err := session.Open()
		if err != nil {
			glog.Errorf("fail to open yamux stream: %s", err)
			return err
		}
		wg := &sync.WaitGroup{}
		once := &sync.Once{}
		cleanup := func() {
			conn.Close()
			stream.Close()
		}
		copyStream := func(dst io.Writer, src io.Reader) {
			_, err := io.Copy(dst, src)
			if err != nil {
				once.Do(cleanup)
			}
			wg.Done()
		}

		wg.Add(2)
		go copyStream(conn, stream)
		go copyStream(stream, conn)
		go func() {
			wg.Wait()
			once.Do(cleanup)
		}()
	}
}

func main() {
	channel := flag.String("s", "/tmp/target.sock", "unix socket to multiplex on")
	proxyAddr := flag.String("l", "/tmp/proxy.sock", "unix socket to listen at")

	flag.Parse()

	// yamux connection
	servConn, err := net.Dial("unix", *channel)
	if err != nil {
		glog.Errorf("fail to dial channel(%s): %s", channel, err)
		return
	}
	defer servConn.Close()

	serv(servConn, "unix", *proxyAddr)
}
