// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// gRPC client wrapper UT

package client

import (
	"fmt"
	"net"
	"os"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	context "golang.org/x/net/context"
	"google.golang.org/grpc"

	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/agent/protocols/mockserver"
)

const (
	mockSockAddr       = "/tmp/agentserver.sock"
	unixMockAddr       = "unix://" + mockSockAddr
	mockBadSchemeAddr  = "foobar://" + mockSockAddr
	mockFakeVsockAddr  = "vsock://0:100"
	mockVsockBadCid    = "vsock://foo:100"
	mockVsockBadPort   = "vsock://100:bar"
	mockBadVsockScheme = "vsock://100"
)

func startMockServer(t *testing.T) (*grpc.Server, chan error, error) {
	os.Remove(mockSockAddr)

	l, err := net.Listen("unix", mockSockAddr)
	assert.Nil(t, err, "Listen on %s failed: %s", mockSockAddr, err)

	mock := mockserver.NewMockServer()

	stopWait := make(chan error, 1)
	go func() {
		mock.Serve(l)
		stopWait <- nil
	}()

	return mock, stopWait, nil
}

func checkHealth(cli *AgentClient) error {
	resp, err := cli.Check(context.Background(), &pb.CheckRequest{})
	if err != nil {
		return err
	}
	if resp.Status != pb.HealthCheckResponse_SERVING {
		return fmt.Errorf("unexpected health status: %s", resp.Status)
	}

	return nil
}

func checkVersion(cli *AgentClient) error {
	resp, err := cli.Version(context.Background(), &pb.CheckRequest{})
	if err != nil {
		return err
	}
	if resp.GrpcVersion != pb.APIVersion {
		return fmt.Errorf("unexpected grpc API version: %s", resp.GrpcVersion)
	}
	if resp.AgentVersion != mockserver.MockServerVersion {
		return fmt.Errorf("unexpected mock server version: %s", resp.AgentVersion)
	}

	return nil
}

func TestNewAgentClient(t *testing.T) {
	mock, waitCh, err := startMockServer(t)
	assert.Nil(t, err, "failed to start mock server: %s", err)

	cliFunc := func(sock string, success bool, expect string) {
		cli, err := NewAgentClient(sock)
		if success {
			assert.Nil(t, err, "Failed to create new agent client: %s", err)
		} else if !success {
			assert.NotNil(t, err, "Unexpected success with sock address: %s", sock)
		}
		if err == nil {
			err = checkHealth(cli)
			assert.Nil(t, err, "failed checking grpc server status: %s", err)
			err = checkVersion(cli)
			assert.Nil(t, err, "failed checking grpc server version: %s", err)

			cli.Close()
		} else if expect != "" {
			assert.True(t, strings.Contains(err.Error(), expect), "expect err message: %s\tgot: %s", expect, err)
		}
	}

	cliFunc(mockSockAddr, true, "")
	cliFunc(unixMockAddr, true, "")
	cliFunc(mockBadSchemeAddr, false, "Invalid scheme:")
	cliFunc(mockBadVsockScheme, false, "Invalid vsock scheme:")
	cliFunc(mockVsockBadCid, false, "Invalid vsock cid")
	cliFunc(mockVsockBadPort, false, "Invalid vsock port")
	cliFunc(mockFakeVsockAddr, false, "context deadline exceeded")

	// wait mock server to stop
	mock.Stop()
	<-waitCh
}
