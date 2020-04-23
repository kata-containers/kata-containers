// Copyright 2017 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"context"
	"fmt"
	"net"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
	"google.golang.org/grpc"

	pb "github.com/kata-containers/agent/protocols/grpc"
	"github.com/kata-containers/agent/protocols/mockserver"
)

const mockSockAddr = "/tmp/agentserver.sock"
const unixMockAddr = "unix:///" + mockSockAddr
const badMockAddr = "vsock://" + mockSockAddr

type testAgent struct {
	t *testing.T

	server *grpc.Server
	waitCh chan error

	ctx    context.Context
	client *shimAgent
}

func startMockServer(t *testing.T) (*grpc.Server, chan error) {
	os.Remove(mockSockAddr)

	l, err := net.Listen("unix", mockSockAddr)
	assert.Nil(t, err, "Listen on %s failed: %s", mockSockAddr, err)

	mock := mockserver.NewMockServer()

	stopWait := make(chan error, 1)
	go func() {
		mock.Serve(l)
		stopWait <- nil
	}()

	return mock, stopWait
}

func testSetup(t *testing.T) *testAgent {
	mock, waitCh := startMockServer(t)

	agent, err := newShimAgent(context.Background(), mockSockAddr)
	if !assert.Nil(t, err, "Failed to create new agent client: %s", err) {
		t.FailNow()
	}

	ctx := context.Background()
	_, err = agent.CreateSandbox(ctx, &pb.CreateSandboxRequest{
		Hostname:     "testSandbox",
		Dns:          []string{},
		Storages:     []*pb.Storage{},
		SandboxPidns: true,
	})
	if !assert.Nil(t, err, "Failed to create sandbox: %s", err) {
		agent.Close()
		t.FailNow()
	}

	return &testAgent{
		t:      t,
		server: mock,
		waitCh: waitCh,
		ctx:    ctx,
		client: agent,
	}
}

func testTearDown(t *testAgent) {
	t.client.Close()
	t.server.Stop()
	<-t.waitCh
}

func newTestSpec() *pb.Spec {
	return &pb.Spec{
		Version: "testGrpcVersion",
		Process: &pb.Process{
			Terminal:     true,
			ConsoleSize:  &pb.Box{Height: 10, Width: 10},
			User:         pb.User{UID: 0, GID: 0, Username: "root:root"},
			Capabilities: &pb.LinuxCapabilities{},
			Rlimits:      []pb.POSIXRlimit{},
		},
		Root:     &pb.Root{Path: "rootpath", Readonly: true},
		Hostname: "testGuest",
	}
}

func (t *testAgent) addContainer(containerID, execID string) error {
	_, err := t.client.CreateContainer(t.ctx, &pb.CreateContainerRequest{
		ContainerId: containerID,
		ExecId:      execID,
		StringUser:  &pb.StringUser{Uid: "root", Gid: "root"},
		OCI:         newTestSpec(),
	})
	if err != nil {
		return fmt.Errorf("failed to create new container: %s", err)
	}

	_, err = t.client.StartContainer(t.ctx, &pb.StartContainerRequest{ContainerId: containerID})
	if err != nil {
		return fmt.Errorf("failed to create new container: %s", err)
	}

	return nil
}

func TestNewShimAgent(t *testing.T) {
	mock, waitCh := startMockServer(t)

	cliFunc := func(sock string, success bool) {
		agent, err := newShimAgent(context.Background(), sock)
		if success {
			assert.Nil(t, err, "Failed to create new agent client: %s", err)
		} else if !success {
			assert.NotNil(t, err, "Unexpected success with sock address: %s", sock)
		}
		if err == nil {
			agent.Close()
		}
	}

	cliFunc(mockSockAddr, true)
	cliFunc(unixMockAddr, true)
	cliFunc(badMockAddr, false)

	// wait mock server to stop
	mock.Stop()
	<-waitCh
}

func TestAddContainer(t *testing.T) {
	agent := testSetup(t)
	defer testTearDown(agent)

	contID := "testContainer"
	execID := "testExec"
	err := agent.addContainer(contID, execID)
	assert.Nil(t, err, "%s", err)
	err = agent.addContainer(contID, execID)
	assert.NotNil(t, err, "unexpected success when adding duplicated container")
}
