// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// gRPC client wrapper

package client

import (
	"fmt"
	"net"
	"net/url"
	"strconv"
	"time"

	"github.com/mdlayher/vsock"
	"google.golang.org/grpc"

	agentgrpc "github.com/kata-containers/agent/protocols/grpc"
)

const (
	unixSocketScheme  = "unix"
	vsockSocketScheme = "vsock"
	dialTimeout       = 5 * time.Second
)

// AgentClient is an agent gRPC client connection wrapper for agentgrpc.AgentServiceClient
type AgentClient struct {
	agentgrpc.AgentServiceClient
	conn *grpc.ClientConn
}

type dialer func(string, time.Duration) (net.Conn, error)

// NewAgentClient creates a new agent gRPC client and handles both unix and vsock addresses.
//
// Supported sock address formats are:
//   - unix://<unix socket path>
//   - vsock://<cid>:<port>
//   - <unix socket path>
func NewAgentClient(sock string) (*AgentClient, error) {
	addr, err := parse(sock)
	if err != nil {
		return nil, err
	}
	dialOpts := []grpc.DialOption{grpc.WithInsecure(), grpc.WithBlock(), grpc.WithTimeout(dialTimeout)}
	dialOpts = append(dialOpts, grpc.WithDialer(agentDialer(addr)))
	conn, err := grpc.Dial(sock, dialOpts...)
	if err != nil {
		return nil, err
	}

	return &AgentClient{
		AgentServiceClient: agentgrpc.NewAgentServiceClient(conn),
		conn:               conn,
	}, nil
}

// Close an existing connection to the agent gRPC server.
func (c *AgentClient) Close() error {
	return c.conn.Close()
}

func parse(sock string) (*url.URL, error) {
	addr, err := url.Parse(sock)
	if err != nil {
		return nil, err
	}

	// validate more
	switch addr.Scheme {
	case vsockSocketScheme:
		if addr.Hostname() == "" || addr.Port() == "" || addr.Path != "" {
			return nil, fmt.Errorf("Invalid vsock scheme: %s", sock)
		}
	case unixSocketScheme:
		fallthrough
	case "":
		if (addr.Host == "" && addr.Path == "") || addr.Port() != "" {
			return nil, fmt.Errorf("Invalid unix scheme: %s", sock)
		}
	default:
		return nil, fmt.Errorf("Invalid scheme: %s", sock)
	}

	return addr, nil
}

func agentDialer(addr *url.URL) dialer {
	switch addr.Scheme {
	case vsockSocketScheme:
		return vsockDialer
	case unixSocketScheme:
		fallthrough
	default:
		return unixDialer
	}
}

func unixDialer(sock string, timeout time.Duration) (net.Conn, error) {
	addr, err := parse(sock)
	if err != nil {
		return nil, err
	}

	if addr.Scheme != unixSocketScheme && addr.Scheme != "" {
		return nil, fmt.Errorf("Invalid URL scheme: %s", addr.Scheme)
	}

	return net.DialTimeout("unix", addr.Host+addr.Path, timeout)
}

func vsockDialer(sock string, timeout time.Duration) (net.Conn, error) {
	addr, err := parse(sock)
	if err != nil {
		return nil, err
	}

	if addr.Scheme != vsockSocketScheme {
		return nil, fmt.Errorf("Invalid URL scheme: %s", addr.Scheme)
	}

	invalidVsockMsgErr := fmt.Errorf("invalid vsock destination: %s", sock)
	cid, err := strconv.ParseUint(addr.Hostname(), 10, 32)
	if err != nil {
		return nil, invalidVsockMsgErr
	}
	port, err := strconv.ParseUint(addr.Port(), 10, 32)
	if err != nil {
		return nil, invalidVsockMsgErr
	}

	t := time.NewTimer(timeout)
	cancel := make(chan bool)
	ch := make(chan net.Conn)
	go func() {
		for {
			select {
			case <-cancel:
				// canceled or channel closed
				return
			default:
			}

			conn, err := vsock.Dial(uint32(cid), uint32(port))
			if err == nil {
				// Send conn back iff timer is not fired
				// Otherwise there might be no one left reading it
				if t.Stop() {
					ch <- conn
				} else {
					conn.Close()
				}
				return
			}
		}
	}()

	var conn net.Conn
	var ok bool
	timeoutErrMsg := fmt.Errorf("timed out connecting to vsock %d:%d", cid, port)
	select {
	case <-t.C:
		cancel <- true
		return nil, timeoutErrMsg
	case conn, ok = <-ch:
		if !ok {
			return nil, timeoutErrMsg
		}
	}

	return conn, nil
}
