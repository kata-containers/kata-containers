// A simple proxy that multiplexes a unix socket connection
//
// Copyright 2017 HyperHQ Inc.

package main

import (
	"io"
	"net"
	"os"

	"github.com/hashicorp/yamux"
)

func server(channel string) error {
	// just remove old ones for testing
	os.Remove(channel)
	l, err := net.Listen("unix", channel)
	if err != nil {
		return err
	}
	defer l.Close()

	// listen once
	conn, err := l.Accept()
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

func main() {
	vmChannel := "/tmp/target.sock"

	server(vmChannel)
}
