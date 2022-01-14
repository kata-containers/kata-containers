// Copyright (c) 2017-2019 Intel Corporation
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"strings"

	"sync"
	"time"

	"github.com/containerd/console"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/katautils"
	"github.com/kata-containers/kata-containers/src/runtime/pkg/utils/shimclient"
	clientUtils "github.com/kata-containers/kata-containers/src/runtime/virtcontainers/pkg/agent/protocols/client"
	"github.com/pkg/errors"
	"github.com/urfave/cli"
)

const (

	// The buffer size used to specify the buffer for IO streams copy
	bufSize = 1024 * 2

	defaultTimeout = 3 * time.Second

	subCommandName = "exec"
	// command-line parameters name
	paramDebugConsolePort                    = "kata-debug-port"
	defaultKernelParamDebugConsoleVPortValue = 1026
)

var (
	bufPool = sync.Pool{
		New: func() interface{} {
			buffer := make([]byte, bufSize)
			return &buffer
		},
	}
)

var kataExecCLICommand = cli.Command{
	Name:  subCommandName,
	Usage: "Enter into guest by debug console",
	Flags: []cli.Flag{
		cli.Uint64Flag{
			Name:  paramDebugConsolePort,
			Usage: "Port that debug console is listening on. (Default: 1026)",
		},
	},
	Action: func(context *cli.Context) error {
		port := context.Uint64(paramDebugConsolePort)
		if port == 0 {
			port = defaultKernelParamDebugConsoleVPortValue
		}

		sandboxID := context.Args().Get(0)

		if err := katautils.VerifyContainerID(sandboxID); err != nil {
			return err
		}

		conn, err := getConn(sandboxID, port)

		if err != nil {
			return err
		}
		defer conn.Close()

		con := console.Current()
		defer con.Reset()

		if err := con.SetRaw(); err != nil {
			return err
		}

		iostream := &iostream{
			conn:   conn,
			exitch: make(chan struct{}),
			closed: false,
		}

		ioCopy(iostream, con)

		<-iostream.exitch
		return nil
	},
}

func ioCopy(stream *iostream, con console.Console) {
	var wg sync.WaitGroup

	// stdin
	go func() {
		p := bufPool.Get().(*[]byte)
		defer bufPool.Put(p)
		io.CopyBuffer(stream, con, *p)
	}()

	// stdout
	wg.Add(1)
	go func() {
		p := bufPool.Get().(*[]byte)
		defer bufPool.Put(p)
		io.CopyBuffer(os.Stdout, stream, *p)
		wg.Done()
	}()

	wg.Wait()
	close(stream.exitch)
}

type iostream struct {
	conn   net.Conn
	exitch chan struct{}
	closed bool
}

func (s *iostream) Write(data []byte) (n int, err error) {
	if s.closed {
		return 0, errors.New("stream closed")
	}
	return s.conn.Write(data)
}

func (s *iostream) Close() error {
	if s.closed {
		return errors.New("stream closed")
	}

	err := s.conn.Close()
	if err == nil {
		s.closed = true
	}

	return err
}

func (s *iostream) Read(data []byte) (n int, err error) {
	if s.closed {
		return 0, errors.New("stream closed")
	}

	return s.conn.Read(data)
}

func getConn(sandboxID string, port uint64) (net.Conn, error) {
	client, err := shimclient.BuildShimClient(sandboxID, defaultTimeout)
	if err != nil {
		return nil, err
	}

	resp, err := client.Get("http://shim/agent-url")
	if err != nil {
		return nil, err
	}

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("Failure from %s shim-monitor: %d", sandboxID, resp.StatusCode)
	}

	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	sock := strings.TrimSuffix(string(data), "\n")
	addr, err := url.Parse(sock)
	if err != nil {
		return nil, err
	}

	// validate more
	switch addr.Scheme {
	case clientUtils.VSockSocketScheme:
		// vsock://31513974:1024
		cidAndPort := strings.Split(addr.Host, ":")
		if len(cidAndPort) != 2 {
			return nil, fmt.Errorf("Invalid vsock scheme: %s", sock)
		}
		shimAddr := fmt.Sprintf("%s:%s:%d", clientUtils.VSockSocketScheme, cidAndPort[0], port)
		return clientUtils.VsockDialer(shimAddr, defaultTimeout)

	case clientUtils.HybridVSockScheme:
		// addr: hvsock:///run/vc/firecracker/340b412c97bf1375cdda56bfa8f18c8a/root/kata.hvsock:1024
		hvsocket := strings.Split(addr.Path, ":")
		if len(hvsocket) != 2 {
			return nil, fmt.Errorf("Invalid hybrid vsock scheme: %s", sock)
		}

		// hvsock:///run/vc/firecracker/340b412c97bf1375cdda56bfa8f18c8a/root/kata.hvsock
		shimAddr := fmt.Sprintf("%s:%s:%d", clientUtils.HybridVSockScheme, hvsocket[0], port)
		return clientUtils.HybridVSockDialer(shimAddr, defaultTimeout)
	}

	return nil, fmt.Errorf("schema %s not found", addr.Scheme)
}
