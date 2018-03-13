// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// gRPC client wrapper

package client

import (
	"context"
	"net"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/mdlayher/vsock"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	grpcStatus "google.golang.org/grpc/status"

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
	agentgrpc.HealthClient
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
	grpcAddr, parsedAddr, err := parse(sock)
	if err != nil {
		return nil, err
	}
	dialOpts := []grpc.DialOption{grpc.WithInsecure(), grpc.WithBlock()}
	dialOpts = append(dialOpts, grpc.WithDialer(agentDialer(parsedAddr)))
	ctx := context.Background()
	ctx, cancel := context.WithTimeout(ctx, dialTimeout)
	defer cancel()
	conn, err := grpc.DialContext(ctx, grpcAddr, dialOpts...)
	if err != nil {
		return nil, err
	}

	return &AgentClient{
		AgentServiceClient: agentgrpc.NewAgentServiceClient(conn),
		HealthClient:       agentgrpc.NewHealthClient(conn),
		conn:               conn,
	}, nil
}

// Close an existing connection to the agent gRPC server.
func (c *AgentClient) Close() error {
	return c.conn.Close()
}

// vsock scheme is self-defined to be kept from being parsed by grpc.
// Any format starting with "scheme://" will be parsed by grpc and we lose
// all address information because vsock scheme is not supported by grpc.
// Therefore we use the format vsock:<cid>:<port> for vsock address.
//
// See https://github.com/grpc/grpc/blob/master/doc/naming.md
//
// In the long term, we should patch grpc to support vsock scheme and also
// upstream the timed vsock dialer.
func parse(sock string) (string, *url.URL, error) {
	addr, err := url.Parse(sock)
	if err != nil {
		return "", nil, err
	}

	var grpcAddr string
	// validate more
	switch addr.Scheme {
	case vsockSocketScheme:
		if addr.Hostname() == "" || addr.Port() == "" || addr.Path != "" {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock scheme: %s", sock)
		}
		if _, err := strconv.ParseUint(addr.Hostname(), 10, 32); err != nil {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock cid: %s", sock)
		}
		if _, err := strconv.ParseUint(addr.Port(), 10, 32); err != nil {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock port: %s", sock)
		}
		grpcAddr = vsockSocketScheme + ":" + addr.Host
	case unixSocketScheme:
		fallthrough
	case "":
		if (addr.Host == "" && addr.Path == "") || addr.Port() != "" {
			return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid unix scheme: %s", sock)
		}
		if addr.Host == "" {
			grpcAddr = unixSocketScheme + ":///" + addr.Path
		} else {
			grpcAddr = unixSocketScheme + ":///" + addr.Host + "/" + addr.Path
		}
	default:
		return "", nil, grpcStatus.Errorf(codes.InvalidArgument, "Invalid scheme: %s", sock)
	}

	return grpcAddr, addr, nil
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

// unix addr are parsed by grpc
func unixDialer(sock string, timeout time.Duration) (net.Conn, error) {
	return net.DialTimeout("unix", sock, timeout)
}

func parseGrpcVsockAddr(sock string) (uint32, uint32, error) {
	sp := strings.Split(sock, ":")
	if len(sp) != 3 {
		return 0, 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock address: %s", sock)
	}
	if sp[0] != vsockSocketScheme {
		return 0, 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock URL scheme: %s", sp[0])
	}

	cid, err := strconv.ParseUint(sp[1], 10, 32)
	if err != nil {
		return 0, 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock cid: %s", sp[1])
	}
	port, err := strconv.ParseUint(sp[2], 10, 32)
	if err != nil {
		return 0, 0, grpcStatus.Errorf(codes.InvalidArgument, "Invalid vsock port: %s", sp[2])
	}

	return uint32(cid), uint32(port), nil
}

func vsockDialer(sock string, timeout time.Duration) (net.Conn, error) {
	cid, port, err := parseGrpcVsockAddr(sock)
	if err != nil {
		return nil, err
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

			conn, err := vsock.Dial(cid, port)
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
	timeoutErrMsg := grpcStatus.Errorf(codes.DeadlineExceeded, "timed out connecting to vsock %d:%d", cid, port)
	select {
	case conn, ok = <-ch:
		if !ok {
			return nil, timeoutErrMsg
		}
	}

	return conn, nil
}
